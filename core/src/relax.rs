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

use std::collections::HashMap;

use crate::geometry::{place_scheme, PlacementError};
use crate::graph::{Scheme, StitchRef};
use crate::stitch::StitchRegistry;
use crate::vec3::Vec3;

/// Stiffness of the working-order continuity edge between consecutive
/// stitches in a thread — the physical yarn strand linking them. Not
/// stitch-kind-dependent (unlike insertion stiffness): it's the same
/// strand of yarn regardless of what's formed at either end.
const CONTINUITY_STIFFNESS: f64 = 0.6;

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
                let rest_length = raw.threads[thread_idx][i]
                    .top
                    .distance(&raw.threads[thread_idx][i - 1].top);
                constraints.push(SpringConstraint {
                    a: r,
                    b: prev,
                    rest_length,
                    stiffness: CONTINUITY_STIFFNESS,
                });
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
