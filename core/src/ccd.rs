//! M10: Continuous Collision Detection (CCD) — edge-edge time-of-contact.
//!
//! `crate::validate`'s self-intersection check is *discrete*: it looks at
//! the relaxed shape's final positions only. That's enough to catch a
//! scheme that ends up self-intersecting, but it can miss a segment that
//! swept *through* another entirely within a single relaxation step and
//! ended up on the far side — never actually overlapping at either
//! endpoint, so nothing at the start or end of the step looks wrong, even
//! though the yarn would have to pass through itself to get there
//! ("tunnelling," the report's term for this failure mode of naive
//! discrete-only collision handling). This module answers a narrower,
//! purely geometric question instead: given two edges' positions at the
//! start and end of a step (linearly interpolated in between, the same
//! assumption `relax.rs`'s Euler integration already makes about motion
//! within a step), did they actually cross at some point during it, and
//! if so, when?
//!
//! **The algorithm** (standard in cloth/rod collision literature — see
//! e.g. Bridson et al., "Robust Treatment of Collisions, Contact, and
//! Friction for Cloth Animation"): four points moving linearly in time
//! are coplanar at time `t` exactly when a cubic polynomial in `t`
//! (the scalar triple product of the three edge vectors between them)
//! is zero. Solving that cubic gives every candidate instant the two
//! edges *could* cross; each candidate is then checked against the
//! *actual* finite segments (not just their infinite extensions), since
//! being coplanar somewhere in space doesn't mean the two segments
//! themselves meet there.
//!
//! **Scope, deliberately** (per GOALS.md's M10 entry): this implements
//! real edge-edge CCD, robust against the near-parallel/near-coplanar
//! cases that make a naive approach unreliable (see
//! [`real_roots_of_cubic`]'s degree-reduction fallbacks and this
//! module's tests), but doesn't reach for the report's full exact-
//! arithmetic machinery (TightCCD/Bernstein Sign Classification, Exact
//! Root Parity) — those exist to give hard *guarantees* even under
//! adversarial floating-point inputs; this gives correct results
//! validated against deliberately-adversarial test cases, which is the
//! bar M10 actually needs to clear. Wiring this into the relaxation loop
//! itself (so a detected crossing actually gets *prevented*, not just
//! detected) is M11's job, not this module's — this is pure geometry,
//! same relationship `rod.rs` has to `relax.rs`.

use crate::vec3::Vec3;

/// Below this, a coefficient is treated as exactly zero for the purpose
/// of degree-reducing a polynomial (cubic -> quadratic -> linear ->
/// constant). Yarn-scale coordinates and per-step displacements are
/// order 0.01-2.0, so a coefficient genuinely at this scale would be
/// numerically meaningless noise, not a real leading term.
const POLY_COEFF_EPS: f64 = 1e-10;

/// Returns every **real** root of `a*x^3 + b*x^2 + c*x + d = 0`, via the
/// standard depressed-cubic + trigonometric/Cardano method, degree-
/// reducing through quadratic/linear/constant when leading coefficients
/// are (numerically) zero rather than dividing by something tiny.
/// Repeated roots are returned once. An identically-zero polynomial
/// (every coefficient below [`POLY_COEFF_EPS`] — every real number is a
/// root) returns an empty `Vec`: there's no finite list of "the" roots,
/// and callers here always want *specific candidate times*, not a
/// statement that every time works — see [`edge_edge_time_of_contact`]'s
/// own always-coplanar fallback for how that degenerate case is actually
/// handled.
pub fn real_roots_of_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    if a.abs() < POLY_COEFF_EPS {
        return real_roots_of_quadratic(b, c, d);
    }

    // Normalize to x^3 + A x^2 + B x + C = 0, then depress via x = u -
    // A/3 to eliminate the quadratic term: u^3 + p u + q = 0.
    let big_a = b / a;
    let big_b = c / a;
    let big_c = d / a;
    let shift = big_a / 3.0;
    let p = big_b - big_a * big_a / 3.0;
    let q = 2.0 * big_a * big_a * big_a / 27.0 - big_a * big_b / 3.0 + big_c;

    if p.abs() < POLY_COEFF_EPS {
        // Depressed cubic is just u^3 + q = 0.
        return vec![(-q).cbrt() - shift];
    }

    let discriminant = (q / 2.0) * (q / 2.0) + (p / 3.0) * (p / 3.0) * (p / 3.0);

    if discriminant > POLY_COEFF_EPS {
        // One real root (the other two are a complex-conjugate pair) —
        // Cardano's formula.
        let sqrt_disc = discriminant.sqrt();
        let u = cbrt_signed(-q / 2.0 + sqrt_disc) + cbrt_signed(-q / 2.0 - sqrt_disc);
        vec![u - shift]
    } else if discriminant < -POLY_COEFF_EPS {
        // Three distinct real roots — trigonometric method (only valid
        // when p < 0, which a negative discriminant here guarantees).
        let r = (-p / 3.0).sqrt();
        let cos_arg = ((3.0 * q) / (2.0 * p) * (-3.0 / p).sqrt()).clamp(-1.0, 1.0);
        let theta = cos_arg.acos();
        (0..3)
            .map(|k| {
                2.0 * r * ((theta - 2.0 * std::f64::consts::PI * k as f64) / 3.0).cos() - shift
            })
            .collect()
    } else {
        // Discriminant ~0: a simple root and a double root, both real
        // and expressible without complex intermediates — the standard
        // degenerate-Cardano result (verified against a known factored
        // cubic in this module's own tests, not just transcribed: for
        // x^3-3x+2=(x-1)^2(x+2), this correctly gives {1, 1, -2}).
        let base = cbrt_signed(-q / 2.0);
        vec![2.0 * base - shift, -base - shift]
    }
}

/// `f64::cbrt` already handles negative inputs correctly (real cube
/// root, not NaN) — this alias exists purely so the call sites above
/// read as "the real cube root of a possibly-negative number," matching
/// how the trigonometric-method derivation is usually written.
fn cbrt_signed(x: f64) -> f64 {
    x.cbrt()
}

/// Real roots of `a*x^2 + b*x + c = 0`, degree-reducing to linear when
/// `a` is numerically zero.
pub fn real_roots_of_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < POLY_COEFF_EPS {
        return real_roots_of_linear(b, c);
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < -POLY_COEFF_EPS {
        vec![]
    } else if discriminant < POLY_COEFF_EPS {
        vec![-b / (2.0 * a)]
    } else {
        let sqrt_d = discriminant.sqrt();
        vec![(-b - sqrt_d) / (2.0 * a), (-b + sqrt_d) / (2.0 * a)]
    }
}

/// Real root of `a*x + b = 0`. Empty when `a` is numerically zero too
/// (either no root, if `b` isn't also ~0, or every real number is a
/// root, if it is — see [`real_roots_of_cubic`]'s doc comment for why
/// that case returns empty rather than a sentinel).
pub fn real_roots_of_linear(a: f64, b: f64) -> Vec<f64> {
    if a.abs() < POLY_COEFF_EPS {
        vec![]
    } else {
        vec![-b / a]
    }
}

/// How close two (near-)coplanar edges' closest approach must be, in the
/// *unitless* barycentric coordinate sense used internally (see
/// [`closest_line_params`]), for a coplanarity root to count as an actual
/// crossing rather than two lines that merely happened to be coplanar
/// somewhere off to the side of both finite segments.
const SEGMENT_MEMBERSHIP_EPS: f64 = 1e-6;

/// Unclamped closest-approach parameters `(s, u)` between the *infinite*
/// lines through `p0 + s*d1` and `q0 + u*d2` — i.e. where the two lines
/// actually cross, if they're coplanar and not parallel (unlike
/// `validate.rs`'s `segment_segment_distance`, which clamps to the
/// finite segments and is answering a different question, "how close do
/// the segments get," not "where would the lines meet"). Returns `None`
/// when the lines are parallel (no unique intersection).
fn closest_line_params(p0: Vec3, d1: Vec3, q0: Vec3, d2: Vec3) -> Option<(f64, f64)> {
    let w0 = p0 - q0;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let b = d1.dot(d2);
    let c1 = d1.dot(w0);
    let c2 = d2.dot(w0);
    let denom = a * e - b * b;
    if denom.abs() < 1e-12 {
        return None;
    }
    let s = (b * c2 - e * c1) / denom;
    let u = (a * c2 - b * c1) / denom;
    Some((s, u))
}

/// When two edges' directions are (near-)parallel at a candidate time
/// (so [`closest_line_params`] can't find a unique crossing point),
/// they can still be genuinely colliding by lying on the *same* line and
/// overlapping along it — e.g. two segments sliding into exact overlap.
/// Checks that `q0` lies on the infinite line through `p0` in direction
/// `d1` (perpendicular distance ~0), then whether `q0`/`q1`'s projected
/// parameters along that line overlap `p`'s own `[0, 1]` range.
fn parallel_segments_overlap(p0: Vec3, d1: Vec3, q0: Vec3, q1: Vec3) -> bool {
    let len_sq = d1.dot(d1);
    if len_sq < 1e-12 {
        return false;
    }
    let to_q0 = q0 - p0;
    let along = d1 * (to_q0.dot(d1) / len_sq);
    let perpendicular = to_q0 - along;
    if perpendicular.length() > 1e-4 {
        return false; // parallel but not actually collinear
    }
    let s0 = to_q0.dot(d1) / len_sq;
    let s1 = (q1 - p0).dot(d1) / len_sq;
    let (lo, hi) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
    hi >= -SEGMENT_MEMBERSHIP_EPS && lo <= 1.0 + SEGMENT_MEMBERSHIP_EPS
}

/// One point's straight-line motion across a step: from `start` (`t=0`)
/// to `end` (`t=1`) — the same per-step-linear assumption `relax.rs`'s
/// Euler integration already makes about how a position gets from one
/// step to the next.
#[derive(Debug, Clone, Copy)]
pub struct PointMotion {
    pub start: Vec3,
    pub end: Vec3,
}

impl PointMotion {
    pub fn new(start: Vec3, end: Vec3) -> Self {
        PointMotion { start, end }
    }
}

/// The earliest time `t` in `[0, 1]` at which edge `p0-p1` and edge
/// `q0-q1` actually cross, given each point's motion across the step.
/// Returns `None` if they never do.
///
/// This detects genuine topological crossings of the two (zero-
/// thickness) segments, not "came within some distance" — yarn's real
/// thickness is a separate concern for M11's barrier contact response to
/// apply on top of this, the same way `validate.rs`'s discrete check
/// already separates "self-intersection" (this kind of question) from
/// the yarn-diameter margin it's checked against.
pub fn edge_edge_time_of_contact(
    p0: PointMotion,
    p1: PointMotion,
    q0: PointMotion,
    q1: PointMotion,
) -> Option<f64> {
    let (p0_start, p0_end) = (p0.start, p0.end);
    let (p1_start, p1_end) = (p1.start, p1.end);
    let (q0_start, q0_end) = (q0.start, q0.end);
    let (q1_start, q1_end) = (q1.start, q1.end);

    // e1(t) = p1(t)-p0(t), e2(t) = q0(t)-p0(t), e3(t) = q1(t)-p0(t), each
    // linear in t: e_i(t) = e_i0 + t*de_i. Coplanarity is
    // e1(t) . (e2(t) x e3(t)) = 0, a cubic in t once expanded (see the
    // module doc comment for the derivation this mirrors).
    let e1_0 = p1_start - p0_start;
    let de1 = (p1_end - p0_end) - e1_0;
    let e2_0 = q0_start - p0_start;
    let de2 = (q0_end - p0_end) - e2_0;
    let e3_0 = q1_start - p0_start;
    let de3 = (q1_end - p0_end) - e3_0;

    let d = e1_0.dot(e2_0.cross(e3_0));
    let c = e1_0.dot(e2_0.cross(de3) + de2.cross(e3_0)) + de1.dot(e2_0.cross(e3_0));
    let b = e1_0.dot(de2.cross(de3)) + de1.dot(e2_0.cross(de3) + de2.cross(e3_0));
    let a = de1.dot(de2.cross(de3));

    let mut candidates: Vec<f64> = if a.abs() < POLY_COEFF_EPS
        && b.abs() < POLY_COEFF_EPS
        && c.abs() < POLY_COEFF_EPS
        && d.abs() < POLY_COEFF_EPS
    {
        // The two edges are coplanar for the *entire* step (every
        // coefficient of the cubic vanishes) — a genuinely degenerate
        // configuration (in real relaxation dynamics this needs
        // suspiciously exact parallel motion to occur at all) that the
        // cubic-root approach has no isolated roots to offer for, since
        // literally every t satisfies the coplanarity condition. Rather
        // than the full treatment a persistently-coplanar pair would
        // need in general, sample a small, fixed set of instants
        // (endpoints plus midpoint) as a pragmatic fallback — documented
        // as such, not claimed to be exhaustive.
        vec![0.0, 0.5, 1.0]
    } else {
        real_roots_of_cubic(a, b, c, d)
            .into_iter()
            .filter(|t| (-1e-9..=1.0 + 1e-9).contains(t))
            .map(|t| t.clamp(0.0, 1.0))
            .collect()
    };
    candidates.sort_by(|x, y| x.partial_cmp(y).unwrap());

    for t in candidates {
        let p0 = p0_start + (p0_end - p0_start) * t;
        let p1 = p1_start + (p1_end - p1_start) * t;
        let q0 = q0_start + (q0_end - q0_start) * t;
        let q1 = q1_start + (q1_end - q1_start) * t;

        let d1 = p1 - p0;
        let d2 = q1 - q0;
        let Some((s, u)) = closest_line_params(p0, d1, q0, d2) else {
            if parallel_segments_overlap(p0, d1, q0, q1) {
                return Some(t);
            }
            continue;
        };
        if !(-SEGMENT_MEMBERSHIP_EPS..=1.0 + SEGMENT_MEMBERSHIP_EPS).contains(&s)
            || !(-SEGMENT_MEMBERSHIP_EPS..=1.0 + SEGMENT_MEMBERSHIP_EPS).contains(&u)
        {
            continue;
        }
        // Confirm actual 3D coincidence, not just an in-range crossing
        // of lines that only *approximately* satisfied coplanarity at
        // this candidate t (floating-point root-finding is inexact).
        let closest_p = p0 + d1 * s.clamp(0.0, 1.0);
        let closest_q = q0 + d2 * u.clamp(0.0, 1.0);
        if closest_p.distance(&closest_q) < 1e-4 {
            return Some(t);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() < tol,
            "expected {a} to approximately equal {b}"
        );
    }

    mod cubic_solver_tests {
        use super::*;

        #[test]
        fn three_distinct_real_roots() {
            // (x-1)(x-2)(x-3) = x^3 - 6x^2 + 11x - 6
            let mut roots = real_roots_of_cubic(1.0, -6.0, 11.0, -6.0);
            roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(roots.len(), 3);
            approx_eq(roots[0], 1.0, 1e-9);
            approx_eq(roots[1], 2.0, 1e-9);
            approx_eq(roots[2], 3.0, 1e-9);
        }

        #[test]
        fn one_real_root_two_complex() {
            // (x-2)(x^2+1) = x^3 - 2x^2 + x - 2, real root only at x=2.
            let roots = real_roots_of_cubic(1.0, -2.0, 1.0, -2.0);
            assert_eq!(roots.len(), 1);
            approx_eq(roots[0], 2.0, 1e-9);
        }

        #[test]
        fn repeated_root() {
            // (x-1)^2 (x+2) = x^3 - 3x + 2, roots 1 (double), -2.
            let mut roots = real_roots_of_cubic(1.0, 0.0, -3.0, 2.0);
            roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert!(roots.iter().any(|r| (r - 1.0).abs() < 1e-6));
            assert!(roots.iter().any(|r| (r - (-2.0)).abs() < 1e-6));
        }

        #[test]
        fn degenerates_to_quadratic_when_leading_coefficient_is_zero() {
            // 0*x^3 + 1*x^2 - 3x + 2 = (x-1)(x-2)
            let mut roots = real_roots_of_cubic(0.0, 1.0, -3.0, 2.0);
            roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(roots.len(), 2);
            approx_eq(roots[0], 1.0, 1e-9);
            approx_eq(roots[1], 2.0, 1e-9);
        }

        #[test]
        fn degenerates_to_linear_when_leading_two_coefficients_are_zero() {
            // 2x - 4 = 0 -> x = 2
            let roots = real_roots_of_cubic(0.0, 0.0, 2.0, -4.0);
            assert_eq!(roots.len(), 1);
            approx_eq(roots[0], 2.0, 1e-9);
        }

        #[test]
        fn identically_zero_polynomial_returns_no_roots() {
            assert!(real_roots_of_cubic(0.0, 0.0, 0.0, 0.0).is_empty());
        }

        #[test]
        fn never_produces_nan_or_infinite_roots() {
            // A grid of deliberately awkward coefficients, including
            // near-zero leading terms that must degree-reduce cleanly.
            for a in [0.0, 1e-12, 1e-9, 0.5, -3.0] {
                for b in [0.0, 1e-9, 2.0, -5.0] {
                    for c in [0.0, 1.0, -1.0] {
                        for d in [0.0, 4.0, -4.0] {
                            for r in real_roots_of_cubic(a, b, c, d) {
                                assert!(
                                    r.is_finite(),
                                    "non-finite root for ({a},{b},{c},{d}): {r}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    mod edge_edge_tests {
        use super::*;

        fn motion(start: Vec3, end: Vec3) -> PointMotion {
            PointMotion::new(start, end)
        }

        fn still(p: Vec3) -> PointMotion {
            PointMotion::new(p, p)
        }

        #[test]
        fn detects_a_clean_crossing() {
            // p slides through z=0 (crossing q's plane) while q stays
            // put — a genuine 3D crossing (not always-coplanar, so this
            // exercises the actual cubic-root path, not the persistent-
            // coplanarity fallback) at the midpoint, x=y=z=0.
            let p0 = motion(Vec3::new(-1.0, 0.0, 1.0), Vec3::new(-1.0, 0.0, -1.0));
            let p1 = motion(Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 0.0, -1.0));
            let q0 = still(Vec3::new(0.0, -1.0, 0.0));
            let q1 = still(Vec3::new(0.0, 1.0, 0.0));

            let t = edge_edge_time_of_contact(p0, p1, q0, q1);
            assert!(t.is_some(), "expected a detected crossing");
            approx_eq(t.unwrap(), 0.5, 1e-6);
        }

        #[test]
        fn no_collision_when_segments_never_meet() {
            // Two segments sliding in parallel planes, never coming
            // close.
            let p0 = motion(Vec3::new(-1.0, 5.0, 0.0), Vec3::new(-1.0, 6.0, 0.0));
            let p1 = motion(Vec3::new(1.0, 5.0, 0.0), Vec3::new(1.0, 6.0, 0.0));
            let q0 = still(Vec3::new(0.0, -1.0, 0.0));
            let q1 = still(Vec3::new(0.0, 1.0, 0.0));

            assert!(edge_edge_time_of_contact(p0, p1, q0, q1).is_none());
        }

        #[test]
        fn coplanar_but_crossing_point_outside_the_finite_segments_is_not_a_collision() {
            // The two *lines* would cross, but only outside where either
            // finite segment actually reaches.
            let p0 = motion(Vec3::new(-5.0, 1.0, 0.0), Vec3::new(-5.0, -1.0, 0.0));
            let p1 = motion(Vec3::new(-3.0, 1.0, 0.0), Vec3::new(-3.0, -1.0, 0.0));
            let q0 = still(Vec3::new(0.0, -1.0, 0.0));
            let q1 = still(Vec3::new(0.0, 1.0, 0.0));

            assert!(edge_edge_time_of_contact(p0, p1, q0, q1).is_none());
        }

        #[test]
        fn near_parallel_edges_that_still_cross_are_detected() {
            // q is fixed along the x-axis; p is tilted only a few
            // degrees away from parallel to it, and slides through q's
            // z=0 plane — at the crossing instant the two edges meet at
            // a shallow angle instead of a clean right angle, the case a
            // naive/ill-conditioned root-finder (and closest_line_params'
            // near-singular denominator) is most likely to lose.
            let p0 = motion(Vec3::new(-2.0, -0.1, 1.0), Vec3::new(-2.0, -0.1, -1.0));
            let p1 = motion(Vec3::new(2.0, 0.1, 1.0), Vec3::new(2.0, 0.1, -1.0));
            let q0 = still(Vec3::new(-2.0, 0.0, 0.0));
            let q1 = still(Vec3::new(2.0, 0.0, 0.0));

            let t = edge_edge_time_of_contact(p0, p1, q0, q1);
            assert!(
                t.is_some(),
                "expected the shallow near-parallel crossing to be detected"
            );
            approx_eq(t.unwrap(), 0.5, 1e-3);
        }

        #[test]
        fn already_intersecting_at_the_very_start_is_detected_at_t_zero() {
            let p0 = motion(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(-1.0, 2.0, 0.0));
            let p1 = motion(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 0.0));
            let q0 = motion(Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 3.0, 0.0));
            let q1 = motion(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 5.0, 0.0));

            let t = edge_edge_time_of_contact(p0, p1, q0, q1);
            assert!(t.is_some());
            approx_eq(t.unwrap(), 0.0, 1e-6);
        }

        #[test]
        fn stationary_non_intersecting_edges_report_no_collision() {
            let p0 = still(Vec3::new(-1.0, 5.0, 0.0));
            let p1 = still(Vec3::new(1.0, 5.0, 0.0));
            let q0 = still(Vec3::new(0.0, -1.0, 0.0));
            let q1 = still(Vec3::new(0.0, 1.0, 0.0));

            assert!(edge_edge_time_of_contact(p0, p1, q0, q1).is_none());
        }

        #[test]
        fn persistently_coplanar_edges_that_never_cross_do_not_false_positive() {
            // Both edges live in the z=0 plane throughout (coplanar the
            // whole step — every cubic coefficient vanishes) but stay
            // clearly separated in y the entire time.
            let p0 = motion(Vec3::new(-1.0, 5.0, 0.0), Vec3::new(-1.0, 4.0, 0.0));
            let p1 = motion(Vec3::new(1.0, 5.0, 0.0), Vec3::new(1.0, 4.0, 0.0));
            let q0 = motion(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(-1.0, 0.5, 0.0));
            let q1 = motion(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.5, 0.0));

            assert!(edge_edge_time_of_contact(p0, p1, q0, q1).is_none());
        }

        #[test]
        fn persistently_coplanar_edges_that_do_cross_are_detected() {
            // Same z=0-plane setup, but this time p actually sweeps
            // across q.
            let p0 = motion(Vec3::new(-1.0, 1.0, 0.0), Vec3::new(-1.0, -1.0, 0.0));
            let p1 = motion(Vec3::new(1.0, 1.0, 0.0), Vec3::new(1.0, -1.0, 0.0));
            let q0 = still(Vec3::new(-1.0, 0.0, 0.0));
            let q1 = still(Vec3::new(1.0, 0.0, 0.0));

            assert!(edge_edge_time_of_contact(p0, p1, q0, q1).is_some());
        }

        #[test]
        fn shared_endpoint_edges_are_reported_touching_at_the_shared_vertex() {
            // Two edges sharing a vertex (the ordinary "adjacent
            // stitches" case in this project's own thread model) are
            // genuinely, correctly touching *at that vertex* for the
            // entire step — this primitive reports exactly that, the
            // same way `validate.rs`'s raw-distance check would find
            // zero distance there. Deciding that shared-vertex contact is
            // *expected* and shouldn't be treated as a defect is a
            // caller-level policy question (`validate.rs`'s own
            // `segments_are_adjacent` already draws exactly this line for
            // the discrete check) — not something a general-purpose
            // "do these two segments touch" primitive should silently
            // bake in.
            let shared = motion(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.0));
            let p1 = motion(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.2, 0.3, 0.0));
            let q1 = motion(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.2, 1.2, 0.0));

            let t = edge_edge_time_of_contact(shared, p1, shared, q1);
            assert!(t.is_some());
            approx_eq(t.unwrap(), 0.0, 1e-6);
        }
    }

    /// Runs the primitive against a *real* scheme's actual raw-vs-relaxed
    /// motion, not just hand-crafted synthetic edge pairs — the
    /// synthetic tests above prove the math is right in isolation; these
    /// prove it doesn't fall over on the kind of genuinely messy,
    /// adjacent/coincident/coplanar geometry an ordinary scheme actually
    /// produces (shared vertices everywhere, zero-length bridge segments,
    /// large single-step displacements). `PathSegment` already carries
    /// both its raw (M1) and relaxed (M2/M9) endpoints, so a segment's
    /// own raw-to-relaxed motion is exactly the kind of `_start`/`_end`
    /// pair this module expects — treating the *entire* relaxation as
    /// one large conceptual step is a stress test, not how M11 will
    /// eventually call this per-step, but it's a strong adversarial
    /// input precisely because the motion is large.
    mod scheme_integration_tests {
        use super::*;
        use crate::graph::{Scheme, StitchInstance, StitchRef, Thread};
        use crate::path::relaxed_yarn_segments;
        use crate::relax::{relax_scheme, RelaxationParams};
        use crate::stitch::{StitchRegistry, CH, SS};

        #[test]
        fn never_panics_or_produces_nan_across_every_segment_pair_of_a_real_scheme() {
            // The ring-closure scheme (M9's own motivating case): six
            // chains closed with a slip stitch, known to move segments
            // dramatically from raw to relaxed — about as adversarial a
            // real motion as this project's own schemes produce.
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

            let relaxed = relax_scheme(&scheme, &registry, &RelaxationParams::default()).unwrap();
            let segments = relaxed_yarn_segments(&scheme, &registry, &relaxed).unwrap();

            let mut checked_pairs = 0;
            for i in 0..segments.len() {
                for j in (i + 1)..segments.len() {
                    let a = &segments[i];
                    let b = &segments[j];
                    let t = edge_edge_time_of_contact(
                        PointMotion::new(a.raw_start, a.start),
                        PointMotion::new(a.raw_end, a.end),
                        PointMotion::new(b.raw_start, b.start),
                        PointMotion::new(b.raw_end, b.end),
                    );
                    if let Some(t) = t {
                        assert!(
                            t.is_finite(),
                            "non-finite time-of-contact for pair ({i}, {j})"
                        );
                        assert!(
                            (0.0..=1.0).contains(&t),
                            "time-of-contact out of range for pair ({i}, {j}): {t}"
                        );
                    }
                    checked_pairs += 1;
                }
            }
            assert!(
                checked_pairs > 0,
                "expected at least one segment pair to actually check"
            );
        }
    }
}
