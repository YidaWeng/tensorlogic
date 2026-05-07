//! # tensorlogic-oxicuda-sparse
//!
//! GPU-accelerated sparse matrix operations for the TensorLogic project,
//! backed by OxiCUDA on NVIDIA hardware with a pure-Rust CPU fallback.
//!
//! ## Feature flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `cpu` (default) | Pure-Rust CSR sparse routines — no CUDA required. |
//! | `gpu` | Routes computation through `oxicuda-sparse`; requires `libcuda.so` at run-time. Falls back to CPU automatically when the driver is unavailable. |
//!
//! ## Core types
//!
//! - [`SparseCsr`] — host-resident CSR matrix with `i32` indices and `f32` values.
//!
//! ## Core operations
//!
//! - [`spmv`] — `y = alpha * A * x + beta * y`
//! - [`spmm`] — `C = alpha * A * B + beta * C`  (B and C are row-major dense)

#![warn(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::suspicious)]

pub mod csr;
pub mod error;

pub use csr::SparseCsr;
pub use error::SparseError;

// ---------------------------------------------------------------------------
// GPU path
// ---------------------------------------------------------------------------

/// GPU-side sparse operations (requires the `gpu` feature and a CUDA driver).
#[cfg(feature = "gpu")]
mod gpu {
    use std::sync::Arc;

    use oxicuda_blas::{Layout, MatrixDesc, MatrixDescMut};
    use oxicuda_driver::{Context, Device};
    use oxicuda_memory::DeviceBuffer;
    use oxicuda_sparse::format::CsrMatrix;
    use oxicuda_sparse::handle::SparseHandle;
    use oxicuda_sparse::ops::{spmm as oxicuda_spmm, spmv as oxicuda_spmv, SpMVAlgo};

    use crate::csr::SparseCsr;
    use crate::error::SparseError;

    /// Attempt to initialise CUDA device 0 and create a [`SparseHandle`].
    ///
    /// Returns `Err` when the driver is absent or there are no devices; the
    /// public API treats this as a cue to fall back to the CPU path.
    fn try_init(device_id: i32) -> Result<(Arc<Context>, SparseHandle), SparseError> {
        let device = Device::get(device_id)
            .map_err(|e| SparseError::GpuError(format!("CUDA device {device_id} init: {e}")))?;
        let ctx = Arc::new(
            Context::new(&device)
                .map_err(|e| SparseError::GpuError(format!("CUDA context create: {e}")))?,
        );
        let handle = SparseHandle::new(&ctx)
            .map_err(|e| SparseError::GpuError(format!("SparseHandle create: {e}")))?;
        Ok((ctx, handle))
    }

    /// Upload a host-resident [`SparseCsr`] to the GPU.
    fn upload_csr(a: &SparseCsr) -> Result<CsrMatrix<f32>, SparseError> {
        CsrMatrix::from_host(a.rows as u32, a.cols as u32, &a.indptr, &a.indices, &a.data)
            .map_err(|e| SparseError::GpuError(format!("CsrMatrix upload: {e}")))
    }

    /// GPU-accelerated SpMV: `y = alpha * A * x + beta * y`.
    ///
    /// Returns `Err(SparseError::GpuError(_))` when CUDA initialisation fails;
    /// callers should fall back to the CPU path in that case.
    pub(crate) fn gpu_spmv(
        a: &SparseCsr,
        x: &[f32],
        alpha: f32,
        beta: f32,
        y: &mut [f32],
    ) -> Result<(), SparseError> {
        let (_ctx, handle) = try_init(0)?;
        let d_a = upload_csr(a)?;

        let d_x = DeviceBuffer::from_host(x)
            .map_err(|e| SparseError::GpuError(format!("x upload: {e}")))?;
        let d_y = DeviceBuffer::from_host(y)
            .map_err(|e| SparseError::GpuError(format!("y upload: {e}")))?;

        let x_ptr = d_x.as_device_ptr();
        let y_ptr = d_y.as_device_ptr();

        oxicuda_spmv(&handle, SpMVAlgo::Adaptive, alpha, &d_a, x_ptr, beta, y_ptr)
            .map_err(|e| SparseError::GpuError(format!("spmv kernel: {e}")))?;

        // Download result back to the host output buffer.
        // Both d_x and d_y stay alive until end of scope (CUDA stream is synchronous).
        d_y.copy_to_host(y)
            .map_err(|e| SparseError::GpuError(format!("y download: {e}")))?;

        Ok(())
    }

    /// GPU-accelerated SpMM: `C = alpha * A * B + beta * C`.
    ///
    /// Returns `Err(SparseError::GpuError(_))` when CUDA initialisation fails;
    /// callers should fall back to the CPU path in that case.
    pub(crate) fn gpu_spmm(
        a: &SparseCsr,
        b: &[f32],
        b_cols: usize,
        alpha: f32,
        beta: f32,
        c: &mut [f32],
    ) -> Result<(), SparseError> {
        let (_ctx, handle) = try_init(0)?;
        let d_a = upload_csr(a)?;

        let d_b = DeviceBuffer::from_host(b)
            .map_err(|e| SparseError::GpuError(format!("B upload: {e}")))?;
        let mut d_c = DeviceBuffer::from_host(c)
            .map_err(|e| SparseError::GpuError(format!("C upload: {e}")))?;

        // Build row-major matrix descriptors from the device buffers.
        let b_desc = MatrixDesc::from_buffer(&d_b, a.cols as u32, b_cols as u32, Layout::RowMajor)
            .map_err(|e| SparseError::GpuError(format!("MatrixDesc(B): {e}")))?;
        let mut c_desc =
            MatrixDescMut::from_buffer(&mut d_c, a.rows as u32, b_cols as u32, Layout::RowMajor)
                .map_err(|e| SparseError::GpuError(format!("MatrixDescMut(C): {e}")))?;

        oxicuda_spmm(&handle, alpha, &d_a, &b_desc, beta, &mut c_desc)
            .map_err(|e| SparseError::GpuError(format!("spmm kernel: {e}")))?;

        d_c.copy_to_host(c)
            .map_err(|e| SparseError::GpuError(format!("C download: {e}")))
    }
}

// ---------------------------------------------------------------------------
// CPU path (always compiled; used when the `gpu` feature is absent or when
// the CUDA driver fails to initialise at runtime).
// ---------------------------------------------------------------------------

/// Pure-Rust O(nnz) CSR sparse matrix-vector multiply: `y = alpha * A * x + beta * y`.
fn cpu_spmv(
    a: &SparseCsr,
    x: &[f32],
    alpha: f32,
    beta: f32,
    y: &mut [f32],
) -> Result<(), SparseError> {
    if x.len() != a.cols {
        return Err(SparseError::ShapeMismatch(format!(
            "spmv: x.len()={} but A.cols={}",
            x.len(),
            a.cols
        )));
    }
    if y.len() != a.rows {
        return Err(SparseError::ShapeMismatch(format!(
            "spmv: y.len()={} but A.rows={}",
            y.len(),
            a.rows
        )));
    }

    for (i, y_i) in y.iter_mut().enumerate().take(a.rows) {
        let start = a.indptr[i] as usize;
        let end = a.indptr[i + 1] as usize;
        let mut dot = 0.0f32;
        for k in start..end {
            dot += a.data[k] * x[a.indices[k] as usize];
        }
        *y_i = alpha * dot + beta * *y_i;
    }

    Ok(())
}

/// Pure-Rust CSR sparse-dense matrix multiply: `C = alpha * A * B + beta * C`.
///
/// A is `(m, k)` sparse, B is `(k, n)` row-major dense, C is `(m, n)` row-major dense.
/// Each column of B is extracted, passed through [`cpu_spmv`], then written back to C.
fn cpu_spmm(
    a: &SparseCsr,
    b: &[f32],
    b_cols: usize,
    alpha: f32,
    beta: f32,
    c: &mut [f32],
) -> Result<(), SparseError> {
    let m = a.rows;
    let k = a.cols;
    let n = b_cols;

    if b.len() != k * n {
        return Err(SparseError::ShapeMismatch(format!(
            "spmm: B.len()={} but expected A.cols*b_cols={}*{}={}",
            b.len(),
            k,
            n,
            k * n,
        )));
    }
    if c.len() != m * n {
        return Err(SparseError::ShapeMismatch(format!(
            "spmm: C.len()={} but expected A.rows*b_cols={}*{}={}",
            c.len(),
            m,
            n,
            m * n,
        )));
    }

    // Pre-allocate temporary column-vector buffers to avoid repeated heap allocation.
    let mut x_col = vec![0.0f32; k];
    let mut y_col = vec![0.0f32; m];

    for j in 0..n {
        // Extract column j of B (row-major, stride n).
        for row_b in 0..k {
            x_col[row_b] = b[row_b * n + j];
        }

        // Extract column j of C (row-major, stride n).
        for row_c in 0..m {
            y_col[row_c] = c[row_c * n + j];
        }

        // y_col = alpha * A * x_col + beta * y_col
        cpu_spmv(a, &x_col, alpha, beta, &mut y_col)?;

        // Write column j back into C.
        for row_c in 0..m {
            c[row_c * n + j] = y_col[row_c];
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Sparse matrix-vector multiply: `y = alpha * A * x + beta * y`.
///
/// When compiled with the `gpu` feature and a CUDA driver is present this
/// routes through `oxicuda-sparse`'s PTX kernels.  On failure (no GPU,
/// missing driver, etc.) it transparently falls back to the pure-Rust CPU
/// implementation.  Without the `gpu` feature the CPU path is always taken.
///
/// # Arguments
///
/// * `a`     – Sparse CSR matrix.
/// * `x`     – Input dense vector; must have length `a.cols`.
/// * `alpha` – Scalar multiplier for `A * x`.
/// * `beta`  – Scalar multiplier for the current content of `y`.
/// * `y`     – In-out dense vector; must have length `a.rows`.
///
/// # Errors
///
/// Returns [`SparseError::ShapeMismatch`] when `x.len() != a.cols` or
/// `y.len() != a.rows`.
pub fn spmv(
    a: &SparseCsr,
    x: &[f32],
    alpha: f32,
    beta: f32,
    y: &mut [f32],
) -> Result<(), SparseError> {
    #[cfg(feature = "gpu")]
    {
        if gpu::gpu_spmv(a, x, alpha, beta, y).is_ok() {
            return Ok(());
        }
        // GPU unavailable — fall through to CPU path.
    }

    cpu_spmv(a, x, alpha, beta, y)
}

/// Sparse-dense matrix multiply: `C = alpha * A * B + beta * C`.
///
/// `A` is a sparse CSR matrix of shape `(m, k)`.  `B` is a dense row-major
/// matrix of shape `(k, b_cols)`.  `C` is a dense row-major matrix of shape
/// `(m, b_cols)`.
///
/// When compiled with the `gpu` feature and a CUDA driver is present this
/// routes through `oxicuda-sparse`'s PTX kernels.  On failure it falls back
/// to the pure-Rust CPU implementation.
///
/// # Arguments
///
/// * `a`      – Sparse CSR matrix.
/// * `b`      – Dense input matrix in row-major order; length `a.cols * b_cols`.
/// * `b_cols` – Number of columns in `B` (and `C`).
/// * `alpha`  – Scalar multiplier for `A * B`.
/// * `beta`   – Scalar multiplier for the current content of `C`.
/// * `c`      – In-out dense output matrix in row-major order; length `a.rows * b_cols`.
///
/// # Errors
///
/// Returns [`SparseError::ShapeMismatch`] when buffer lengths are inconsistent
/// with the declared shape.
pub fn spmm(
    a: &SparseCsr,
    b: &[f32],
    b_cols: usize,
    alpha: f32,
    beta: f32,
    c: &mut [f32],
) -> Result<(), SparseError> {
    #[cfg(feature = "gpu")]
    {
        if gpu::gpu_spmm(a, b, b_cols, alpha, beta, c).is_ok() {
            return Ok(());
        }
        // GPU unavailable — fall through to CPU path.
    }

    cpu_spmm(a, b, b_cols, alpha, beta, c)
}
