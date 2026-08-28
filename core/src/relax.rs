//! M2: elasticity/relaxation — see docs/crochet-context.md §6.
//!
//! Turns M1's raw (rigidly-placed) shape into a physically plausible
//! relaxed one by modelling every insertion-target edge, and every
//! working-order continuity edge between consecutive stitches in a
//! thread, as a Hookean spring: `force = stiffness * (distance -
//! rest_length) * direction`. Stiffness comes from the stitch registry
//! (`StitchDef::insertion_stiffness`, §3a) — dense stitches are stiff
//! (low give), open/tall stitches and chains are soft (more give) — so
//! elasticity is a direct consequence of stitch topology, never a
//! separate "yarn material" parameter (§6).
//!
//! Nothing here assumes unconstrained-3D-only (§6a): positions are plain
//! `Vec3`s with no z-axis special-casing, so a future planar-constraint
//! mode can clamp or project them without touching this solver's core.
//!
//! **Sibling repulsion (§5a).** Springs alone only constrain a stitch's
//! distance to its own target and to its immediate working-order
//! neighbour — nothing stops two *non-adjacent* stitches sharing the same
//! target (a shell, a magic-ring round) from swinging past each other and
//! folding together over many relaxation steps, since neither kind of
//! spring involves them directly. Confirmed this empirically: a 7-stitch
//! shell could relax two non-adjacent siblings to ~1e-16 apart before
//! this was added. Every pair of stitches that share a single target now
//! also gets a one-sided repulsion force, active only once they're closer
//! than `SIBLING_REPULSION_MIN_DISTANCE`.

use std::collections::HashMap;

use crate::geometry::{place_scheme, PlacementError};
use crate::graph::{Scheme, StitchRef};
use crate::rod::curvature_binormal;
use crate::stitch::{StitchRegistry, SS};
use crate::vec3::Vec3;

/// Stiffness of the working-order continuity edge between consecutive
/// stitches in a thread — the physical yarn strand linking them. Not
/// stitch-kind-dependent (unlike insertion stiffness): it's the same
/// strand of yarn regardless of what's formed at either end.
const CONTINUITY_STIFFNESS: f64 = 0.6;

/// M9: real bending resistance, derived from the actual Discrete Elastic
/// Rod curvature measure (`rod::curvature_binormal`) rather than a plain
/// distance spring. Each interior working-order vertex `i` (with edges
/// `e_prev = p[i]-p[i-1]`, `e_curr = p[i+1]-p[i]`) contributes a bending
/// energy `stiffness * |kb_i|^2 / l_i` (`l_i` the average of the two edge
/// lengths — the standard DER Voronoi-length normalization, Bergou et al.
/// eq. 2-ish), and the force applied each step is the negative gradient of
/// that energy, computed via central finite differences
/// (`bending_energy_gradient` below) rather than the hand-derived analytic
/// Jacobian of `kb` (Bergou et al.'s appendix gives it in closed form, but
/// it's a nontrivial per-component 3x3-matrix derivation with real risk of
/// a silent sign/index error that wouldn't be obvious from a passing test
/// suite — a numerical gradient of the *actual* energy is exactly as
/// physically correct, cheap enough at this scheme scale (tens to low
/// hundreds of vertices, well under real-time budget even at 150 steps),
/// and impossible to get subtly wrong the way a mis-transcribed formula
/// could be).
///
/// Deliberately still force-based Euler integration, not a full XPBD
/// constraint-projection solve: this solver's existing spring forces are
/// already empirically stable at the stiffness values in use (nothing here
/// pushes toward the near-infinite-stiffness regime XPBD specifically
/// exists to stabilize), so extending the same proven integration scheme
/// with a real curvature-derived force is the lower-risk way to add
/// genuine bending resistance without standing up a second solver
/// architecture alongside this one.
///
/// `|kb_i|` is exactly zero for a perfectly collinear triple by
/// construction (see `curvature_binormal`'s own doc comment) — a chain's
/// raw placement lays every link on one straight line
/// (`geometry.rs::lays_out_as_line`), so this force alone has nothing to
/// act on until something breaks that exact symmetry; see
/// `CHAIN_SYMMETRY_BREAK_AMPLITUDE` in `geometry.rs` for the (tiny,
/// deterministic) seed that does that.
const BENDING_STIFFNESS: f64 = 0.2;

/// The finite-difference step used to numerically differentiate the
/// bending energy (see `BENDING_STIFFNESS`). Small relative to typical
/// yarn-scale distances (segment lengths are order 0.1-1.0) so the
/// approximation error is negligible, large enough relative to f64
/// epsilon that the subtraction in `bending_energy_gradient` doesn't lose
/// precision to cancellation.
const BENDING_GRADIENT_EPS: f64 = 1e-6;

/// A bending triple is skipped entirely (no force computed at all) when
/// either of its raw edges is longer than this multiple of the thread's
/// own *typical* raw continuity-edge length (see `bending_triples`'
/// construction, which computes that typical length per thread before
/// applying this). This isn't a numerical band-aid on an otherwise-
/// arbitrary cutoff: `curvature_binormal`'s own doc comment already notes
/// its denominator (`|e_prev||e_curr| + e_prev·e_curr`) approaches zero —
/// a real singularity of the discrete-curvature-binormal representation
/// itself (Bergou et al. note the same limitation), not an artifact of
/// computing its gradient by finite differences — as the turn angle
/// approaches 180 degrees, and this data model's raw placement produces
/// exactly that configuration in two ordinary, expected cases: a working-
/// order jump back across a row (chain's end to the next row's first
/// stitch, placed near the *start* of the row below) and a slip-stitch's
/// continuity edge back to an early target (the ring-closure case this
/// milestone exists for). Neither is a real physical rod bending 180
/// degrees; they're this model's insertion-graph topology, already
/// handled by the ordinary continuity/insertion springs.
///
/// A **length ratio**, not a turn-angle cutoff, is what actually
/// identifies these: an angle threshold was tried first and doesn't scale
/// — a row-transition edge's angle relative to the row depends on the
/// target stitch's own height (confirmed empirically: dc's transition is
/// ~166 degrees from straight, dtr's only ~143, and taller registry
/// stitches keep trending further from 180 as height grows, so no single
/// angle cutoff catches all of them without either missing tall-stitch
/// transitions or wrongly excluding genuine sharp-but-real bends). A
/// row-transition or ring-closing edge, though, is characteristically
/// much *longer* than an ordinary within-row/within-chain continuity
/// edge regardless of stitch height — it spans back across the whole row
/// or chain — so comparing against the thread's own typical spacing is
/// scale-invariant in exactly the way the pipeline's existing calibrated
/// behaviour (docs §5a, the dc/tr/dtr differential-pull demo) needs it to
/// be.
const BENDING_MAX_EDGE_RATIO: f64 = 2.0;

/// Pure numerical-safety guard, re-checked every step against the
/// *current* dynamic configuration (unlike `BENDING_MAX_EDGE_RATIO`,
/// which is about identifying topological jumps and is checked against
/// current length too but exists for a different reason — see its own
/// doc comment). `curvature_binormal`'s denominator is
/// `|e_prev||e_curr|(1 + cos(theta))`, which approaches zero as the turn
/// angle `theta` approaches 180 degrees *regardless of edge length* — a
/// short, sharp U-turn is exactly as numerically dangerous for finite-
/// difference differentiation as a long one. -0.9 leaves real margin
/// before the denominator gets small enough to matter (edges would need
/// to be within ~26 degrees of exactly anti-parallel), while still
/// letting genuinely sharp — but not near-singular — bends (e.g. a tight
/// corner well short of a full fold-back) get real bending resistance.
const BENDING_SAFETY_MIN_TURN_COSINE: f64 = -0.9;

/// The Discrete-Elastic-Rod-style bending energy at one interior
/// working-order vertex, given its two neighbouring positions and the
/// vertex's **rest curvature binormal** (`kb_rest` — see
/// `bending_triples`' construction below for where this comes from and
/// why it isn't simply zero). See `BENDING_STIFFNESS`'s doc comment for
/// the rest of the formula and its provenance.
fn bending_energy(p_prev: Vec3, p_curr: Vec3, p_next: Vec3, kb_rest: Vec3, stiffness: f64) -> f64 {
    let e_prev = p_curr - p_prev;
    let e_curr = p_next - p_curr;
    let l = (e_prev.length() + e_curr.length()) * 0.5;
    if l < 1e-9 {
        // Coincident/near-coincident vertices (e.g. a run of zero-height
        // ss/mr stitches): no well-defined curvature contribution here:
        // other constraints (continuity/insertion springs) are what pull
        // these apart, not bending.
        return 0.0;
    }
    let kb = curvature_binormal(e_prev, e_curr);
    let delta = kb - kb_rest;
    stiffness * delta.dot(delta) / l
}

/// Returns `(F_prev, F_curr, F_next)` — the force on each of the three
/// vertices from the bending energy at their shared interior vertex,
/// i.e. the negative central-finite-difference gradient of
/// [`bending_energy`] with respect to each point in turn.
fn bending_energy_gradient(
    p_prev: Vec3,
    p_curr: Vec3,
    p_next: Vec3,
    kb_rest: Vec3,
    stiffness: f64,
) -> (Vec3, Vec3, Vec3) {
    let axes = [
        Vec3::new(BENDING_GRADIENT_EPS, 0.0, 0.0),
        Vec3::new(0.0, BENDING_GRADIENT_EPS, 0.0),
        Vec3::new(0.0, 0.0, BENDING_GRADIENT_EPS),
    ];

    let perturbed = |which: u8, offset: Vec3| -> (Vec3, Vec3, Vec3) {
        match which {
            0 => (p_prev + offset, p_curr, p_next),
            1 => (p_prev, p_curr + offset, p_next),
            _ => (p_prev, p_curr, p_next + offset),
        }
    };

    let gradient_for = |which: u8| -> Vec3 {
        let mut components = [0.0f64; 3];
        for (axis_idx, axis) in axes.iter().enumerate() {
            let (pp, pc, pn) = perturbed(which, *axis);
            let e_plus = bending_energy(pp, pc, pn, kb_rest, stiffness);
            let (pp, pc, pn) = perturbed(which, *axis * -1.0);
            let e_minus = bending_energy(pp, pc, pn, kb_rest, stiffness);
            components[axis_idx] = (e_plus - e_minus) / (2.0 * BENDING_GRADIENT_EPS);
        }
        Vec3::new(components[0], components[1], components[2])
    };

    (
        gradient_for(0) * -1.0,
        gradient_for(1) * -1.0,
        gradient_for(2) * -1.0,
    )
}

/// M11/M12: barrier-based contact response (C-IPC-lite) — see GOALS.md's
/// M11/M12 entries. Every pair of the scheme's *reconstructed yarn-path
/// segments* (each stitch's own base-to-top body, and the bridge to its
/// working-order predecessor — see `virtual_segments` below) that isn't
/// structurally adjacent (a raw-coincident shared point — the same rule
/// `validate.rs`'s own adjacency check uses) gets a smooth **barrier**
/// potential: an IPC-style energy (Li et al., "Incremental Potential
/// Contact") that is exactly zero — value *and* gradient — at or beyond
/// `BARRIER_ACTIVE_DISTANCE`, and grows without bound as the pair
/// approaches zero distance:
/// `E(d) = -stiffness * (d - d_hat)^2 * ln(d / d_hat)` for `0 < d < d_hat`.
/// Being exactly zero beyond `d_hat` (not just small) is the whole point:
/// this cannot perturb any already-well-separated scheme, however
/// slightly, so it adds coverage only where there was none before rather
/// than risking a knock-on change to schemes the existing springs/
/// repulsion/bending forces are already calibrated against.
///
/// **Why segments, not just stitch tops (M12 revision).** M11's first cut
/// only ever pushed apart the *tops* relax.rs tracks as free variables —
/// real, but incomplete: the Owner's own framing is "we need simulation,
/// not verification — verification is only a fallback for configurations
/// that truly *can't* be distributed correctly." A top-only barrier can't
/// deliver that, because a stitch's rendered *body* (base-to-top) and the
/// *bridge* to its predecessor are both real yarn that can still cross
/// even when every stitch's own top is comfortably placed — confirmed
/// concretely on two real, previously-undiagnosed cases: (1) a fan's own
/// siblings, pushed unevenly by an external force, can swap angular order
/// and cross their connecting bridges even with `SIBLING_REPULSION_*`
/// keeping their tops apart; (2) an ordinary two-round flat circle (ring +
/// round 1 + a round-2 increase row — about as ordinary a scheme as this
/// project has) already collides round-2 children of *neighbouring*
/// round-1 targets via their bridges, a real M4-era limitation
/// (docs/crochet-context.md §5a) that turned out to be the *same* root
/// cause, not a separate one. Both are now covered by modelling the full
/// path (see `BaseSource`/`Endpoint`/`virtual_segments` below), not by
/// two separate patches.
///
/// **Why "barrier," not another linear spring**: a linear repulsion
/// (like `SIBLING_REPULSION_STRENGTH` below) has *finite* force even at
/// `d=0` — stiff enough forces and a long relaxation can still let a pair
/// settle uncomfortably close, or even swap sides across a step, since
/// nothing about the force shape itself prevents `d` from reaching zero.
/// A barrier's force grows toward infinity as `d -> 0`, so it's not just
/// discouraging closeness, it's actively unable to let genuine
/// interpenetration become a *stable equilibrium*.
///
/// **Scope, deliberately** (matching M9/M10's established pattern of
/// honest, bounded engineering over the research-grade original): this
/// is the report's simplified analogue, not a full C-IPC implementation.
/// Two real differences: (1) it's evaluated at *discrete* distances each
/// step (force-based Euler integration, same as everything else in this
/// solver), not integrated into a genuine constraint-projection/line-
/// search scheme that could guarantee zero interpenetration under
/// arbitrarily large steps; (2) `core/src/ccd.rs`'s M10 continuous
/// collision primitive is used to *verify* this mechanism actually
/// resolves deliberately-adversarial starting configurations (see this
/// module's own `barrier_contact_tests`), not wired in as a live per-step
/// gate limiting how far the solver is allowed to move in one step (the
/// report's own conservative-step-size role for CCD) — a real, identified
/// limitation, not an oversight, the same honesty standard M9 held its
/// own twist-deferral to.
const BARRIER_ACTIVE_DISTANCE: f64 = 0.3;
const BARRIER_STIFFNESS: f64 = 1.0;

/// A virtual segment is excluded from barrier contact for any step where
/// its *current* length exceeds this multiple of its thread's own
/// typical continuity-edge length — same mechanism, same reasoning, and
/// (deliberately) the same ratio as `BENDING_MAX_EDGE_RATIO`: a row
/// transition or a ring-closing join's bridge starts out running right
/// through/alongside a straight raw chain (confirmed concretely — without
/// this, the M2-era idempotency test failed: an *already-at-rest* scheme
/// moved anyway, because its own row-transition bridge sat well within
/// `BARRIER_ACTIVE_DISTANCE` of ordinary chain geometry it isn't actually
/// touching, just running near). Re-checked every step against the
/// *current* length, not just the raw one — a ring-closing bridge is
/// *supposed* to shrink as the ring closes, and once it's back to
/// ordinary length it should re-engage in barrier contact like any other
/// segment, the same "live, not one-time" re-check `BENDING_MAX_EDGE_
/// RATIO`'s own per-step guard already established.
const BARRIER_MAX_SEGMENT_RATIO: f64 = BENDING_MAX_EDGE_RATIO;

/// The IPC-style barrier energy for one pair at distance `d` — see
/// `BARRIER_ACTIVE_DISTANCE`'s doc comment for the formula and why this
/// shape (not a linear spring) is the point. Exactly `0.0` at/beyond
/// `d_hat`; grows without bound as `d -> 0`. Only `barrier_energy_
/// derivative` (the force) is actually needed by the solve loop below —
/// this exists so that derivative can be checked against a numerical
/// derivative of the *energy* in this module's own tests, rather than
/// trusting the hand-derived formula on faith.
#[cfg_attr(not(test), allow(dead_code))]
fn barrier_energy(d: f64, d_hat: f64, stiffness: f64) -> f64 {
    if d <= 0.0 || d >= d_hat {
        return 0.0;
    }
    let diff = d - d_hat;
    -stiffness * diff * diff * (d / d_hat).ln()
}

/// `d(barrier_energy)/d(d)` — the scalar the force magnitude is built
/// from (see `barrier_force_on_pair` below for how the sign becomes an
/// actual repulsive direction). Derived by hand (not via finite
/// differences, unlike M9's bending gradient) since this is a single-
/// variable scalar function, not the multi-point vector cross-product
/// expression that made a numerical gradient the safer choice there —
/// verified against a numerical derivative in this module's own tests
/// regardless, the same "don't just trust the algebra" discipline.
fn barrier_energy_derivative(d: f64, d_hat: f64, stiffness: f64) -> f64 {
    if d <= 0.0 || d >= d_hat {
        return 0.0;
    }
    let diff = d - d_hat;
    -stiffness * (2.0 * diff * (d / d_hat).ln() + diff * diff / d)
}

/// How close two of the barrier's virtual segments' *raw* endpoints must
/// be to count as the same structural point (hence not a collision to
/// resist) — the exact value `validate.rs`'s own `SAME_POINT_EPSILON`
/// uses, for the same reason: this is deciding the identical question
/// that module already answers for the final check, just applied to the
/// solver's live segments instead of the finished ones.
const SEGMENT_ADJACENCY_EPS: f64 = 1e-6;

/// A stitch's relaxed **base** position, expressed as a live linear
/// function of one or more *other* (or, for a thread's very first
/// stitch, its own) tracked top positions, plus a fixed offset — mirrors
/// `path.rs`'s own three-case `relaxed_base` computation exactly (zero
/// targets: the working-order predecessor's top, or — for a thread's
/// first stitch — a fixed offset from its own top, the M9 fix; one
/// target: that target's top, offset by the target-relative raw base
/// position; several targets: their tops' average, offset the same way),
/// just kept in this "sources + constant" form instead of evaluated
/// once, so the barrier below can recompute it fresh every step *and*
/// distribute a force back through it via simple linear algebra (a
/// weighted-average base moves by that same weighted average of however
/// much its sources moved).
struct BaseSource {
    /// `(source stitch, weight)` pairs — weights sum to 1.0. Empty is
    /// never produced (every case above has at least one source, even if
    /// it's the stitch's own self for a thread's first, targetless
    /// stitch).
    sources: Vec<(StitchRef, f64)>,
    /// `raw_base - sum(weight * raw_top(source))` — the part of the base
    /// position that *doesn't* move with any tracked variable, computed
    /// once from raw placement so it stays exactly consistent with
    /// wherever `def.height()`/loop-target/post offsets etc. actually
    /// placed this stitch's base in the first place.
    constant_offset: Vec3,
}

/// A thread's typical raw working-order continuity-edge length (median,
/// not mean — robust against the very outliers, like a row transition or
/// a ring-closing join, this exists to help identify — see
/// `BENDING_MAX_EDGE_RATIO`'s and `BARRIER_MAX_SEGMENT_RATIO`'s doc
/// comments, the two places this feeds into). `None` when the thread has
/// no real edges to measure (fewer than 2 stitches, or every stitch
/// coincident).
fn thread_typical_continuity_length(
    raw: &crate::geometry::PlacedScheme,
    thread_idx: usize,
    thread_len: usize,
) -> Option<f64> {
    let mut edge_lengths: Vec<f64> = (1..thread_len)
        .map(|i| {
            raw.threads[thread_idx][i]
                .top
                .distance(&raw.threads[thread_idx][i - 1].top)
        })
        .filter(|d| *d > 1e-9)
        .collect();
    if edge_lengths.is_empty() {
        return None;
    }
    edge_lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(edge_lengths[edge_lengths.len() / 2])
}

fn compute_base_source(
    scheme: &Scheme,
    raw: &crate::geometry::PlacedScheme,
    thread_idx: usize,
    i: usize,
) -> BaseSource {
    let stitch = &scheme.threads[thread_idx].stitches[i];
    let raw_base = raw.threads[thread_idx][i].base;
    match stitch.targets.as_slice() {
        [] => {
            if i > 0 {
                let prev = StitchRef::new(thread_idx, i - 1);
                let raw_prev_top = raw.threads[thread_idx][i - 1].top;
                BaseSource {
                    sources: vec![(prev, 1.0)],
                    constant_offset: raw_base - raw_prev_top,
                }
            } else {
                let self_ref = StitchRef::new(thread_idx, i);
                let raw_self_top = raw.threads[thread_idx][i].top;
                BaseSource {
                    sources: vec![(self_ref, 1.0)],
                    constant_offset: raw_base - raw_self_top,
                }
            }
        }
        [single] => {
            let raw_target_top = raw.threads[single.thread][single.index].top;
            BaseSource {
                sources: vec![(*single, 1.0)],
                constant_offset: raw_base - raw_target_top,
            }
        }
        multiple => {
            let n = multiple.len() as f64;
            let mut raw_sum = Vec3::ZERO;
            let sources = multiple
                .iter()
                .map(|t| {
                    raw_sum = raw_sum + raw.threads[t.thread][t.index].top;
                    (*t, 1.0 / n)
                })
                .collect();
            BaseSource {
                sources,
                constant_offset: raw_base - raw_sum * (1.0 / n),
            }
        }
    }
}

/// One endpoint of a barrier virtual segment: either a stitch's own top
/// (a directly-tracked free variable) or its base (a computed function of
/// one or more other tops — see `BaseSource`).
enum Endpoint {
    Top(StitchRef),
    Base(StitchRef),
}

fn endpoint_value(
    endpoint: &Endpoint,
    positions: &HashMap<StitchRef, Vec3>,
    base_sources: &HashMap<StitchRef, BaseSource>,
) -> Vec3 {
    match endpoint {
        Endpoint::Top(r) => positions[r],
        Endpoint::Base(r) => {
            let source = &base_sources[r];
            source
                .sources
                .iter()
                .fold(source.constant_offset, |acc, (src, weight)| {
                    acc + positions[src] * *weight
                })
        }
    }
}

/// Applies `force` at this endpoint back onto the underlying tracked
/// variable(s) it's actually a function of — for a `Top`, directly; for a
/// `Base`, split across its sources by the same weights that determine
/// its position (the chain rule for a linear function: moving a source
/// by `dx` moves a weighted average of it by `weight * dx`, so a force
/// conjugate to the base's position distributes the same way).
fn apply_force_to_endpoint(
    endpoint: &Endpoint,
    force: Vec3,
    base_sources: &HashMap<StitchRef, BaseSource>,
    forces: &mut HashMap<StitchRef, Vec3>,
) {
    match endpoint {
        Endpoint::Top(r) => {
            let existing = forces[r];
            forces.insert(*r, existing + force);
        }
        Endpoint::Base(r) => {
            for (src, weight) in &base_sources[r].sources {
                let existing = forces[src];
                forces.insert(*src, existing + force * *weight);
            }
        }
    }
}

/// Closest points between two line segments `[p1,q1]` and `[p2,q2]`,
/// returning `(distance, s, t)` where the closest points are
/// `p1 + s*(q1-p1)` and `p2 + t*(q2-p2)`, `s`/`t` clamped to `[0,1]`.
/// Same standard algorithm as `validate.rs`'s `segment_segment_distance`
/// (Ericson, "Real-Time Collision Detection" §5.1.9) — that one only
/// returns the distance, since that's all the discrete post-hoc checker
/// needs; the barrier force below also needs `s`/`t` themselves, to know
/// how to split a force at the closest point back onto each segment's
/// two endpoints.
fn closest_points_on_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f64, f64, f64) {
    const EPS: f64 = 1e-12;
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    let (s, t) = if a <= EPS && e <= EPS {
        (0.0, 0.0)
    } else if a <= EPS {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            (((-c) / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom.abs() > EPS {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = ((-c) / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };

    let c1 = p1 + d1 * s;
    let c2 = p2 + d2 * t;
    (c1.distance(&c2), s, t)
}

/// The continuity edge leading *into* a slip stitch uses this instead of
/// the raw-placement distance every other stitch's continuity edge uses —
/// see that edge's own comment below for why. Near-zero rather than
/// exactly zero: real yarn still has some thickness, and it keeps the
/// spring's direction well-defined from the first relaxation step instead
/// of starting exactly coincident.
const SLIP_STITCH_CONTINUITY_SLACK: f64 = 0.05;
/// Minimum distance same-target siblings try to keep from each other
/// during relaxation, independent of whatever the springs alone produce.
/// Larger than `crate::validate::DEFAULT_YARN_DIAMETER` (0.15) on
/// purpose — keeping real margin *during* relaxation, not just barely
/// legal at the end, is what stops a wide fan of siblings from folding
/// together over many steps (see module docs).
const SIBLING_REPULSION_MIN_DISTANCE: f64 = 0.3;
const SIBLING_REPULSION_STRENGTH: f64 = 0.5;

struct SpringConstraint {
    a: StitchRef,
    b: StitchRef,
    rest_length: f64,
    stiffness: f64,
}

#[derive(Debug, Clone, Default)]
pub struct RelaxedScheme {
    pub positions: HashMap<StitchRef, Vec3>,
}

impl RelaxedScheme {
    pub fn position(&self, r: StitchRef) -> Option<Vec3> {
        self.positions.get(&r).copied()
    }
}

#[derive(Debug, Clone)]
pub struct RelaxationParams {
    pub steps: u32,
    pub dt: f64,
    pub damping: f64,
    /// Stitches held at a fixed position throughout relaxation — e.g. a
    /// held edge, or a deliberately displaced point for a stretch test.
    pub pinned: HashMap<StitchRef, Vec3>,
}

impl Default for RelaxationParams {
    fn default() -> Self {
        RelaxationParams {
            steps: 150,
            dt: 0.1,
            damping: 0.8,
            pinned: HashMap::new(),
        }
    }
}

/// Relaxes `scheme` from its M1 raw placement toward a spring equilibrium.
/// Errors exactly when `place_scheme` would (unknown stitch kind, or a
/// target referenced before it's placed) — relaxation never introduces a
/// new failure mode of its own.
pub fn relax_scheme(
    scheme: &Scheme,
    registry: &StitchRegistry,
    params: &RelaxationParams,
) -> Result<RelaxedScheme, PlacementError> {
    let raw = place_scheme(scheme, registry)?;

    let mut refs: Vec<StitchRef> = Vec::new();
    let mut positions: HashMap<StitchRef, Vec3> = HashMap::new();
    for (thread_idx, thread) in raw.threads.iter().enumerate() {
        for (i, placed_stitch) in thread.iter().enumerate() {
            let r = StitchRef::new(thread_idx, i);
            refs.push(r);
            positions.insert(r, placed_stitch.top);
        }
    }
    for (r, pos) in &params.pinned {
        positions.insert(*r, *pos);
    }

    let mut constraints: Vec<SpringConstraint> = Vec::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        for (i, stitch) in thread.stitches.iter().enumerate() {
            let r = StitchRef::new(thread_idx, i);
            let def = registry
                .get(stitch.kind)
                .ok_or(PlacementError::UnknownStitchKind(stitch.kind))?;

            for target in &stitch.targets {
                constraints.push(SpringConstraint {
                    a: r,
                    b: *target,
                    rest_length: def.height(),
                    stiffness: def.insertion_stiffness(),
                });
            }

            if i > 0 {
                let prev = StitchRef::new(thread_idx, i - 1);
                // Ordinary stitches: the raw-placement distance is a
                // reasonable rest length — it reflects real row/round
                // geometry (stitch spacing), and relaxation's job is to
                // let that shape breathe a little, not erase it.
                //
                // Slip stitches are different: a real `ss` is a
                // near-zero-slack join (the working loop pulled straight
                // through, no extra yarn used), and using raw distance
                // here breaks the common "chain N, slip stitch to the
                // first chain to close it into a ring" pattern. Raw
                // placement lays the chain out straight with no idea a
                // later stitch will join back to its start, so the raw
                // distance from the chain's far end to the join point
                // already happens to equal the chain's own straight-line
                // length — the very edge meant to pull the ring shut
                // starts out already "satisfied" by the straight shape,
                // and relaxation never moves anything (confirmed: all
                // 150 steps are a no-op, not just slow to converge).
                // Giving `ss` its real near-zero slack instead means that
                // edge actually pulls the chain's ends together, rather
                // than sitting inert — see `BENDING_STIFFNESS` above for
                // the other half (M9): without genuine bending resistance,
                // that pull alone just folded the chain back onto itself,
                // since raw placement lays every `ch` on one perfectly
                // straight line (`geometry.rs`'s `lays_out_as_line`) with
                // no out-of-line component for a plain pull to curl
                // around.
                let rest_length = if stitch.kind == SS {
                    SLIP_STITCH_CONTINUITY_SLACK
                } else {
                    raw.threads[thread_idx][i]
                        .top
                        .distance(&raw.threads[thread_idx][i - 1].top)
                };
                constraints.push(SpringConstraint {
                    a: r,
                    b: prev,
                    rest_length,
                    stiffness: CONTINUITY_STIFFNESS,
                });
            }
        }
    }

    // M9 bending resistance (see `BENDING_STIFFNESS`'s doc comment): one
    // triple per interior working-order vertex, i.e. every vertex with
    // both a predecessor and a successor in the same thread. Each triple
    // carries its own **rest curvature binormal**, computed once from
    // *raw* placement (same convention every other spring's rest length
    // already uses) rather than assumed to be zero. This matters: this
    // model's raw placement legitimately has real corners in it that
    // aren't defects to be flattened — e.g. working order jumping from
    // the end of a foundation-chain row back to the row above's first
    // stitch, or a shell's siblings fanning out at a real angle around a
    // shared target. Bending resistance's job is to resist *further*
    // curvature change from whatever raw placement already established,
    // the same way `CONTINUITY_STIFFNESS` resists further *stretch* from
    // the raw distance rather than pulling everything to zero length. A
    // freshly-placed chain has zero raw curvature (it's laid out on one
    // straight line), so `kb_rest` is zero there and this reduces to
    // "resist introducing curvature" exactly as needed for the ring-
    // closure fix — but a scheme with genuine raw corners doesn't get
    // fought by its own bending term.
    // A "fan edge" connects two consecutive-in-working-order stitches that
    // are both converging on the same insertion point — either literal
    // siblings (identical single-element target lists: a shell, a magic-
    // ring round) or the edge from the shared target itself to the first
    // sibling worked into it (`b`'s one target *is* `a`). Per `rod.rs`'s
    // own module docs, insertion-target branching stays outside the rod's
    // own bend math — a fan's angular spread is `SIBLING_REPULSION_*`'s
    // job, tuned and calibrated against real capacity limits (docs §5a)
    // before this milestone existed. A bending triple touching a fan edge
    // would treat that tuned angular spread as "curvature to resist
    // changing," fighting the repulsion calibration — confirmed
    // empirically: excluding only sibling-sibling edges wasn't enough, an
    // 11-into-one-stitch shell (calibrated to correctly fail as physically
    // impossible) still validated cleanly, because the *first* fan edge
    // (target -> its first dependent) was still getting bent-resistance
    // treatment and that alone was enough to reshape the fan's base.
    let is_fan_edge = |thread_idx: usize, a_idx: usize, b_idx: usize| -> bool {
        let a = &scheme.threads[thread_idx].stitches[a_idx];
        let b = &scheme.threads[thread_idx].stitches[b_idx];
        if let [only] = b.targets.as_slice() {
            if *only == StitchRef::new(thread_idx, a_idx) {
                return true;
            }
        }
        matches!(
            (a.targets.as_slice(), b.targets.as_slice()),
            ([x], [y]) if x == y
        )
    };

    let mut bending_triples: Vec<(StitchRef, StitchRef, StitchRef, Vec3, f64)> = Vec::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        if thread.stitches.len() < 3 {
            continue;
        }

        let Some(typical_length) =
            thread_typical_continuity_length(&raw, thread_idx, thread.stitches.len())
        else {
            continue;
        };
        let max_edge_length = typical_length * BENDING_MAX_EDGE_RATIO;

        for i in 1..thread.stitches.len() - 1 {
            if is_fan_edge(thread_idx, i - 1, i) || is_fan_edge(thread_idx, i, i + 1) {
                continue;
            }

            let raw_prev = raw.threads[thread_idx][i - 1].top;
            let raw_curr = raw.threads[thread_idx][i].top;
            let raw_next = raw.threads[thread_idx][i + 1].top;
            let e_prev_raw = raw_curr - raw_prev;
            let e_curr_raw = raw_next - raw_curr;

            // Skip a triple spanning an unusually long raw edge (a row
            // transition, a ring-closing join) — see
            // `BENDING_MAX_EDGE_RATIO`'s doc comment for why length,
            // not turn angle, is the robust signal here.
            if e_prev_raw.length() > max_edge_length || e_curr_raw.length() > max_edge_length {
                continue;
            }

            let kb_rest = curvature_binormal(e_prev_raw, e_curr_raw);
            bending_triples.push((
                StitchRef::new(thread_idx, i - 1),
                StitchRef::new(thread_idx, i),
                StitchRef::new(thread_idx, i + 1),
                kb_rest,
                max_edge_length,
            ));
        }
    }

    // Every pair of stitches sharing a single target (§5a) repels once
    // too close — see module docs for why springs alone don't cover this.
    let mut same_target_groups: HashMap<StitchRef, Vec<StitchRef>> = HashMap::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        for (i, stitch) in thread.stitches.iter().enumerate() {
            if let [single] = stitch.targets.as_slice() {
                same_target_groups
                    .entry(*single)
                    .or_default()
                    .push(StitchRef::new(thread_idx, i));
            }
        }
    }
    let mut repulsion_pairs: Vec<(StitchRef, StitchRef)> = Vec::new();
    for group in same_target_groups.values() {
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                repulsion_pairs.push((group[i], group[j]));
            }
        }
    }

    // A slip stitch's target and its own working-order predecessor are
    // *both* pulled toward the same near-zero-slack junction (`ss`'s
    // insertion spring wants it exactly at its target; its continuity
    // spring wants it within `SLIP_STITCH_CONTINUITY_SLACK` of its
    // predecessor) — without this, nothing stops those two, otherwise
    // unrelated, stitches from collapsing onto each other as ss itself
    // is squeezed toward both simultaneously (confirmed empirically: the
    // ring-closure regression test's remaining failure was exactly this —
    // the chain's start and its second-to-last link landing within 1e-15
    // of each other, a real zero-clearance overlap, not a false
    // positive). Same mechanism and tuning as sibling repulsion above,
    // for the same reason: keep genuinely distinct points of yarn from
    // occupying the same space during relaxation, not just after it.
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        for (i, stitch) in thread.stitches.iter().enumerate() {
            if stitch.kind == SS {
                if let ([target], true) = (stitch.targets.as_slice(), i > 0) {
                    let predecessor = StitchRef::new(thread_idx, i - 1);
                    if *target != predecessor {
                        repulsion_pairs.push((*target, predecessor));
                    }
                }
            }
        }
    }

    // M12: barrier contact against the *full reconstructed yarn path*
    // (every stitch's own base-to-top body, and the bridge to its
    // working-order predecessor), not just stitch tops — see
    // `BARRIER_ACTIVE_DISTANCE`'s doc comment for why the earlier M11
    // top-only version wasn't enough. `base_sources`/`virtual_segments`
    // mirror `path.rs`'s own relaxed-base computation (same three cases:
    // zero targets, one target, several) so this stays in lockstep with
    // what the final validator actually checks, rather than a separately
    // hand-maintained approximation that could drift out of sync with it.
    let mut base_sources: HashMap<StitchRef, BaseSource> = HashMap::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        for i in 0..thread.stitches.len() {
            let r = StitchRef::new(thread_idx, i);
            base_sources.insert(r, compute_base_source(scheme, &raw, thread_idx, i));
        }
    }

    // Every stitch contributes its own body (base -> top) and, from the
    // second stitch of a thread onward, a bridge from its working-order
    // predecessor's top to its own base. Raw endpoints are carried
    // alongside for the adjacency test below — same convention
    // `validate.rs`'s own `PathSegment` uses, and for the same reason:
    // whether two points are *structurally* the same point is a fact
    // about the graph, fixed at raw-placement time, not something that
    // should change because relaxation moved things. `max_length` (see
    // `BARRIER_MAX_SEGMENT_RATIO`) flags a segment that's an unusually
    // long structural bridge for its own thread (a row transition, a
    // ring-closing join) — raw placement lays these out running right
    // through/alongside other, unrelated geometry (a straight chain's own
    // continuation, before anything has curled into its final relaxed
    // shape), which a plain distance check can't tell apart from a real
    // collision.
    let mut virtual_segments: Vec<(Endpoint, Endpoint, Vec3, Vec3, f64)> = Vec::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        let max_length = thread_typical_continuity_length(&raw, thread_idx, thread.stitches.len())
            .map(|typical| typical * BARRIER_MAX_SEGMENT_RATIO)
            .unwrap_or(f64::INFINITY);
        for i in 0..thread.stitches.len() {
            let r = StitchRef::new(thread_idx, i);
            let raw_top = raw.threads[thread_idx][i].top;
            let raw_base = raw.threads[thread_idx][i].base;
            virtual_segments.push((
                Endpoint::Base(r),
                Endpoint::Top(r),
                raw_base,
                raw_top,
                max_length,
            ));
            if i > 0 {
                let prev = StitchRef::new(thread_idx, i - 1);
                let raw_prev_top = raw.threads[thread_idx][i - 1].top;
                virtual_segments.push((
                    Endpoint::Top(prev),
                    Endpoint::Base(r),
                    raw_prev_top,
                    raw_base,
                    max_length,
                ));
            }
        }
    }

    // A pair of (virtual) segments is excluded from barrier contact when
    // they share a raw endpoint — this is deliberately the *same* rule
    // `validate.rs`'s `segments_are_adjacent` uses for the exact same
    // reason (see that module's own docs): whether two points are
    // structurally "the same point" (a stitch's base *is* its target's
    // top; two siblings' bases *are* the same shared target) is a graph
    // fact, decided once from raw placement, not something relaxation
    // should ever need to fight. Anything the final checker would exempt
    // as structural, the solver now also leaves alone — anything it
    // wouldn't, the solver actively keeps apart during settling instead
    // of only finding out afterward. Precomputed once: raw positions
    // never change across steps. (The length-based exclusion is checked
    // separately, dynamically, every step — see the main loop below.)
    let mut segment_pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..virtual_segments.len() {
        for j in (i + 1)..virtual_segments.len() {
            let (_, _, a_raw_start, a_raw_end, _) = &virtual_segments[i];
            let (_, _, b_raw_start, b_raw_end, _) = &virtual_segments[j];
            let adjacent = a_raw_start.distance(b_raw_start) < SEGMENT_ADJACENCY_EPS
                || a_raw_start.distance(b_raw_end) < SEGMENT_ADJACENCY_EPS
                || a_raw_end.distance(b_raw_start) < SEGMENT_ADJACENCY_EPS
                || a_raw_end.distance(b_raw_end) < SEGMENT_ADJACENCY_EPS;
            if !adjacent {
                segment_pairs.push((i, j));
            }
        }
    }

    let mut velocities: HashMap<StitchRef, Vec3> = refs.iter().map(|r| (*r, Vec3::ZERO)).collect();

    for _ in 0..params.steps {
        let mut forces: HashMap<StitchRef, Vec3> = refs.iter().map(|r| (*r, Vec3::ZERO)).collect();

        for c in &constraints {
            let pa = positions[&c.a];
            let pb = positions[&c.b];
            let delta = pb - pa;
            let dist = delta.length();
            if dist < 1e-9 {
                continue; // coincident points: no defined direction, skip.
            }
            let dir = delta * (1.0 / dist);
            let force_on_a = dir * (c.stiffness * (dist - c.rest_length));

            let fa = forces[&c.a];
            let fb = forces[&c.b];
            forces.insert(c.a, fa + force_on_a);
            forces.insert(c.b, fb - force_on_a);
        }

        for &(prev, curr, next, kb_rest, max_edge_length) in &bending_triples {
            let p_prev = positions[&prev];
            let p_curr = positions[&curr];
            let p_next = positions[&next];

            // Re-check the *length* exclusion against the current (not
            // just raw) configuration every step — see
            // `BENDING_MAX_EDGE_RATIO`'s doc comment. A triple that
            // started well clear of the excluded zone can shrink back
            // into ordinary range as relaxation proceeds (that's the
            // whole point of a ring closing up), so this has to be a
            // live check, not just a one-time filter at construction.
            let e_prev = p_curr - p_prev;
            let e_curr = p_next - p_curr;
            if e_prev.length() > max_edge_length || e_curr.length() > max_edge_length {
                continue;
            }

            // Separately: a genuine numerical-safety guard, independent
            // of the length-based topological exclusion above. Length
            // identifies *which* edges are row-transition/ring-closing
            // jumps (for calibration correctness — see
            // `BENDING_MAX_EDGE_RATIO`), but the actual numerical
            // singularity `curvature_binormal` warns about is a function
            // of *angle* alone (`|e_prev||e_curr|(1 + cos(theta))`, which
            // -> 0 as theta -> 180 degrees regardless of edge length) — a
            // short, sharp U-turn is exactly as numerically dangerous for
            // finite differences as a long one. This can develop mid-
            // relaxation even on a triple that passed the length check
            // (confirmed empirically: without this second guard, the
            // ring-closure test still diverged even after the length
            // exclusion alone was enough to fix every calibration test),
            // so both checks run independently, every step.
            let cos_turn = e_prev.normalized().dot(e_curr.normalized());
            if cos_turn < BENDING_SAFETY_MIN_TURN_COSINE {
                continue;
            }

            let (f_prev, f_curr, f_next) =
                bending_energy_gradient(p_prev, p_curr, p_next, kb_rest, BENDING_STIFFNESS);
            let fp = forces[&prev];
            let fc = forces[&curr];
            let fn_ = forces[&next];
            forces.insert(prev, fp + f_prev);
            forces.insert(curr, fc + f_curr);
            forces.insert(next, fn_ + f_next);
        }

        for &(a, b) in &repulsion_pairs {
            let pa = positions[&a];
            let pb = positions[&b];
            let delta = pb - pa;
            let dist = delta.length();
            if !(1e-9..SIBLING_REPULSION_MIN_DISTANCE).contains(&dist) {
                continue;
            }
            let dir = delta * (1.0 / dist);
            let push = dir * (SIBLING_REPULSION_STRENGTH * (SIBLING_REPULSION_MIN_DISTANCE - dist));
            let fa = forces[&a];
            let fb = forces[&b];
            forces.insert(a, fa - push);
            forces.insert(b, fb + push);
        }

        for &(i, j) in &segment_pairs {
            let (a0, a1, _, _, a_max_length) = &virtual_segments[i];
            let (b0, b1, _, _, b_max_length) = &virtual_segments[j];
            let pa0 = endpoint_value(a0, &positions, &base_sources);
            let pa1 = endpoint_value(a1, &positions, &base_sources);
            let pb0 = endpoint_value(b0, &positions, &base_sources);
            let pb1 = endpoint_value(b1, &positions, &base_sources);

            // Live re-check (not just at raw) — see
            // `BARRIER_MAX_SEGMENT_RATIO`'s doc comment for why an
            // unusually long segment for its thread gets skipped, and why
            // this has to be checked against the *current* length every
            // step rather than decided once.
            if pa0.distance(&pa1) > *a_max_length || pb0.distance(&pb1) > *b_max_length {
                continue;
            }

            let (dist, s, t) = closest_points_on_segments(pa0, pa1, pb0, pb1);
            if !(1e-9..BARRIER_ACTIVE_DISTANCE).contains(&dist) {
                continue; // exactly zero beyond d_hat, by construction — see the barrier's own doc comment.
            }
            let pa = pa0 + (pa1 - pa0) * s;
            let pb = pb0 + (pb1 - pb0) * t;
            let dir = (pb - pa) * (1.0 / dist);
            let derivative =
                barrier_energy_derivative(dist, BARRIER_ACTIVE_DISTANCE, BARRIER_STIFFNESS);
            // Force at the closest point on segment `a` = derivative *
            // dir (same sign derivation as the barrier's own doc
            // comment); `dir` points a -> b, and `derivative` is
            // negative for d < d_hat, so this pushes the closest point
            // on `a` *away* from `b`. That force is then distributed
            // back onto `a`'s two endpoints by the closest point's own
            // barycentric weight (`1-s` toward `a0`, `s` toward `a1`) —
            // standard virtual-work force splitting for a point
            // parametrised linearly along a segment — and, since each
            // endpoint may itself be a computed `base` (a weighted
            // combination of one or more *other* stitches' tops, not a
            // free variable of its own), `apply_force_to_endpoint`
            // carries that split through the chain rule one more level.
            let force_on_a = dir * derivative;
            apply_force_to_endpoint(a0, force_on_a * (1.0 - s), &base_sources, &mut forces);
            apply_force_to_endpoint(a1, force_on_a * s, &base_sources, &mut forces);
            apply_force_to_endpoint(b0, force_on_a * (-(1.0 - t)), &base_sources, &mut forces);
            apply_force_to_endpoint(b1, force_on_a * (-t), &base_sources, &mut forces);
        }

        for r in &refs {
            if params.pinned.contains_key(r) {
                continue;
            }
            let new_v = (velocities[r] + forces[r] * params.dt) * params.damping;
            velocities.insert(*r, new_v);
            positions.insert(*r, positions[r] + new_v * params.dt);
        }
    }

    Ok(RelaxedScheme { positions })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::stitch::{CH, DC, DTR, TR};

    fn ref_at(thread: usize, index: usize) -> StitchRef {
        StitchRef::new(thread, index)
    }

    /// A 2-row swatch: a foundation chain of `width` links, then a row of
    /// `stitch_kind` on top, each targeting the chain link below it.
    fn two_row_swatch(width: usize, stitch_kind: crate::stitch::StitchId) -> Scheme {
        let mut thread = Thread::new();
        for _ in 0..width {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        for i in 0..width {
            thread
                .stitches
                .push(StitchInstance::new(stitch_kind, vec![ref_at(0, i)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);
        scheme
    }

    #[test]
    fn relaxing_an_already_satisfied_scheme_barely_moves_it() {
        // No increases, no pins: raw M1 placement already satisfies every
        // spring's rest length exactly, so relaxation should be close to
        // a no-op.
        let registry = StitchRegistry::with_uk_basics();
        let scheme = two_row_swatch(4, DC);
        let raw = place_scheme(&scheme, &registry).unwrap();
        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();

        for (thread_idx, thread) in raw.threads.iter().enumerate() {
            for (i, stitch) in thread.iter().enumerate() {
                let r = StitchRef::new(thread_idx, i);
                let relaxed_pos = relaxed.position(r).unwrap();
                assert!(
                    stitch.top.distance(&relaxed_pos) < 1e-6,
                    "expected stitch {:?} to stay near its raw position, moved to {:?} from {:?}",
                    r,
                    relaxed_pos,
                    stitch.top
                );
            }
        }
    }

    #[test]
    fn pinned_stitches_stay_exactly_at_their_pinned_position() {
        let registry = StitchRegistry::with_uk_basics();
        let scheme = two_row_swatch(3, TR);
        let pin_target = Vec3::new(5.0, 1.0, 2.0);
        let mut pinned = HashMap::new();
        pinned.insert(ref_at(0, 5), pin_target); // last tr stitch
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        assert_eq!(relaxed.position(ref_at(0, 5)).unwrap(), pin_target);
    }

    #[test]
    fn dense_stitch_swatch_resists_a_pull_more_than_an_open_stitch_swatch() {
        // Pull the last stitch of the top row sideways and hold the whole
        // foundation chain fixed; measure how far the *next* stitch over
        // (not itself pinned) gets dragged along, for a dense stitch (dc)
        // vs. a taller/softer one (tr). Docs §6: elasticity is a topology
        // property, so this must come purely from stitch kind.
        let width = 5;
        let pull = Vec3::new(0.0, 3.0, 0.0);

        let displacement_for = |kind: crate::stitch::StitchId| -> f64 {
            let registry = StitchRegistry::with_uk_basics();
            let scheme = two_row_swatch(width, kind);
            let raw = place_scheme(&scheme, &registry).unwrap();

            let last_row2 = ref_at(0, 2 * width - 1);
            let neighbor_row2 = ref_at(0, 2 * width - 2);
            let raw_neighbor_pos = raw.threads[0][2 * width - 2].top;

            let mut pinned = HashMap::new();
            // Hold the whole foundation chain fixed.
            for i in 0..width {
                pinned.insert(ref_at(0, i), raw.threads[0][i].top);
            }
            // Pull the last top-row stitch away from its rest position.
            let raw_last_pos = raw.threads[0][2 * width - 1].top;
            pinned.insert(last_row2, raw_last_pos + pull);

            let params = RelaxationParams {
                pinned,
                ..RelaxationParams::default()
            };
            let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
            let relaxed_neighbor_pos = relaxed.position(neighbor_row2).unwrap();
            raw_neighbor_pos.distance(&relaxed_neighbor_pos)
        };

        let dc_displacement = displacement_for(DC);
        let tr_displacement = displacement_for(TR);
        let dtr_displacement = displacement_for(DTR);

        assert!(
            dc_displacement < tr_displacement,
            "expected the dense dc swatch to resist the pull more than tr: dc={dc_displacement}, tr={tr_displacement}"
        );
        assert!(
            tr_displacement < dtr_displacement,
            "expected tr to resist more than the even taller/softer dtr: tr={tr_displacement}, dtr={dtr_displacement}"
        );
    }

    #[test]
    fn sibling_repulsion_prevents_non_adjacent_shell_members_from_coinciding() {
        // Regression guard: before sibling repulsion existed, a wide
        // shell (7+ stitches into one target) could relax two
        // *non-adjacent* siblings (e.g. index 6 and index 7 of a 7-dc
        // shell) to ~1e-16 apart — springs only constrain a stitch to its
        // own target and its immediate working-order neighbour, never to
        // siblings further round the fan. Confirmed empirically that this
        // no longer happens.
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![]));
        for _ in 0..7 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(0, 0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        for i in 1..=7 {
            for j in (i + 1)..=7 {
                let d = relaxed
                    .position(ref_at(0, i))
                    .unwrap()
                    .distance(&relaxed.position(ref_at(0, j)).unwrap());
                assert!(
                    d > SIBLING_REPULSION_MIN_DISTANCE * 0.5,
                    "siblings {i} and {j} folded too close together: {d}"
                );
            }
        }
    }

    #[test]
    fn relaxed_positions_are_always_finite() {
        let registry = StitchRegistry::with_uk_basics();
        let scheme = two_row_swatch(4, DTR);
        let raw = place_scheme(&scheme, &registry).unwrap();
        let mut pinned = HashMap::new();
        pinned.insert(
            ref_at(0, 7),
            raw.threads[0][7].top + Vec3::new(10.0, -4.0, 6.0),
        );
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        for pos in relaxed.positions.values() {
            assert!(pos.is_finite(), "non-finite relaxed position: {:?}", pos);
        }
    }
}

#[cfg(test)]
mod slip_stitch_join_tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::path::relaxed_yarn_segments;
    use crate::stitch::CH;
    use crate::validate::{check_self_intersections, DEFAULT_YARN_DIAMETER};

    /// Regression test for a real, reported bug: joining a chain into a
    /// ring with a slip stitch (`ch 6, ss` into the first chain — an
    /// extremely common real technique) used to render straight with
    /// flagged intersections instead of closing into a circle. Two
    /// distinct fixes were needed, in order, and both are load-bearing —
    /// see HANDOVER.md's M9 entry for the full account:
    /// 1. `ss`'s continuity edge needed its own real near-zero slack
    ///    (`SLIP_STITCH_CONTINUITY_SLACK`) instead of the raw-placement
    ///    distance every other stitch's continuity edge uses — without
    ///    this, the join spring was a literal no-op (confirmed
    ///    empirically: 150 relaxation steps moved nothing at all, since
    ///    raw placement's straight chain already exactly satisfies that
    ///    spring's naive rest length).
    /// 2. Real bending resistance (`BENDING_STIFFNESS`, M9's Discrete-
    ///    Elastic-Rod-inspired second-neighbour spring — see `rod.rs`)
    ///    plus a tiny deterministic symmetry-breaking seed in `ch`'s raw
    ///    placement (`geometry.rs`'s `CHAIN_SYMMETRY_BREAK_AMPLITUDE`) —
    ///    without *both*, a working join spring alone just folded the
    ///    chain back onto itself (confirmed: bending resistance alone
    ///    doesn't help either, since a perfectly collinear input makes
    ///    the bending force itself compute to exactly zero too — nothing
    ///    breaks the symmetry on its own).
    #[test]
    fn slip_stitch_join_closes_a_chain_into_a_genuine_non_intersecting_ring() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..6 {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        thread
            .stitches
            .push(StitchInstance::new(SS, vec![StitchRef::new(0, 0)]));
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let raw = place_scheme(&scheme, &registry).unwrap();
        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();

        // The far end of the chain (index 5) has been pulled
        // substantially away from its raw (straight-line) position.
        let raw_far_end = raw.threads[0][5].top;
        let relaxed_far_end = relaxed.position(StitchRef::new(0, 5)).unwrap();
        assert!(
            raw_far_end.distance(&relaxed_far_end) > 1.0,
            "expected the slip-stitch join to pull the chain's far end substantially \
             toward the start; raw={:?} relaxed={:?}",
            raw_far_end,
            relaxed_far_end
        );

        // The slip stitch itself lands close to its target (chain 0) —
        // a real near-zero-slack join, not floating far away.
        let ss_pos = relaxed.position(StitchRef::new(0, 6)).unwrap();
        let target_pos = relaxed.position(StitchRef::new(0, 0)).unwrap();
        assert!(
            ss_pos.distance(&target_pos) < 0.5,
            "expected the slip stitch to land close to its target: ss={:?} target={:?}",
            ss_pos,
            target_pos
        );

        // The actual acceptance bar: the closed ring is a genuine,
        // self-intersection-free loop, not just "moved."
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();
        let report = check_self_intersections(&segments, DEFAULT_YARN_DIAMETER);
        assert!(
            report.ok,
            "expected the closed ring to validate cleanly, found {} violation(s): {:?}",
            report.violations.len(),
            report.violations
        );
    }
}

#[cfg(test)]
mod barrier_contact_tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::path::relaxed_yarn_segments;
    use crate::stitch::DC;
    use crate::validate::{check_self_intersections, DEFAULT_YARN_DIAMETER};

    fn ref_at(thread: usize, index: usize) -> StitchRef {
        StitchRef::new(thread, index)
    }

    fn approx_eq(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() < tol,
            "expected {a} to approximately equal {b}"
        );
    }

    mod barrier_math_tests {
        use super::*;

        #[test]
        fn zero_at_and_beyond_the_active_distance() {
            approx_eq(barrier_energy(0.3, 0.3, 1.0), 0.0, 1e-12);
            approx_eq(barrier_energy(0.5, 0.3, 1.0), 0.0, 1e-12);
            approx_eq(barrier_energy_derivative(0.3, 0.3, 1.0), 0.0, 1e-12);
            approx_eq(barrier_energy_derivative(0.5, 0.3, 1.0), 0.0, 1e-12);
        }

        #[test]
        fn positive_energy_and_negative_derivative_inside_the_active_range() {
            let e = barrier_energy(0.15, 0.3, 1.0);
            assert!(e > 0.0, "expected positive barrier energy, got {e}");
            let d = barrier_energy_derivative(0.15, 0.3, 1.0);
            assert!(d < 0.0, "expected negative derivative (repulsive), got {d}");
        }

        #[test]
        fn grows_without_bound_as_distance_approaches_zero() {
            let far = barrier_energy(0.29, 0.3, 1.0);
            let near = barrier_energy(0.01, 0.3, 1.0);
            let very_near = barrier_energy(0.001, 0.3, 1.0);
            assert!(
                near > far,
                "expected energy to grow as d shrinks: far={far}, near={near}"
            );
            assert!(
                very_near > near,
                "expected energy to keep growing closer to zero: near={near}, very_near={very_near}"
            );
        }

        #[test]
        fn derivative_matches_a_numerical_derivative_of_the_energy() {
            // Independent check of the hand-derived formula against a
            // central finite difference - the same "don't just trust the
            // algebra" discipline used throughout this project.
            let d_hat = 0.3;
            let stiffness = 0.7;
            const EPS: f64 = 1e-6;
            for d in [0.05, 0.1, 0.15, 0.2, 0.25, 0.29] {
                let analytic = barrier_energy_derivative(d, d_hat, stiffness);
                let numerical = (barrier_energy(d + EPS, d_hat, stiffness)
                    - barrier_energy(d - EPS, d_hat, stiffness))
                    / (2.0 * EPS);
                approx_eq(analytic, numerical, 1e-3);
            }
        }

        #[test]
        fn never_produces_nan_or_infinite_values_across_the_active_range() {
            let d_hat = 0.3;
            let stiffness = 0.05;
            let mut d = 1e-4;
            while d < d_hat {
                assert!(
                    barrier_energy(d, d_hat, stiffness).is_finite(),
                    "non-finite energy at d={d}"
                );
                assert!(
                    barrier_energy_derivative(d, d_hat, stiffness).is_finite(),
                    "non-finite derivative at d={d}"
                );
                d += 0.001;
            }
        }
    }

    /// Builds two *independent* single-stitch dc's, each targeting its
    /// own pinned anchor `anchor_distance` apart — deliberately not two
    /// multi-sibling fans/shells. An earlier version of this test used
    /// two 5-sibling shells and found a real, separate issue: an external
    /// force (this test's own adversarial setup, or M11's barrier itself)
    /// pushing unevenly on a fan's members can swap their angular order,
    /// crossing the *bridges* between them — since `SIBLING_REPULSION_*`
    /// only ever kept siblings' *tops* apart, never their connecting
    /// bridges, this was already a latent gap in the M2-era mechanism,
    /// just never exercised by anything before. That's real and worth
    /// fixing eventually, but it's a *sibling*-repulsion limitation, not
    /// an M11 one — conflating the two in one test would make M11 own a
    /// bug that predates it. Two lone dc's, each the *only* thing worked
    /// into its own anchor, have no fan/angular-ordering question at all:
    /// each is governed purely by its own insertion spring, so this
    /// isolates exactly the case M11 is actually about — a pair with *no
    /// existing repulsion mechanism at all* (not siblings, not graph-
    /// adjacent) that used to be free to land on top of each other.
    fn two_independent_stitches_pinned_apart(anchor_distance: f64) -> (Scheme, RelaxationParams) {
        let mut thread = Thread::new();
        thread
            .stitches
            .push(StitchInstance::new(crate::stitch::CH, vec![])); // 0: anchor A
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0, 0)])); // 1: free A
        thread
            .stitches
            .push(StitchInstance::new(crate::stitch::CH, vec![])); // 2: buffer
        thread
            .stitches
            .push(StitchInstance::new(crate::stitch::CH, vec![])); // 3: anchor B
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0, 3)])); // 4: free B
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let mut pinned = HashMap::new();
        pinned.insert(ref_at(0, 0), Vec3::new(0.0, 0.0, 0.0));
        // The buffer only exists to stop "free A"'s working-order
        // continuity edge landing directly on "anchor B" (an artifact of
        // this being one contiguous thread — multi-thread schemes are
        // still deferred, docs §4a); pinned off to the side, clear of
        // either dc's own ~1.0 insertion-spring radius, so it can't
        // distort either one.
        pinned.insert(ref_at(0, 2), Vec3::new(anchor_distance / 2.0, 1.5, 0.0));
        pinned.insert(ref_at(0, 3), Vec3::new(anchor_distance, 0.0, 0.0));
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };
        (scheme, params)
    }

    /// The actual M11 acceptance bar: a deliberately-adversarial starting
    /// configuration that used to only get flagged by `validate.rs` now
    /// settles into a genuinely non-intersecting shape instead. Two
    /// unrelated dc's, each the sole occupant of its own target, aren't
    /// siblings and aren't graph-adjacent — before M11, *nothing* stopped
    /// them landing on top of each other if their anchors happened to be
    /// close (docs §5a's sibling repulsion only ever covered same-target
    /// pairs). Pinning the two anchors within 2x either dc's own ~1.0
    /// natural radius (confirmed empirically) forces exactly that.
    #[test]
    fn two_independent_stitches_pinned_close_together_no_longer_collide() {
        let registry = StitchRegistry::with_uk_basics();
        let (scheme, params) = two_independent_stitches_pinned_apart(1.3);

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();
        let report = check_self_intersections(&segments, DEFAULT_YARN_DIAMETER);
        assert!(
            report.ok,
            "expected the two pinned-close stitches to settle apart, found {} violation(s): {:?}",
            report.violations.len(),
            report.violations
        );
    }

    /// Same adversarial setup, checked directly against `ccd.rs`'s M10
    /// primitive rather than `validate.rs`'s discrete end-state check -
    /// confirms the barrier genuinely prevents *passing through* during
    /// relaxation, not just landing separated by the end (the actual
    /// failure mode M10's own module docs describe: tunnelling that a
    /// discrete-only check can miss even when the final positions look
    /// fine). Compares each segment's *raw* (M1) position against its
    /// *relaxed* (M2/M9/M11) position as one large motion, the same
    /// stress-testing convention `ccd.rs`'s own scheme-integration test
    /// uses.
    #[test]
    fn barrier_prevents_tunnelling_not_just_final_overlap() {
        let registry = StitchRegistry::with_uk_basics();
        let (scheme, params) = two_independent_stitches_pinned_apart(1.3);

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();

        let mut tunnelled = Vec::new();
        for i in 0..segments.len() {
            for j in (i + 1)..segments.len() {
                let a = &segments[i];
                let b = &segments[j];
                if let Some(t) = crate::ccd::edge_edge_time_of_contact(
                    crate::ccd::PointMotion::new(a.raw_start, a.start),
                    crate::ccd::PointMotion::new(a.raw_end, a.end),
                    crate::ccd::PointMotion::new(b.raw_start, b.start),
                    crate::ccd::PointMotion::new(b.raw_end, b.end),
                ) {
                    // Endpoint-only contact (t genuinely at the very start,
                    // e.g. two segments that share a raw vertex) isn't
                    // tunnelling - only a mid-motion crossing is.
                    if t > 1e-6 {
                        tunnelled.push((i, j, t));
                    }
                }
            }
        }
        assert!(
            tunnelled.is_empty(),
            "expected no mid-relaxation tunnelling between the two shells, found: {:?}",
            tunnelled
        );
    }
}

/// M12: regression coverage for the raw-placement fix to §5a's long-
/// documented "local density across different targets" limitation (see
/// `geometry.rs`'s `NEIGHBOR_ARC_SAFETY_FACTOR`/fan-rotation doc comments).
/// Neither scenario here is fully clean — both are honestly reported as
/// known, narrow residual limitations in HANDOVER.md/GOALS.md's M12 entry
/// — but both are dramatically better than before the fix, and these
/// tests exist to catch a *regression* back toward the old numbers, not
/// to claim full resolution.
#[cfg(test)]
mod density_regression_tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::path::relaxed_yarn_segments;
    use crate::stitch::{DC, MR};
    use crate::validate::{check_self_intersections, DEFAULT_YARN_DIAMETER};

    /// A 6-stitch round-1 ring where every round-1 stitch gets 2 round-2
    /// children (18 stitches total) — the scenario that first exposed the
    /// "every fan bulges the same fixed global direction regardless of
    /// where its own target sits on the ring" bug. Before the M12
    /// raw-placement fix (neighbour-aware angular budget +
    /// per-parent-angle rotation in `geometry.rs`), this produced 25
    /// self-intersection violations; after, it produces at most a handful,
    /// all clustered at the ring's own wrap-around seam (round-1's last
    /// member's own children vs. the long working-order bridge back to
    /// round-1's first member to start round 2) — a distinct, narrower,
    /// separately-understood limitation: that bridge is deliberately
    /// excluded from barrier contact (`BARRIER_MAX_SEGMENT_RATIO`) because
    /// treating it as an ordinary short segment produced false positives
    /// elsewhere, so it isn't pushed away from geometry it passes close to.
    #[test]
    fn nested_round_density_is_far_better_than_pre_m12_baseline() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(MR, vec![]));
        for _ in 0..6 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![StitchRef::new(0, 0)]));
        }
        for i in 1..=6 {
            for _ in 0..2 {
                thread
                    .stitches
                    .push(StitchInstance::new(DC, vec![StitchRef::new(0, i)]));
            }
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);
        let params = RelaxationParams::default();
        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();
        let report = check_self_intersections(&segments, DEFAULT_YARN_DIAMETER);
        assert!(
            report.violations.len() <= 4,
            "expected the M12 raw-placement fix's improvement to hold \
             (pre-fix baseline was 25 violations); got {} — a regression: {:?}",
            report.violations.len(),
            report.violations
        );
    }

    /// Two independent 5-dc shells, pinned artificially close together
    /// (1.3 units apart) to stress-test contact under strong external
    /// pull — not a nested-fan case (the shells share no common ancestor
    /// fan), so the M12 raw-placement fix doesn't apply here; this is the
    /// M11-documented "a fan's own siblings can still cross under a
    /// strong enough external pull" limitation, narrowed (M12's
    /// segment-aware barrier now also covers stitch *bodies*, not just
    /// tops) but not eliminated — same-target siblings squeezed together
    /// by the pull toward the other shell can still end up closer than
    /// `DEFAULT_YARN_DIAMETER`. Kept as a regression guard, not a claim
    /// of resolution.
    #[test]
    fn two_pinned_close_shells_do_not_regress_past_the_m12_baseline() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(MR, vec![]));
        for _ in 0..5 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![StitchRef::new(0, 0)]));
        }
        thread
            .stitches
            .push(StitchInstance::new(crate::stitch::CH, vec![]));
        thread.stitches.push(StitchInstance::new(MR, vec![]));
        for _ in 0..5 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![StitchRef::new(0, 7)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);
        let mut pinned = HashMap::new();
        pinned.insert(StitchRef::new(0, 0), Vec3::new(0.0, 0.0, 0.0));
        pinned.insert(StitchRef::new(0, 6), Vec3::new(0.65, 1.5, 0.0));
        pinned.insert(StitchRef::new(0, 7), Vec3::new(1.3, 0.0, 0.0));
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };
        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();
        let report = check_self_intersections(&segments, DEFAULT_YARN_DIAMETER);
        assert!(
            report.violations.len() <= 5,
            "expected at most the known M11/M12 residual (same-fan \
             compression under strong external pull); got {} violations \
             — investigate before raising this bound: {:?}",
            report.violations.len(),
            report.violations
        );
    }
}
