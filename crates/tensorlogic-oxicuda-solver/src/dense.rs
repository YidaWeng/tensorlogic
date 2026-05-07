//! Dense linear solvers: LU (with partial pivoting), Cholesky, and QR least-squares.
//!
//! All routines operate on row-major `f32` slices.  No external linear-algebra
//! crates are required: the CPU paths implement Doolittle LU, Cholesky-Banachiewicz,
//! and modified Gram-Schmidt QR in plain Rust, which keeps the dependency surface
//! minimal and avoids the ndarray import that the SciRS2 policy forbids here.
//!
//! # GPU dispatch
//!
//! When compiled with `--features gpu` and `gpu_available()` returns `true`, each
//! public entry point calls the corresponding OxiCUDA GPU kernel.  The GPU paths
//! are scaffolded in Round 5; the full wiring is completed in Round 6.

use crate::error::SolverError;

// ---------------------------------------------------------------------------
// Small helpers (dot product, matrix–vector multiply)
// ---------------------------------------------------------------------------

/// Dot product of two equal-length slices.
#[inline]
fn dot(u: &[f32], v: &[f32]) -> f32 {
    u.iter().zip(v.iter()).map(|(&a, &b)| a * b).sum()
}

/// Row-major matrix–vector multiply: y = A * x, where A is m×n.
/// Only used in tests for residual verification.
#[cfg(test)]
#[inline]
fn mat_vec(a: &[f32], m: usize, n: usize, x: &[f32], y: &mut [f32]) {
    for (i, y_i) in y.iter_mut().enumerate().take(m) {
        let row_start = i * n;
        *y_i = dot(&a[row_start..row_start + n], &x[..n]);
    }
}

// ---------------------------------------------------------------------------
// CPU LU solve (Doolittle decomposition with partial pivoting)
// ---------------------------------------------------------------------------

/// Solve the n×n square system `A · x = b` using LU factorisation with partial
/// (row-wise) pivoting.
///
/// # Algorithm
/// 1. Copy A into a working n×n buffer.
/// 2. Doolittle factorisation with partial pivoting: at each step k the row
///    with the largest absolute value in column k (at or below the diagonal)
///    is swapped to position k, then the sub-column is scaled.
/// 3. Forward substitution (L · y = Pb).
/// 4. Back substitution (U · x = y).
///
/// # Errors
/// Returns [`SolverError::NotSquare`] if `a.len() != n*n` or `b.len() != n`,
/// and [`SolverError::Singular`] if a pivot is below machine epsilon.
pub(crate) fn solve_lu_cpu(a: &[f32], n: usize, b: &[f32]) -> Result<Vec<f32>, SolverError> {
    // --- dimension checks ---
    if a.len() != n * n {
        let rows = a.len() / n.max(1);
        return Err(SolverError::NotSquare { rows, cols: n });
    }
    if b.len() != n {
        return Err(SolverError::DimMismatch(format!(
            "rhs has length {} but matrix is {n}x{n}",
            b.len()
        )));
    }
    if n == 0 {
        return Ok(vec![]);
    }

    // --- working copies ---
    let mut lu: Vec<f32> = a.to_vec(); // row-major LU in-place
    let mut perm: Vec<usize> = (0..n).collect(); // pivot permutation

    // --- Doolittle with partial pivoting ---
    for k in 0..n {
        // find pivot row
        let pivot_row = (k..n)
            .max_by(|&r1, &r2| {
                lu[r1 * n + k]
                    .abs()
                    .partial_cmp(&lu[r2 * n + k].abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(k);

        if lu[pivot_row * n + k].abs() < f32::EPSILON * 64.0 {
            return Err(SolverError::Singular);
        }

        if pivot_row != k {
            // swap rows k and pivot_row in lu
            for j in 0..n {
                lu.swap(k * n + j, pivot_row * n + j);
            }
            perm.swap(k, pivot_row);
        }

        let pivot_inv = 1.0 / lu[k * n + k];
        // update sub-column (multipliers stored below diagonal)
        for i in (k + 1)..n {
            lu[i * n + k] *= pivot_inv;
            // update remaining submatrix
            for j in (k + 1)..n {
                let mult = lu[i * n + k];
                lu[i * n + j] -= mult * lu[k * n + j];
            }
        }
    }

    // --- apply permutation to b ---
    let mut pb = vec![0.0f32; n];
    for i in 0..n {
        pb[i] = b[perm[i]];
    }

    // --- forward substitution: L · y = Pb (L has implicit unit diagonal) ---
    let mut y = pb;
    for i in 1..n {
        for j in 0..i {
            y[i] -= lu[i * n + j] * y[j];
        }
    }

    // --- back substitution: U · x = y ---
    let mut x = y;
    for i in (0..n).rev() {
        for j in (i + 1)..n {
            x[i] -= lu[i * n + j] * x[j];
        }
        let u_ii = lu[i * n + i];
        if u_ii.abs() < f32::EPSILON * 64.0 {
            return Err(SolverError::Singular);
        }
        x[i] /= u_ii;
    }

    Ok(x)
}

// ---------------------------------------------------------------------------
// CPU Cholesky solve (Banachiewicz decomposition, SPD matrices only)
// ---------------------------------------------------------------------------

/// Solve the n×n symmetric positive-definite system `A · x = b` using the
/// Cholesky–Banachiewicz decomposition `A = L · Lᵀ`.
///
/// # Algorithm
/// 1. Compute lower-triangular L such that A = L Lᵀ, using the column-wise
///    Banachiewicz recurrence.
/// 2. Forward substitution: L · y = b.
/// 3. Backward substitution: Lᵀ · x = y.
///
/// # Errors
/// Returns [`SolverError::Singular`] if a diagonal entry of L would be
/// non-positive (i.e. the matrix is not positive-definite).
pub(crate) fn solve_cholesky_cpu(a: &[f32], n: usize, b: &[f32]) -> Result<Vec<f32>, SolverError> {
    if a.len() != n * n {
        let rows = a.len() / n.max(1);
        return Err(SolverError::NotSquare { rows, cols: n });
    }
    if b.len() != n {
        return Err(SolverError::DimMismatch(format!(
            "rhs has length {} but matrix is {n}x{n}",
            b.len()
        )));
    }
    if n == 0 {
        return Ok(vec![]);
    }

    // l[i][j] stored row-major; only the lower-triangular part is filled.
    let mut l = vec![0.0f32; n * n];

    for j in 0..n {
        // diagonal element
        let mut diag = a[j * n + j];
        for k in 0..j {
            let l_jk = l[j * n + k];
            diag -= l_jk * l_jk;
        }
        if diag <= 0.0 {
            return Err(SolverError::Singular);
        }
        l[j * n + j] = diag.sqrt();

        let l_jj = l[j * n + j];
        // sub-diagonal elements in column j
        for i in (j + 1)..n {
            let mut val = a[i * n + j];
            for k in 0..j {
                val -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = val / l_jj;
        }
    }

    // forward substitution: L · y = b
    let mut y = vec![0.0f32; n];
    for i in 0..n {
        let mut sum = b[i];
        for j in 0..i {
            sum -= l[i * n + j] * y[j];
        }
        y[i] = sum / l[i * n + i];
    }

    // backward substitution: Lᵀ · x = y
    // Lᵀ[i][j] = L[j][i], so element (i, j) of Lᵀ (j >= i) is L[j * n + i].
    let mut x = vec![0.0f32; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for j in (i + 1)..n {
            sum -= l[j * n + i] * x[j];
        }
        x[i] = sum / l[i * n + i];
    }

    Ok(x)
}

// ---------------------------------------------------------------------------
// CPU QR least-squares (modified Gram-Schmidt orthogonalisation)
// ---------------------------------------------------------------------------

/// Compute the minimum-norm least-squares solution to the (possibly
/// overdetermined) m×n system `A · x ≈ b` using modified Gram-Schmidt QR.
///
/// When m < n the system is underdetermined and the first n columns of Q span
/// the column space; the algorithm still produces a valid answer for the
/// determined part (Gram-Schmidt stops at column min(m, n)).
///
/// # Algorithm
/// 1. Modified Gram-Schmidt: compute Q (m×k) and R (k×n) where k = min(m, n).
/// 2. Form `Qᵀ · b` (the projected right-hand side, length k).
/// 3. Back-substitution on the k×n upper-triangular system R̂ · x̂ = Qᵀb,
///    where R̂ is the leading k×k part of R (assuming rank k).
///
/// # Errors
/// Returns [`SolverError::Singular`] when R has a near-zero diagonal (rank
/// deficient system).
pub(crate) fn solve_qr_lstsq_cpu(
    a: &[f32],
    m: usize,
    n: usize,
    b: &[f32],
) -> Result<Vec<f32>, SolverError> {
    if a.len() != m * n {
        return Err(SolverError::DimMismatch(format!(
            "A has {} elements but m={m}, n={n} implies {}",
            a.len(),
            m * n
        )));
    }
    if b.len() != m {
        return Err(SolverError::DimMismatch(format!(
            "b has length {} but A has {m} rows",
            b.len()
        )));
    }
    if n == 0 || m == 0 {
        return Ok(vec![0.0f32; n]);
    }

    let k = m.min(n); // rank at most min(m, n)

    // Q stored as a Vec of column vectors (column-major): q_cols[j] has length m.
    // R stored row-major (k × n); only the upper-triangular k×k block matters for back-sub.
    let mut q_cols: Vec<Vec<f32>> = Vec::with_capacity(k);
    let mut r = vec![0.0f32; k * n]; // row-major k×n

    // Working matrix: columns of A as mutable Vec<f32> each of length m.
    // Modified Gram-Schmidt operates directly on these — no separate `v` clone needed.
    // After step j, a_cols[j] holds the orthogonal (not yet normalised) basis vector,
    // and a_cols[jj] (jj > j) have been deflated by all q_i computed so far.
    let mut a_cols: Vec<Vec<f32>> = (0..n)
        .map(|j| (0..m).map(|i| a[i * n + j]).collect())
        .collect();

    for j in 0..k {
        // Compute the diagonal R[j,j] = ||a_cols[j]||.
        let norm_j = dot(&a_cols[j], &a_cols[j]).sqrt();
        if norm_j < f32::EPSILON * 64.0 {
            return Err(SolverError::Singular);
        }
        r[j * n + j] = norm_j;

        // Normalise column j to get q_j.
        let inv_norm = 1.0 / norm_j;
        let q_j: Vec<f32> = a_cols[j].iter().map(|&v| v * inv_norm).collect();

        // Project and deflate all subsequent columns against q_j:
        //   R[j, jj] = q_j · a_cols[jj]   (off-diagonal of R, row j)
        //   a_cols[jj] -= R[j,jj] * q_j   (orthogonalise future columns)
        // This is the "modified" step that gives MGS better numerical properties
        // than classical GS (which computes all projections against the original column).
        for jj in (j + 1)..n {
            let r_j_jj = dot(&q_j, &a_cols[jj]);
            r[j * n + jj] = r_j_jj;
            // deflate in-place to avoid a temporary Vec allocation
            for l in 0..m {
                let subtract = q_j[l] * r_j_jj;
                a_cols[jj][l] -= subtract;
            }
        }

        q_cols.push(q_j);
    }

    // Qᵀ · b  (length k)
    let mut qtb = vec![0.0f32; k];
    for (i, q_i) in q_cols.iter().enumerate() {
        qtb[i] = dot(q_i, b);
    }

    // Back-substitution on the k×k upper-triangular leading block of R.
    // Result x has length n; entries beyond k are set to 0 (underdetermined case).
    let mut x = vec![0.0f32; n];
    for i in (0..k).rev() {
        let mut sum = qtb[i];
        for j in (i + 1)..k {
            sum -= r[i * n + j] * x[j];
        }
        let r_ii = r[i * n + i];
        if r_ii.abs() < f32::EPSILON * 64.0 {
            return Err(SolverError::Singular);
        }
        x[i] = sum / r_ii;
    }

    Ok(x)
}

// ---------------------------------------------------------------------------
// GPU stubs (Round 6 wiring)
// ---------------------------------------------------------------------------

/// GPU-accelerated LU solve.  Stub: returns a [`SolverError::GpuError`] until
/// the full OxiCUDA solver API is wired in Round 6.
#[cfg(feature = "gpu")]
pub(crate) fn solve_lu_gpu(_a: &[f32], _n: usize, _b: &[f32]) -> Result<Vec<f32>, SolverError> {
    Err(SolverError::GpuError(
        "GPU LU solver requires CUDA runtime (wired in Round 6)".to_string(),
    ))
}

/// GPU-accelerated Cholesky solve.  Stub: see [`solve_lu_gpu`].
#[cfg(feature = "gpu")]
pub(crate) fn solve_cholesky_gpu(
    _a: &[f32],
    _n: usize,
    _b: &[f32],
) -> Result<Vec<f32>, SolverError> {
    Err(SolverError::GpuError(
        "GPU Cholesky solver requires CUDA runtime (wired in Round 6)".to_string(),
    ))
}

/// GPU-accelerated QR least-squares.  Stub: see [`solve_lu_gpu`].
#[cfg(feature = "gpu")]
pub(crate) fn solve_qr_lstsq_gpu(
    _a: &[f32],
    _m: usize,
    _n: usize,
    _b: &[f32],
) -> Result<Vec<f32>, SolverError> {
    Err(SolverError::GpuError(
        "GPU QR solver requires CUDA runtime (wired in Round 6)".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Internal tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Helper: max absolute error between two equal-length slices.
    fn max_abs_err(u: &[f32], v: &[f32]) -> f32 {
        u.iter()
            .zip(v.iter())
            .map(|(&a, &b)| (a - b).abs())
            .fold(0.0f32, f32::max)
    }

    // --- LU tests ---

    #[test]
    fn lu_identity_3x3() {
        let a = vec![1f32, 0., 0., 0., 1., 0., 0., 0., 1.];
        let b = vec![7f32, -3., 5.];
        let x = solve_lu_cpu(&a, 3, &b).unwrap();
        assert!(max_abs_err(&x, &b) < 1e-5, "x={x:?}");
    }

    #[test]
    fn lu_2x2_known_solution() {
        // A = [[3,1],[1,2]], b = [9,8]
        // Solving: 3x+y=9, x+2y=8 → x=2, y=3
        let a = vec![3f32, 1., 1., 2.];
        let b = vec![9f32, 8.];
        let x = solve_lu_cpu(&a, 2, &b).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-4, "x[0]={}", x[0]);
        assert!((x[1] - 3.0).abs() < 1e-4, "x[1]={}", x[1]);
        // Also verify residual: A*x - b should be ~0
        let ax0 = 3.0 * x[0] + 1.0 * x[1];
        let ax1 = 1.0 * x[0] + 2.0 * x[1];
        assert!((ax0 - 9.0).abs() < 1e-4, "ax0={ax0}");
        assert!((ax1 - 8.0).abs() < 1e-4, "ax1={ax1}");
    }

    #[test]
    fn lu_singular_returns_error() {
        let a = vec![1f32, 2., 2., 4.]; // rank 1
        let b = vec![1f32, 2.];
        assert!(matches!(
            solve_lu_cpu(&a, 2, &b),
            Err(SolverError::Singular)
        ));
    }

    #[test]
    fn lu_4x4_random_ish() {
        // Hand-constructed non-singular 4×4
        #[rustfmt::skip]
        let a = vec![
             4f32, 3., 2., 1.,
             3., 4., 3., 2.,
             2., 3., 4., 3.,
             1., 2., 3., 4.,
        ];
        let b = vec![10f32, 12., 12., 10.];
        let x = solve_lu_cpu(&a, 4, &b).unwrap();
        // Verify: A * x ≈ b
        let mut ax = vec![0.0f32; 4];
        mat_vec(&a, 4, 4, &x, &mut ax);
        assert!(max_abs_err(&ax, &b) < 1e-4, "residual ax={ax:?} vs b={b:?}");
    }

    // --- Cholesky tests ---

    #[test]
    fn cholesky_2x2_spd() {
        // A = [[4,2],[2,3]], b = [6,5] → x = [1,1]
        let a = vec![4f32, 2., 2., 3.];
        let b = vec![6f32, 5.];
        let x = solve_cholesky_cpu(&a, 2, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-5, "x[0]={}", x[0]);
        assert!((x[1] - 1.0).abs() < 1e-5, "x[1]={}", x[1]);
    }

    #[test]
    fn cholesky_identity_3x3() {
        let a = vec![1f32, 0., 0., 0., 1., 0., 0., 0., 1.];
        let b = vec![2f32, -1., 4.];
        let x = solve_cholesky_cpu(&a, 3, &b).unwrap();
        assert!(max_abs_err(&x, &b) < 1e-5, "x={x:?}");
    }

    #[test]
    fn cholesky_non_spd_returns_singular() {
        // A = [[1,2],[2,1]] has eigenvalues 3 and -1 → not SPD
        let a = vec![1f32, 2., 2., 1.];
        let b = vec![1f32, 1.];
        assert!(matches!(
            solve_cholesky_cpu(&a, 2, &b),
            Err(SolverError::Singular)
        ));
    }

    // --- QR tests ---

    #[test]
    fn qr_square_2x2() {
        // Same SPD system as cholesky test
        let a = vec![4f32, 2., 2., 3.];
        let b = vec![6f32, 5.];
        let x = solve_qr_lstsq_cpu(&a, 2, 2, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-4, "x[0]={}", x[0]);
        assert!((x[1] - 1.0).abs() < 1e-4, "x[1]={}", x[1]);
    }

    #[test]
    fn qr_overdetermined_3x2() {
        // A (3×2) = [[1,0],[0,1],[1,1]], b = [1,1,2] → x = [1,1] is exact
        let a = vec![1f32, 0., 0., 1., 1., 1.];
        let b = vec![1f32, 1., 2.];
        let x = solve_qr_lstsq_cpu(&a, 3, 2, &b).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-4, "x[0]={}", x[0]);
        assert!((x[1] - 1.0).abs() < 1e-4, "x[1]={}", x[1]);
    }
}
