# tensorlogic-oxicuda-rng — TODO

**Status**: Alpha | **Version**: 0.1.0 | **Last Updated**: 2026-04-27

## Completed

- [x] `RngEngine` with `Cpu` / `Gpu` variants
- [x] `uniform_f32` — uniform [0,1) via PCG-XSH-RR
- [x] `normal_f32` — Gaussian via Box-Muller
- [x] `bernoulli` — 0/1 Bernoulli samples
- [x] `RngEngineKind::as_str()` for display
- [x] `is_gpu()` query
- [x] 47 passing tests
- [x] Pure-Rust CPU path (default features)
- [x] GPU stub path (feature-gated)

## Planned

- [ ] `uniform_f64` / `normal_f64` — f64 variants
- [ ] GPU path wired to real `oxicuda-rand` kernel (currently stubbed)
- [ ] Streaming API for large buffers
- [ ] `RngEngine: Send + Sync` for parallel workloads (currently `Send + !Sync`)
