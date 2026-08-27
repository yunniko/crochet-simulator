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
use crate::stitch::{StitchRegistry, SS};
use crate::vec3::Vec3;

/// Stiffness of the working-order continuity edge between consecutive
/// stitches in a thread — the physical yarn strand linking them. Not
/// stitch-kind-dependent (unlike insertion stiffness): it's the same
/// strand of yarn regardless of what's formed at either end.
const CONTINUITY_STIFFNESS: f64 = 0.6;

/// M9: bending resistance, realized as a spring between each vertex's two
/// *second* working-order neighbours (`i-2` and `i`, spanning across
/// vertex `i-1`) rather than a full analytic Discrete-Elastic-Rod
/// curvature-gradient projection — see `rod.rs`'s module docs for the
/// underlying DER math this approximates and why an XPBD-projection layer
/// wasn't worth the added integration complexity here: this solver's
/// existing spring forces are already empirically stable at the stiffness
/// values in use (nothing here pushes toward the near-infinite-stiffness
/// regime XPBD specifically exists to stabilize), so extending the same
/// proven force-based mechanism to a second-neighbour pair is the lower-
/// risk way to add genuine bending resistance. The rest length is the
/// *raw*-placement second-neighbour distance (mirroring every other
/// spring's rest-length convention) — for an initially-straight run this
/// is the sum of the two edge lengths (the maximum possible second-
/// neighbour distance), so the spring's preferred state is "stay as
/// straight as this thread started," which is exactly what resists an
/// external pull from concentrating all the curvature at one sharp fold:
/// bending a little at *every* interior vertex costs less energy than
/// bending a lot at one, so the whole run distributes curvature smoothly
/// under an end-closing force instead of kinking.
const BENDING_STIFFNESS: f64 = 0.5;

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

            // M9 bending resistance — see `BENDING_STIFFNESS`'s own
            // comment for the mechanism. Needs a second predecessor, so
            // only from the third stitch in a thread onward.
            if i > 1 {
                let second_prev = StitchRef::new(thread_idx, i - 2);
                let rest_length = raw.threads[thread_idx][i]
                    .top
                    .distance(&raw.threads[thread_idx][i - 2].top);
                constraints.push(SpringConstraint {
                    a: r,
                    b: second_prev,
                    rest_length,
                    stiffness: BENDING_STIFFNESS,
                });
            }
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
