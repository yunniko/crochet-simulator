//! M4: WASM bridge exposing `crochet-core` to the browser viewer.
//!
//! Deliberately thin: build a hardcoded demo scheme, run it through the
//! full core pipeline (raw placement -> relaxation -> validation, exactly
//! as `crochet-core`'s own tests do), and hand the *relaxed* yarn path
//! plus which parts of it are flagged back to JS as plain, serialisable
//! data. No scheme-building/editing capability here — that's M5.

use std::collections::HashSet;

use crochet_core::geometry::PlacementError;
use crochet_core::graph::{Scheme, StitchInstance, StitchRef, Thread};
use crochet_core::path::{relaxed_yarn_segments, SegmentOwner};
use crochet_core::relax::{relax_scheme, RelaxationParams};
use crochet_core::stitch::{StitchRegistry, DC, MR};
use crochet_core::validate::{check_self_intersections, DEFAULT_YARN_DIAMETER};
use crochet_core::vec3::Vec3;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
pub struct WasmVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<Vec3> for WasmVec3 {
    fn from(v: Vec3) -> Self {
        WasmVec3 {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

#[derive(Serialize)]
pub struct WasmSegment {
    pub start: WasmVec3,
    pub end: WasmVec3,
    /// True if this segment is part of a self-intersection M3 flagged —
    /// the "visible flag" GOALS.md M4 asks for.
    pub flagged: bool,
    /// Human-readable owner, for tooltips/debugging — e.g. "stitch[7]" or
    /// "bridge[6->7]" (see `crochet_core::path::SegmentOwner`).
    pub label: String,
}

#[derive(Serialize)]
pub struct DemoResult {
    pub stitch_count: usize,
    pub ok: bool,
    pub violation_count: usize,
    pub segments: Vec<WasmSegment>,
}

fn owner_label(owner: SegmentOwner) -> String {
    match owner {
        SegmentOwner::Stitch(r) => format!("stitch[{}]", r.index),
        SegmentOwner::Bridge(a, b) => format!("bridge[{}->{}]", a.index, b.index),
    }
}

fn ref_at(index: usize) -> StitchRef {
    StitchRef::new(0, index)
}

/// Round 1 of a standard flat-circle start (docs §5a): a tightened magic
/// ring, 6 dc into it — the first round of the classic amigurumi opening.
/// Deliberately stops there rather than adding round 2's 2-in-each
/// increase: that surfaced a real, distinct limitation while building
/// this demo — a dense round's several increases, each individually fine
/// against *its own* target, can still collide with a *neighbouring*
/// increase's children, since capacity/ring placement (§5a) only reasons
/// about siblings of the *same* target, not local density across nearby
/// targets. Documented in `docs/crochet-context.md` §5a and
/// `HANDOVER.md` rather than forced to "pass" by more constant-tuning,
/// which made other things worse when tried. A follow-up problem for
/// M2/M3, not M4.
fn build_flat_circle_scheme() -> Scheme {
    let mut thread = Thread::new();
    thread.stitches.push(StitchInstance::new(MR, vec![])); // 0
    for _ in 0..6 {
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 1-6
    }
    let mut scheme = Scheme::new();
    scheme.add_thread(thread);
    scheme
}

/// A deliberately overloaded ring (docs §5a: "eleven won't fit") — 15 dc
/// crammed into one tightened magic ring, well past comfortable capacity.
/// Exists to prove the "visible flag" path actually lights up, not just
/// the clean path.
fn build_overloaded_ring_scheme() -> Scheme {
    let mut thread = Thread::new();
    thread.stitches.push(StitchInstance::new(MR, vec![])); // 0
    for _ in 0..15 {
        thread
            .stitches
            .push(StitchInstance::new(DC, vec![ref_at(0)])); // 1-15
    }
    let mut scheme = Scheme::new();
    scheme.add_thread(thread);
    scheme
}

fn compute(scheme: &Scheme) -> Result<DemoResult, PlacementError> {
    let registry = StitchRegistry::with_uk_basics();
    let relaxed = relax_scheme(scheme, &registry, &RelaxationParams::default())?;
    let path_segments = relaxed_yarn_segments(scheme, &registry, &relaxed)?;
    let report = check_self_intersections(&path_segments, DEFAULT_YARN_DIAMETER);

    let flagged_owners: HashSet<SegmentOwner> =
        report.violations.iter().flat_map(|v| [v.a, v.b]).collect();

    let segments = path_segments
        .into_iter()
        .map(|s| WasmSegment {
            start: s.start.into(),
            end: s.end.into(),
            flagged: flagged_owners.contains(&s.owner),
            label: owner_label(s.owner),
        })
        .collect();

    Ok(DemoResult {
        stitch_count: scheme.total_stitch_count(),
        ok: report.ok,
        violation_count: report.violations.len(),
        segments,
    })
}

fn to_js(result: Result<DemoResult, PlacementError>) -> Result<JsValue, JsValue> {
    let result = result.map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// A clean, validating demo scheme (a standard flat-circle start).
#[wasm_bindgen]
pub fn compute_flat_circle_demo() -> Result<JsValue, JsValue> {
    to_js(compute(&build_flat_circle_scheme()))
}

/// A deliberately overloaded demo scheme, expected to be flagged.
#[wasm_bindgen]
pub fn compute_overloaded_demo() -> Result<JsValue, JsValue> {
    to_js(compute(&build_overloaded_ring_scheme()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_circle_demo_validates_clean() {
        let result = compute(&build_flat_circle_scheme()).unwrap();
        assert_eq!(result.stitch_count, 7);
        assert!(
            result.ok,
            "expected the flat circle demo to validate cleanly: {:?}",
            result
                .segments
                .iter()
                .filter(|s| s.flagged)
                .map(|s| &s.label)
                .collect::<Vec<_>>()
        );
        assert_eq!(result.violation_count, 0);
        assert!(!result.segments.is_empty());
        assert!(result.segments.iter().all(|s| !s.flagged));
    }

    #[test]
    fn overloaded_demo_is_flagged() {
        let result = compute(&build_overloaded_ring_scheme()).unwrap();
        assert_eq!(result.stitch_count, 16);
        assert!(
            !result.ok,
            "expected the overloaded ring demo to be flagged"
        );
        assert!(result.violation_count > 0);
        assert!(result.segments.iter().any(|s| s.flagged));
    }
}
