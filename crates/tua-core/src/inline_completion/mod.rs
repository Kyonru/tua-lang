//! Local, deterministic inline completion.
//!
//! Phase 0 of `better-autocompletion.md` provides the cursor-eligibility rules
//! and an offline evaluator that decides, from measurements, which candidate
//! sources ship and in what order.

pub mod eligibility;
#[cfg(any(test, feature = "test-fixture"))]
pub mod eval;

#[cfg(test)]
mod tests;
