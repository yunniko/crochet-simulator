//! Raw placement geometry for M1 — see GOALS.md M1 and
//! docs/crochet-context.md §6 note: this is deliberately the *raw*
//! placement, with no relaxation/elasticity (that's M2) and no
//! self-intersection validation (that's M3). It exists to prove the
//! insertion graph produces sane 3D point/segment coordinates at all.

use std::collections::HashMap;

use crate::graph::{LoopTarget, Scheme, StitchRef};
use crate::stitch::{StitchId, StitchRegistry};
use crate::vec3::Vec3;

/// Horizontal step between successive links of a chain with no target.
const CHAIN_STEP_X: f64 = 1.0;
/// Lateral spread applied to each additional stitch sharing a target
/// (an increase), purely so siblings don't all land on the same point.
/// 0.5, not something smaller: for any stitch taller than `dc` (more than
/// one own-path sub-segment — see `StitchDef::path_segments`), a sibling's
/// *lower* sub-segment passes close to the bridge connecting it to the
/// *next* sibling, at roughly half this spread — empirically, 0.3 put
/// that near-miss right under `crate::validate::DEFAULT_YARN_DIAMETER`
/// (a false positive on an entirely ordinary 2-stitch increase in `tr`
/// or taller). 0.5 clears it with margin. See `crate::validate` module
/// docs for the fuller picture, including a *deeper*, not-yet-fixed
/// limitation this does not solve: wide multi-way shares (roughly 5+
/// stitches into one point) can still fold onto themselves during
/// relaxation, independent of this constant.
const INCREASE_SPREAD_X: f64 = 0.5;
/// Depth offset applied to a front/back post stitch's base, along the
/// axis orthogonal to both the lateral (x) and height (z) axes. A post
/// stitch reaches around an earlier stitch's post instead of inserting
/// into its top loops (docs §2), so its yarn path genuinely does not
/// occupy the same space as the stitch(es) it reaches past — this is
/// what lets M3's self-intersection checker treat it correctly as *not*
/// a collision without needing to special-case "is this a post stitch."
/// Larger than `crate::validate::DEFAULT_YARN_DIAMETER` on purpose.
const POST_DEPTH_OFFSET: f64 = 0.4;

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
    let mut placed: HashMap<StitchRef, PlacedStitch> = HashMap::new();
    let mut increase_use_count: HashMap<StitchRef, u32> = HashMap::new();
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
                    // `ch`: no target at all (docs §3/§4/§8 invariant 2) —
                    // laid out as a step from wherever the thread left off.
                    let base = prev_top.unwrap_or(Vec3::ZERO);
                    let top = base + Vec3::new(CHAIN_STEP_X, 0.0, 0.0);
                    (base, top)
                }
                [single] => {
                    let target_top = placed
                        .get(single)
                        .ok_or(PlacementError::TargetNotYetPlaced(*single))?
                        .top;
                    let sibling_index = *increase_use_count.get(single).unwrap_or(&0);
                    increase_use_count.insert(*single, sibling_index + 1);
                    let depth_offset = match stitch.loop_target {
                        LoopTarget::FrontPost => Vec3::new(0.0, POST_DEPTH_OFFSET, 0.0),
                        LoopTarget::BackPost => Vec3::new(0.0, -POST_DEPTH_OFFSET, 0.0),
                        LoopTarget::Both | LoopTarget::FrontOnly | LoopTarget::BackOnly => {
                            Vec3::ZERO
                        }
                    };
                    let base = target_top
                        + Vec3::new(sibling_index as f64 * INCREASE_SPREAD_X, 0.0, 0.0)
                        + depth_offset;
                    let top = base + Vec3::new(0.0, 0.0, def.height());
                    (base, top)
                }
                multiple => {
                    // Decrease: base is the average of every target's top.
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
    use crate::stitch::{CH, DC, DTR, TR};

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
        assert_ne!(
            first.base.x, second.base.x,
            "increase siblings must not coincide"
        );
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
