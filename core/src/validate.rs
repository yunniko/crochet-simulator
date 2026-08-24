//! M3: geometry validation — see docs/crochet-context.md §8 invariant 4
//! (self-intersection) and §7/§8 invariant 3 (stitch-count self-check).
//!
//! The self-intersection checker runs on the *relaxed* (M2) yarn path
//! (`crate::path`), not the raw M1 placement — docs §8 invariant 4 is
//! explicit that it's the relaxed shape that must be checked. It flags
//! any two non-adjacent segments whose closest approach is below a yarn-
//! diameter threshold. "Adjacent" (never flagged) means the two segments'
//! *1-hop neighbourhoods* overlap (see `build_neighborhoods`) — not just
//! literally the same stitch, but anything sharing a common directly-
//! linked point (e.g. a chain stitch and a later stitch worked into the
//! chain stitch *before* it both legitimately touch that shared point).
//! This is deliberately permissive around what counts as "the same
//! connective structure" (favouring no false positives, per the
//! milestone's explicit priority on post stitches not false-positiving,
//! over catching every possible true positive) — a known, documented
//! limitation, not an oversight.

use std::collections::{HashMap, HashSet};

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
    let neighborhoods = build_neighborhoods(scheme);
    Ok(check_self_intersections(
        &segments,
        &neighborhoods,
        min_distance,
    ))
}

/// Flags any two non-adjacent segments closer than `min_distance` as a
/// self-intersection (docs §8 invariant 4). `neighborhoods` names, per
/// stitch, the set of points it's expected to touch by construction (see
/// `build_neighborhoods`); two segments are never flagged against each
/// other if any of their owning stitches' neighbourhoods overlap.
/// O(n^2) over segment count — fine for design-tool-sized swatches; a
/// spatial hash would be the first thing to add if this ever needs to
/// scale to large schemes.
pub fn check_self_intersections(
    segments: &[PathSegment],
    neighborhoods: &HashMap<StitchRef, HashSet<StitchRef>>,
    min_distance: f64,
) -> IntersectionReport {
    let mut violations = Vec::new();
    for i in 0..segments.len() {
        for j in (i + 1)..segments.len() {
            if segments_are_adjacent(&segments[i], &segments[j], neighborhoods) {
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

/// For every stitch, the set of points it shares *by construction* — used
/// to tell "two segments touch because they're structurally the same
/// connection" from "two segments happen to be close, which is exactly
/// what this checker exists to catch." A stitch's own neighbourhood is
/// itself, plus each of its insertion target(s) (that's what an insertion
/// point *is*: loops sharing a location on purpose), plus — only when it
/// has *no* targets (a `ch`) — its immediate predecessor in the same
/// thread, since a zero-target stitch's base is *defined* as the previous
/// stitch's top (docs §4), not merely close to it.
///
/// Two segments are adjacent when their owning stitches' neighbourhoods
/// *overlap* (not merely identical) — this is what correctly excludes,
/// say, a chain stitch and a later stitch worked into the chain stitch
/// immediately before it: both touch that earlier chain stitch's top, so
/// their neighbourhoods share that point, even though the chain stitch
/// and the later stitch aren't linked to each other directly. It is
/// deliberately *not* full transitive closure over the whole graph (which
/// would eventually connect everything through the working-order
/// backbone) — only this one hop.
fn build_neighborhoods(scheme: &Scheme) -> HashMap<StitchRef, HashSet<StitchRef>> {
    let mut neighborhoods: HashMap<StitchRef, HashSet<StitchRef>> = HashMap::new();
    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        for (i, stitch) in thread.stitches.iter().enumerate() {
            let r = StitchRef::new(thread_idx, i);
            let entry = neighborhoods.entry(r).or_default();
            entry.insert(r);
            for target in &stitch.targets {
                entry.insert(*target);
            }
            if stitch.targets.is_empty() && i > 0 {
                entry.insert(StitchRef::new(thread_idx, i - 1));
            }
        }
    }
    neighborhoods
}

fn owner_refs(owner: SegmentOwner) -> [Option<StitchRef>; 2] {
    match owner {
        SegmentOwner::Stitch(r) => [Some(r), None],
        SegmentOwner::Bridge(a, b) => [Some(a), Some(b)],
    }
}

fn segments_are_adjacent(
    a: &PathSegment,
    b: &PathSegment,
    neighborhoods: &HashMap<StitchRef, HashSet<StitchRef>>,
) -> bool {
    let a_refs = owner_refs(a.owner);
    let b_refs = owner_refs(b.owner);
    for ra in a_refs.into_iter().flatten() {
        let Some(na) = neighborhoods.get(&ra) else {
            continue;
        };
        for rb in b_refs.into_iter().flatten() {
            let Some(nb) = neighborhoods.get(&rb) else {
                continue;
            };
            if !na.is_disjoint(nb) {
                return true;
            }
        }
    }
    false
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
