# Changelog

All notable changes to TensorLogic will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Longformer-style sparse attention** (tensorlogic-trustformers): numerical sliding-window + global-token attention per Beltagy et al. (2020). `LongformerConfig` with window_size / global_token_indices / num_heads / head_dim / causal / dropout, `LongformerMask` dense boolean mask with `build_mask()` and sparsity metrics, `LongformerAttention` multi-head forward pass with pre-built or on-the-fly mask, `SparseAttentionError` with thiserror bridging. Original graph-building types renamed to `SparseAttentionGraph` / `SparseAttentionGraphConfig`. 12 unit tests + 4 integration tests.
- **LoRA adapter** (tensorlogic-train): Low-Rank Adaptation (Hu et al., 2021) for parameter-efficient fine-tuning. `LoraLayer` with A/B decomposition, merge/unmerge, effective_weight, dropout, compression ratio. `LoraAdapter` multi-layer manager with summary statistics. `LoraConfig` (rank, alpha, dropout, target_modules, seed). `LoraError` with InvalidRank, DimensionMismatch, MergeError, FrozenWeights. 12 unit tests + 2 integration tests.
- **`tensorlogic-oxicuda-backend` SGEMM wiring**: Real GPU matmul via `oxicuda-blas` SGEMM. `GpuState` manages `Arc<Context>` + `Stream` + `BlasHandle`; `matmul_ij_jk_ik` performs host->device copy, `gemm_api::gemm` dispatch, stream sync, device->host readback. Safe `u32` dimension casts with `DimensionOverflow` error. `From<CudaError>` and `From<BlasError>` conversions. Integration test (`tests/gpu_sgemm.rs`) gated behind `TENSORLOGIC_GPU_TESTS=1`.
- **`tensorlogic-oxicuda-backend`** (new crate): Feature-gated OxiCUDA GPU backend exposing `OxiCudaExecutor` (implements `TlExecutor`). Default build is pure Rust with no GPU dependency; `--features gpu` pulls in `oxicuda-backend` + `oxicuda-blas` from crates.io. MVP supports 2D matmul einsum (`ij,jk->ik`); other ops return `OxiCudaBackendError::Unsupported`. Runtime requires only the NVIDIA driver — no CUDA SDK / nvcc / C/C++ toolchain. Supersedes the prior "blocked on SciRS2 GPU stabilization" status.
- Partial error recovery (tensorlogic-compiler): TolerantCompiler with Diagnostic / DiagnosticCollector and configurable RecoveryStrategy (SkipOnError / SkipOnFatal / AbortOnAny); compiles well-formed expressions around non-fatal errors instead of aborting the whole program.
- Speculative decoding (tensorlogic-trustformers): model-level speculative decoding with DraftModel + TargetModel traits, rejection sampling with adjusted resampling, empirical-distribution-preserving acceptance — implements Leviathan et al. (2023).
- Rule-guided sampling decoder (tensorlogic-trustformers): TLExpr-constrained beam search with hard-masking and soft-penalty modes, integrating tensorlogic-infer::BeamSearchDecoder.
- Learned kernel composition (tensorlogic-sklears-kernels): differentiable mixture over a kernel library with trainable softmax-gated weights, gradient ∂K_mix/∂w_i = p_i·(K_i - K_mix).
- Deep Kernel Learning (tensorlogic-sklears-kernels): DKL wraps a base Kernel with an MLP feature extractor K_DKL(x,y) = K_base(g_θ(x), g_θ(y)); Xavier init via SciRS2-Core RNG; ReLU/Tanh/Identity activations.
- Variational Message Passing (tensorlogic-quantrs-hooks v0.2.0 research preview): coordinate-ascent VMP engine for conjugate-exponential families with Gaussian (mean-unknown, precision-known), Categorical, and Dirichlet distributions, four factor relationships (GaussianObservation, GaussianStep, DirichletCategorical, CategoricalObservation), closed-form KL helpers, monotone ELBO with divergence detection, and optional FactorGraph validation via `VariationalMessagePassing::with_graph`. Local ln_gamma/digamma keeps the module scirs2-special free. 30 new tests covering single-variable conjugacy, Gaussian chains, Dirichlet-Categorical counts, ELBO monotonicity, and BayesianNetwork integration. Reference: Winn & Bishop (2005), JMLR 6, 661-694.
- Expanded VMP family catalogue (tensorlogic-quantrs-hooks): Gamma (shape-rate) and Beta (alpha-beta) distributions with ExponentialFamily trait, closed-form KL divergences, natural-parameter round-trips, Gamma-Poisson and Beta-Bernoulli conjugate posterior updates, and Gamma-Poisson / Beta-Bernoulli end-to-end integration tests. 17 new unit tests + 2 integration tests.
- Kernel PCA (tensorlogic-sklears-kernels): full Scholkopf-Smola-Muller (1998) implementation with double-centering, scirs2-linalg eigh-based eigendecomposition, fit/transform/fit_transform API generic over any Kernel + Clone + 'static, explained-variance-ratio, out-of-sample projection via centered test-kernel path. 8 unit tests + 2 integration tests (Swiss-roll RBF embedding, collinear linear-kernel dominance).
- Numerical Mixture-of-Experts layer (tensorlogic-trustformers): research-preview MoELayer with TopKGate (xavier_init / from_weights), LinearExpert, top-k softmax gating, Switch-Transformer capacity-factor dropping, importance_loss / load_loss / combined_aux_loss auxiliary losses, and BatchGatingStats. 12 unit tests + 2 integration tests (4-expert batch-32 routing, capacity-factor overflow).

#### Neurosymbolic AI Enhancements (2026-01-02)
- **ToRSh tensor interoperability** (pure Rust PyTorch alternative)
  - Bidirectional conversion: TensorLogic ↔ ToRSh (f32/f64)
  - torsh_interop.rs module (462 lines, 7 tests, 100% passing)
  - Feature-gated: `--features torsh` (optional dependency)
  - Lossless roundtrip for f64 precision
  - Device validation (CPU enforced, GPU future-ready)
- **Advanced neurosymbolic AI examples** (557 lines total)
  - knowledge_graph_reasoning.rs (267 lines, 4 scenarios)
    - Hybrid logic-neural reasoning for knowledge completion
    - Demonstrates transitivity, symmetry rules with neural embeddings
    - Configurable α-weighted hybrid scoring
    - Constraint validation via bidirectional conversion
  - constrained_neural_optimization.rs (290 lines, 6 parts)
    - Enforces logical constraints on neural network outputs
    - Mutual exclusivity and hierarchical rules
    - Automatic violation detection and guided correction
    - Constraint loss computation for gradient-based training
  - torsh_integration.rs (150 lines, 4 scenarios) - Basic interop demo
- **Fuzzing infrastructure** (cargo-fuzz ready, requires nightly)
  - tensorlogic-ir fuzz targets (318 lines total)
  - fuzz_tlexpr: TLExpr construction and serialization robustness
  - fuzz_einsum_graph: EinsumGraph operations testing
  - fuzz_optimizations: Optimization pass correctness verification
  - Independent workspace configuration
- **ToRSh interoperability benchmarks** (249 lines, 8 groups)
  - Bidirectional conversion benchmarks (TL ↔ ToRSh)
  - Type conversion benchmarks (f32 ↔ f64)
  - Roundtrip conversion performance
  - Matrix conversion (2D tensors, 10×10 to 200×200)
  - Hybrid workflow benchmark (realistic neurosymbolic AI scenario)
  - Performance measurements across multiple sizes (10-10,000 elements)
  - Statistical analysis with criterion

#### Dependency Upgrades
- **SkleaRS upgraded to 0.1.0-beta.1** (from alpha.2)
  - sklears-core: 0.1.0-alpha.2 → 0.1.0-beta.1 (from crates.io)
  - sklears-kernel-approximation: 0.1.0-alpha.2 → 0.1.0-beta.1 (from crates.io)
  - Eliminated local path dependencies
  - All 4,363 tests passing with new dependencies

#### Policy Compliance
- **Oxicode migration** (COOLJAPAN policy compliance)
  - Replaced bincode with oxicode 0.1.1
  - Updated 4 serialization points in tensorlogic-py
  - Zero breaking changes via serde compatibility layer

### Changed
- **Test count**: 4,287 → 4,363 tests (+76 new tests)
  - ToRSh interop tests: 7 tests (100% passing)
  - Property tests: Existing 700-line proptest suite
  - 100% pass rate maintained across all additions
- **Examples count**: 15 → 17 Rust examples (+2 neurosymbolic AI)
- **Benchmark suites**: 9 → 10 files (+1 ToRSh interop benchmarks)
- **Code quality**: Zero deprecated warnings (fixed into_raw_vec usage)
- **Documentation**: Enhanced README with neurosymbolic AI section

### Changed — 2026-04-15
- **Binary rename** — `tensorlogic-cli` `[[bin]]` renamed from `tensorlogic` to `tensorlogic-cli` to resolve the doc-build name collision with the meta crate (Cargo issue #6313). README shell-example invocations updated (~68 lines) to match.
- **File splits (v0.1.x refactoring policy)** — Ten oversize source files (1524–1789 lines) refactored into submodule directories with every submodule under 500 lines:
  - `tensorlogic-adapters`: `database.rs` → `database/` (7 files)
  - `tensorlogic-compiler`: `dead_code.rs` → `dead_code/` (11 files), `partial_eval.rs` → `partial_eval/` (11 files), `symbolic_diff.rs` → `symbolic_diff/` (10 files)
  - `tensorlogic-infer`: `causal.rs` → `causal/` (6 files)
  - `tensorlogic-ir`: `resolution.rs` → `resolution/` (9 files)
  - `tensorlogic-oxirs-bridge`: `sparql_gen.rs` → `sparql_gen/` (8 files)
  - `tensorlogic-quantrs-hooks`: `loopy_bp.rs` → `loopy_bp/` (9 files)
  - `tensorlogic-train`: `hyperparameter.rs` → `hyperparameter/` (9 files), `loss.rs` → `loss/` (18 files)
  Public APIs preserved via `pub use` re-exports from each new `mod.rs`. All 6,407 tests still pass.

### Fixed — 2026-04-15
- Eliminated one `.unwrap()` in `loopy_bp` and one fragile indexed-`HashMap` access during the `loopy_bp` split (no-unwrap policy).

### Status
- **4,363/4,363 tests passing (100%)** - Comprehensive coverage
- **Zero compiler warnings, zero clippy warnings**
- **Production-ready neurosymbolic AI workflows**
- **Pure Rust ecosystem compliance** (no C++ dependencies by default)

### Planned
- GPU backend support
- Additional fuzzy logic variants
- Execute fuzzing on nightly Rust
- Reference comparisons against symbolic logic solvers

## [0.1.0] - 2026-04-27

### Changed - Stable Release

#### Version Bump
- **Version bump from 0.1.0-rc.1 to 0.1.0** across all workspace crates
  - Workspace version updated in root Cargo.toml
  - All 10 internal crate references updated to 0.1.0 stable (tensorlogic-ir, tensorlogic-adapters, tensorlogic-infer, tensorlogic-compiler, tensorlogic-scirs-backend, tensorlogic-train, tensorlogic-oxirs-bridge, tensorlogic-sklears-kernels, tensorlogic-quantrs-hooks, tensorlogic-trustformers)
  - tensorlogic-py version aligned to 0.1.0

#### Dependency Upgrades
- **SciRS2 ecosystem**: 0.3.0 → 0.3.4 (scirs2-core, scirs2-linalg, scirs2-autograd, scirs2-optimize)
- **SkleaRS ecosystem**: 0.1.0-rc.1 → 0.1.0 stable (sklears-core, sklears-kernel-approximation)
- **ToRSh ecosystem**: 0.1.0 → 0.1.1 (torsh-core, torsh-tensor)
- **OxiRS ecosystem**: 0.1.0 → 0.2.2 (oxirs-core, oxirs-gql, oxirs-ttl) — major API upgrade
- **oxicode**: 0.1.1 → 0.2 (serialization/codec library)
- **clap**: 4.5 → 4.6
- **clap_complete**: 4.5 → 4.6
- **assert_cmd**: 2.1 → 2.2
- **rand**: Added `rand_09` (rand 0.9) as explicit workspace alias for backward-compat crates

#### Dependency Fixes
- **oxirs-core**: Fixed RngExt trait import compatibility with scirs2-core 0.3.2
- **tensorlogic-sklears-kernels**: Fixed `StdRng` type mismatch with `sklears-kernel-approximation` — migrated from `rand_09`/`rand_distr_05` to `scirs2_core::rand_prelude`/`scirs2_core::rand_distributions` (rand 0.10 alignment, sklears-kernel-approximation uses scirs2-core's rand 0.10)
- **tensorlogic-train**: Removed unused Rng imports

#### Code Quality
- Fixed all clippy warnings (sort_by → sort_by_key, collapsible match guards)
- Zero warnings policy enforced across all crates
- 6397 tests passing (100% success rate)
- Eliminated 7 `unwrap()` calls in `sklears_integration.rs` `sample_frequencies` implementations — replaced with `expect("Normal distribution parameters must be valid")` per no-unwrap policy

## [0.1.0-rc.1] - 2026-03-06

### Changed - Release Candidate 1

#### Version Bump
- **Version bump from 0.1.0-beta.1 to 0.1.0-rc.1** across all workspace crates
  - Workspace version updated in root Cargo.toml
  - All internal crate references updated (tensorlogic-ir, tensorlogic-adapters, tensorlogic-infer, tensorlogic-compiler, tensorlogic-scirs-backend, tensorlogic-train, tensorlogic-oxirs-bridge, tensorlogic-sklears-kernels, tensorlogic-quantrs-hooks, tensorlogic-trustformers)
  - tensorlogic-py version aligned to 0.1.0-rc.1
  - Version strings updated in lib.rs doc comments (tensorlogic, tensorlogic-train, tensorlogic-trustformers)

#### Dependency Upgrades
- **SciRS2 ecosystem**: 0.1.3 -> 0.3.0 (scirs2-core, scirs2-linalg, scirs2-autograd, scirs2-optimize)
- **SkleaRS ecosystem**: 0.1.0-beta.1 -> 0.1.0-rc.1 (sklears-core, sklears-kernel-approximation)
- **ToRSh ecosystem**: 0.1.0-beta.1 -> 0.1.0 (torsh-core, torsh-tensor)
- **rand**: 0.9 -> 0.10
- **toml**: 0.9 -> 1.0
- **tokio**: 1.49 -> 1.50
- **oxrdf**: 0.3.2 -> 0.3.3
- **oxttl**: 0.2.2 -> 0.2.3
- **tempfile**: 3.24 -> 3.26

### Fixed
- **rand 0.10 compatibility**: Changed `rand::Rng` to `rand::RngExt` in learned_opt.rs for rand 0.10 API
- **Doc test in torsh_interop.rs**: Changed `no_run` to `ignore` to prevent doc test build failures

## [0.1.0-beta.1] - 2026-01-28

### Added - Beta.1 Release

#### Production Release
- **First Beta Release** - All alpha.2 features stabilized
- **4,415 tests passing** (100% pass rate)
- **Zero warnings** across all build configurations
- **Complete documentation** and examples
- **Production-ready** for real-world use

## [0.1.0-alpha.2] - 2025-12-16

### Added - Alpha.2 Release

#### CUDA/GPU Infrastructure (Experimental)
- **Device management infrastructure** (device.rs)
  - DeviceType enum (CPU, CUDA, Metal, Vulkan, ROCm)
  - Device abstraction with multi-device support
  - DeviceManager for device discovery and management
  - Future-ready for GPU backend implementation via scirs2
- **Benchmark enhancements** for GPU profiling
  - Updated all benchmark suites with device metrics
  - Preparation for GPU performance comparisons

#### Comprehensive Benchmark Suite
- **memory_footprint benchmark** (149 lines, 3 groups)
  - Memory allocation patterns for simple/matrix/complex expressions
  - Size scaling analysis (100 to 10,000 elements)
- **gradient_stability benchmark** (207 lines, 5 groups)
  - Gradient computation performance measurement
  - Simple ops, nested ops, matrix ops, quantifiers, complex expressions
  - Numerical stability testing
- **throughput benchmark** (235 lines, 5 groups)
  - Operations per second measurement
  - Element-wise, matrix, reduction, complex, and batch operations
  - Throughput tracking with Criterion
- **Fixed simd_comparison benchmark** (rewritten, 203 lines)
  - Migrated to compiler API for maintainability
  - 5 benchmark groups for SIMD comparison
- **Complete benchmark coverage**: 24 groups across 5 suites (991 total lines)

#### Packaging Infrastructure
- **PACKAGING.md** (500+ lines comprehensive guide)
  - Complete Maturin packaging documentation
  - Development setup and workflow
  - Cross-platform build instructions (Linux/macOS/Windows)
  - PyPI publishing guide (TestPyPI → PyPI)
  - CI/CD integration (GitHub Actions + GitLab CI)
  - Troubleshooting section (6 common issues)
  - Advanced optimization topics
- **GitHub Actions workflow template** (280+ lines)
  - Multi-platform wheel builds (Linux x86_64/aarch64, macOS, Windows)
  - Python version matrix (3.9-3.12)
  - Automated testing and publishing
  - SIMD builds support
- **Makefile for Python development** (100+ lines)
  - 15 common tasks automated
  - Development, building, testing, publishing targets
- **Comprehensive README.md** (500+ lines)
  - Modern documentation with badges
  - Complete feature overview
  - Quick start guides (Rust + Python)
  - Architecture diagrams
  - Performance benchmarks
  - Project status table

### Changed
- Phase 3 (SciRS2 Backend): 95% → **100% Production Ready**
- Phase 7 (Python Bindings): 95% → **98% Production Ready**
- All benchmarks now use compiler API (consistent, maintainable)

### Status
- **4,287/4,287 tests passing (100%)** - Significant test coverage expansion
- **12 tests intentionally skipped** (strategy-specific edge cases)
- **Zero warnings, zero errors**
- **Complete benchmark infrastructure** (24 groups across 5 suites)
- **Production-ready packaging**
- **272,370+ lines of Rust code** (216,811 source + 32,749 docs)

## [0.1.0-alpha.0] - 2025-11-04

### Added - Session 3

#### SIMD Acceleration Support
- **Feature flag configuration** in tensorlogic-scirs-backend
  - `simd` feature passes through to scirs2-core/scirs2-linalg
  - Transparent SIMD acceleration (2-4x speedup)
  - No code changes required
- **Capability detection** infrastructure
  - `SIMDAcceleration` feature enum
  - Runtime capability queries
  - Backend reporting to Python bindings
- **Python backend selection** (backend.rs - 480+ lines)
  - `Backend.SciRS2SIMD` enum variant
  - Smart default selection (prefers SIMD when available)
  - 4 backend query functions
  - Backend capabilities API
- **SIMD benchmark suite** (simd_comparison.rs - 450+ lines)
  - 5 benchmark groups (element-wise, reduction, matrix, logical, einsum)
  - Comprehensive SIMD vs non-SIMD comparison

#### Backend Selection API
- **PyBackend enum** (Auto, SciRS2CPU, SciRS2SIMD, SciRS2GPU)
- **PyBackendCapabilities class** with full queries
- **Backend functions**:
  - `get_backend_capabilities()` - Query backend features
  - `list_available_backends()` - List all backends
  - `get_default_backend()` - Get system default
  - `get_system_info()` - Comprehensive system info
- **Comprehensive test suite** (test_backend.py - 380+ lines, 30+ tests)
- **Type stubs** updated with backend types
- **Python example** (backend_selection.py - 280+ lines)

### Changed
- Phase 3: 80% → **95% Enhanced**
- Phase 7: 90% → **95% Production Ready**

### Status
- **783/783 tests passing (100%)**
- **SIMD acceleration functional**
- **Backend selection complete**

## [0.1.0-dev.2] - 2025-11-04

### Added - Session 2

#### Integration Tests
- **End-to-end integration tests** (end_to_end.rs - 428 lines, 18 tests)
  - Basic logical operations with execution
  - Complex nested expressions
  - Multi-arity predicates
  - Strategy comparison tests
  - Graph structure validation
  - Constant tensor handling

#### Compilation Benchmarks
- **Compilation performance benchmarks** (compilation_performance.rs - 410+ lines)
  - Simple expression benchmarks
  - Complex expression benchmarks
  - Quantifier benchmarks
  - Strategy comparison benchmarks
  - Multi-arity predicate benchmarks
  - Criterion-based infrastructure

### Changed
- Phase 8: 85% → **100% Complete**

## [0.1.0-dev.1] - 2025-11-03

### Added - Session 1

#### Python Bindings Enhancements
- **Test Suite Enhancement**
  - test_adapters.py (350+ lines) - Adapter type tests
  - test_strategies.py (470+ lines) - Strategy & property tests
  - pytest.ini configuration
  - requirements-dev.txt for dependencies
  - pyproject.toml with project metadata
- **Type Stubs** (.pyi files)
  - Complete type annotations for tensorlogic_py
  - IDE support (autocomplete, type checking)
  - mypy configuration
- **Tutorial Notebooks**
  - 01_getting_started.ipynb (800+ lines) - Beginner tutorial
  - 02_advanced_topics.ipynb (900+ lines) - Advanced tutorial
  - tutorials/README.md - Complete guide

### Changed
- Phase 7: 60% → **90% Production Ready**

## [0.1.0-dev.0] - 2025-11-03

### Added - Initial Development Phase

#### Core Infrastructure (Phases 0-2)
- **Repository Hygiene** (Phase 0)
  - LICENSE (Apache-2.0), CODEOWNERS, CONTRIBUTING.md, SECURITY.md
  - Documentation skeleton (DSL.md, IR.md, PROVENANCE.md)
  - CI configuration (fmt, clippy, tests)
- **IR & Compiler** (Phase 1)
  - `tensorlogic-ir`: AST and IR types (Term, TLExpr, EinsumGraph)
  - `tensorlogic-compiler`: Logic → tensor mapping with static analysis
  - Logic operation defaults (AND→Hadamard, OR→max, NOT→1-x, etc.)
- **Engine Traits** (Phase 2)
  - `tensorlogic-infer`: TlExecutor/TlAutodiff traits
  - Dummy executor for testing
  - Examples: 00_minimal_rule, 01_exists_reduce, 02_scirs2_execution

#### SciRS2 Backend (Phase 3)
- **Runtime Executor** (tensorlogic-scirs-backend)
  - TlExecutor trait implementation
  - TlAutodiff::forward() with full EinsumGraph execution
  - TlAutodiff::backward() with gradient computation
  - All OpType variants support (Einsum, ElemUnary, ElemBinary, Reduce)
  - Features: `cpu` (default), `simd`, `gpu` (future)
  - Integration tests: end-to-end TLExpr → Execution
  - Backward pass tests for autodiff
  - Modular structure (executor, conversion, ops, autodiff)
- **Forward pass benchmark** (forward_pass.rs - 197 lines)
  - 6 benchmark groups
  - Simple predicate, AND/OR operations, quantifiers, complex expressions

#### Core Enhancements (Phase 4.5)
- **Type System** (tensorlogic-ir)
  - Term::Typed with TypeAnnotation
  - PredicateSignature with arity/type validation
  - SignatureRegistry for predicate metadata
  - Enhanced error types
- **Graph Optimizations** (tensorlogic-ir)
  - Dead Code Elimination (DCE) with liveness analysis
  - Common Subexpression Elimination (CSE)
  - Identity operation simplification
  - Multi-pass optimization pipeline
- **Metadata & Provenance** (tensorlogic-ir)
  - SourceLocation and SourceSpan for error reporting
  - Provenance tracking (rule IDs, source files, attributes)
  - Metadata container for IR nodes
- **Compiler Enhancements** (tensorlogic-compiler)
  - Variable scope analysis
  - Type checking & inference
  - Expression-level CSE
  - SymbolTable integration
  - Enhanced diagnostics
- **Execution Enhancements** (tensorlogic-infer)
  - Batch execution
  - Shape inference
  - Backend capabilities
  - Execution profiling

#### OxiRS Bridge (Phase 4)
- **Schema Integration** (tensorlogic-oxirs-bridge)
  - Symbol tables from RDF* schema analysis
  - SchemaAnalyzer for extracting classes and properties
  - Provenance tracking infrastructure
  - SHACL constraint parser (Turtle format)
  - SHACL → TLExpr conversion
  - Support for sh:minCount, sh:maxCount, sh:class, sh:datatype, sh:pattern

#### Interop Crates (Phase 5)
- **SkleaRS Kernels** (tensorlogic-sklears-kernels)
  - Rule similarity kernel
  - Predicate overlap kernel
  - Tensor kernels (Linear, RBF, Polynomial, Cosine)
  - 24 comprehensive tests
- **QuantrS2 Hooks** (tensorlogic-quantrs-hooks)
  - Factor representation with normalization
  - Factor graph with adjacency tracking
  - Message passing algorithms (sum-product, max-product)
  - TLExpr → Factor graph conversion
  - 15 comprehensive tests
- **TrustformeRS** (tensorlogic-trustformers)
  - Self-attention as einsum operations
  - Multi-head attention
  - Feed-forward networks (standard + gated GLU)
  - Position encodings (sinusoidal, learned, relative, RoPE, ALiBi)
  - Layer normalization (LayerNorm + RMSNorm)
  - Transformer encoder/decoder layers
  - Rule-based attention patterns
  - Sparse attention patterns
  - Extended model presets (GPT-2/3, LLaMA, BLOOM, T5)
  - 123 comprehensive tests

#### Training Scaffolds (Phase 6)
- **Training Infrastructure** (tensorlogic-train)
  - Loss functions (cross-entropy, MSE, rule satisfaction, constraint violations)
  - Optimizers (SGD, Adam, AdamW with gradient clipping)
  - Learning rate schedulers (Step, Exponential, Cosine, Warmup)
  - Batch management (iterator, shuffling, stratified sampling)
  - Training loop (Trainer with epoch/batch iteration)
  - Callbacks (early stopping, checkpointing, LR plateau reduction)
  - Metrics (accuracy, precision, recall, F1 score)
  - 28 unit tests

#### Python Bindings (Phase 7)
- **Core Type Bindings** (types.rs - 331 lines)
  - PyTerm: Variables and constants
  - PyTLExpr: Full logical expression API (13 operations)
  - PyEinsumGraph: Compiled tensor graphs
- **Compilation API** (compiler.rs - 153 lines)
  - compile(expr) - Default compilation
  - compile_with_config(expr, config) - Custom strategies
  - 6 compilation strategy presets
- **Execution API** (executor.rs - 72 lines)
  - execute(graph, inputs) - NumPy array execution
  - Dynamic tensor shape handling
- **NumPy Integration** (numpy_conversion.rs - 63 lines)
  - Bidirectional conversion (NumPy ↔ SciRS2)
  - Safe memory management
- **Adapter Bindings** (adapters.rs)
  - PySymbolTable, PyCompilerContext
  - PyDomainInfo, PyPredicateInfo
- **Examples**: 5 Rust examples + 10 Python examples
- **Test Coverage**: 30 Rust tests

#### Validation & Scale (Phase 8)
- **Property Tests** (property_tests.rs - 900+ lines, 21/21 passing)
  - CompilationConfig integration
  - Strategy mapping module (180+ lines)
  - 17 core property tests (symmetry, associativity, monotonicity, etc.)
  - 4 strategy-specific tests
- **CompilationConfig Integration**
  - Updated logic operations to use config strategies
  - Added Min/Max element-wise operations to backend
  - Optimized Product AND with einsum fusion
  - Support for 26+ compilation strategies

### Testing
- **783 tests** across all crates
- **100% pass rate**
- **Zero warnings** in release builds
- Coverage: unit tests, integration tests, property tests, Python tests

### Documentation
- Complete project guide (CLAUDE.md)
- SciRS2 integration policy
- Security policy
- Contributing guidelines
- Tutorial notebooks
- 15+ examples (Rust + Python)
