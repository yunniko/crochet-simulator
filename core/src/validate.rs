//! M3: geometry validation — see docs/crochet-context.md §8 invariant 4
//! (self-intersection) and §7/§8 invariant 3 (stitch-count self-check).
//!
//! The self-intersection checker runs on the *relaxed* (M2) yarn path
//! (`crate::path`), not the raw M1 placement — docs §8 invariant 4 is
//! explicit that it's the relaxed shape that must be checked. It flags
//! any two non-adjacent segments whose closest approach is below a yarn-
//! diameter threshold.
//!
//! "Adjacent" (never flagged) means the two segments share an endpoint
//! **in the raw (M1) placement** — see `crate::path`'s `raw_start`/
//! `raw_end`. Raw coordinates, not relaxed ones, are what should decide
//! this: whether two points are *structurally* the same point (a chain
//! stitch's base *is defined as* the previous stitch's top; the first
//! stitch built on a target sits exactly *at* that target's top) is a
//! fact about the graph, fixed at raw-placement time — not something that
//! should change because relaxation moved things, or because a caller
//! pinned two stitches together to test the checker.
//!
//! An earlier version of this checker used a stitch-reference-based
//! "neighbourhood overlap" rule instead. It correctly handled the
//! chain-continuity case above, but was **too permissive for lace/shell
//! constructions**: any two stitches sharing a single target (e.g. a
//! 7-tr shell into one chain space — completely ordinary in lace) were
//! excluded from checking *against each other*, not just against the
//! shared target, because both "neighbourhoods" contained that target.
//! Verified empirically that this let two shell siblings pinned to ~0.01
//! apart pass silently. Raw-coincidence fixes this directly: two shell
//! siblings' raw bases differ (each gets its own lateral offset in
//! `geometry.rs`, unless one is the sole/first user of the target, which
//! *does* coincide and is correctly excluded) — so genuine crowding
//! between them is checked like anything else, while each sibling's
//! shared touch point with the target itself is still recognised as
//! structural, not a defect.

use std::collections::HashSet;

use crate::graph::{Scheme, StitchRef};
use crate::path::{PathSegment, SegmentOwner};
use crate::vec3::Vec3;

/// Default minimum allowed distance between non-adjacent yarn segments,
/// standing in for yarn diameter. Smaller than
/// `crate::geometry::POST_DEPTH_OFFSET` on purpose, so a correctly
/// modelled post stitch never trips this by construction.
pub const DEFAULT_YARN_DIAMETER: f64 = 0.15;

#[derive(Debug, Clone, Copy)]
pub struct Intersection {
    pub a: SegmentOwner,
    pub b: SegmentOwner,
    pub distance: f64,
}

#[derive(Debug, Clone)]
pub struct IntersectionReport {
    pub ok: bool,
    pub violations: Vec<Intersection>,
}

/// Convenience wrapper: builds the relaxed yarn path and checks it in one
/// call.
pub fn validate_scheme(
    scheme: &Scheme,
    registry: &crate::stitch::StitchRegistry,
    relaxed: &crate::relax::RelaxedScheme,
    min_distance: f64,
) -> Result<IntersectionReport, crate::geometry::PlacementError> {
    let segments = crate::path::relaxed_yarn_segments(scheme, registry, relaxed)?;
    Ok(check_self_intersections(&segments, min_distance))
}

/// Minimum distance between two points for them to count as "the same
/// structural point" when compared in raw placement — see module docs.
const SAME_POINT_EPSILON: f64 = 1e-6;

/// Flags any two non-adjacent segments closer than `min_distance` as a
/// self-intersection (docs §8 invariant 4). Two segments are "adjacent"
/// (never flagged) when they share an endpoint in *raw* placement — see
/// module docs for why raw, not relaxed, coordinates decide this.
/// O(n^2) over segment count — fine for design-tool-sized swatches; a
/// spatial hash would be the first thing to add if this ever needs to
/// scale to large schemes.
pub fn check_self_intersections(segments: &[PathSegment], min_distance: f64) -> IntersectionReport {
    let mut violations = Vec::new();
    for i in 0..segments.len() {
        for j in (i + 1)..segments.len() {
            if segments_are_adjacent(&segments[i], &segments[j]) {
                continue;
            }
            let distance = segment_segment_distance(
                segments[i].start,
                segments[i].end,
                segments[j].start,
                segments[j].end,
            );
            if distance < min_distance {
                violations.push(Intersection {
                    a: segments[i].owner,
                    b: segments[j].owner,
                    distance,
                });
            }
        }
    }
    IntersectionReport {
        ok: violations.is_empty(),
        violations,
    }
}

fn segments_are_adjacent(a: &PathSegment, b: &PathSegment) -> bool {
    a.raw_start.distance(&b.raw_start) < SAME_POINT_EPSILON
        || a.raw_start.distance(&b.raw_end) < SAME_POINT_EPSILON
        || a.raw_end.distance(&b.raw_start) < SAME_POINT_EPSILON
        || a.raw_end.distance(&b.raw_end) < SAME_POINT_EPSILON
}

/// Shortest distance between two 3D line segments [p1,q1] and [p2,q2].
/// Standard closest-point-between-segments algorithm (Ericson,
/// "Real-Time Collision Detection" §5.1.9), handling parallel and
/// degenerate (zero-length) segments.
fn segment_segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f64 {
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
    c1.distance(&c2)
}

#[derive(Debug, Clone)]
pub struct RoundCountError {
    pub expected: usize,
    pub actual: usize,
    /// Any stitch in `new_round` whose target fell outside `previous_round`
    /// — i.e. it isn't actually built on the round it's claimed to be.
    pub stray_targets: Vec<StitchRef>,
}

/// Programmatic equivalent of a pattern's `(N sts)` self-check (docs §7,
/// §8 invariant 3) — generalised for a model with no real row/round
/// objects (docs §4): the caller names which stitches make up "the
/// previous round" and "the new round" explicitly, and this checks (a)
/// the new round's stitch count matches `expected`, and (b) every new
/// stitch's target(s) actually lie in the claimed previous round.
pub fn check_round(
    scheme: &Scheme,
    previous_round: &HashSet<StitchRef>,
    new_round: &[StitchRef],
    expected: usize,
) -> Result<(), RoundCountError> {
    let mut stray_targets = Vec::new();
    for r in new_round {
        let stitch = scheme
            .get(*r)
            .expect("check_round given a StitchRef not in this scheme");
        for target in &stitch.targets {
            if !previous_round.contains(target) {
                stray_targets.push(*target);
            }
        }
    }

    if new_round.len() != expected || !stray_targets.is_empty() {
        Err(RoundCountError {
            expected,
            actual: new_round.len(),
            stray_targets,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::relax::{relax_scheme, RelaxationParams};
    use crate::stitch::{CH, DC, TR};
    use crate::vec3::Vec3;
    use std::collections::HashMap;

    fn ref_at(index: usize) -> StitchRef {
        StitchRef::new(0, index)
    }

    #[test]
    fn capacity_calibrated_shell_sizes_validate_as_expected() {
        // Owner calibration (docs §5a): 7 into an ordinary stitch is
        // "hard but possible," 11 "won't fit physically." Confirms the
        // whole pipeline (radial placement + capacity styles + relaxation
        // with sibling repulsion) lands on the same boundary end-to-end,
        // not just in isolated unit checks of the pieces.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let validate_shell = |count: usize| -> bool {
            let mut thread = Thread::new();
            thread.stitches.push(StitchInstance::new(CH, vec![]));
            for _ in 0..count {
                thread
                    .stitches
                    .push(StitchInstance::new(DC, vec![ref_at(0)]));
            }
            let mut scheme = Scheme::new();
            scheme.add_thread(thread);
            let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
            validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER)
                .unwrap()
                .ok
        };

        assert!(
            validate_shell(7),
            "7 into one stitch should validate (hard but possible)"
        );
        assert!(
            !validate_shell(11),
            "11 into one stitch should be flagged (won't fit physically)"
        );
    }

    #[test]
    fn mosaic_style_back_loop_row_with_front_loop_spike_does_not_false_positive() {
        // Docs §5b (Owner): mosaic crochet works a row into back-loop-only,
        // leaving the front loops free, then a later row reaches back
        // *two* rows (skipping the back-loop row entirely) with a taller
        // stitch worked into those left-free front loops.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..3 {
            thread.stitches.push(StitchInstance::new(CH, vec![])); // row0: 0-2
        }
        for i in 0..3 {
            // row1: back-loop-only, into row0 reversed.
            let target = ref_at(2 - i);
            thread.stitches.push(
                StitchInstance::new(DC, vec![target])
                    .with_loop_target(crate::graph::LoopTarget::BackOnly),
            ); // 3,4,5
        }
        for i in 0..3 {
            // row2: ordinary, into row1 reversed.
            let target = ref_at(5 - i);
            thread.stitches.push(StitchInstance::new(DC, vec![target])); // 6,7,8
        }
        // A taller "mosaic" stitch reaching back two rows into row1's
        // middle stitch (index 4), specifically its front loop — the one
        // left free when row1 was worked back-loop-only.
        thread.stitches.push(
            StitchInstance::new(TR, vec![ref_at(4)])
                .with_loop_target(crate::graph::LoopTarget::FrontOnly),
        ); // 9

        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            report.ok,
            "mosaic-style front-loop spike should not false-positive as a self-intersection: {:?}",
            report.violations
        );
    }

    #[test]
    fn small_shell_of_taller_stitches_does_not_false_positive() {
        // A 3-tr shell all worked into one chain-space stitch — the
        // ordinary/common case in lace and general shaping (docs §5:
        // "multiple stitches share the same insertion target"). Every
        // sibling legitimately touches the shared target; none should be
        // flagged against it or against each other under ordinary
        // (non-adversarial) relaxation.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0: chain-space anchor
        for _ in 0..3 {
            thread
                .stitches
                .push(StitchInstance::new(TR, vec![ref_at(0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            report.ok,
            "unexpected self-intersections in an ordinary 3-tr shell: {:?}",
            report.violations
        );
    }

    #[test]
    fn shell_siblings_squeezed_together_are_still_caught() {
        // Regression guard: an earlier version of this checker excluded
        // *any* two stitches sharing a target from checking against each
        // other (not just against the target), which silently passed two
        // shell siblings forced to ~0.01 apart. Two genuinely distinct
        // loops that end up that close — whether via heavy relaxation or,
        // as here, a deliberately engineered squeeze — must still be
        // flagged; sharing a target only excuses touching *the target*,
        // not each other.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0: chain-space anchor
        for _ in 0..3 {
            thread
                .stitches
                .push(StitchInstance::new(TR, vec![ref_at(0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let raw = crate::geometry::place_scheme(&scheme, &registry).unwrap();
        let squeeze_point = raw.threads[0][1].top;
        let mut pinned = HashMap::new();
        pinned.insert(ref_at(1), squeeze_point);
        pinned.insert(ref_at(2), squeeze_point + Vec3::new(0.01, 0.0, 0.0));
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            !report.ok,
            "expected two shell siblings squeezed to ~0.01 apart to be flagged"
        );
    }

    #[test]
    fn segment_distance_zero_for_touching_segments() {
        let d = segment_segment_distance(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        );
        assert!(d.abs() < 1e-9);
    }

    #[test]
    fn segment_distance_matches_known_parallel_case() {
        // Two unit segments on parallel lines 1 unit apart, fully overlapping.
        let d = segment_segment_distance(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        );
        assert!((d - 1.0).abs() < 1e-9, "expected distance 1.0, got {d}");
    }

    /// Real crochet turns at the end of every row, so each row targets
    /// the one below it in *reverse* — the first stitch after a turn
    /// works into the stitch nearest the hook, which is the last stitch
    /// made in the row below, not the first. Targeting a row in the same
    /// direction it was made (no reversal) creates a bridge that runs the
    /// entire width of the previous row collinear with it — a modelling
    /// mistake in a test scheme, not something the engine should be
    /// expected to tolerate as "ordinary."
    fn push_row_reversed(
        thread: &mut Thread,
        kind: crate::stitch::StitchId,
        prev_row_start: usize,
        width: usize,
    ) {
        for i in 0..width {
            let target = ref_at(prev_row_start + (width - 1 - i));
            thread
                .stitches
                .push(StitchInstance::new(kind, vec![target]));
        }
    }

    #[test]
    fn ordinary_swatch_has_no_self_intersections() {
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..5 {
            thread.stitches.push(StitchInstance::new(CH, vec![])); // row0: 0-4
        }
        push_row_reversed(&mut thread, DC, 0, 5); // row1: 5-9
        push_row_reversed(&mut thread, DC, 5, 5); // row2: 10-14
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            report.ok,
            "unexpected self-intersections: {:?}",
            report.violations
        );
    }

    #[test]
    fn front_post_stitch_reaching_past_a_neighbour_does_not_false_positive() {
        // Row0: chain of 3 (0-2). Row1: dc into row0, reversed (3-5).
        // Row2: dc into row1, reversed again (6-8) — row2[1] (index 7)
        // ends up targeting row1[1] (index 4) directly below it. Index 9
        // is a front-post tr *also* targeting row1[1] — the classic
        // "front post treble two rows back" construction, reaching past
        // row2[1] without occupying its space.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..3 {
            thread.stitches.push(StitchInstance::new(CH, vec![])); // 0,1,2
        }
        push_row_reversed(&mut thread, DC, 0, 3); // row1: 3,4,5
        push_row_reversed(&mut thread, DC, 3, 3); // row2: 6,7,8
        thread.stitches.push(
            StitchInstance::new(TR, vec![ref_at(4)])
                .with_loop_target(crate::graph::LoopTarget::FrontPost),
        ); // 9: front-post tr, also targeting row1[1] (index 4)
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            report.ok,
            "front-post stitch should not false-positive as a self-intersection: {:?}",
            report.violations
        );
    }

    #[test]
    fn pinning_two_unrelated_stitches_to_the_same_point_is_flagged() {
        // A directly engineered collision: force two unrelated stitches
        // (not sharing any stitch reference, so not "adjacent") to the
        // exact same relaxed position, simulating a scheme whose geometry
        // is genuinely impossible once relaxed.
        let registry = crate::stitch::StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..2 {
            thread.stitches.push(StitchInstance::new(CH, vec![])); // 0,1: two independent anchors
        }
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 2
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(1)])); // 3
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let raw = crate::geometry::place_scheme(&scheme, &registry).unwrap();
        let collision_point = raw.threads[0][2].top;
        let mut pinned = HashMap::new();
        pinned.insert(ref_at(2), collision_point);
        pinned.insert(ref_at(3), collision_point); // force onto the same point
        let params = RelaxationParams {
            pinned,
            ..RelaxationParams::default()
        };

        let relaxed = relax_scheme(&scheme, &registry, &params).unwrap();
        let report = validate_scheme(&scheme, &registry, &relaxed, DEFAULT_YARN_DIAMETER).unwrap();
        assert!(
            !report.ok,
            "expected a flagged self-intersection for two stitches pinned to the same point"
        );
    }

    #[test]
    fn check_round_passes_for_a_consistent_flat_circle_first_round() {
        // ring anchor (0), then 6 dc all targeting it — a standard flat
        // circle round 1.
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0: ring anchor
        for _ in 0..6 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![ref_at(0)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let previous_round: HashSet<StitchRef> = [ref_at(0)].into_iter().collect();
        let new_round: Vec<StitchRef> = (1..=6).map(ref_at).collect();
        assert!(check_round(&scheme, &previous_round, &new_round, 6).is_ok());
    }

    #[test]
    fn check_round_flags_wrong_count_and_stray_targets() {
        let mut thread = Thread::new();
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 0: claimed round
        thread.stitches.push(StitchInstance::new(CH, vec![])); // 1: NOT in claimed round
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 2
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(1)])); // 3: stray target
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let previous_round: HashSet<StitchRef> = [ref_at(0)].into_iter().collect();
        let new_round: Vec<StitchRef> = vec![ref_at(2), ref_at(3)];
        let err = check_round(&scheme, &previous_round, &new_round, 3).unwrap_err();
        assert_eq!(err.expected, 3);
        assert_eq!(err.actual, 2);
        assert_eq!(err.stray_targets, vec![ref_at(1)]);
    }
}
