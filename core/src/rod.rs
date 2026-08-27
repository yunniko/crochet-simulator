//! M9: Discrete Elastic Rod (DER) mechanics — Bergou et al. 2008, and the
//! position-based variant (Umetani et al., "Position-Based Elastic Rods")
//! that lets it be solved with XPBD instead of a full Newton/FEM system.
//! See GOALS.md's M9 entry for why this exists: `relax.rs`'s original
//! solver was a plain point-mass spring system with no bending resistance
//! at all, which is why a chain pulled into a ring by a slip stitch just
//! folded flat instead of curving — nothing in that model preferred a
//! curved shape over a straight one satisfying the same distances.
//!
//! This module is pure geometry/energy math — no solving, no knowledge of
//! the insertion graph. It operates on a single ordered polyline (one
//! thread's working-order backbone, which is always a single sequential
//! rod by construction: a thread IS one continuous strand). `relax.rs`
//! wires this into the actual solve loop and handles insertion-target
//! attachment constraints separately, same conceptual role those already
//! had before this module existed.
//!
//! **Staged scope, not an oversight**: this first pass covers stretch and
//! bending only — twist (the material frame's own rotation about the
//! tangent, relevant for helical buckling / cable-like twisting) is
//! deferred. M9's actual acceptance criteria (a chain closes into a
//! genuine ring; existing calibrated behaviour still holds) is a bending
//! problem, not a twist one, so this is a real, complete slice rather
//! than a stepping-stone that doesn't stand on its own.
//!
//! **Isotropic bending**: a real rod's bending stiffness is generally a
//! 2x2 matrix — different resistance in different cross-section
//! directions, relevant for e.g. a flat ribbon. Yarn is close enough to
//! round in cross-section that a single scalar bending stiffness
//! (resistance to curvature magnitude, regardless of direction) is a
//! physically reasonable simplification, not a full anisotropic `B`
//! matrix — chosen deliberately for tractability, not an oversight.

use crate::vec3::Vec3;

/// A Bishop frame at one point along the rod: an orthonormal basis
/// `{tangent, u, v}` where `tangent` runs along the rod and `u`/`v` span
/// the perpendicular plane. "Bishop" specifically means *twist-free* —
/// propagated via parallel transport (see [`parallel_transport`]), so any
/// twist you'd observe between frames reflects the rod's own material
/// twist, never an artifact of how the frame happens to be tracked. (This
/// module doesn't track material twist yet — see the module docs — but
/// keeping the frame genuinely twist-free now means twist can be added
/// later without redoing this part.)
#[derive(Debug, Clone, Copy)]
pub struct BishopFrame {
    pub tangent: Vec3,
    pub u: Vec3,
    pub v: Vec3,
}

/// Rotates `v` (assumed perpendicular to `from_tangent`) by the minimal
/// rotation that takes `from_tangent` to `to_tangent`, landing it
/// perpendicular to `to_tangent` — the core operation parallel transport
/// is built from. Both tangents must already be unit length.
///
/// Uses Rodrigues' rotation formula about the axis `from_tangent ×
/// to_tangent`. Degenerate cases (the tangents already coincide, or are
/// exactly opposite) fall back to returning `v` unrotated — coinciding
/// tangents need no rotation at all, and an exact 180° flip has no unique
/// minimal-rotation axis (never occurs in practice: consecutive rod edges
/// don't reverse direction, and this is documented rather than silently
/// producing a NaN).
pub fn parallel_transport(v: Vec3, from_tangent: Vec3, to_tangent: Vec3) -> Vec3 {
    let axis = from_tangent.cross(to_tangent);
    let sin_angle = axis.length();
    let cos_angle = from_tangent.dot(to_tangent).clamp(-1.0, 1.0);

    if sin_angle < 1e-12 {
        // Either already aligned (cos ~ 1, no rotation needed) or exactly
        // opposed (cos ~ -1, no well-defined minimal axis) — both return
        // `v` as-is rather than dividing by a near-zero axis length.
        return v;
    }

    let axis = axis * (1.0 / sin_angle);
    // Rodrigues' rotation formula: v_rot = v*cos + (axis × v)*sin +
    // axis*(axis·v)*(1 - cos).
    v * cos_angle + axis.cross(v) * sin_angle + axis * (axis.dot(v) * (1.0 - cos_angle))
}

/// Any unit vector perpendicular to `tangent` — used to seed the first
/// Bishop frame when the caller has no preferred orientation. Picks
/// whichever of the world X/Z axes is less parallel to `tangent`, so the
/// result stays numerically well-conditioned for any input direction.
pub fn arbitrary_perpendicular(tangent: Vec3) -> Vec3 {
    let reference = if tangent.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    (reference - tangent * tangent.dot(reference)).normalized()
}

/// Propagates a Bishop frame along an ordered polyline (a rod's vertex
/// positions), one frame per **edge** (so `points.len() - 1` frames).
/// `initial_u` seeds the very first edge's frame — it's projected
/// perpendicular to that edge's own tangent and normalized, so it doesn't
/// need to be exactly perpendicular going in.
///
/// Returns an empty vec for fewer than 2 points (no edges to have a frame
/// at all).
pub fn bishop_frames_along(points: &[Vec3], initial_u: Vec3) -> Vec<BishopFrame> {
    if points.len() < 2 {
        return Vec::new();
    }

    let mut frames = Vec::with_capacity(points.len() - 1);

    let first_tangent = (points[1] - points[0]).normalized();
    let seed_u = if initial_u.length() < 1e-12 {
        arbitrary_perpendicular(first_tangent)
    } else {
        (initial_u - first_tangent * first_tangent.dot(initial_u)).normalized()
    };
    let seed_u = if seed_u.length() < 1e-12 {
        // `initial_u` was (anti)parallel to the tangent, so the
        // projection above collapsed to zero — fall back to a
        // guaranteed-perpendicular choice instead.
        arbitrary_perpendicular(first_tangent)
    } else {
        seed_u
    };
    let first_v = first_tangent.cross(seed_u);
    frames.push(BishopFrame {
        tangent: first_tangent,
        u: seed_u,
        v: first_v,
    });

    for i in 1..points.len() - 1 {
        let prev = frames[i - 1];
        let tangent = (points[i + 1] - points[i]).normalized();
        let u = parallel_transport(prev.u, prev.tangent, tangent).normalized();
        let v = tangent.cross(u);
        frames.push(BishopFrame { tangent, u, v });
    }

    frames
}

/// The discrete curvature binormal at an interior vertex (Bergou et al.
/// eq. 1): `2 * (e_prev × e_curr) / (|e_prev||e_curr| + e_prev·e_curr)`.
/// Its direction is the axis the rod bends around at that vertex, and its
/// magnitude grows with how sharply the rod turns there — zero exactly
/// when the two edges are collinear (a straight rod through that vertex).
///
/// `e_prev`/`e_curr` are the incoming/outgoing edge vectors (not
/// required to be unit length or equal length — real segments along a
/// scheme rarely are).
pub fn curvature_binormal(e_prev: Vec3, e_curr: Vec3) -> Vec3 {
    let len_prev = e_prev.length();
    let len_curr = e_curr.length();
    let denom = len_prev * len_curr + e_prev.dot(e_curr);
    if denom.abs() < 1e-12 {
        // e_prev and e_curr point in exactly opposite directions (the
        // rod folds back on itself at this vertex, denom -> 0) — the
        // binormal direction is genuinely undefined there (any
        // perpendicular axis is "a" bend axis). Zero is a safe, finite
        // fallback rather than a divide-by-near-zero blowup; a real
        // solver will pull the rod out of this configuration via other
        // constraints (stretch, sibling repulsion) before this matters.
        return Vec3::ZERO;
    }
    e_prev.cross(e_curr) * (2.0 / denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Vec3, b: Vec3) {
        assert!(
            a.distance(&b) < 1e-9,
            "expected {:?} to approximately equal {:?}",
            a,
            b
        );
    }

    mod parallel_transport_tests {
        use super::*;

        #[test]
        fn identity_when_tangents_already_match() {
            let t = Vec3::new(1.0, 0.0, 0.0).normalized();
            let v = Vec3::new(0.0, 1.0, 0.0);
            approx_eq(parallel_transport(v, t, t), v);
        }

        #[test]
        fn rotates_perpendicular_vector_by_a_quarter_turn() {
            // Tangent rotates from +X to +Y (a 90 degree turn about +Z) —
            // a vector already perpendicular to both (i.e. along +Z)
            // should be completely unaffected by that turn.
            let from = Vec3::new(1.0, 0.0, 0.0);
            let to = Vec3::new(0.0, 1.0, 0.0);
            approx_eq(
                parallel_transport(Vec3::new(0.0, 0.0, 1.0), from, to),
                Vec3::new(0.0, 0.0, 1.0),
            );
        }

        #[test]
        fn transported_vector_stays_perpendicular_to_the_new_tangent() {
            let from = Vec3::new(1.0, 0.2, 0.0).normalized();
            let to = Vec3::new(0.3, 1.0, 0.4).normalized();
            let v = arbitrary_perpendicular(from);
            let transported = parallel_transport(v, from, to);
            assert!(
                transported.dot(to).abs() < 1e-9,
                "transported vector {:?} not perpendicular to new tangent {:?}",
                transported,
                to
            );
        }

        #[test]
        fn transported_vector_preserves_length() {
            let from = Vec3::new(1.0, 0.0, 0.0);
            let to = Vec3::new(0.0, 1.0, 0.0);
            let v = Vec3::new(0.0, 0.0, 2.5); // deliberately not unit length
            assert!((parallel_transport(v, from, to).length() - v.length()).abs() < 1e-9);
        }

        #[test]
        fn handles_a_small_bend_without_blowing_up() {
            let from = Vec3::new(1.0, 0.0, 0.0);
            let to = Vec3::new(0.9999, 0.014, 0.0).normalized();
            let v = arbitrary_perpendicular(from);
            let result = parallel_transport(v, from, to);
            assert!(result.is_finite());
            assert!((result.length() - 1.0).abs() < 1e-6);
        }
    }

    mod bishop_frames_tests {
        use super::*;

        #[test]
        fn a_perfectly_straight_rod_keeps_the_same_frame_on_every_edge() {
            let points = vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(3.0, 0.0, 0.0),
            ];
            let frames = bishop_frames_along(&points, Vec3::new(0.0, 1.0, 0.0));
            assert_eq!(frames.len(), 3);
            for f in &frames {
                approx_eq(f.tangent, Vec3::new(1.0, 0.0, 0.0));
                approx_eq(f.u, frames[0].u);
                approx_eq(f.v, frames[0].v);
            }
        }

        #[test]
        fn every_frame_is_orthonormal() {
            let points = vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.5, 0.8, 0.0),
                Vec3::new(1.0, 1.6, 0.3),
                Vec3::new(0.2, 1.9, 1.0),
            ];
            let frames = bishop_frames_along(&points, Vec3::new(0.0, 0.0, 1.0));
            for f in &frames {
                assert!((f.tangent.length() - 1.0).abs() < 1e-9);
                assert!((f.u.length() - 1.0).abs() < 1e-9);
                assert!((f.v.length() - 1.0).abs() < 1e-9);
                assert!(f.tangent.dot(f.u).abs() < 1e-9);
                assert!(f.tangent.dot(f.v).abs() < 1e-9);
                assert!(f.u.dot(f.v).abs() < 1e-9);
            }
        }

        #[test]
        fn seed_u_is_normalized_and_orthogonalized_even_when_input_is_not() {
            let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
            // Deliberately not perpendicular to the tangent, and not unit
            // length either.
            let frames = bishop_frames_along(&points, Vec3::new(5.0, 3.0, 0.0));
            assert_eq!(frames.len(), 1);
            assert!((frames[0].u.length() - 1.0).abs() < 1e-9);
            assert!(frames[0].tangent.dot(frames[0].u).abs() < 1e-9);
        }

        #[test]
        fn falls_back_to_an_arbitrary_perpendicular_when_seed_is_parallel_to_tangent() {
            let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
            let frames = bishop_frames_along(&points, Vec3::new(3.0, 0.0, 0.0)); // parallel to tangent
            assert_eq!(frames.len(), 1);
            assert!(frames[0].u.is_finite());
            assert!((frames[0].u.length() - 1.0).abs() < 1e-9);
        }

        #[test]
        fn fewer_than_two_points_gives_no_frames() {
            assert!(bishop_frames_along(&[], Vec3::new(0.0, 1.0, 0.0)).is_empty());
            assert!(bishop_frames_along(&[Vec3::ZERO], Vec3::new(0.0, 1.0, 0.0)).is_empty());
        }
    }

    mod curvature_binormal_tests {
        use super::*;

        #[test]
        fn zero_for_a_perfectly_straight_run() {
            let e_prev = Vec3::new(1.0, 0.0, 0.0);
            let e_curr = Vec3::new(1.0, 0.0, 0.0);
            approx_eq(curvature_binormal(e_prev, e_curr), Vec3::ZERO);
        }

        #[test]
        fn zero_when_edges_have_different_lengths_but_are_still_collinear() {
            let e_prev = Vec3::new(2.0, 0.0, 0.0);
            let e_curr = Vec3::new(0.5, 0.0, 0.0);
            approx_eq(curvature_binormal(e_prev, e_curr), Vec3::ZERO);
        }

        #[test]
        fn nonzero_and_perpendicular_to_both_edges_for_a_genuine_bend() {
            let e_prev = Vec3::new(1.0, 0.0, 0.0);
            let e_curr = Vec3::new(0.0, 1.0, 0.0); // a 90 degree turn
            let kb = curvature_binormal(e_prev, e_curr);
            assert!(
                kb.length() > 0.5,
                "expected a substantial curvature for a 90-degree turn, got {:?}",
                kb
            );
            assert!(kb.dot(e_prev).abs() < 1e-9);
            assert!(kb.dot(e_curr).abs() < 1e-9);
        }

        #[test]
        fn magnitude_increases_with_sharper_turns() {
            let e_prev = Vec3::new(1.0, 0.0, 0.0);
            let gentle = curvature_binormal(e_prev, Vec3::new(1.0, 0.1, 0.0));
            let sharp = curvature_binormal(e_prev, Vec3::new(1.0, 0.5, 0.0));
            let sharper = curvature_binormal(e_prev, Vec3::new(0.0, 1.0, 0.0));
            assert!(gentle.length() < sharp.length());
            assert!(sharp.length() < sharper.length());
        }

        #[test]
        fn does_not_blow_up_when_the_rod_folds_back_on_itself() {
            let e_prev = Vec3::new(1.0, 0.0, 0.0);
            let e_curr = Vec3::new(-1.0, 0.0, 0.0); // a perfect fold-back, denom -> 0
            let kb = curvature_binormal(e_prev, e_curr);
            assert!(kb.is_finite());
        }
    }
}
