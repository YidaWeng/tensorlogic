//! Integration tests for the VMP research preview.
//!
//! Layer VMP on top of the existing `BayesianNetwork` / `FactorGraph` API.
//! The `FactorGraph` is only used here for *structural* bookkeeping — VMP
//! operates entirely in continuous natural-parameter space via `VmpConfig`.
//! The integration test therefore confirms that:
//!
//! 1. A user can declare variables through the BayesianNetwork builder and then
//!    attach a matching `VmpConfig` that passes `with_graph` validation.
//! 2. End-to-end the run converges and the posterior matches the analytical
//!    Gaussian-conjugate solution to high precision.

use scirs2_core::ndarray::ArrayD;
use scirs2_core::random::{RngExt, SeedableRng, StdRng};

use tensorlogic_quantrs_hooks::vmp::beta as vmp_beta;
use tensorlogic_quantrs_hooks::vmp::gamma as vmp_gamma;
use tensorlogic_quantrs_hooks::{
    BayesianNetwork, BetaNP, GammaNP, VariationalMessagePassing, VariationalState, VmpConfig,
    VmpFactor,
};

#[test]
fn vmp_on_bayesian_network_structure() {
    // Declare a single continuous latent mean "mu" through the Bayesian
    // Network builder (cardinality is irrelevant for VMP — the BN is used
    // purely for its FactorGraph payload). Attach a Gaussian observation and
    // check that VMP run() converges to the analytical posterior.
    let mut bn = BayesianNetwork::new();
    bn.add_variable("mu".to_string(), 1);
    // Stub CPD so the FactorGraph carries at least one factor (not used by VMP).
    let dummy_cpd = ArrayD::from_shape_vec(vec![1], vec![1.0]).expect("cpd");
    bn.add_cpd("mu".to_string(), vec![], dummy_cpd)
        .expect("cpd");

    // Build the VMP annotation side-by-side.
    let config = VmpConfig::new()
        .with_gaussian("mu", 0.0, 1.0)
        .expect("register mu")
        .with_factor(VmpFactor::GaussianObservation {
            target: "mu".to_string(),
            observation: 4.0,
            precision: 3.0,
        })
        .with_limits(100, 1e-10);

    let mut engine = VariationalMessagePassing::with_graph(bn.graph(), config).expect("engine");
    let result = engine.run().expect("run");
    assert!(result.converged);

    // Analytical conjugate posterior:
    //   τ_post = τ_prior + τ_obs = 1 + 3 = 4
    //   μ_post = (τ_obs · y) / τ_post = 12 / 4 = 3
    match result.states.get("mu").expect("mu") {
        VariationalState::Gaussian { q, .. } => {
            assert!((q.mean - 3.0).abs() < 1e-9, "posterior mean = {}", q.mean);
            assert!(
                (q.precision - 4.0).abs() < 1e-9,
                "posterior precision = {}",
                q.precision
            );
        }
        _ => panic!("expected Gaussian"),
    }

    // ELBO history must be non-empty and non-decreasing.
    assert!(!result.elbo_history.is_empty());
    for window in result.elbo_history.windows(2) {
        assert!(
            window[1] + 1e-7 >= window[0],
            "ELBO decreased: {} -> {}",
            window[0],
            window[1]
        );
    }
}

#[test]
fn vmp_rejects_variables_missing_from_graph() {
    // If the user registers a variable in VmpConfig but forgets to add it to
    // the underlying FactorGraph, `with_graph` must fail loudly.
    let bn = BayesianNetwork::new();
    let config = VmpConfig::new()
        .with_gaussian("missing", 0.0, 1.0)
        .expect("register missing");
    let result = VariationalMessagePassing::with_graph(bn.graph(), config);
    assert!(result.is_err(), "missing variable must be rejected");
}

#[test]
fn vmp_dirichlet_categorical_conjugate_integration() {
    // BN with a categorical child "x" and Dirichlet parent "pi".
    // We only use the BN for variable declarations; VMP drives inference.
    let mut bn = BayesianNetwork::new();
    bn.add_variable("pi".to_string(), 3); // Dirichlet prior
    bn.add_variable("x".to_string(), 3); // Categorical child

    let config = VmpConfig::new()
        .with_dirichlet("pi", vec![1.0, 1.0, 1.0])
        .expect("dir")
        .with_categorical("x", 3)
        .expect("cat")
        .with_factor(VmpFactor::DirichletCategorical {
            dirichlet: "pi".to_string(),
            categorical: "x".to_string(),
        })
        .with_factor(VmpFactor::CategoricalObservation {
            dirichlet: "pi".to_string(),
            observation: 1,
            num_categories: 3,
        })
        .with_factor(VmpFactor::CategoricalObservation {
            dirichlet: "pi".to_string(),
            observation: 1,
            num_categories: 3,
        })
        .with_limits(100, 1e-8);

    let mut engine = VariationalMessagePassing::with_graph(bn.graph(), config).expect("engine");
    let result = engine.run().expect("run");
    assert!(result.converged);

    // The model combines a latent Categorical "x" tied to "pi" via
    // DirichletCategorical, plus two CategoricalObservation factors at index 1.
    // At the VMP fixed point pi's posterior concentration receives contributions
    // from (a) the prior α = [1,1,1], (b) the two direct observations at index 1,
    // and (c) q(x)'s expected sufficient statistics (one virtual count spread
    // over categories according to the posterior of x). The exact α vector is
    // the solution of a fixed-point equation involving digamma and has no clean
    // closed form — so we assert structural properties instead.
    match result.states.get("pi").expect("pi") {
        VariationalState::Dirichlet { q, .. } => {
            let alpha = &q.concentration;
            assert_eq!(alpha.len(), 3);
            for &a in alpha {
                assert!(a > 0.0, "concentrations must be strictly positive");
            }
            // Symmetry: categories 0 and 2 never see an observation and sit
            // symmetrically under the model, so their concentrations must
            // agree to high precision.
            assert!(
                (alpha[0] - alpha[2]).abs() < 1e-8,
                "α[0] ({}) should equal α[2] ({}) by symmetry",
                alpha[0],
                alpha[2]
            );
            // Dominance: the two observations at index 1 must pull α[1]
            // strictly above the unobserved categories.
            assert!(
                alpha[1] > alpha[0],
                "α[1] ({}) must dominate α[0] ({})",
                alpha[1],
                alpha[0]
            );
            assert!(
                alpha[1] > alpha[2],
                "α[1] ({}) must dominate α[2] ({})",
                alpha[1],
                alpha[2]
            );
            // Sum constraint: α_sum = prior(3) + observations(2) + q(x) total
            // mass(1) = 6, regardless of how q(x) distributes across categories.
            let alpha_sum: f64 = alpha.iter().sum();
            assert!(
                (alpha_sum - 6.0).abs() < 1e-8,
                "Σα = {} (expected 6 = 3 prior + 2 obs + 1 latent)",
                alpha_sum
            );
        }
        _ => panic!("expected Dirichlet"),
    }
}

/// Simulate a Poisson(lambda) draw using Knuth's algorithm with the
/// given RNG. Deterministic for a fixed seed.
fn sample_poisson(lambda: f64, rng: &mut StdRng) -> u64 {
    let l = (-lambda).exp();
    let mut k: u64 = 0;
    let mut p: f64 = 1.0;
    loop {
        k += 1;
        let u: f64 = rng.random();
        p *= u;
        if p <= l {
            return k - 1;
        }
    }
}

#[test]
fn vmp_gamma_poisson_end_to_end() {
    // 100 Poisson(λ = 2.5) counts with a Gamma(1, 1) prior.
    // The conjugate posterior is Gamma(1 + Σy, 1 + N) and its mean
    // (α/β) should be close to the true rate 2.5.
    let true_lambda = 2.5_f64;
    let n = 100_usize;
    let mut rng = StdRng::seed_from_u64(42);
    let observations: Vec<u64> = (0..n)
        .map(|_| sample_poisson(true_lambda, &mut rng))
        .collect();

    let prior = GammaNP::new(1.0, 1.0).expect("prior");
    let posterior =
        vmp_gamma::posterior_from_prior_and_observations(&prior, &observations).expect("posterior");

    let posterior_mean = posterior.alpha / posterior.beta;
    assert!(
        (posterior_mean - true_lambda).abs() < 0.3,
        "Gamma-Poisson posterior mean {:.4} should be within 0.3 of true λ = {}",
        posterior_mean,
        true_lambda
    );
    // Posterior shape = 1 + sum(obs), rate = 1 + N.
    let sum: u64 = observations.iter().sum();
    assert!(
        (posterior.alpha - (1.0 + sum as f64)).abs() < 1e-12,
        "posterior alpha = {}, expected {}",
        posterior.alpha,
        1.0 + sum as f64
    );
    assert!(
        (posterior.beta - (1.0 + n as f64)).abs() < 1e-12,
        "posterior beta = {}, expected {}",
        posterior.beta,
        1.0 + n as f64
    );
    // KL(posterior || prior) must be strictly positive (they differ).
    let kl = posterior.kl_to(&prior);
    assert!(kl > 0.0, "KL(posterior || prior) = {}", kl);
}

#[test]
fn vmp_beta_bernoulli_end_to_end() {
    // 500 Bernoulli(p = 0.7) draws with a Beta(1, 1) prior.
    // The conjugate posterior is Beta(1 + n_s, 1 + n_f) and its mean
    // α/(α+β) should be close to 0.7. With n=500 the posterior
    // standard deviation is ~0.02 so a 0.05 tolerance is ample.
    let true_p = 0.7_f64;
    let n = 500_usize;
    let mut rng = StdRng::seed_from_u64(99);
    let mut successes: u64 = 0;
    let mut failures: u64 = 0;
    for _ in 0..n {
        let u: f64 = rng.random();
        if u < true_p {
            successes += 1;
        } else {
            failures += 1;
        }
    }

    let prior = BetaNP::new(1.0, 1.0).expect("prior");
    let posterior = vmp_beta::posterior_from_prior_and_observations(&prior, successes, failures)
        .expect("posterior");

    let posterior_mean = posterior.alpha / (posterior.alpha + posterior.beta);
    assert!(
        (posterior_mean - true_p).abs() < 0.05,
        "Beta-Bernoulli posterior mean {:.4} should be within 0.05 of true p = {}",
        posterior_mean,
        true_p
    );
    // Posterior shape: α = 1 + n_s, β = 1 + n_f.
    assert!(
        (posterior.alpha - (1.0 + successes as f64)).abs() < 1e-12,
        "posterior alpha = {}, expected {}",
        posterior.alpha,
        1.0 + successes as f64
    );
    assert!(
        (posterior.beta - (1.0 + failures as f64)).abs() < 1e-12,
        "posterior beta = {}, expected {}",
        posterior.beta,
        1.0 + failures as f64
    );
    // N = successes + failures = 500.
    assert_eq!(successes + failures, n as u64);
    // KL(posterior || prior) must be strictly positive.
    let kl = posterior.kl_to(&prior);
    assert!(kl > 0.0, "KL(posterior || prior) = {}", kl);
}
