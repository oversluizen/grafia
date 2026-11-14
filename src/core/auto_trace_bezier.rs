//! Schneider least-squares cubic Bézier curve fitting and Newton-Raphson reparameterization
//! for `SPEC-009 §2.5` Bitmap & Camera Auto-Trace (`< 399` eSLOC).

use crate::core::math_consts::{
    BERNSTEIN_CUBIC_COEFF, RAD_TO_DEG_F64, SOLVER_DET_EPSILON, TRACE_BEZIER_FIT_MAX_DEPTH,
    TRACE_CHORD_EPS, TRACE_HANDLE_MIN_FRACTION, TRACE_HANDLE_ONE_THIRD, TRACE_NEWTON_ITERS,
    TRACE_STRAIGHT_ANGLE_DEG,
};
use kurbo::{CubicBez, ParamCurve, ParamCurveDeriv, Point};

#[must_use]
pub(crate) fn included_angle_deg(prev: Point, cur: Point, next: Point) -> f64 {
    let ux = prev.x - cur.x;
    let uy = prev.y - cur.y;
    let vx = next.x - cur.x;
    let vy = next.y - cur.y;
    let lu = ux.hypot(uy);
    let lv = vx.hypot(vy);
    if lu <= TRACE_CHORD_EPS || lv <= TRACE_CHORD_EPS {
        return TRACE_STRAIGHT_ANGLE_DEG;
    }
    let cos_a = ((ux * vx + uy * vy) / (lu * lv)).clamp(-1.0, 1.0);
    cos_a.acos() * RAD_TO_DEG_F64
}

#[must_use]
pub(crate) fn unit_vec(dx: f64, dy: f64) -> (f64, f64) {
    let len = dx.hypot(dy);
    if len <= TRACE_CHORD_EPS {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

#[must_use]
pub(crate) fn is_collinear_chain(pts: &[Point], tol: f64) -> bool {
    if pts.len() <= 2 {
        return true;
    }
    let p0 = pts[0];
    let p1 = pts[pts.len() - 1];
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let len = dx.hypot(dy);
    if len <= TRACE_CHORD_EPS {
        return true;
    }
    pts.iter()
        .all(|p| ((p.x - p0.x) * dy - (p.y - p0.y) * dx).abs() / len <= tol)
}

#[must_use]
pub(crate) fn solve_schneider_cubic_lsq(
    endpoints: (Point, Point),
    pts: &[Point],
    u: &[f64],
    tan_l: (f64, f64),
    tan_r: (f64, f64),
    min_alpha: f64,
    fallback_alphas: (f64, f64),
) -> (f64, f64, CubicBez) {
    let (p0, p3) = endpoints;
    debug_assert_eq!(pts.len(), u.len());
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c11 = 0.0;
    let mut x0 = 0.0;
    let mut x1 = 0.0;
    for (pt, &t) in pts.iter().zip(u.iter()) {
        let mt = 1.0 - t;
        let b0 = mt * mt * mt;
        let b1 = BERNSTEIN_CUBIC_COEFF * t * mt * mt;
        let b2 = BERNSTEIN_CUBIC_COEFF * t * t * mt;
        let b3 = t * t * t;
        let a1x = tan_l.0 * b1;
        let a1y = tan_l.1 * b1;
        let a2x = tan_r.0 * b2;
        let a2y = tan_r.1 * b2;
        c00 += a1x * a1x + a1y * a1y;
        c01 += a1x * a2x + a1y * a2y;
        c11 += a2x * a2x + a2y * a2y;
        let rx = pt.x - (p0.x * (b0 + b1) + p3.x * (b2 + b3));
        let ry = pt.y - (p0.y * (b0 + b1) + p3.y * (b2 + b3));
        x0 += a1x * rx + a1y * ry;
        x1 += a2x * rx + a2y * ry;
    }
    let det = c00 * c11 - c01 * c01;
    let (alpha_l, alpha_r) = if det.abs() > SOLVER_DET_EPSILON {
        let al = (x0 * c11 - x1 * c01) / det;
        let ar = (c00 * x1 - c01 * x0) / det;
        if al >= min_alpha && ar >= min_alpha {
            (al, ar)
        } else {
            fallback_alphas
        }
    } else {
        fallback_alphas
    };
    let bez = CubicBez::new(
        p0,
        Point::new(p0.x + tan_l.0 * alpha_l, p0.y + tan_l.1 * alpha_l),
        Point::new(p3.x + tan_r.0 * alpha_r, p3.y + tan_r.1 * alpha_r),
        p3,
    );
    (alpha_l, alpha_r, bez)
}

fn chord_length_params(pts: &[Point]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    u.push(0.0);
    let mut total = 0.0;
    for w in pts.windows(2) {
        total += (w[1].x - w[0].x).hypot(w[1].y - w[0].y);
        u.push(total);
    }
    if total > TRACE_CHORD_EPS {
        for slot in &mut u {
            *slot /= total;
        }
    }
    if let Some(last) = u.last_mut() {
        *last = 1.0;
    }
    u
}

pub(crate) fn reparameterize_newton(
    bez: CubicBez,
    pts: &[Point],
    u: &mut [f64],
    skip_ends: bool,
    clamp_bounds: (f64, f64),
) {
    let d1 = bez.deriv();
    let d2 = d1.deriv();
    let start_idx = usize::from(skip_ends);
    for (pt, param) in pts.iter().zip(u.iter_mut()).skip(start_idx) {
        let t = *param;
        if skip_ends && t >= 1.0 {
            continue;
        }
        let q = bez.eval(t);
        let q1 = d1.eval(t);
        let q2 = d2.eval(t);
        let dx = q.x - pt.x;
        let dy = q.y - pt.y;
        let num = dx * q1.x + dy * q1.y;
        let den = q1.x * q1.x + q1.y * q1.y + dx * q2.x + dy * q2.y;
        if den.abs() > SOLVER_DET_EPSILON {
            *param = (t - num / den).clamp(clamp_bounds.0, clamp_bounds.1);
        }
    }
}

fn max_fit_error_sq(bez: CubicBez, pts: &[Point], u: &[f64]) -> (f64, usize) {
    let mut worst = 0.0;
    let mut idx = pts.len() / 2;
    for (i, (pt, &t)) in pts.iter().zip(u.iter()).enumerate() {
        let q = bez.eval(t);
        let d2 = (q.x - pt.x) * (q.x - pt.x) + (q.y - pt.y) * (q.y - pt.y);
        if d2 > worst {
            worst = d2;
            idx = i;
        }
    }
    (worst, idx)
}

pub(crate) fn fit_cubic_chain(
    pts: &[Point],
    tan_l: (f64, f64),
    tan_r: (f64, f64),
    max_err_sq: f64,
    depth: u8,
    out: &mut Vec<CubicBez>,
) {
    debug_assert!(!pts.is_empty());
    let p0 = pts[0];
    let p3 = pts[pts.len() - 1];
    if pts.len() <= 2 {
        let h1 = Point::new(
            p0.x + (p3.x - p0.x) * TRACE_HANDLE_ONE_THIRD,
            p0.y + (p3.y - p0.y) * TRACE_HANDLE_ONE_THIRD,
        );
        let h2 = Point::new(
            p3.x - (p3.x - p0.x) * TRACE_HANDLE_ONE_THIRD,
            p3.y - (p3.y - p0.y) * TRACE_HANDLE_ONE_THIRD,
        );
        out.push(CubicBez::new(p0, h1, h2, p3));
        return;
    }
    let chord = (p3.x - p0.x).hypot(p3.y - p0.y);
    let fallback = (
        chord * TRACE_HANDLE_ONE_THIRD,
        chord * TRACE_HANDLE_ONE_THIRD,
    );
    let min_alpha = chord * TRACE_HANDLE_MIN_FRACTION;
    let mut u = chord_length_params(pts);
    let (_, _, mut bez) =
        solve_schneider_cubic_lsq((p0, p3), pts, &u, tan_l, tan_r, min_alpha, fallback);
    for _ in 0..TRACE_NEWTON_ITERS {
        let (err_sq, _) = max_fit_error_sq(bez, pts, &u);
        if err_sq <= max_err_sq {
            out.push(bez);
            return;
        }
        reparameterize_newton(bez, pts, &mut u, true, (0.0, 1.0));
        (_, _, bez) =
            solve_schneider_cubic_lsq((p0, p3), pts, &u, tan_l, tan_r, min_alpha, fallback);
    }
    let (err_sq, split_idx) = max_fit_error_sq(bez, pts, &u);
    if err_sq <= max_err_sq || depth >= TRACE_BEZIER_FIT_MAX_DEPTH || pts.len() <= 3 {
        out.push(bez);
        return;
    }
    let mid = split_idx.clamp(1, pts.len() - 2);
    let dir = unit_vec(
        pts[mid + 1].x - pts[mid - 1].x,
        pts[mid + 1].y - pts[mid - 1].y,
    );
    fit_cubic_chain(
        &pts[..=mid],
        tan_l,
        (-dir.0, -dir.1),
        max_err_sq,
        depth + 1,
        out,
    );
    fit_cubic_chain(&pts[mid..], dir, tan_r, max_err_sq, depth + 1, out);
}
