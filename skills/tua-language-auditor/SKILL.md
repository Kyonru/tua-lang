---
name: tua-language-auditor
description: Audit Tua parser, checker, inference, lowering, emitted Lua, module behavior, and diagnostic correctness against the documented language contract. Use for repository-wide or subsystem-focused language correctness and compatibility audits, not a routine review of one patch.
---

# Tua Language Auditor

Find semantic mismatches that can miscompile code, accept invalid programs,
reject valid programs, or make editor results disagree with builds.

## Contract And Oracles

- Treat `docs/specification.md` and focused language docs as the stated
  contract, but verify claims against implementation and tests.
- Read `docs/architecture.md` for implementation invariants. Use `ROADMAP.md`
  to prioritize findings, never as evidence of current behavior.
- Treat emitted Lua behavior under the supported Lua/LuaJIT and LOVE targets as
  the runtime oracle where practical. Record the runtime and version used,
  validate Lua 5.1 syntax independently, and label target-specific behavior
  unverified when LuaJIT or LOVE is unavailable.
- Distinguish a parser error, type-system limitation, diagnostic quality issue,
  lowering defect, emitter defect, and unsupported feature.
- Check `.tua` strictness without weakening ordinary `.lua` gradual behavior.

## High-Risk Boundaries

- precedence, associativity, recovery, source spans, and malformed input;
- scopes, shadowing, control-flow narrowing, mutation, and closures;
- tables, metatables, classes, generics, unions, optional values, `any`, and
  multi-return semantics;
- function arity, colon/dot calls, overloads, callbacks, and module exports;
- runtime and type module behavior across `require`, static imports,
  re-exports, declaration libraries, cycles, root precedence, and mixed Tua/Lua
  projects;
- lowering order, generated helpers, evaluation count, source maps, and readable
  Lua output;
- statement-layout variants such as semicolons, inline blocks, comments, and
  line-ending differences for every source-text lowering;
- recursion, cycle, and complexity guards on both sides of their thresholds;
- snapshot/VFS behavior versus legacy helpers that read or cache disk state;
- agreement among diagnostics, hover, completion, signature help, rename, and
  build results when they consume the same semantic facts.

Use focused examples and existing public entry points before inventing a new
harness. For suspected miscompilation, preserve a minimal Tua input, emitted
Lua, expected result, and actual result. Never infer runtime equivalence from
textual output alone.

Inspect automation to confirm that relevant tests gate trusted integration and
releases. Tua intentionally avoids GitHub Actions for open or untrusted pull
requests; do not recommend enabling those runs without a separate security
design. A passing local suite with no trusted gate remains an audit finding.

Record the worktree and test baseline before attributing a failure to the
audited subsystem. Preserve unrelated changes and report pre-existing failures
separately.

## Findings

Report file and line evidence, minimal reproduction, contract or oracle,
severity, and the missing regression test. Label unsupported syntax or a
documented limitation separately from a defect. Prefer root-cause findings in
shared semantic infrastructure over repeated surface symptoms.
