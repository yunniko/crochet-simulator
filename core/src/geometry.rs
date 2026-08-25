//! Raw placement geometry for M1 — see GOALS.md M1 and
//! docs/crochet-context.md §6 note: this is deliberately the *raw*
//! placement, with no relaxation/elasticity (that's M2) and no
//! self-intersection validation (that's M3). It exists to prove the
//! insertion graph produces sane 3D point/segment coordinates at all.
//!
//! §5a (multi-way shares): siblings sharing a single target are arranged
//! **radially** around it, not linearly — see `radius_and_wave`. How far
//! out, and whether excess siblings bulge into the third dimension, comes
//! from the target's `CapacityStyle` (`crate::stitch`), calibrated against
//! the Owner's own crochet experience (docs §5a).

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::graph::{LoopTarget, Scheme, StitchRef};
use crate::stitch::{CapacityStyle, StitchId, StitchRegistry};
use crate::vec3::Vec3;

/// Horizontal step between successive links of a chain with no target.
const CHAIN_STEP_X: f64 = 1.0;
/// How many siblings comfortably share one target before it's under
/// strain — the Owner's own calibration (docs §5a): "seven is hard but
/// possible [into one stitch], eleven won't fit"; a tightened magic ring
/// reads as a flat circle at 6-8 and ripples in 3D beyond that. One
/// shared threshold serves both cases reasonably (7 sits inside both
/// ranges) rather than needing separate tuning per target kind.
const COMFORTABLE_CAPACITY: usize = 7;
/// Radius siblings sit at, at or below comfortable capacity, for a
/// `Fixed` target, and the radius a `TightenedRing` plateaus at once it
/// reaches comfortable capacity.
const BASE_RING_RADIUS: f64 = 0.4;
/// Target arc-length between adjacent siblings used to *grow* a
/// `TightenedRing`'s radius with sibling count, below its plateau (docs
/// §5a: narrow at 3-5 siblings, reaching the flat plateau by 6-8).
const MIN_STITCH_ARC_WIDTH: f64 = 0.5;
/// Same idea as `MIN_STITCH_ARC_WIDTH`, but for `Elastic` targets (`ch`,
/// an open ring) — deliberately a *separate* constant, not shared with
/// `MIN_STITCH_ARC_WIDTH`: they were coupled once, and widening it to fix
/// an `Elastic`-target near-miss immediately broke `TightenedRing`'s
/// narrow/flat calibration, which reads that same constant. `Elastic`
/// targets have no plateau to protect and no Owner-specified narrow/flat
/// boundary to hit, so this can be tuned purely for clearance.
const ELASTIC_ARC_WIDTH: f64 = 0.9;
/// Maximum out-of-plane bulge for siblings past comfortable capacity on a
/// `Fixed` or `TightenedRing` target — a 3D wave/ripple, not in-plane
/// crowding. Deliberately bounded (see `radius_and_wave`): capacity
/// doesn't grow without limit just because more siblings arrive, so a
/// truly excessive count (the Owner: "much more ... can not be
/// tightened ... because of yarn thickness") still ends up close enough
/// to trip `crate::validate`'s self-intersection check, rather than the
/// engine quietly finding room for it forever.
const WAVE_AMPLITUDE: f64 = 0.4;
/// Depth offset applied to a front/back post stitch's base, along the
/// same y axis the sibling ring (`radius_and_wave`) also uses — see that
/// axis's fuller explanation below. A post stitch reaches around an
/// earlier stitch's post instead of inserting into its top loops (docs
/// §2), so its yarn path genuinely does not occupy the same space as the
/// stitch(es) it reaches past — this is what lets M3's self-intersection
/// checker treat it correctly as *not* a collision without needing to
/// special-case "is this a post stitch." Larger than
/// `crate::validate::DEFAULT_YARN_DIAMETER` on purpose — and larger than
/// `LOOP_HALF_OFFSET` too: reaching around a whole post is a bigger
/// displacement than picking one strand of a top loop.
const POST_DEPTH_OFFSET: f64 = 0.7;
/// Offset applied for `FrontOnly`/`BackOnly` (docs §2, §5b): working into
/// only one strand of the target's top "V" instead of both. Docs §5b
/// (Owner) is explicit that the *other* strand stays free for a later,
/// different stitch to use, and that has to read as a genuinely different
/// point, not the same point used twice. Somewhat smaller than
/// `POST_DEPTH_OFFSET` (picking a strand is a smaller displacement than
/// reaching around a whole post) but not dramatically so: empirically,
/// a value much smaller left a "mosaic crochet"-style scheme (a taller
/// stitch spiking back two rows into a front loop deliberately left free
/// by a back-loop-only row) with a near-miss against nearby geometry,
/// the same class of issue `POST_DEPTH_OFFSET` needed fixing for.
const LOOP_HALF_OFFSET: f64 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub enum PlacementError {
    UnknownStitchKind(StitchId),
    TargetNotYetPlaced(StitchRef),
}

#[derive(Debug, Clone)]
pub struct PlacedStitch {
    /// Where this stitch's yarn path starts (derived from its target(s)).
    pub base: Vec3,
    /// Where this stitch's yarn path ends — what later stitches target.
    pub top: Vec3,
    /// Approximate polyline through this stitch, subdivided per
    /// `StitchDef::path_segments` (docs §3: more pre-wraps -> more points).
    pub path: Vec<Vec3>,
}

#[derive(Debug, Clone, Default)]
pub struct PlacedScheme {
    pub threads: Vec<Vec<PlacedStitch>>,
}

impl PlacedScheme {
    pub fn total_stitch_count(&self) -> usize {
        self.threads.iter().map(|t| t.len()).sum()
    }
}

/// The comfortable angular gap between adjacent siblings before a target
/// is under any strain. Used to fan a *small* group of siblings out
/// gradually (see `sibling_angle`) instead of always wrapping a full
/// circle. Deliberately a standalone constant, not derived from
/// `MIN_STITCH_ARC_WIDTH`/`BASE_RING_RADIUS` (arc-length-per-radian):
/// that coupling means widening the radius to fix one near-miss *shrinks*
/// this angle and can reopen another — confirmed empirically when tuning
/// this. The angular gap and the radius address different failure modes
/// (own-body-vs-neighbouring-bridge separation vs. general crowding) and
/// need to be tunable independently.
const COMFORTABLE_ANGULAR_STEP: f64 = 1.25;

/// This sibling's angular position around its target. **Not** simply
/// `2*PI*index/total` for every target kind: that wraps every group
/// around a full circle regardless of size, which is right for a target
/// that genuinely represents "the whole round" (a ring or chain-space,
/// §5a) — nothing else is nearby to collide with — but wrong for an
/// ordinary small increase into one existing stitch in the middle of an
/// otherwise dense round. An increase of 2 into a `dc` mid-round should
/// land its second stitch a modest step away from the first, not swing to
/// the diametrically opposite side, where it can collide with the
/// *neighbouring* increase's own children — confirmed concretely with a
/// full flat-circle scheme (magic ring, several-stitch round 1, a plain
/// 2-in-each round 2): full-circle wrapping for round 2's increases put
/// adjacent increases' far siblings close enough to *each other* to trip
/// M3, even though every individual increase was fine in isolation.
/// Conversely, forcing the *ring's own* round to fan narrowly instead of
/// wrapping produced an obviously wrong lopsided partial arc instead of
/// an actual round.
///
/// So: `Fixed` targets (ordinary stitches — the usual increase case) fan
/// out at `COMFORTABLE_ANGULAR_STEP` per sibling, starting from
/// `index == 0` at angle 0 (preserving the exact-coincidence case §8
/// invariant 2 relies on for a target's sole/first user), for as long as
/// that stays under a full turn, falling back to full-circle even spacing
/// once the group is large enough that it would wrap past 2*PI anyway
/// (`COMFORTABLE_CAPACITY`'s regime, matching `radius_and_wave`'s own
/// capacity/overflow split). `Elastic` and `TightenedRing` targets always
/// wrap the full circle, at any size — they represent an isolated
/// round/space, not an increase embedded in one.
fn sibling_angle(style: CapacityStyle, index: usize, total: usize) -> f64 {
    if total <= 1 {
        return 0.0;
    }
    let full_wrap = 2.0 * PI * index as f64 / total as f64;
    if style != CapacityStyle::Fixed {
        return full_wrap;
    }
    // Compare against `total` steps, not `total - 1`: the fan's *last*
    // sibling and its wrap-around back to the first need at least one
    // comfortable step of clearance too, or a fan that just barely fits
    // under a full turn (e.g. 6 siblings at 1.25 rad each = 7.5 rad, only
    // just past 2*PI) would leave its two ends almost coincident instead
    // of comfortably spaced — caught by a real test at exactly 6 siblings
    // into a magic ring.
    if total as f64 * COMFORTABLE_ANGULAR_STEP < 2.0 * PI {
        COMFORTABLE_ANGULAR_STEP * index as f64
    } else {
        full_wrap
    }
}

/// How far out (radius) and how far out of plane (z) a sibling at
/// `sibling_index` of `total` sharing one target should sit, given the
/// target's `CapacityStyle` (docs §5a). `angle` is this sibling's
/// position around the ring — see `sibling_angle`.
fn radius_and_wave(style: CapacityStyle, total: usize, angle: f64) -> (f64, f64) {
    match style {
        CapacityStyle::Elastic => {
            let grown_radius = total as f64 * ELASTIC_ARC_WIDTH / (2.0 * PI);
            (grown_radius, 0.0)
        }
        CapacityStyle::Fixed => {
            if total <= COMFORTABLE_CAPACITY {
                (BASE_RING_RADIUS, 0.0)
            } else {
                (BASE_RING_RADIUS, overflow_wave(total, angle))
            }
        }
        CapacityStyle::TightenedRing => {
            if total <= COMFORTABLE_CAPACITY {
                // Cinched ring: radius grows with sibling count up to the
                // flat plateau. Few siblings (docs §5a: 3-5) stay narrow —
                // a taller-than-wide silhouette reads as "pointy" without
                // needing a separate per-stitch lean model. 6-8 reaches
                // (or nearly reaches) the plateau: a flat circle.
                let grown_radius = total as f64 * MIN_STITCH_ARC_WIDTH / (2.0 * PI);
                (grown_radius.min(BASE_RING_RADIUS), 0.0)
            } else {
                (BASE_RING_RADIUS, overflow_wave(total, angle))
            }
        }
    }
}

/// Out-of-plane ripple for siblings past `COMFORTABLE_CAPACITY`. The
/// overflow ratio's effect on amplitude is capped at 1.0x
/// `WAVE_AMPLITUDE` (not let to grow with `total` indefinitely) so a
/// genuinely excessive sibling count doesn't just keep finding more room
/// — see `WAVE_AMPLITUDE`'s docs.
fn overflow_wave(total: usize, angle: f64) -> f64 {
    let overflow = total as f64 / COMFORTABLE_CAPACITY as f64; // > 1.0
    WAVE_AMPLITUDE * (overflow - 1.0).min(1.0) * (angle * overflow).sin()
}

/// Places every stitch in `scheme` in working order. Threads are placed
/// in list order; a target must already be placed when referenced —
/// i.e. it must live earlier in the same thread, or in an
/// already-fully-placed earlier thread (see docs/crochet-context.md §4a:
/// this is the forward-reference discipline a "crochet join" will need
/// once multi-thread schemes exist).
pub fn place_scheme(
    scheme: &Scheme,
    registry: &StitchRegistry,
) -> Result<PlacedScheme, PlacementError> {
    // Pre-pass: how many stitches (scheme-wide) share each single target —
    // needed up front so the *first* sibling placed already knows the
    // eventual ring size (docs §5a), not just a running count.
    let mut target_total: HashMap<StitchRef, usize> = HashMap::new();
    for thread in &scheme.threads {
        for stitch in &thread.stitches {
            if let [single] = stitch.targets.as_slice() {
                *target_total.entry(*single).or_insert(0) += 1;
            }
        }
    }

    let mut placed: HashMap<StitchRef, PlacedStitch> = HashMap::new();
    let mut sibling_index: HashMap<StitchRef, usize> = HashMap::new();
    let mut out_threads: Vec<Vec<PlacedStitch>> = Vec::with_capacity(scheme.threads.len());

    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        let mut out_thread: Vec<PlacedStitch> = Vec::with_capacity(thread.stitches.len());
        let mut prev_top: Option<Vec3> = None;

        for (i, stitch) in thread.stitches.iter().enumerate() {
            let def = registry
                .get(stitch.kind)
                .ok_or(PlacementError::UnknownStitchKind(stitch.kind))?;
            let segments = def.path_segments().max(1);

            let (base, top) = match stitch.targets.as_slice() {
                [] => {
                    // No target at all (docs §3/§4/§8 invariant 2). `ch`
                    // lays out as a step from wherever the thread left off
                    // (`lays_out_as_line`); `mr` and anything else
                    // zero-target instead stays a point anchor at that
                    // same spot — a ring, not a line (docs §5a).
                    let base = prev_top.unwrap_or(Vec3::ZERO);
                    let top = if def.lays_out_as_line {
                        base + Vec3::new(CHAIN_STEP_X, 0.0, 0.0)
                    } else {
                        base
                    };
                    (base, top)
                }
                [single] => {
                    let target_stitch = placed
                        .get(single)
                        .ok_or(PlacementError::TargetNotYetPlaced(*single))?;
                    let target_top = target_stitch.top;

                    let total = *target_total.get(single).unwrap_or(&1);
                    let index = *sibling_index.get(single).unwrap_or(&0);
                    sibling_index.insert(*single, index + 1);

                    let style = target_capacity_style(scheme, registry, *single)?;
                    let angle = sibling_angle(style, index, total);
                    let (radius, z_wave) = radius_and_wave(style, total, angle);
                    // angle = 0 (the first/sole sibling) always lands at
                    // zero lateral offset, matching the pre-§5a behaviour
                    // exactly — see `radius_and_wave`'s callers.
                    let ring_offset =
                        Vec3::new(radius * (angle.cos() - 1.0), radius * angle.sin(), z_wave);

                    let depth_offset = match stitch.loop_target {
                        LoopTarget::FrontPost => Vec3::new(0.0, POST_DEPTH_OFFSET, 0.0),
                        LoopTarget::BackPost => Vec3::new(0.0, -POST_DEPTH_OFFSET, 0.0),
                        LoopTarget::FrontOnly => Vec3::new(0.0, LOOP_HALF_OFFSET, 0.0),
                        LoopTarget::BackOnly => Vec3::new(0.0, -LOOP_HALF_OFFSET, 0.0),
                        LoopTarget::Both => Vec3::ZERO,
                    };
                    let base = target_top + ring_offset + depth_offset;
                    let top = base + Vec3::new(0.0, 0.0, def.height());
                    (base, top)
                }
                multiple => {
                    // Decrease: base is the average of every target's top.
                    // Capacity/ring modelling (§5a) doesn't apply here —
                    // out of scope for this round, see HANDOVER.
                    let mut sum = Vec3::ZERO;
                    for target in multiple {
                        let target_top = placed
                            .get(target)
                            .ok_or(PlacementError::TargetNotYetPlaced(*target))?
                            .top;
                        sum = sum + target_top;
                    }
                    let base = sum * (1.0 / multiple.len() as f64);
                    let top = base + Vec3::new(0.0, 0.0, def.height());
                    (base, top)
                }
            };

            let placed_stitch = PlacedStitch {
                base,
                top,
                path: linspace(base, top, segments),
            };
            let stitch_ref = StitchRef::new(thread_idx, i);
            prev_top = Some(placed_stitch.top);
            placed.insert(stitch_ref, placed_stitch.clone());
            out_thread.push(placed_stitch);
        }

        out_threads.push(out_thread);
    }

    Ok(PlacedScheme {
        threads: out_threads,
    })
}

/// The `CapacityStyle` a specific target StitchRef behaves as: its own
/// instance override (docs §5a — e.g. a magic ring deliberately left
/// open) if set, else the registry default for its stitch kind.
fn target_capacity_style(
    scheme: &Scheme,
    registry: &StitchRegistry,
    target: StitchRef,
) -> Result<CapacityStyle, PlacementError> {
    let target_instance = scheme
        .get(target)
        .expect("target_capacity_style called with a StitchRef not in this scheme");
    if let Some(style) = target_instance.capacity_override {
        return Ok(style);
    }
    let def = registry
        .get(target_instance.kind)
        .ok_or(PlacementError::UnknownStitchKind(target_instance.kind))?;
    Ok(def.capacity_style)
}

fn linspace(a: Vec3, b: Vec3, segments: u32) -> Vec<Vec3> {
    let n = segments.max(1);
    (0..=n)
        .map(|i| a + (b - a) * (i as f64 / n as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LoopTarget, StitchInstance, Thread};
    use crate::stitch::{CH, DC, DTR, MR, TR};

    fn ref_at(index: usize) -> StitchRef {
        StitchRef::new(0, index)
    }

    #[test]
    fn foundation_chain_lays_out_in_a_line_with_no_targets() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..5 {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        assert_eq!(placed.total_stitch_count(), 5);
        let chain = &placed.threads[0];
        for (i, stitch) in chain.iter().enumerate() {
            assert!(stitch.base.is_finite() && stitch.top.is_finite());
            assert_eq!(stitch.base.x, i as f64 * CHAIN_STEP_X);
        }
    }

    #[test]
    fn row_of_dc_into_a_chain_produces_correct_count_and_finite_geometry() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        // Foundation chain of 4.
        for _ in 0..4 {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        // A row of 4 dc, each targeting the chain stitch below it.
        for i in 0..4 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(i)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        assert_eq!(placed.total_stitch_count(), 8);
        for stitch in &placed.threads[0] {
            for p in &stitch.path {
                assert!(p.is_finite());
            }
        }
        // dc stitches should sit above their chain target, not at it.
        let dc_stitch = &placed.threads[0][4];
        let chain_stitch = &placed.threads[0][0];
        assert!(dc_stitch.top.z > chain_stitch.top.z);
    }

    #[test]
    fn increase_spreads_siblings_that_share_a_target() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // index 0: anchor
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // index 1
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // index 2: increase sibling
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let first = &placed.threads[0][1];
        let second = &placed.threads[0][2];
        assert!(
            first.base.distance(&second.base) > 0.1,
            "increase siblings must not coincide"
        );
    }

    #[test]
    fn many_siblings_are_spread_around_a_ring_not_a_line() {
        // 7 dc sharing one target: comfortably within COMFORTABLE_CAPACITY
        // (docs §5a: "seven is hard but possible"), so should sit on a
        // circle, not collapse onto a single axis like a linear offset
        // would leave them vulnerable to.
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![]));
        for _ in 0..7 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let ys: Vec<f64> = placed.threads[0][1..].iter().map(|s| s.base.y).collect();
        assert!(
            ys.iter().any(|&y| y.abs() > 0.05),
            "siblings should be spread in y (a ring), not collinear along x: {:?}",
            ys
        );
        for pair_i in 1..8 {
            for pair_j in (pair_i + 1)..8 {
                let d = placed.threads[0][pair_i]
                    .base
                    .distance(&placed.threads[0][pair_j].base);
                assert!(d > 0.1, "siblings {pair_i} and {pair_j} too close: {d}");
            }
        }
    }

    #[test]
    fn tightened_magic_ring_radius_grows_then_plateaus() {
        // Docs §5a (Owner calibration): 3-5 stitches into a tightened
        // magic ring stay narrow (small radius, no wave); 6-8 reach the
        // flat plateau (also no wave — that's the "forms a circle" case).
        let (r_narrow, z_narrow) = radius_and_wave(CapacityStyle::TightenedRing, 4, 0.3);
        let (r_flat, z_flat) = radius_and_wave(CapacityStyle::TightenedRing, 7, 0.3);
        assert_eq!(z_narrow, 0.0, "no waviness below comfortable capacity");
        assert_eq!(z_flat, 0.0, "no waviness right at comfortable capacity");
        assert!(
            r_narrow < r_flat,
            "narrow (4 siblings) should have a smaller radius than flat (7): {r_narrow} vs {r_flat}"
        );
        assert!(
            (r_flat - BASE_RING_RADIUS).abs() < 1e-9,
            "7 siblings should sit at (or essentially at) the flat plateau radius"
        );
    }

    #[test]
    fn overloaded_tightened_ring_ripples_past_comfortable_capacity() {
        // Docs §5a: 9+ into a tightened magic ring can't open further and
        // ripples into a wavy 3D circle instead.
        let (radius, z) = radius_and_wave(CapacityStyle::TightenedRing, 9, 0.7);
        assert_eq!(
            radius, BASE_RING_RADIUS,
            "radius plateaus, doesn't keep growing"
        );
        assert_ne!(
            z, 0.0,
            "expected out-of-plane waviness once past comfortable capacity"
        );
    }

    #[test]
    fn magic_ring_is_a_point_anchor_not_a_line() {
        // Unlike `ch`, `mr` must not pick up the CHAIN_STEP_X used to lay
        // a foundation chain out in a line — it's a ring/point anchor
        // (docs §5a), and other stitches gather *around* it.
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(MR, vec![]));
        for _ in 0..6 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let ring = &placed.threads[0][0];
        assert_eq!(
            ring.base, ring.top,
            "mr should stay a point, not step forward like ch"
        );
        // 6 siblings — right in the Owner's "forms a circle" range —
        // should all sit apart from each other, not collinear/coincident.
        for i in 1..=6 {
            for j in (i + 1)..=6 {
                let d = placed.threads[0][i]
                    .base
                    .distance(&placed.threads[0][j].base);
                assert!(d > 0.1, "siblings {i} and {j} too close: {d}");
            }
        }
    }

    #[test]
    fn overloaded_fixed_target_bulges_out_of_plane() {
        // 9 stitches sharing one *ordinary stitch* target (dc, index 1 —
        // not the chain anchor at index 0, which is deliberately Elastic
        // per docs §5a): past COMFORTABLE_CAPACITY (7), should push some
        // siblings off the z=0 plane rather than crowd the in-plane ring
        // tighter.
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0: anchor
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 1: the Fixed-capacity target
        for _ in 0..9 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(1)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let target_z = placed.threads[0][1].top.z;
        let any_out_of_plane = placed.threads[0][2..]
            .iter()
            .any(|s| (s.base.z - target_z).abs() > 1e-6);
        assert!(
            any_out_of_plane,
            "expected at least one sibling to bulge out of the z=0 plane once overloaded"
        );
    }

    #[test]
    fn front_and_back_loop_only_stay_geometrically_distinct() {
        // Docs §5b (Owner): working into only one strand of a target's
        // top loop leaves the other strand free for a *different* later
        // stitch — that has to read as a genuinely different point, not
        // the same point twice.
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)]).with_loop_target(LoopTarget::FrontOnly)); // 1
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)]).with_loop_target(LoopTarget::BackOnly)); // 2
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let front = &placed.threads[0][1];
        let back = &placed.threads[0][2];
        assert!(
            front.base.distance(&back.base) > 0.1,
            "front-loop-only and back-loop-only siblings must not coincide"
        );
        assert!(front.base.y > 0.0, "front loop should lean toward +y");
        assert!(back.base.y < 0.0, "back loop should lean toward -y");
    }

    #[test]
    fn decrease_averages_multiple_targets() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 1
                                                               // dc2tog-style decrease: one stitch, two targets.
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0), ref_at(1)]));
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let a = placed.threads[0][0].top;
        let b = placed.threads[0][1].top;
        let dec = &placed.threads[0][2];
        let expected_base_x = (a.x + b.x) / 2.0;
        assert!((dec.base.x - expected_base_x).abs() < 1e-9);
        assert!(dec.base.is_finite() && dec.top.is_finite());
    }

    #[test]
    fn spike_stitch_targets_further_back_than_the_immediately_preceding_stitch() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..3 {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(2)])); // index 3: ordinary
                                                             // A spike: targets index 0, two stitches further back than index 3's immediate predecessor.
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)]));
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry);
        assert!(
            placed.is_ok(),
            "spike stitches need no special handling to place"
        );
    }

    #[test]
    fn freeform_scheme_with_no_row_structure_places_successfully() {
        // Not a conventional row/round scheme at all: each stitch below
        // targets an arbitrary earlier point, proving nothing in the
        // placement code assumes "the previous row" (docs §4).
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread
            .stitches
            .push(StitchInstance::new(CH, vec![]).with_loop_target(LoopTarget::Both)); // 0
        thread
            .stitches
            .push(StitchInstance::new(TR, vec![ref_at(0)])); // 1
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 2, also off the anchor
        thread
            .stitches
            .push(StitchInstance::new(DTR, vec![ref_at(1)])); // 3
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(2), ref_at(1)])); // 4, cross-link

        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        assert_eq!(placed.total_stitch_count(), 5);
        for stitch in &placed.threads[0] {
            assert!(stitch.base.is_finite());
            assert!(stitch.top.is_finite());
            for p in &stitch.path {
                assert!(p.is_finite());
            }
        }
    }

    #[test]
    fn unplaced_forward_target_is_an_error_not_a_panic() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        // Targets index 1, which doesn't exist yet at index 0 — must
        // error, never panic or silently produce garbage geometry.
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(1)]));
        thread.stitches.push(StitchInstance::new(CH, vec![]));
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let result = place_scheme(&scheme, &registry);
        assert!(matches!(result, Err(PlacementError::TargetNotYetPlaced(_))));
    }

    #[test]
    fn taller_stitch_has_more_evenly_spaced_path_segments() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![]));
        thread
            .stitches
            .push(StitchInstance::new(DTR, vec![ref_at(0)]));
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let placed = place_scheme(&scheme, &registry).unwrap();
        let dtr = &placed.threads[0][1];
        // dtr: 2 pre-wraps -> 3 path segments -> 4 points.
        assert_eq!(dtr.path.len(), 4);
        let lengths: Vec<f64> = dtr.path.windows(2).map(|w| w[0].distance(&w[1])).collect();
        for pair in lengths.windows(2) {
            assert!(
                (pair[0] - pair[1]).abs() < 1e-9,
                "segments should be equal length: {:?}",
                lengths
            );
        }
    }
}
