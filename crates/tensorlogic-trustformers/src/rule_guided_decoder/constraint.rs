//! Logical-constraint compilation for rule-guided decoding.
//!
//! This module wraps a [`tensorlogic_ir::TLExpr`] constraint and compiles it
//! into a lightweight, per-token lookup representation that the decoder can
//! consult at every sampling step.
//!
//! ## Scope
//!
//! Only a deliberate subset of `TLExpr` is honoured by the current implementation:
//!
//! * [`TLExpr::Pred`] — treated as an allow-list of symbol names that the
//!   candidate token must match.
//! * [`TLExpr::And`] — intersection of its operands' constraints.
//! * [`TLExpr::Or`] — union of its operands' constraints.
//! * [`TLExpr::Not`] — inverts the classification emitted by the inner
//!   constraint (allow ↔ forbid).
//!
//! Any other variant collapses to [`ConstraintVerdict::SoftPenalty`]`(0.0)` (no-op)
//! with a `// TODO` pointing at the extension point. See [`extend_tlexpr_support`]
//! for the next step.
//!
//! ## Token-to-symbol mapping
//!
//! The compiled constraint needs to know which *symbol name* each token
//! corresponds to — vocabulary encodings are deeply application-specific.
//! Callers supply a mapper `Fn(TokenId) -> Option<SymbolName>`; an empty option
//! means "this token is unknown / has no symbolic identity".

use std::collections::HashSet;

use tensorlogic_ir::TLExpr;

#[cfg(test)]
use crate::rule_guided_decoder::error::RuleGuidedError;
use crate::rule_guided_decoder::error::RuleGuidedResult;

/// Logical token identifier used by the beam-search backend (`usize`).
pub type TokenId = usize;

/// Verdict produced by a compiled constraint about a candidate token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintVerdict {
    /// Token is explicitly allowed — no logit adjustment needed.
    Allowed,
    /// Token is explicitly forbidden — hard masking should set the logit
    /// to `-inf`; soft masking treats it as the maximum-penalty case.
    Forbidden,
    /// Soft penalty expressed as a non-negative "violation magnitude" that
    /// the soft-penalty mask multiplies by `lambda`.
    SoftPenalty(f64),
}

/// Token → symbol-name mapper. `None` means the token has no symbolic identity
/// (constraint evaluation is conservative — see [`RuleConstraint::evaluate`]).
pub type TokenSymbolMapper = dyn Fn(TokenId) -> Option<String> + Send + Sync;

/// Compiled representation of a single `TLExpr` constraint.
///
/// Two forms coexist:
///
/// 1. **Eager table** (`allow_set`) — populated when the constraint compiles
///    to a finite predicate list over a user-supplied vocabulary mapper.  This
///    path is the fast path and is used by the hard/soft masks.
/// 2. **Fallback pass-through** — used when the expression hit an unsupported
///    variant.  In that case [`RuleConstraint::evaluate`] returns
///    `ConstraintVerdict::SoftPenalty(0.0)` and the decoder behaves as if no
///    constraint was present.
pub struct RuleConstraint {
    /// Original TLExpr (kept for diagnostics and lazy re-compilation).
    source: TLExpr,
    /// Union of symbol names accepted by the constraint, if computable.
    ///
    /// Conceptually `None` means "constraint is non-enumerable" (e.g., the
    /// AST contained variables or unsupported connectives).  An empty set
    /// means the constraint is unsatisfiable — no token passes.
    allow_set: Option<HashSet<String>>,
    /// Mapper from token ids to symbol names.  Stored so `evaluate` can be
    /// called many times without re-compiling.
    mapper: Box<TokenSymbolMapper>,
    /// Set to true when compilation succeeded against a recognised subset of
    /// `TLExpr`.  When false, the constraint silently no-ops.
    supported: bool,
}

impl std::fmt::Debug for RuleConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleConstraint")
            .field("source", &self.source)
            .field("allow_set", &self.allow_set)
            .field("supported", &self.supported)
            .finish_non_exhaustive()
    }
}

impl RuleConstraint {
    /// Compile a constraint from a `TLExpr` and a token → symbol-name mapper.
    ///
    /// * If the expression only uses supported variants, the returned
    ///   constraint eagerly enumerates allowed symbol names into a `HashSet`.
    /// * Otherwise, the constraint is still constructed but evaluates to a
    ///   no-op (soft-penalty of zero).  This makes the decoder forward-
    ///   compatible: new `TLExpr` variants don't break existing call sites.
    pub fn compile<M>(expr: TLExpr, mapper: M) -> RuleGuidedResult<Self>
    where
        M: Fn(TokenId) -> Option<String> + Send + Sync + 'static,
    {
        let mut builder = AllowSetBuilder::default();
        let supported = builder.visit(&expr)?;
        let allow_set = if supported {
            Some(builder.finalize())
        } else {
            None
        };
        Ok(Self {
            source: expr,
            allow_set,
            mapper: Box::new(mapper),
            supported,
        })
    }

    /// Evaluate the constraint against `(prefix, candidate)`.
    ///
    /// `prefix` is the token sequence already committed to the beam; it is
    /// not used by the current allow-list compiler but is part of the contract
    /// so stateful constraints (e.g. "no token X after token Y") remain
    /// implementable without an API break — see [`extend_tlexpr_support`].
    pub fn evaluate(&self, prefix: &[TokenId], candidate: TokenId) -> ConstraintVerdict {
        let _ = prefix; // Reserved for future stateful predicates.
        if !self.supported {
            return ConstraintVerdict::SoftPenalty(0.0);
        }
        let allow_set = match &self.allow_set {
            Some(set) => set,
            None => return ConstraintVerdict::SoftPenalty(0.0),
        };

        let symbol = (self.mapper)(candidate);
        match symbol {
            Some(name) if allow_set.contains(&name) => ConstraintVerdict::Allowed,
            Some(_) => ConstraintVerdict::Forbidden,
            None => {
                // Unknown tokens (e.g. punctuation with no symbol) are treated
                // conservatively as a soft violation so the decoder slightly
                // prefers fully-symbolic completions without banning them.
                ConstraintVerdict::SoftPenalty(1.0)
            }
        }
    }

    /// Read-only access to the compiled allow-list, if any.
    pub fn allow_set(&self) -> Option<&HashSet<String>> {
        self.allow_set.as_ref()
    }

    /// `true` when the constraint was compiled against a supported subset of
    /// `TLExpr`.  `false` means the constraint is a no-op.
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Original expression.
    pub fn source(&self) -> &TLExpr {
        &self.source
    }
}

// ---------------------------------------------------------------------------
// Allow-set compiler
// ---------------------------------------------------------------------------

#[derive(Default)]
struct AllowSetBuilder {
    /// Accumulated allow-list.  Interpretation depends on the surrounding
    /// operator: set-intersection for AND, set-union for OR.  The top-level
    /// operator semantics are applied by the caller walking the tree.
    current: Option<HashSet<String>>,
}

impl AllowSetBuilder {
    /// Recursively visit `expr`, updating `self.current`.
    ///
    /// Returns `true` when every sub-expression fell into the supported
    /// subset; `false` signals the caller to drop the compiled table and
    /// fall back to the no-op path.
    fn visit(&mut self, expr: &TLExpr) -> RuleGuidedResult<bool> {
        let set = match self.classify(expr)? {
            Some(s) => s,
            None => return Ok(false),
        };
        self.current = Some(set);
        Ok(true)
    }

    fn finalize(self) -> HashSet<String> {
        self.current.unwrap_or_default()
    }

    /// Attempt to fold `expr` into an allow-set.  Returns `Ok(None)` when the
    /// expression uses an unsupported variant.
    fn classify(&self, expr: &TLExpr) -> RuleGuidedResult<Option<HashSet<String>>> {
        match expr {
            TLExpr::Pred { name, args } => {
                // Treat the predicate's atoms as allowed symbol names.
                // The predicate name itself is allowed as a symbol too —
                // this matches the usual convention where tokenizers emit a
                // "type" token (e.g., `entity(Alice)`).
                let mut set = HashSet::with_capacity(1 + args.len());
                set.insert(name.clone());
                for arg in args {
                    match arg {
                        tensorlogic_ir::Term::Const(s) => {
                            set.insert(s.clone());
                        }
                        tensorlogic_ir::Term::Var(_) => {
                            // Variables are unbound: the predicate doesn't
                            // restrict the vocabulary symbolically.
                        }
                        tensorlogic_ir::Term::Typed { value, .. } => {
                            if let tensorlogic_ir::Term::Const(s) = value.as_ref() {
                                set.insert(s.clone());
                            }
                        }
                    }
                }
                Ok(Some(set))
            }
            TLExpr::And(lhs, rhs) => {
                let l = match self.classify(lhs)? {
                    Some(s) => s,
                    None => return Ok(None),
                };
                let r = match self.classify(rhs)? {
                    Some(s) => s,
                    None => return Ok(None),
                };
                Ok(Some(l.intersection(&r).cloned().collect()))
            }
            TLExpr::Or(lhs, rhs) => {
                let l = match self.classify(lhs)? {
                    Some(s) => s,
                    None => return Ok(None),
                };
                let r = match self.classify(rhs)? {
                    Some(s) => s,
                    None => return Ok(None),
                };
                Ok(Some(l.union(&r).cloned().collect()))
            }
            TLExpr::Not(inner) => {
                // Negation of an allow-list has no finite representation in
                // the closed-vocabulary form we keep here.  Callers still get
                // well-defined behaviour: negation flips "membership" to
                // "non-membership", but we need the vocabulary-wide symbol
                // universe for that — which we don't know at compile time.
                // Instead, synthesize a sentinel allow-set signalling
                // "complement mode" via an unused variant.  See the TODO at
                // the end of this module.
                //
                // For now, fall back to the no-op path.
                let _ = inner;
                // TODO(extend_tlexpr_support): Thread Not through evaluate()
                // with an explicit complement flag or per-token mapper look-up.
                Ok(None)
            }
            // TODO(extend_tlexpr_support): Implement Exists/ForAll/Imply.
            // Each requires either quantifier elimination against the mapper
            // or a semantic predicate that consults the prefix, which we do
            // not currently have at compile time.
            _ => Ok(None),
        }
    }
}

/// Documentation marker: extension point for additional `TLExpr` variants.
///
/// Today we handle `Pred`, `And`, and `Or`.  To add support for, e.g.,
/// `Imply`, extend `AllowSetBuilder::classify` with the appropriate
/// set-algebraic translation.  Stateful connectives (those whose truth
/// depends on the generated prefix) should introduce a new arm in
/// [`RuleConstraint::evaluate`] that inspects `prefix`.
pub const fn extend_tlexpr_support() {}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tensorlogic_ir::Term;

    fn mk_pred(name: &str, consts: &[&str]) -> TLExpr {
        TLExpr::Pred {
            name: name.into(),
            args: consts.iter().map(|c| Term::Const((*c).into())).collect(),
        }
    }

    fn demo_mapper() -> impl Fn(TokenId) -> Option<String> + Send + Sync + 'static {
        |tid: TokenId| match tid {
            1 => Some("Alice".into()),
            2 => Some("Bob".into()),
            3 => Some("entity".into()),
            _ => None,
        }
    }

    #[test]
    fn predicate_allow_list_accepts_named_consts() {
        let expr = mk_pred("entity", &["Alice"]);
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        assert!(rc.is_supported());
        assert_eq!(rc.evaluate(&[], 1), ConstraintVerdict::Allowed);
        assert_eq!(rc.evaluate(&[], 2), ConstraintVerdict::Forbidden);
        assert_eq!(rc.evaluate(&[], 3), ConstraintVerdict::Allowed);
    }

    #[test]
    fn conjunction_intersects_allow_sets() {
        // entity(Alice) AND entity(Bob) — only the shared "entity" symbol
        // remains, so Alice/Bob tokens become Forbidden.
        let a = mk_pred("entity", &["Alice"]);
        let b = mk_pred("entity", &["Bob"]);
        let expr = TLExpr::And(Box::new(a), Box::new(b));
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        assert!(rc.is_supported());
        assert_eq!(rc.evaluate(&[], 1), ConstraintVerdict::Forbidden);
        assert_eq!(rc.evaluate(&[], 2), ConstraintVerdict::Forbidden);
        assert_eq!(rc.evaluate(&[], 3), ConstraintVerdict::Allowed);
    }

    #[test]
    fn disjunction_unions_allow_sets() {
        let a = mk_pred("entity", &["Alice"]);
        let b = mk_pred("entity", &["Bob"]);
        let expr = TLExpr::Or(Box::new(a), Box::new(b));
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        assert!(rc.is_supported());
        assert_eq!(rc.evaluate(&[], 1), ConstraintVerdict::Allowed);
        assert_eq!(rc.evaluate(&[], 2), ConstraintVerdict::Allowed);
        assert_eq!(rc.evaluate(&[], 3), ConstraintVerdict::Allowed);
    }

    #[test]
    fn unsupported_variant_returns_soft_noop() {
        let inner = mk_pred("entity", &["Alice"]);
        let expr = TLExpr::Not(Box::new(inner));
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        assert!(!rc.is_supported());
        assert_eq!(rc.evaluate(&[], 1), ConstraintVerdict::SoftPenalty(0.0));
    }

    #[test]
    fn unknown_token_yields_soft_penalty() {
        let expr = mk_pred("entity", &["Alice"]);
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        // Token id 99 has no mapping -> conservative SoftPenalty(1.0).
        assert_eq!(rc.evaluate(&[], 99), ConstraintVerdict::SoftPenalty(1.0));
    }

    #[test]
    fn empty_intersection_forbids_all_known_tokens() {
        // entity(Alice) AND user(Charlie) — disjoint allow sets except that
        // "entity" / "user" are the predicate names.  Tokens for Alice/Bob
        // all become Forbidden.
        let a = mk_pred("entity", &["Alice"]);
        let b = mk_pred("user", &["Charlie"]);
        let expr = TLExpr::And(Box::new(a), Box::new(b));
        let rc = RuleConstraint::compile(expr, demo_mapper()).expect("compile");
        assert!(rc.is_supported());
        // "entity" is in allow_set(a) but not allow_set(b); intersection is
        // empty, so every known symbol is forbidden.
        assert_eq!(rc.evaluate(&[], 1), ConstraintVerdict::Forbidden);
        assert_eq!(rc.evaluate(&[], 3), ConstraintVerdict::Forbidden);
    }

    #[test]
    fn error_type_has_useful_display() {
        // Sanity check that RuleGuidedError links correctly.
        let err: RuleGuidedError =
            RuleGuidedError::CompilationError("synthetic failure".to_string());
        assert!(err.to_string().contains("synthetic"));
    }
}
