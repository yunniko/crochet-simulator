//! WASM bridge exposing `crochet-core` to the browser viewer/editor.
//!
//! `compute_scheme` (M5) is the real API: it takes a JSON-ish wire
//! description of a scheme (whatever the editor has built so far) and
//! runs it through the exact same core pipeline `crochet-core`'s own
//! tests use (raw placement -> relaxation -> validation), handing the
//! *relaxed* yarn path plus which parts are flagged back to JS. No
//! scheme-building logic lives here — the editor (`web/`) owns the
//! stitch list; this crate only ever *computes* whatever it's given.
//! M4's two hardcoded demo builders (`build_flat_circle_scheme`,
//! `build_overloaded_ring_scheme`) stay as internal regression-test
//! fixtures (§ tests below) but are no longer exported — their JS-side
//! equivalents are now ordinary presets built from `compute_scheme`,
//! same as anything else the editor can build.

use std::collections::HashSet;

use crochet_core::geometry::PlacementError;
use crochet_core::graph::{LoopTarget, Scheme, StitchInstance, StitchRef, Thread};
use crochet_core::path::{relaxed_yarn_segments, SegmentOwner};
use crochet_core::relax::{relax_scheme, RelaxationParams};
use crochet_core::stitch::{
    CapacityStyle, StitchId, StitchRegistry, CH, DC, DTR, HTR, MR, QUAD_TR, SS, TR, TRTR,
};
use crochet_core::validate::{check_self_intersections, DEFAULT_YARN_DIAMETER};
use crochet_core::vec3::Vec3;
use serde::{Deserialize, Serialize};
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
    /// True if this segment is part of a self-intersection M3 flagged.
    pub flagged: bool,
    /// Human-readable owner, for tooltips/debugging — e.g. "stitch[7]" or
    /// "bridge[6->7]" (see `crochet_core::path::SegmentOwner`).
    pub label: String,
}

#[derive(Serialize)]
pub struct ComputeResult {
    pub stitch_count: usize,
    pub ok: bool,
    pub violation_count: usize,
    pub segments: Vec<WasmSegment>,
}

/// The wire format for a single stitch, as the editor builds it up.
/// `targets` are indices into the *same* scheme's stitch list (M5 is
/// single-thread only — docs §4a's multi-thread support is still
/// deferred). Every field but `kind` is optional so the editor doesn't
/// need to send boilerplate for the common case.
#[derive(Deserialize)]
pub struct WireStitch {
    pub kind: String,
    #[serde(default)]
    pub targets: Vec<usize>,
    pub loop_target: Option<String>,
    pub capacity_override: Option<String>,
}

#[derive(Deserialize)]
pub struct WireScheme {
    pub stitches: Vec<WireStitch>,
}

fn parse_kind(s: &str) -> Result<StitchId, String> {
    match s {
        "ch" => Ok(CH),
        "ss" => Ok(SS),
        "dc" => Ok(DC),
        "htr" => Ok(HTR),
        "tr" => Ok(TR),
        "dtr" => Ok(DTR),
        "trtr" => Ok(TRTR),
        "quad_tr" => Ok(QUAD_TR),
        "mr" => Ok(MR),
        other => Err(format!("unknown stitch kind: \"{other}\"")),
    }
}

fn parse_loop_target(s: &str) -> Result<LoopTarget, String> {
    match s {
        "Both" => Ok(LoopTarget::Both),
        "FrontOnly" => Ok(LoopTarget::FrontOnly),
        "BackOnly" => Ok(LoopTarget::BackOnly),
        "FrontPost" => Ok(LoopTarget::FrontPost),
        "BackPost" => Ok(LoopTarget::BackPost),
        other => Err(format!("unknown loop_target: \"{other}\"")),
    }
}

fn parse_capacity_style(s: &str) -> Result<CapacityStyle, String> {
    match s {
        "Fixed" => Ok(CapacityStyle::Fixed),
        "Elastic" => Ok(CapacityStyle::Elastic),
        "TightenedRing" => Ok(CapacityStyle::TightenedRing),
        other => Err(format!("unknown capacity_override: \"{other}\"")),
    }
}

/// Builds a `Scheme` from the wire format, validating targets reference
/// only *earlier* stitches (docs §4: the forward-reference discipline the
/// whole model relies on) — an editor bug or a malformed request gets a
/// clear error back, never a panic or silently-wrong geometry.
fn build_scheme_from_wire(wire: &WireScheme) -> Result<Scheme, String> {
    let mut thread = Thread::new();
    for (i, s) in wire.stitches.iter().enumerate() {
        let kind = parse_kind(&s.kind)?;
        let mut targets = Vec::with_capacity(s.targets.len());
        for &idx in &s.targets {
            if idx >= i {
                return Err(format!(
                    "stitch {i} targets stitch {idx}, which isn't placed yet (targets must be earlier in the scheme)"
                ));
            }
            targets.push(StitchRef::new(0, idx));
        }
        let mut instance = StitchInstance::new(kind, targets);
        if let Some(lt) = &s.loop_target {
            instance = instance.with_loop_target(parse_loop_target(lt)?);
        }
        if let Some(co) = &s.capacity_override {
            instance = instance.with_capacity_override(parse_capacity_style(co)?);
        }
        thread.stitches.push(instance);
    }
    let mut scheme = Scheme::new();
    scheme.add_thread(thread);
    Ok(scheme)
}

fn owner_label(owner: SegmentOwner) -> String {
    match owner {
        SegmentOwner::Stitch(r) => format!("stitch[{}]", r.index),
        SegmentOwner::Bridge(a, b) => format!("bridge[{}->{}]", a.index, b.index),
    }
}

fn compute(scheme: &Scheme) -> Result<ComputeResult, PlacementError> {
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

    Ok(ComputeResult {
        stitch_count: scheme.total_stitch_count(),
        ok: report.ok,
        violation_count: report.violations.len(),
        segments,
    })
}

/// The M5 API: computes whatever scheme the editor has built so far.
/// Errors (a bad stitch kind, a forward target reference, an unplaced
/// target) come back as a rejected promise on the JS side with a message
/// good enough to show the Owner directly, not a generic failure.
#[wasm_bindgen]
pub fn compute_scheme(wire: JsValue) -> Result<JsValue, JsValue> {
    let wire_scheme: WireScheme = serde_wasm_bindgen::from_value(wire)
        .map_err(|e| JsValue::from_str(&format!("couldn't read scheme: {e}")))?;
    let scheme = build_scheme_from_wire(&wire_scheme).map_err(|e| JsValue::from_str(&e))?;
    let result = compute(&scheme).map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ref_at(index: usize) -> StitchRef {
        StitchRef::new(0, index)
    }

    /// Round 1 of a standard flat-circle start (docs §5a): a tightened
    /// magic ring, 6 dc into it. Deliberately stops there rather than
    /// adding round 2's 2-in-each increase — building that surfaced the
    /// cross-target local-density limitation documented in
    /// `docs/crochet-context.md` §5a; still open, so kept out of the
    /// preset used as a "known good" regression fixture.
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

    /// A deliberately overloaded ring (docs §5a: "eleven won't fit") — 15
    /// dc crammed into one tightened magic ring, well past comfortable
    /// capacity. Exists to prove the "visible flag" path actually lights
    /// up, not just the clean path.
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

    #[test]
    fn wire_scheme_builds_the_same_flat_circle() {
        let wire = WireScheme {
            stitches: {
                let mut v = vec![WireStitch {
                    kind: "mr".into(),
                    targets: vec![],
                    loop_target: None,
                    capacity_override: None,
                }];
                for _ in 0..6 {
                    v.push(WireStitch {
                        kind: "dc".into(),
                        targets: vec![0],
                        loop_target: None,
                        capacity_override: None,
                    });
                }
                v
            },
        };
        let scheme = build_scheme_from_wire(&wire).unwrap();
        let result = compute(&scheme).unwrap();
        assert_eq!(result.stitch_count, 7);
        assert!(result.ok);
    }

    #[test]
    fn wire_scheme_rejects_a_forward_target_reference() {
        let wire = WireScheme {
            stitches: vec![
                WireStitch {
                    kind: "ch".into(),
                    targets: vec![],
                    loop_target: None,
                    capacity_override: None,
                },
                WireStitch {
                    kind: "dc".into(),
                    targets: vec![1],
                    loop_target: None,
                    capacity_override: None,
                },
            ],
        };
        let err = build_scheme_from_wire(&wire).unwrap_err();
        assert!(
            err.contains("isn't placed yet"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn wire_scheme_rejects_an_unknown_stitch_kind() {
        let wire = WireScheme {
            stitches: vec![WireStitch {
                kind: "bobble".into(),
                targets: vec![],
                loop_target: None,
                capacity_override: None,
            }],
        };
        let err = build_scheme_from_wire(&wire).unwrap_err();
        assert!(
            err.contains("unknown stitch kind"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn wire_scheme_supports_freeform_non_row_targeting() {
        // A spike stitch: index 4 targets index 0, two stitches further
        // back than its immediate predecessor (index 3) — docs §4: the
        // model never assumes row/round structure. Mirrors crochet-core's
        // own freeform test. Unlike an earlier draft of this preset (a
        // three-way cross-link that turned out to genuinely self-
        // intersect — see HANDOVER.md's M5 entry), this one is checked
        // end-to-end for a clean result, not just stitch count, so a
        // regression here can't hide the way that one did.
        let wire = WireScheme {
            stitches: vec![
                WireStitch {
                    kind: "ch".into(),
                    targets: vec![],
                    loop_target: None,
                    capacity_override: None,
                },
                WireStitch {
                    kind: "ch".into(),
                    targets: vec![],
                    loop_target: None,
                    capacity_override: None,
                },
                WireStitch {
                    kind: "ch".into(),
                    targets: vec![],
                    loop_target: None,
                    capacity_override: None,
                },
                WireStitch {
                    kind: "dc".into(),
                    targets: vec![2],
                    loop_target: None,
                    capacity_override: None,
                },
                WireStitch {
                    kind: "dc".into(),
                    targets: vec![0],
                    loop_target: None,
                    capacity_override: None,
                },
            ],
        };
        let scheme = build_scheme_from_wire(&wire).unwrap();
        let result = compute(&scheme).unwrap();
        assert_eq!(result.stitch_count, 5);
        assert!(
            result.ok,
            "expected the freeform spike demo to validate cleanly: {:?}",
            result
                .segments
                .iter()
                .filter(|s| s.flagged)
                .map(|s| &s.label)
                .collect::<Vec<_>>()
        );
        assert_eq!(result.violation_count, 0);
    }
}
