# TensorLogic — TODO

**Status**: Stable | **Version**: 0.1.0 | **Released**: 2026-04-06 | **Last Updated**: 2026-04-27
**History**: See [CHANGELOG.md](./CHANGELOG.md) for release history.

## Round 5 (2026-04-17) — Complete

### Deliverables
- [x] **BackendKind enum** — `crates/tensorlogic-infer/src/backend_kind.rs`: 7-variant enum (`Scirs`, `OxiCuda` live; `Metal`/`Vulkan`/`Rocm`/`Webgpu`/`Levelzero` doc-hidden stubs). Implements `std::str::FromStr`. Methods: `default_backend()`, `from_env()`, `is_gpu()`, `supports_autodiff()`, `validate()`, `as_str()`.
- [x] **tensorlogic-oxicuda-sparse** — new wrapper crate: `SparseCsr`, `spmv`, `spmm`. CPU path: manual CSR arithmetic. GPU path: stubbed (`#[cfg(feature="gpu")]`), wired in Round 6.
- [x] **tensorlogic-oxicuda-solver** — new wrapper crate: `solve_lu`, `solve_cholesky`, `solve_qr_lstsq`, `cg_solve`. Pure-Rust CPU solvers (Doolittle LU, Cholesky-Banachiewicz, Modified Gram-Schmidt QR, CG). GPU path: stubbed.
- [x] **tensorlogic-oxicuda-rng** — new wrapper crate: `RngEngine` (PCG-XSH-RR, Box-Muller), `uniform_f32`, `normal_f32`, `bernoulli`. `Send+!Sync`. GPU path: stubbed.
- [x] **Umbrella feature flags** — `sparse`, `solver`, `rng`, `full-advanced` added to `crates/tensorlogic/Cargo.toml` and re-exported in `crates/tensorlogic/src/lib.rs`.
- [x] **`[patch.crates-io]` extended** — `oxicuda-sparse`, `oxicuda-solver`, `oxicuda-rand` patched to `/notebooks/oxicuda/crates/*`.
- [x] **Native `fill` kernel** — `oxicuda_blas::elementwise::fill<T>()` in `/notebooks/oxicuda/crates/oxicuda-blas/src/elementwise/fill.rs`. PTX via `ElementwiseOp::Fill` (added to oxicuda-ptx).
- [x] **Native `broadcast_axes` kernel** — `oxicuda_blas::elementwise::broadcast_axes<T>()` in `.../broadcast.rs`. `BroadcastTemplate` in oxicuda-ptx uses stride-zero trick; 28-param PTX kernel supports rank ≤ 8.
- [x] **Autodiff rewiring** — `crates/tensorlogic-oxicuda-backend`: `native-broadcast = ["gpu"]` feature. Under this feature, `broadcast_to_shape` and Mean-divisor `fill` use native GPU kernels (upload→kernel→readback). Removed 4 `TODO(perf)` Round-5-follow-up comments.
- [x] **LRU cache correctness fix** — `tensorlogic-infer/src/memo_cache.rs`: `find_lru_key` now uses the deque-front strategy (O(1), tie-safe) instead of `min_by_key(last_accessed)` which was racy under nanosecond-level Instant precision.

### Round 5 Test Coverage
- `tensorlogic-infer/tests/backend_kind_smoke.rs`: 13 tests
- `tensorlogic-oxicuda-sparse/tests/smoke.rs`: 13 tests (2 GPU ignored)
- `tensorlogic-oxicuda-solver/tests/smoke.rs`: 30 tests
- `tensorlogic-oxicuda-rng/tests/smoke.rs`: 47 tests
- `tensorlogic-oxicuda-backend/tests/native_broadcast_smoke.rs`: 6 tests (2 GPU ignored)
- Full workspace: **6784 tests, 6784 passed, 21 skipped (GPU-only), 0 failed**

TensorLogic compiles logical rules into tensor equations (einsum graphs) with a minimal DSL + IR, enabling neural / symbolic / probabilistic models in a unified tensor framework.

## At a glance

- Phase 0 — Repo Hygiene. **Complete.**
- Phase 1 — Minimal IR & Compiler. **Complete.**
- Phase 2 — Engine Traits & Dummy Executor. **Complete.**
- Phase 3 — SciRS2 Backend. **Complete.**
- Phase 4 — OxiRS Bridge. **Complete.**
- Phase 5 — Interop Crates. **Complete.**
- Phase 6 — Training Scaffolds. **Complete.**
- Phase 7 — Python Bindings. **Complete.**
- Phase 8 — Validation & Scale. **Complete.**
- Stable release 0.1.0 — 2026-04-06. **Complete.**
- Post-stable hardening — 2026-04-14: three oversize files split (`kv_cache.rs`, `codegen.rs`, `sparql.rs`), flagship integration tests added, feature flags introduced. **Complete.**
- Documentation hygiene — 2026-04-15: extracted release history to CHANGELOG.md, unified TODO header template, added forward-looking v0.2.0 sections to every crate TODO. **Complete.**
- v0.2.0 research preview — 2026-04-15: six features delivered (rule-guided decoding, learned kernel composition, variational message passing, deep kernel learning, speculative decoding, partial error recovery) + two pre-existing items audited complete (incremental recompilation, streaming inference). **Complete.**

See [CHANGELOG.md](./CHANGELOG.md) for the full record of each phase, RC, and stable-release deliverable.

## Post-stable Roadmap

### v0.1.x Stabilization (in-repo, unblocked)

#### Completed 2026-04-15

- [x] ~~Split the nine remaining files > 1,500 lines using the same `dir/{mod,...}.rs` pattern used 2026-04-14:~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-adapters/src/database.rs` (1,613 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-compiler/src/dead_code.rs` (1,614 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-compiler/src/partial_eval.rs` (1,789 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-compiler/src/symbolic_diff.rs` (1,524 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-infer/src/causal.rs` (1,589 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-ir/src/resolution.rs` (1,712 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-oxirs-bridge/src/sparql_gen.rs` (1,530 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-quantrs-hooks/src/loopy_bp.rs` (1,744 L)~~ (completed 2026-04-15)
  - [x] ~~`crates/tensorlogic-train/src/hyperparameter.rs` (1,641 L) and `loss.rs` (1,551 L)~~ (completed 2026-04-15)
- [x] ~~Fix the doc-build warning: `cargo doc --workspace --no-deps --all-features` reports a bin/lib output-path collision for `tensorlogic`. Rename the CLI `[[bin]]` `name` in `crates/tensorlogic-cli/Cargo.toml` to `tensorlogic-cli` (Cargo issue #6313).~~ (completed 2026-04-15)

#### Remaining

- Coverage-gate per-crate test counts (nextest + tarpaulin or llvm-cov) into CI once SciRS2 stabilizes.

### v0.2.0 (blocked on upstream SciRS2 / ecosystem)

- Distributed training (NCCL / multi-GPU scheduling).
- PyPI release of `tensorlogic-py` via maturin; requires release-ready `abi3-py39` manylinux wheels.

### v0.2.0 / Research (in-repo, forward-looking)

#### Delivered 2026-04-15 (Research Preview)

- [x] ~~Rule-guided sampling decoder in `tensorlogic-trustformers`~~ — `rule_guided_decoder/` module: TLExpr-constrained beam search with hard-masking and soft-penalty modes.
- [x] ~~Learned kernel composition in `tensorlogic-sklears-kernels`~~ — `learned_composition/` module: differentiable softmax-gated mixture over a kernel library.
- [x] ~~Variational Message Passing in `tensorlogic-quantrs-hooks`~~ — `vmp/` module: VMP over conjugate exponential families (Gaussian, Categorical, Dirichlet).
- [x] ~~Deep Kernel Learning in `tensorlogic-sklears-kernels`~~ — `deep_kernel/` module: MLP feature extractor composed with a base kernel (Wilson et al., 2016); Xavier init via SciRS2-Core RNG.
- [x] ~~Speculative decoding in `tensorlogic-trustformers`~~ — `speculative_decoding/` module: DraftModel + TargetModel traits with rejection-sampling acceptance (Leviathan et al., 2023); empirical distribution preservation chi-square-verified.
- [x] ~~Partial error recovery in `tensorlogic-compiler`~~ — `error_recovery/` module: TolerantCompiler with configurable RecoveryStrategy; compiles well-formed expressions around non-fatal errors instead of aborting.
- [x] ~~OxiCUDA GPU backend scaffold in `tensorlogic-oxicuda-backend`~~ — new crate, feature-gated (`gpu`), MVP matmul via `oxicuda-blas`. Supersedes the "blocked on SciRS2 GPU stabilization" status (OxiCUDA is the COOLJAPAN Pure Rust CUDA replacement: ~/work/oxicuda/).
- [x] ~~Expanded VMP family catalogue in `tensorlogic-quantrs-hooks`~~ — `vmp/gamma.rs` and `vmp/beta.rs`: Gamma-Poisson and Beta-Bernoulli conjugate families with ExponentialFamily trait, closed-form KL, and posterior update helpers.
- [x] ~~Kernel PCA in `tensorlogic-sklears-kernels`~~ — `kernel_pca/` module: Scholkopf-Smola-Muller (1998) implementation with double-centering, scirs2-linalg eigendecomp, fit/transform API generic over Kernel + Clone + 'static.
- [x] ~~Numerical MoE layer in `tensorlogic-trustformers`~~ — `moe/` research-preview submodules: TopKGate, LinearExpert, MoELayer with capacity-factor dropping and importance/load auxiliary losses.

#### Audited complete 2026-04-15 (pre-existing implementations)

- [x] ~~Incremental re-compilation in `tensorlogic-compiler`~~ — `incremental.rs` (`IncrementalCompiler`, `ChangeDetector`, `ChangeSet`, `IncrementalStats`) + `cache.rs` hash-based `CompileCache`.
- [x] ~~Streaming inference API in `tensorlogic-infer`~~ — `streaming.rs` (`TlStreamingExecutor` trait, `StreamingConfig`, `StreamProcessor`, `ChunkIterator`, `StreamingConfigV2` with backpressure + watermarks).

#### Delivered 2026-04-16

- [x] ~~Real SGEMM wiring in `tensorlogic-oxicuda-backend`~~ — `GpuState` (Context + BlasHandle), host↔device copies via `oxicuda-memory`, `gemm_api::gemm` SGEMM dispatch, stream sync, `DimensionOverflow` / `From<CudaError>` / `From<BlasError>` error conversions. GPU integration test gated behind `TENSORLOGIC_GPU_TESTS=1`.
- [x] ~~LoRA adapter in `tensorlogic-train`~~ — `lora/` module: Low-Rank Adaptation (Hu et al., 2021) with ΔW=BA decomposition, merge/unmerge, effective_weight, dropout, compression ratio. `LoraAdapter` multi-layer manager with summary statistics. 12 unit tests + 2 integration tests.
- [x] ~~Sparse attention in `tensorlogic-trustformers`~~ — `sparse_attention/` module: Longformer-style (Beltagy et al., 2020) sliding-window + global-token attention. Dense boolean mask with sparsity metrics, multi-head forward pass, causal masking support. 12 unit tests + 4 integration tests.

#### Remaining

All initial v0.2.0 research-preview items for in-repo work are delivered. Next enhancements tracked per-crate: see each `crates/*/TODO.md`.

## Per-crate TODOs

- [crates/tensorlogic/TODO.md](./crates/tensorlogic/TODO.md) — Umbrella / flagship meta-crate.
- [crates/tensorlogic-ir/TODO.md](./crates/tensorlogic-ir/TODO.md) — IR and DSL.
- [crates/tensorlogic-infer/TODO.md](./crates/tensorlogic-infer/TODO.md) — Executor traits + autodiff scaffolding.
- [crates/tensorlogic-compiler/TODO.md](./crates/tensorlogic-compiler/TODO.md) — Logic → einsum compilation.
- [crates/tensorlogic-adapters/TODO.md](./crates/tensorlogic-adapters/TODO.md) — External data + codegen adapters.
- [crates/tensorlogic-scirs-backend/TODO.md](./crates/tensorlogic-scirs-backend/TODO.md) — SciRS2 execution backend.
- [crates/tensorlogic-train/TODO.md](./crates/tensorlogic-train/TODO.md) — Training loop + loss / optimizer / scheduler.
- [crates/tensorlogic-oxirs-bridge/TODO.md](./crates/tensorlogic-oxirs-bridge/TODO.md) — OxiRS / RDF / SPARQL bridge.
- [crates/tensorlogic-quantrs-hooks/TODO.md](./crates/tensorlogic-quantrs-hooks/TODO.md) — QuantRS2 probabilistic hooks.
- [crates/tensorlogic-sklears-kernels/TODO.md](./crates/tensorlogic-sklears-kernels/TODO.md) — Kernel methods.
- [crates/tensorlogic-trustformers/TODO.md](./crates/tensorlogic-trustformers/TODO.md) — Transformer adapters.
- [crates/tensorlogic-oxicuda-backend/TODO.md](./crates/tensorlogic-oxicuda-backend/TODO.md) — OxiCUDA GPU backend (Pure Rust CUDA).
- [crates/tensorlogic-cli/TODO.md](./crates/tensorlogic-cli/TODO.md) — CLI.
- [crates/tensorlogic-py/TODO.md](./crates/tensorlogic-py/TODO.md) — Python bindings.

## References

- [README.md](./README.md) — Project overview.
- [CHANGELOG.md](./CHANGELOG.md) — Release history.
- [CLAUDE.md](./CLAUDE.md) — Project conventions.
