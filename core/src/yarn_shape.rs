//! M14: the real per-stitch yarn path — replacing M1-M13's straight
//! `base`-to-`top` line with actual loop topology. A crochet stitch is
//! not a straight post with a decorative wiggle painted over it after
//! the fact (M7-M13's approach, entirely in the web renderer); it is
//! yarn that genuinely loops back through itself — a chain link loops
//! through its predecessor, each "yarn over, pull through" stage of a
//! taller stitch closes a real loop. Moving this into `core` (rather
//! than leaving it in `web/lib/yarn-shape.ts`) means `validate.rs`'s
//! self-intersection checker and the final rendered path now reason
//! about the same real geometry, instead of the checker validating a
//! simplified line while the renderer draws something the physics never
//! actually confirmed was collision-free.
//!
//! `loop_arc_points` is the shared primitive (ported directly from the
//! TypeScript prototype that was iterated on with the Owner first,
//! preserved here since the math is already validated): a path that
//! sweeps most of the way around a circle — not just a bend — so a tube
//! swept along it shows a genuine near-closed ring with a visible hole,
//! the way a real yarn loop does. Chains use one big loop per link
//! (spanning the *entire* base-to-top step — per the Owner: "chains are
//! essentially one loop, which is pulled through previous loop," not a
//! loop plus straight leads); posts use one smaller loop per real
//! pull-through stage (`bar_count`, mirroring `StitchDef`'s own
//! `pre_wraps`/`draw_through`), threaded onto a straight shaft.
//!
//! Deliberately scoped for M14: this changes the *fine-grained path*
//! reconstruction (what `place_scheme` stores as each stitch's `path`,
//! and what `path.rs` reconstructs post-relaxation) but not the *coarse*
//! relaxation forces themselves (`relax.rs`'s springs/DER/barrier still
//! act on each stitch's representative `base`/`top` points, unchanged).
//! A deeper version of this — a stitch's own base keypoints genuinely
//! routing through its *target's* own head-loop shape, rather than each
//! stitch independently drawing its own loop from a point — is future
//! work, not attempted here; see HANDOVER.md's M14 entry.

use crate::stitch::{DrawThrough, StitchDef};
use crate::vec3::Vec3;

/// A stable (right, up) frame perpendicular to `forward`, for offsetting
/// a curve sideways. Any reference not nearly parallel to `forward`
/// gives a stable frame via two cross products — which reference is used
/// doesn't matter, only that it's consistently not-parallel.
fn perpendicular_frame(forward: Vec3) -> (Vec3, Vec3) {
    let reference = if forward.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let right = forward.cross(reference).normalized();
    let up = right.cross(forward);
    (right, up)
}

/// The core loop primitive, in 2D **local** (along, side) coordinates:
/// `along` runs from 0 at the loop's start to `local_height` at its end;
/// `side` is the sideways bulge (unsigned — the caller applies which
/// direction and axis). Both endpoints are exactly `(0,0)` and
/// `(local_height,0)`, but the path between them sweeps
/// `2*PI - 2*half_gap` radians around a circle — most of a full turn,
/// not a partial bend — so a tube swept along it shows a real
/// near-closed ring with a visible hole, with only a small gap
/// (`half_gap`, in radians) left where the strand enters/exits.
///
/// Derivation: a circle centred at `(local_height/2, R*cos(half_gap))`
/// with radius `R = local_height / (2*sin(half_gap))` passes through
/// exactly `(0,0)` and `(local_height,0)` at angles `-PI/2 -+ half_gap`
/// from its centre (those two points are symmetric about the centre's
/// vertical axis, which is what makes a single `R`/`half_gap` pair work
/// for any `local_height`). Sweeping the *long* way between those two
/// angles — through the top of the circle instead of the short gap at
/// the bottom — is what makes it a loop instead of a simple arc.
fn loop_arc_points(local_height: f64, half_gap: f64, samples: usize) -> Vec<(f64, f64)> {
    let radius = local_height / (2.0 * half_gap.sin());
    let center_along = local_height / 2.0;
    let center_side = radius * half_gap.cos();
    let start_angle = -std::f64::consts::FRAC_PI_2 - half_gap;
    let sweep = 2.0 * std::f64::consts::PI - 2.0 * half_gap;

    let mut points: Vec<(f64, f64)> = (0..=samples)
        .map(|i| {
            let angle = start_angle + sweep * (i as f64 / samples as f64);
            (
                center_along + radius * angle.cos(),
                center_side + radius * angle.sin(),
            )
        })
        .collect();
    points[0] = (0.0, 0.0);
    points[samples] = (local_height, 0.0);
    points
}

/// Maps `loop_arc_points`' local (along, side) coordinates into 3D.
fn place_loop(
    origin: Vec3,
    forward: Vec3,
    side_axis: Vec3,
    sign: f64,
    local_height: f64,
    half_gap: f64,
    samples: usize,
) -> Vec<Vec3> {
    loop_arc_points(local_height, half_gap, samples)
        .into_iter()
        .map(|(along, side)| origin + forward * along + side_axis * (side * sign))
        .collect()
}

/// One real loop pulled through the *previous* link's own loop — per the
/// Owner: "chains are essentially one loop, which is pulled through
/// previous loop." Spans the entire base-to-top step (no shaft — a chain
/// link basically *is* the loop). Every link loops in the *same* plane
/// (a diagonal, 45 degrees off pure up/right): a real crochet chain, once
/// tensioned, is flat and ribbon-like with a consistent orientation down
/// its whole length (the front loop / back loop / back bump a foundation
/// chain is read by only make sense if every link faces the same way) —
/// **corrected 2026-09-07** per a domain-expert review
/// (`docs/crochet-construction-reference.md`, tier-3 finding 1) that found
/// this used to alternate the plane by link parity on the theory that
/// "real chain links alternate their whole orientation... why a keychain's
/// links don't all lie flat" — an invented mental model with no support in
/// real crochet-construction sources; a rigid metal keychain link is a
/// different physical system from tensioned yarn. The diagonal (rather
/// than pure up/right) is kept for a real reason unrelated to that claim:
/// pure `right` can map to pure depth for some travel directions, which
/// rendered nearly edge-on and made links hard to click in the web
/// prototype this was ported from.
///
/// The *coarse contact proxy* (`build_stitch_curve_points_coarse`, used
/// only internally by `relax.rs`'s per-step collision force — never the
/// judged/rendered geometry) still varies this plane by `stitch_index`
/// parity, via `contact_symmetry_break` below. That is **not** a
/// reinstatement of the debunked construction claim — it's the same kind
/// of numerical degeneracy-breaker as `geometry.rs`'s
/// `CHAIN_SYMMETRY_BREAK_AMPLITUDE`. Raw placement lays consecutive `ch`
/// stitches along a near-straight line, so every link's own `forward`
/// starts out nearly identical; a fixed plane would then give consecutive
/// links' coarse contact loops a nearly-coincident starting bulge, which
/// the barrier contact force reads as overlap and pushes apart with noisy
/// early-step forces — confirmed by a real regression (removing the
/// alternation entirely dropped the slip-stitch ring-closing test's join
/// tightness below its tolerance; see that test's own comment in
/// `relax.rs`). Alternating breaks that only where it's actually needed:
/// the solver's own transient contact proxy, not the shape anything gets
/// judged or rendered against.
const CHAIN_HALF_GAP: f64 = 65.0 * std::f64::consts::PI / 180.0;
const CHAIN_LOOP_SAMPLES: usize = 24;
/// M15: relaxation's per-step contact force (`relax.rs`) needs each
/// stitch's real loop shape rebuilt fresh every step (it's a nonlinear
/// function of the current, still-moving base/top) and checked against
/// every other stitch's — full resolution there is real, measured
/// quadratic cost, not a rounding error. A coarser sample count is
/// "good enough" for a force that only has to nudge things apart over
/// many iterative steps; `path.rs`/`validate.rs`'s one-time final
/// reconstruction keeps full resolution, since that's what actually gets
/// judged for correctness.
const CHAIN_LOOP_SAMPLES_COARSE: usize = 8;

fn build_chain_curve_points(
    base: Vec3,
    top: Vec3,
    height: f64,
    contact_symmetry_break: Option<usize>,
    samples: usize,
) -> Vec<Vec3> {
    let forward = (top - base) * (1.0 / height);
    let (right, up) = perpendicular_frame(forward);
    let plane_a = (up + right).normalized();
    let axis = match contact_symmetry_break {
        // See CHAIN_HALF_GAP's doc comment: contact-proxy-only degeneracy
        // breaker, not a claim about real yarn orientation.
        Some(seed) if !seed.is_multiple_of(2) => (up - right).normalized(),
        _ => plane_a,
    };
    place_loop(base, forward, axis, 1.0, height, CHAIN_HALF_GAP, samples)
}

/// `bar_count` real loops — one per pull-through stage — threaded onto
/// an otherwise-straight shaft, every loop bulging toward the **same**
/// side (a consistent one-directional diagonal lean, not an alternating
/// left-right zigzag). Loops sit over the upper portion of the post
/// (`BAR_SPAN_START`-1.0): the drawn-up loop's shaft reaches the
/// insertion point up to hook height first, and the pull-through stages
/// that close it happen there, near the top, one after another — not
/// spread down to the base.
///
/// **Corrected 2026-09-07** per a domain-expert review
/// (`docs/crochet-construction-reference.md`, tier-3 finding 3): this used
/// to alternate the bulge side per bar for "the ribbed, alternating-cross
/// look a real treble-and-taller post has," but real crochet-construction
/// sources were genuinely split on whether that's a real feature or a
/// documented sign of a technique error (twisted hook / wrong yarn-over
/// order). Resolved by direct visual check against real photos of
/// correctly-worked treble crochet: every yarn-over/pull-through stage on
/// a real post leans the *same* direction (the repeated "yarn over, pull
/// through 2" motion always wraps the same rotational way), producing a
/// consistent diagonal texture the whole height of the post — never an
/// alternating zigzag. Alternating was the misconception; a single fixed
/// `sign` is both simpler and the real construction fact.
///
/// `POST_HALF_GAP`/`BAR_SPAN_START` are tighter than an ordinary-first-
/// pass guess would land on (a wider gap, more of the post given to the
/// loop): this was tuned once the full core test suite actually caught
/// it, moving this from the web renderer (purely cosmetic there, never
/// checked against anything) into `place_scheme`/`path.rs` for real —
/// the original, looser constants gave a `dc`'s own loop a ~0.66-unit
/// sideways reach, comfortably more than half the ~1.0-unit spacing
/// between *ordinary, uncrowded* neighbouring stitches, and calibrated
/// cases the Owner had confirmed correct (7 stitches sharing one target
/// — "hard but possible") started failing self-intersection. A real
/// stitch's own internal loop is a stitch-scale detail, not something
/// that should compete for space with the next stitch over.
const POST_HALF_GAP: f64 = 60.0 * std::f64::consts::PI / 180.0;
const BAR_SPAN_START: f64 = 0.82;
const POST_LOOP_SAMPLES: usize = 14;
/// See `CHAIN_LOOP_SAMPLES_COARSE`'s doc comment — same reasoning, for posts.
const POST_LOOP_SAMPLES_COARSE: usize = 5;

fn build_post_curve_points(
    base: Vec3,
    top: Vec3,
    height: f64,
    bar_count: u32,
    samples_per_bar: usize,
) -> Vec<Vec3> {
    let forward = (top - base) * (1.0 / height);
    let (_right, up) = perpendicular_frame(forward);
    let bar_span = ((1.0 - BAR_SPAN_START) / bar_count as f64) * height;

    let mut cursor = base + forward * (height * BAR_SPAN_START);
    let mut points = vec![base, cursor];
    for _ in 0..bar_count {
        let loop_points = place_loop(
            cursor,
            forward,
            up,
            1.0,
            bar_span,
            POST_HALF_GAP,
            samples_per_bar,
        );
        points.extend(loop_points.into_iter().skip(1)); // first point === cursor already
        cursor = *points.last().unwrap();
    }
    points.push(top);
    let last = points.len() - 1;
    points[0] = base;
    points[last] = top;
    points
}

/// Number of visible "bars" (real loops, one per pull-through stage) a
/// post has — mirrors `StitchDef`'s own `pre_wraps`/`draw_through`, not
/// an arbitrary progression: `Repeated2` stitches (tr and taller) get
/// `pre_wraps + 1` bars, matching `StitchDef::height()`'s own formula for
/// that variant exactly (one bar per stage); `Single` (dc) clears its 2
/// loops in one draw-through (1 bar, the finishing knot); `AllAtOnce`
/// (htr) clears everything in a single motion but still has two visually
/// distinct marks — the initial yarn-over sitting diagonally across the
/// post, and the finishing knot where all three loops meet — so it gets
/// 2 bars despite being mechanically one stage, a deliberate visual
/// choice, not a construction-stage count. `SlipClear`/no-insertion
/// (ss/ch/mr) get 0 — they never reach `build_post_curve_points` anyway
/// (zero height or `lays_out_as_line`), this exists for completeness.
pub fn bar_count(def: &StitchDef) -> u32 {
    if !def.has_insertion {
        return 0;
    }
    match def.draw_through {
        DrawThrough::SlipClear => 0,
        DrawThrough::Single => 1,
        DrawThrough::AllAtOnce => 2,
        DrawThrough::Repeated2 => def.pre_wraps + 1,
    }
}

/// A per-stitch curve standing in for its real construction, built
/// purely from that stitch's own `base`/`top` anchor points — wherever
/// placement or relaxation actually put them, capacity fan-out,
/// front/back-loop offset, radial ring placement, spring/bending/barrier
/// forces, etc. all fall out of `base`/`top` already, no need to
/// special-case any of them here.
///
/// `base`/`top` coinciding (true of `ss`/`mr` — zero `height()` in the
/// physics model) collapses to a single point — those are true
/// zero-extent point anchors, unlike `ch`/`start_ch`, which have no
/// *post* but do have a real base-to-top span (`lays_out_as_line`).
///
/// Full resolution — the one `path.rs`/`validate.rs` use for the final,
/// judged reconstruction. `relax.rs`'s per-step contact force uses
/// [`build_stitch_curve_points_coarse`] instead (same shape, far fewer
/// samples — see that function's own doc comment).
pub fn build_stitch_curve_points(base: Vec3, top: Vec3, def: &StitchDef) -> Vec<Vec3> {
    build_stitch_curve_points_with_samples(
        base,
        top,
        def,
        None,
        CHAIN_LOOP_SAMPLES,
        POST_LOOP_SAMPLES,
    )
}

/// Same real-loop-topology curve as [`build_stitch_curve_points`], at
/// reduced resolution — see `CHAIN_LOOP_SAMPLES_COARSE`'s doc comment.
/// Every stitch pair's fine segments get checked against every other's
/// each relaxation step; resolution enters that cost quadratically, so
/// this exists specifically to keep `relax_scheme` fast while the shape
/// is still real enough for the contact force to actually mean something
/// (not just a coarse bounding capsule). `stitch_index` only feeds the
/// chain contact-symmetry-breaker (`CHAIN_HALF_GAP`'s doc comment) —
/// ignored for every other stitch kind.
pub fn build_stitch_curve_points_coarse(
    base: Vec3,
    top: Vec3,
    def: &StitchDef,
    stitch_index: usize,
) -> Vec<Vec3> {
    build_stitch_curve_points_with_samples(
        base,
        top,
        def,
        Some(stitch_index),
        CHAIN_LOOP_SAMPLES_COARSE,
        POST_LOOP_SAMPLES_COARSE,
    )
}

fn build_stitch_curve_points_with_samples(
    base: Vec3,
    top: Vec3,
    def: &StitchDef,
    chain_contact_symmetry_break: Option<usize>,
    chain_samples: usize,
    post_samples_per_bar: usize,
) -> Vec<Vec3> {
    let height = top.distance(&base);
    if height < 1e-9 {
        return vec![base];
    }
    if def.lays_out_as_line {
        return build_chain_curve_points(
            base,
            top,
            height,
            chain_contact_symmetry_break,
            chain_samples,
        );
    }
    let bars = bar_count(def);
    if bars == 0 {
        return vec![base, top];
    }
    build_post_curve_points(base, top, height, bars, post_samples_per_bar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stitch::{CapacityStyle, StitchRegistry, CH, DC, DTR, HTR, MR, QUAD_TR, SS, TR};

    fn def(reg: &StitchRegistry, id: crate::stitch::StitchId) -> StitchDef {
        reg.get(id).unwrap().clone()
    }

    #[test]
    fn starts_exactly_at_base_and_ends_exactly_at_top_for_every_postable_kind() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(1.0, 2.0, 3.0);
        let top = Vec3::new(1.5, 2.2, 5.0);
        for id in [DC, HTR, TR, DTR, QUAD_TR] {
            let d = def(&reg, id);
            let points = build_stitch_curve_points(base, top, &d);
            assert_eq!(points[0], base);
            assert_eq!(*points.last().unwrap(), top);
            assert!(points.len() > 2);
        }
    }

    #[test]
    fn returns_just_base_when_base_and_top_coincide() {
        let reg = StitchRegistry::with_uk_basics();
        let p = Vec3::new(0.0, 0.0, 0.0);
        for id in [SS, MR] {
            let d = def(&reg, id);
            assert_eq!(build_stitch_curve_points(p, p, &d), vec![p]);
        }
    }

    #[test]
    fn ch_has_real_span_and_starts_ends_exactly_at_base_top() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.0, 0.0, 0.0);
        let top = Vec3::new(1.0, 0.0, 0.0);
        let d = def(&reg, CH);
        let points = build_stitch_curve_points(base, top, &d);
        assert_eq!(points[0], base);
        assert_eq!(*points.last().unwrap(), top);
        assert!(points.len() > 2);
    }

    fn path_length(points: &[Vec3]) -> f64 {
        points.windows(2).map(|w| w[0].distance(&w[1])).sum()
    }

    #[test]
    fn ch_is_a_real_near_closed_loop_not_a_gentle_bend() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.0, 0.0, 0.0);
        let top = Vec3::new(1.0, 0.0, 0.0);
        let d = def(&reg, CH);
        let points = build_stitch_curve_points(base, top, &d);
        assert!(path_length(&points) > 2.5);
    }

    /// Corrected 2026-09-07 (see `docs/crochet-construction-reference.md`,
    /// tier-3 finding 1): this used to assert that consecutive chain links
    /// alternate which plane they loop in, on the theory that a real
    /// chain's links alternate their whole orientation the way a metal
    /// keychain's do. A domain-expert review found no support for that in
    /// real crochet-construction sources — a tensioned chain is flat and
    /// ribbon-like with a *consistent* orientation down its whole length —
    /// so the fix is a single fixed loop plane for every link, and the
    /// property worth testing is that consecutive links agree, not that
    /// they alternate.
    #[test]
    fn consecutive_ch_links_use_the_same_loop_plane() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.0, 0.0, 0.0);
        let top = Vec3::new(1.0, 0.0, 0.0);
        let d = def(&reg, CH);
        let first = build_stitch_curve_points(base, top, &d);
        let second = build_stitch_curve_points(base, top, &d);
        let mid = first.len() / 2;
        let first_offset = first[mid] - Vec3::new(first[mid].x, 0.0, 0.0);
        let second_offset = second[mid] - Vec3::new(second[mid].x, 0.0, 0.0);
        let first_mag = first_offset.length();
        let second_mag = second_offset.length();
        assert!(first_mag > 0.1);
        assert!(second_mag > 0.1);
        let cos_angle = first_offset.dot(second_offset) / (first_mag * second_mag);
        assert!((cos_angle - 1.0).abs() < 1e-9, "cos_angle = {cos_angle}");
    }

    #[test]
    fn a_post_is_a_real_loop_threaded_onto_a_shaft_not_a_gentle_bend() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.0, 0.0, 0.0);
        let top = Vec3::new(0.0, 0.0, 2.0);
        let d = def(&reg, TR);
        let points = build_stitch_curve_points(base, top, &d);
        // Straight-line distance is 2.0; a real loop (even the tighter,
        // M14-retuned kind — see `POST_HALF_GAP`/`BAR_SPAN_START`'s own
        // doc comment on why these shrank) still has to travel further
        // than that.
        assert!(path_length(&points) > 2.5);
    }

    #[test]
    fn a_post_with_more_pull_through_stages_has_proportionally_more_loop_length() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.0, 0.0, 0.0);
        let dc_len = path_length(&build_stitch_curve_points(
            base,
            Vec3::new(0.0, 0.0, 1.0),
            &def(&reg, DC),
        ));
        let quad_tr_len = path_length(&build_stitch_curve_points(
            base,
            Vec3::new(0.0, 0.0, 5.0),
            &def(&reg, QUAD_TR),
        ));
        assert!(quad_tr_len > dc_len * 2.0);
    }

    #[test]
    fn never_produces_nan_or_infinite_values() {
        let reg = StitchRegistry::with_uk_basics();
        let base = Vec3::new(0.3, -1.2, 0.0);
        let top = Vec3::new(0.9, -0.4, 0.1); // direction nearly parallel to the frame's z reference
        for id in [CH, DC, TR, QUAD_TR] {
            let points = build_stitch_curve_points(base, top, &def(&reg, id));
            for p in &points {
                assert!(p.is_finite());
            }
        }
    }

    #[test]
    fn bar_count_matches_height_formula_for_repeated2_stitches() {
        let reg = StitchRegistry::with_uk_basics();
        for id in [TR, DTR, QUAD_TR] {
            let d = def(&reg, id);
            assert_eq!(bar_count(&d) as f64, d.height());
        }
    }

    #[test]
    fn zero_target_zero_insertion_stitches_get_zero_bars() {
        let reg = StitchRegistry::with_uk_basics();
        assert_eq!(bar_count(&def(&reg, CH)), 0);
        assert_eq!(bar_count(&def(&reg, MR)), 0);
        assert_eq!(bar_count(&def(&reg, SS)), 0);
    }

    #[test]
    fn quad_tr_target_is_still_fixed_capacity_sanity_check() {
        // Not a yarn_shape property, just guards the fixture above against
        // a future default change silently invalidating this file's tests.
        let reg = StitchRegistry::with_uk_basics();
        assert_eq!(def(&reg, QUAD_TR).capacity_style, CapacityStyle::Fixed);
    }
}
