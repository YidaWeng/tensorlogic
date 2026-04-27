//! # tensorlogic-oxicuda-solver
//!
//! GPU-accelerated linear solver wrapper for TensorLogic with a robust CPU fallback.
//!
//! All public entry points accept row-major `&[f32]` slices and return `Vec<f32>`.
//! When compiled with `--features gpu` and a CUDA device is detected at runtime,
//! calls are dispatched to the corresponding OxiCUDA GPU kernel.  Otherwise the
//! pure-Rust CPU implementation (no external linear-algebra dependency) is used.
//!
//! ## Available solvers
//!
//! | Function | Method | Matrix type |
//! |---|---|---|
//! | [`solve_lu`] | LU with partial pivoting | General square |
//! | [`solve_cholesky`] | Cholesky–Banachiewicz | Symmetric positive-definite |
//! | [`solve_qr_lstsq`] | Modified Gram-Schmidt QR | Rectangular (least-squares) |
//! | [`cg_solve`] | Conjugate Gradient | SPD (iterative) |
//!
//! ## Feature flags
//!
//! | Feature | Effect |
//! |---|---|
//! | `cpu` (default) | Pure-Rust CPU implementations compiled in |
//! | `gpu` | Links `oxicuda-solver`, `oxicuda-driver`, `oxicuda-memory`; adds GPU dispatch |
//!
//! ## Example
//!
//! ```rust
//! use tensorlogic_oxicuda_solver::{solve_lu, solve_cholesky, cg_solve, SolverError};
//!
//! fn example() -> Result<(), SolverError> {
//!     // Solve I * x = [1, 2, 3]  → x = [1, 2, 3]
//!     let identity = vec![1f32, 0., 0.,  0., 1., 0.,  0., 0., 1.];
//!     let b = vec![1f32, 2., 3.];
//!     let x = solve_lu(&identity, 3, &b)?;
//!     assert!((x[0] - 1.0).abs() < 1e-5);
//!
//!     // Solve SPD 2×2 with CG: [[4,2],[2,3]] * x = [6,5] → x = [1,1]
//!     let a_spd = vec![4f32, 2., 2., 3.];
//!     let b2 = vec![6f32, 5.];
//!     let x2 = cg_solve(&a_spd, 2, &b2, 100, 1e-6)?;
//!     assert!((x2[0] - 1.0).abs() < 1e-4);
//!     Ok(())
//! }
//! # example().expect("doctest failed");
//! ```

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod dense;
pub mod error;
pub mod iterative;

pub use error::SolverError;

// ---------------------------------------------------------------------------
// GPU availability probe
// ---------------------------------------------------------------------------

/// Returns `true` when the `gpu` feature is compiled in **and** a CUDA-capable
/// device can be found at runtime.
///
/// In Round 5 this always returns `false`; the GPU wiring is completed in Round 6.
#[cfg(feature = "gpu")]
fn gpu_available() -> bool {
    // Round 6 will replace this with an actual oxicuda-driver device query.
    false
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Solve the n×n linear system `A · x = b` using LU factorisation with
/// partial pivoting.
///
/// # Parameters
/// - `a` – Row-major coefficient matrix; must have exactly `n * n` elements.
/// - `n` – Matrix dimension.
/// - `b` – Right-hand side; must have exactly `n` elements.
///
/// # Errors
/// See [`SolverError`] for the full list of error conditions.
pub fn solve_lu(a: &[f32], n: usize, b: &[f32]) -> Result<Vec<f32>, SolverError> {
    #[cfg(feature = "gpu")]
    if gpu_available() {
        return dense::solve_lu_gpu(a, n, b);
    }
    dense::solve_lu_cpu(a, n, b)
}

/// Solve the n×n symmetric positive-definite (SPD) system `A · x = b` using
/// the Cholesky decomposition `A = L · Lᵀ`.
///
/// # Parameters
/// - `a` – Row-major SPD coefficient matrix; must have exactly `n * n` elements.
/// - `n` – Matrix dimension.
/// - `b` – Right-hand side; must have exactly `n` elements.
///
/// # Errors
/// Returns [`SolverError::Singular`] when A is not positive-definite.
/// See [`SolverError`] for the full list.
pub fn solve_cholesky(a: &[f32], n: usize, b: &[f32]) -> Result<Vec<f32>, SolverError> {
    #[cfg(feature = "gpu")]
    if gpu_available() {
        return dense::solve_cholesky_gpu(a, n, b);
    }
    dense::solve_cholesky_cpu(a, n, b)
}

/// Compute the minimum-norm least-squares solution to the (possibly
/// overdetermined) m×n system `A · x ≈ b` using modified Gram-Schmidt QR.
///
/// # Parameters
/// - `a` – Row-major m×n matrix; must have exactly `m * n` elements.
/// - `m` – Number of rows of A (observations).
/// - `n` – Number of columns of A (unknowns).
/// - `b` – Right-hand side; must have exactly `m` elements.
///
/// # Errors
/// Returns [`SolverError::Singular`] when A is rank-deficient.
/// See [`SolverError`] for the full list.
pub fn solve_qr_lstsq(a: &[f32], m: usize, n: usize, b: &[f32]) -> Result<Vec<f32>, SolverError> {
    #[cfg(feature = "gpu")]
    if gpu_available() {
        return dense::solve_qr_lstsq_gpu(a, m, n, b);
    }
    dense::solve_qr_lstsq_cpu(a, m, n, b)
}

/// Solve the n×n SPD system `A · x = b` iteratively using the Conjugate
/// Gradient (CG) method.
///
/// CG is well-suited to large, sparse, SPD systems where direct factorisation
/// would be expensive.  For dense systems of moderate size, [`solve_cholesky`]
/// is usually faster.
///
/// # Parameters
/// - `a`        – Row-major n×n SPD matrix.
/// - `n`        – System dimension.
/// - `b`        – Right-hand side (length n).
/// - `max_iter` – Iteration budget; a common default is `n` or `2 * n`.
/// - `tol`      – Convergence threshold on ‖r‖₂ (e.g. `1e-6`).
///
/// # Errors
/// Returns [`SolverError::DidNotConverge`] when the budget is exhausted.
/// See [`SolverError`] for the full list.
pub fn cg_solve(
    a: &[f32],
    n: usize,
    b: &[f32],
    max_iter: usize,
    tol: f32,
) -> Result<Vec<f32>, SolverError> {
    #[cfg(feature = "gpu")]
    if gpu_available() {
        return iterative::cg_solve_gpu(a, n, b, max_iter, tol);
    }
    iterative::cg_solve_cpu(a, n, b, max_iter, tol)
}
