//! Reconstructs the complete, continuous relaxed yarn path — see
//! docs/crochet-context.md §4a note on threads: the physical strand
//! doesn't teleport between stitches, so consecutive stitches in working
//! order need a "bridge" segment between them whenever their positions
//! don't already coincide (ordinary same-target rows), not just each
//! stitch's own base-to-top sub-path. This is what M3's self-intersection
//! checker (`crate::validate`) actually tests against.
//!
//! Every segment also carries its **raw** (M1, pre-relaxation,
//! pre-pinning) start/end alongside its relaxed one. `crate::validate`
//! uses the raw pair to decide whether two segments are structurally "the
//! same point" (see that module's docs for why raw coordinates, not
//! relaxed ones, are the right thing to compare).

use crate::geometry::{place_scheme, PlacementError};
use crate::graph::{Scheme, StitchRef};
use crate::relax::RelaxedScheme;
use crate::stitch::StitchRegistry;
use crate::vec3::Vec3;

/// Which stitch(es) a segment belongs to, for reporting *where* a
/// self-intersection is (GOALS.md M3: "the specific problem location").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentOwner {
    /// Part of this stitch's own base-to-top sub-path.
    Stitch(StitchRef),
    /// The connecting strand between two consecutive stitches in working
    /// order (`from` -> `to`), when their positions don't coincide.
    Bridge(StitchRef, StitchRef),
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub start: Vec3,
    pub end: Vec3,
    /// This segment's endpoints in the raw (M1) placement — see module
    /// docs and `crate::validate`.
    pub raw_start: Vec3,
    pub raw_end: Vec3,
    pub owner: SegmentOwner,
}

const BRIDGE_EPSILON: f64 = 1e-9;

/// Builds every thread's continuous relaxed yarn path as a flat list of
/// segments. `relaxed` must have been produced from the same `scheme`
/// (positions are looked up by `StitchRef`, not re-derived).
pub fn relaxed_yarn_segments(
    scheme: &Scheme,
    registry: &StitchRegistry,
    relaxed: &RelaxedScheme,
) -> Result<Vec<PathSegment>, PlacementError> {
    let raw = place_scheme(scheme, registry)?;
    let mut segments = Vec::new();

    for (thread_idx, thread) in scheme.threads.iter().enumerate() {
        let mut prev: Option<(StitchRef, Vec3, Vec3)> = None; // (ref, relaxed top, raw top)

        for (i, stitch) in thread.stitches.iter().enumerate() {
            let r = StitchRef::new(thread_idx, i);
            let def = registry
                .get(stitch.kind)
                .ok_or(PlacementError::UnknownStitchKind(stitch.kind))?;
            let raw_stitch = &raw.threads[thread_idx][i];
            let relaxed_top = relaxed
                .position(r)
                .expect("relaxed position missing for a stitch from the same scheme");

            let relaxed_base = match stitch.targets.as_slice() {
                // A thread's very first stitch has no `prev` and no
                // target — its "base" is the free tail end of the working
                // yarn before any stitch exists, nothing else in the
                // model anchors it. It used to default to a hardcoded
                // world-origin `Vec3::ZERO`, harmless while raw and
                // relaxed placement stay close together (an open chain
                // never moves far from where it started) but wrong once
                // they diverge substantially (M9: a chain pulled into a
                // closed ring by a slip stitch relaxes into a shape
                // nowhere near the origin it started at raw-placement
                // time) — the fixed origin became a phantom anchor point
                // the rest of the (correctly relaxed) ring would cross
                // right through, since nothing was actually there.
                // Fixed: track the same rigid offset from raw that the
                // stitch's own top ended up with, so the tail moves
                // together with the stitch it belongs to instead of
                // staying nailed to a point in space that has no
                // physical meaning once the piece has actually moved.
                [] => prev
                    .map(|(_, top, _)| top)
                    .unwrap_or_else(|| raw_stitch.base + (relaxed_top - raw_stitch.top)),
                [single] => {
                    let raw_target_top = raw.threads[single.thread][single.index].top;
                    let relaxed_target_top = relaxed
                        .position(*single)
                        .expect("relaxed position missing for a target from the same scheme");
                    relaxed_target_top + (raw_stitch.base - raw_target_top)
                }
                multiple => {
                    let mut raw_sum = Vec3::ZERO;
                    let mut relaxed_sum = Vec3::ZERO;
                    for target in multiple {
                        raw_sum = raw_sum + raw.threads[target.thread][target.index].top;
                        relaxed_sum = relaxed_sum
                            + relaxed.position(*target).expect(
                                "relaxed position missing for a target from the same scheme",
                            );
                    }
                    let n = multiple.len() as f64;
                    relaxed_sum * (1.0 / n) + (raw_stitch.base - raw_sum * (1.0 / n))
                }
            };

            if let Some((prev_ref, prev_relaxed_top, prev_raw_top)) = prev {
                if prev_relaxed_top.distance(&relaxed_base) > BRIDGE_EPSILON {
                    segments.push(PathSegment {
                        start: prev_relaxed_top,
                        end: relaxed_base,
                        raw_start: prev_raw_top,
                        raw_end: raw_stitch.base,
                        owner: SegmentOwner::Bridge(prev_ref, r),
                    });
                }
            }

            let n = def.path_segments().max(1);
            let points: Vec<Vec3> = (0..=n)
                .map(|k| relaxed_base + (relaxed_top - relaxed_base) * (k as f64 / n as f64))
                .collect();
            for (w, raw_w) in points.windows(2).zip(raw_stitch.path.windows(2)) {
                segments.push(PathSegment {
                    start: w[0],
                    end: w[1],
                    raw_start: raw_w[0],
                    raw_end: raw_w[1],
                    owner: SegmentOwner::Stitch(r),
                });
            }

            prev = Some((r, relaxed_top, raw_stitch.top));
        }
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{StitchInstance, Thread};
    use crate::relax::{relax_scheme, RelaxationParams};
    use crate::stitch::{CH, DC};

    #[test]
    fn produces_finite_continuous_segments_for_a_simple_row() {
        let registry = StitchRegistry::with_uk_basics();
        let mut thread = Thread::new();
        for _ in 0..3 {
            thread.stitches.push(StitchInstance::new(CH, vec![]));
        }
        for i in 0..3 {
            thread
                .stitches
                .push(StitchInstance::new(DC, vec![StitchRef::new(0, i)]));
        }
        let mut scheme = Scheme::new();
        scheme.add_thread(thread);

        let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
        let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();

        assert!(!segments.is_empty());
        for s in &segments {
            assert!(s.start.is_finite() && s.end.is_finite());
            assert!(s.raw_start.is_finite() && s.raw_end.is_finite());
        }
    }
}
