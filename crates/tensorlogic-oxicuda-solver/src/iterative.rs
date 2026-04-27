//! Iterative solvers: Conjugate Gradient (CG) for symmetric positive-definite systems.
//!
//! The CPU implementation follows the textbook Hestenes–Stiefel algorithm.
//! The GPU path is a stub that will be connected to the `oxicuda-solver`
//! sparse-PCG kernel in Round 6.

use crate::error::SolverError;

// ---------------------------------------------------------------------------
// Internal helpers (inline, no allocation)
// ---------------------------------------------------------------------------

/// Dot product ⟨u, v⟩.
#[inline]
fn dot(u: &[f32], v: &[f32]) -> f32 {
    u.iter().zip(v.iter()).map(|(&a, &b)| a * b).sum()
}

/// Accumulate `dest += scale * src` in-place (AXPY).
#[inline]
fn axpy(dest: &mut [f32], scale: f32, src: &[f32]) {
    dest.iter_mut()
        .zip(src.iter())
        .for_each(|(d, &s)| *d += scale * s);
}

/// Dense matrix–vector multiply y = A · x  (A is n×n row-major, x and y are length n).
#[inline]
fn mat_vec_n(a: &[f32], n: usize, x: &[f32], y: &mut [f32]) {
    for i in 0..n {
        let row = &a[i * n..(i + 1) * n];
        y[i] = dot(row, x);
    }
}

// ---------------------------------------------------------------------------
// Public: CPU conjugate gradient
// ---------------------------------------------------------------------------

/// Solve the n×n symmetric positive-definite system `A · x = b` using the
/// Conjugate Gradient (CG) method.
///
/// # Parameters
/// - `a`        – Row-major n×n coefficient matrix (must be SPD).
/// - `n`        – System dimension.
/// - `b`        – Right-hand side vector (length n).
/// - `max_iter` – Maximum number of CG iterations; typical choice is n (or 2n for safety).
/// - `tol`      – Convergence tolerance on the Euclidean residual norm ‖r‖₂.
///
/// # Errors
/// - [`SolverError::DimMismatch`] if the array lengths are inconsistent.
/// - [`SolverError::DidNotConverge`] if `max_iter` iterations complete without
///   achieving the target residual.
///
/// # Note
/// CG assumes A is SPD. Supplying a non-SPD matrix may cause numerical
/// instability or a division-by-zero (the denominator p·Ap may underflow).
/// Use [`crate::solve_lu`] or [`crate::solve_qr_lstsq`] for general matrices.
pub fn cg_solve_cpu(
    a: &[f32],
    n: usize,
    b: &[f32],
    max_iter: usize,
    tol: f32,
) -> Result<Vec<f32>, SolverError> {
    // --- dimension checks ---
    if a.len() != n * n {
        return Err(SolverError::DimMismatch(format!(
            "A has {} elements but n={n} implies {n}×{n}={}; check matrix dimensions",
            a.len(),
            n * n
        )));
    }
    if b.len() != n {
        return Err(SolverError::DimMismatch(format!(
            "b has length {} but n={n}",
            b.len()
        )));
    }
    if n == 0 {
        return Ok(vec![]);
    }

    let tol_sq = (tol * tol) as f64; // compare squared norms to avoid sqrt

    // --- initialise: x = 0, r = b, p = r ---
    let mut x = vec![0.0f32; n];
    let mut r = b.to_vec(); // r = b - A*0 = b
    let mut p = r.clone();

    let mut r_dot_r = dot(&r, &r) as f64;

    if r_dot_r.sqrt() < tol as f64 {
        // b is already the zero vector; x = 0 is the solution.
        return Ok(x);
    }

    let mut ap = vec![0.0f32; n];

    for _iter in 0..max_iter {
        // ap = A · p
        mat_vec_n(a, n, &p, &mut ap);

        // alpha = r·r / (p · Ap)
        let p_dot_ap = dot(&p, &ap) as f64;
        if p_dot_ap.abs() < f64::EPSILON * 1e6 {
            // Denominator collapsed — matrix is near-singular or not SPD.
            return Err(SolverError::Singular);
        }
        let alpha = (r_dot_r / p_dot_ap) as f32;

        // x = x + alpha * p
        axpy(&mut x, alpha, &p);

        // r_new = r - alpha * Ap
        axpy(&mut r, -alpha, &ap);

        let r_dot_r_new = dot(&r, &r) as f64;

        if r_dot_r_new < tol_sq {
            return Ok(x);
        }

        // beta = r_new·r_new / r·r
        let beta = (r_dot_r_new / r_dot_r) as f32;

        // p = r_new + beta * p
        // (update in-place: p = r + beta*p, but r already IS r_new after the axpy above)
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }

        r_dot_r = r_dot_r_new;
    }

    // Did not converge — report the final residual norm.
    let residual = (r_dot_r as f32).sqrt();
    Err(SolverError::DidNotConverge { max_iter, residual })
}

// ---------------------------------------------------------------------------
// GPU stub
// ---------------------------------------------------------------------------

/// GPU conjugate gradient solve.  Stub — wired in Round 6.
#[cfg(feature = "gpu")]
pub(crate) fn cg_solve_gpu(
    _a: &[f32],
    _n: usize,
    _b: &[f32],
    _max_iter: usize,
    _tol: f32,
) -> Result<Vec<f32>, SolverError> {
    Err(SolverError::GpuError(
        "GPU CG solver requires CUDA runtime (wired in Round 6)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Internal tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn max_abs_err(u: &[f32], v: &[f32]) -> f32 {
        u.iter()
            .zip(v.iter())
            .map(|(&a, &b)| (a - b).abs())
            .fold(0.0f32, f32::max)
    }

    #[test]
    fn cg_identity_3x3() {
        let a = vec![1f32, 0., 0., 0., 1., 0., 0., 0., 1.];
        let b = vec![3f32, -1., 2.];
        let x = cg_solve_cpu(&a, 3, &b, 50, 1e-6).unwrap();
        assert!(max_abs_err(&x, &b) < 1e-4, "x={x:?}");
    }

    #[test]
    fn cg_2x2_spd() {
        // A = [[4,2],[2,3]], b=[6,5], x=[1,1]
        let a = vec![4f32, 2., 2., 3.];
        let b = vec![6f32, 5.];
        let x = cg_solve_cpu(&a, 2, &b, 100, 1e-6).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-4, "x[0]={}", x[0]);
        assert!((x[1] - 1.0).abs() < 1e-4, "x[1]={}", x[1]);
    }

    #[test]
    fn cg_diagonal_4x4() {
        // Diagonal SPD: diag(1,2,3,4), b=[1,2,3,4] → x=[1,1,1,1]
        #[rustfmt::skip]
        let a = vec![
            1f32, 0., 0., 0.,
            0., 2., 0., 0.,
            0., 0., 3., 0.,
            0., 0., 0., 4.,
        ];
        let b = vec![1f32, 2., 3., 4.];
        let x = cg_solve_cpu(&a, 4, &b, 20, 1e-6).unwrap();
        let expected = vec![1f32, 1., 1., 1.];
        assert!(max_abs_err(&x, &expected) < 1e-4, "x={x:?}");
    }

    #[test]
    fn cg_max_iter_exceeded_returns_error() {
        // Use a larger SPD system and restrict max_iter to 0 to force non-convergence.
        let a = vec![4f32, 2., 2., 3.];
        let b = vec![6f32, 5.];
        let result = cg_solve_cpu(&a, 2, &b, 0, 1e-6);
        assert!(
            matches!(result, Err(SolverError::DidNotConverge { .. })),
            "expected DidNotConverge, got {result:?}"
        );
    }

    #[test]
    fn cg_empty_system() {
        let result = cg_solve_cpu(&[], 0, &[], 10, 1e-6).unwrap();
        assert!(result.is_empty());
    }
}
