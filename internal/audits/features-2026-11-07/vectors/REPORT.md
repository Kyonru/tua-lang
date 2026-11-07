# Vectors and game math audit

Bounded audit of candidate 0.6.0 at baseline revision 9d490dc. Four new confirmed defects: two medium, two low. Ordinary vector and helper workflows pass in Lua 5.4.8 and LOVE's bundled LuaJIT 2.1.1700008891 with JIT off (table vectors) and on (FFI/cdata). No product changes.

## Reproduction and coverage

From repository root:

```sh
python3 internal/audits/features-2026-11-07/vectors/probe.py
python3 internal/audits/features-2026-11-07/vectors/luajit-probe.py
node internal/audits/features-2026-11-07/vectors/editor-probe.js
```

`probe.py` runs absolute `target/release/tua build` inside each fixture, then `/opt/homebrew/bin/lua` against generated Lua. The Lua harness supplies an empty `love.math` namespace; actual LOVE functions are not needed. `luajit-probe.py` uses the unmodified `/Applications/LOVE.app/Contents/Frameworks/Lua.framework/Lua` C API with fresh states per case/mode. LuaJIT harness `code=0` means the harness executed; inspect its inner `result` for fixture success/failure. All fixture sources and output are preserved.

| Surface | Status and evidence |
| --- | --- |
| Constructor, fields, methods, split multi-return | Exercised; positive passes on all three runtime configurations |
| Fused addition/scaling, unary negation, vector product, literal division | Exercised; emitted `positive/build/main.lua` shows component fusion, assertions pass |
| Normalize zero; length and angle writes; floor/ceil/copy, dot/trim/clamp/hadamard/angled/rotated | Exercised; ordinary values pass |
| All game helpers, round omitted/negative/positive places, overshoot, LOVE rgba | Exercised; positive assertions pass |
| Zero division | Exercised; runtime rejects as documented |
| Extreme finite vectors, alternate helper syntax | Exercised; findings below |
| Hover, completion, signature help, diagnostics | Actual stdio LSP; normalized is Vec2, length and split second return are number; vector members offered; round optional parameter and activeParameter=1 correct; no diagnostics. `editor-results.json` |
| FFI/table parity | Exercised through bundled LuaJIT library; matching outcomes |
| Full LOVE application | Blocked: headless executable exits signal 6 before Lua output in this environment; `love-results.json`. Library fallback succeeds; no conclusion about app graphics integration |
| Override runtime preservation; module-fed fusion; getter read order; helper shadowing | Existing implementation/tests inspected, not newly exercised |
| NaN, infinity, degenerate ranges | Representative probes only; no exhaustive IEEE-754 matrix |

Existing tests inspected: `crates/tua-cli/tests/cli.rs:1898` fused runtime comparison; `:2042` hoisted read tests; `:2732` runtime bundling; `:2844` custom runtime preservation; `crates/tua-core/src/game_math.rs:270` helper detection tests; `crates/tua-core/src/emit/tests.rs:1107` constructor snapshots. No full suite rerun. ROADMAP and prior audit reports searched for these symptoms; none of the four were found documented there. Their `known:false` means not found in that search, not proof they have never been reported.

## VECTORS-01 — Chained constructor property reads escape lowering

**Medium; confirmed defect; new.** `chained_constructor/src/main.tua`:

```tua
local n = vec2(3,4).normalized
print(n.x,n.y)
```

Build succeeds. Expected `(0.6,0.8)` with bundled BrineVector; generated code preserves `vec2` and fails `attempt to call ... 'vec2' (a nil value)` in Lua and both LuaJIT modes. Clean fixture bundles only main.lua. Evidence: source/output plus `cli-results.json` and `luajit-results.json`.

Owner: `crates/tua-core/src/emit/builtin_calls.rs:79`, visitor implements FunctionCall handling; the expression ending in a field uses a different AST form and escapes it. Smallest improvement: share lexical builtin-prefix lowering across call-bearing AST variants while preserving shadowing and suffixes. Existing constructor snapshots cover plain calls, not this property suffix. Add build/run regressions for property/index chains plus shadowed constructors; consider other builtin helpers sharing this lowerer, outside this audit's scope.

## VECTORS-02 — Helper injection misses valid member spellings

**Medium; confirmed defect; new.** `comment_math/src/main.tua` uses `math. -- helper comment` then newline `clamp(20,0,10)`; `bracket_math/src/main.tua` uses `math["clamp"](20,0,10)`. Both build successfully but no helper initializer is emitted. Expected 10; actual nil `clamp` call on all three runtime configurations.

Owner: `crates/tua-core/src/game_math.rs:127-174`; `helper_label_after`/`dot_member` inspect raw source, recognize only dots and whitespace, and ignore valid comments/static string indexing. Existing tests cover ordinary dotted references and lexical shadowing. Use AST member access plus existing scope resolution to discover helper requirements consistently, retaining lazy emission and shadow behavior. Regressions: comments on either side of the dot, literal bracket keys, and shadowed namespace equivalents. Runtime smoke test must run in a fresh math namespace so another fixture does not mask missing injection.

## VECTORS-03 — Tiny nonzero divisors are rejected

**Low; confirmed defect; new.** `underflow_div/src/main.tua` divides `vec2(1e-200,1e-200)` by another such vector. Expected `(1,1)`; actual `Cannot divide a vector by a vector with a zero component` on every tested runtime. All input components and expected quotient are finite and representable.

Owner: `vendor/runtime/brinevector.lua:249`: checking `op.x * op.y == 0` conflates arithmetic underflow with a zero component. Compare each component directly with zero. Existing fusion runtime tests use ordinary magnitudes and do not expose this runtime guard. Add runtime cases for tiny nonzero components, actual zero in either axis, and mixed-sign nonzero components, in table/FFI modes.

## VECTORS-04 — Normalization loses finite extreme vectors

**Low; confirmed defect; new.** `normalization/src/main.tua` stores `(1e200,0)` and `(1e-200,0)` in locals, then reads `.normalized` (avoiding VECTORS-01). Both become `(0,0)` with length zero in every tested runtime; the documented normalized result for either nonzero axis-aligned input is `(1,0)`.

Owner: `vendor/runtime/brinevector.lua:70-81`: naive squared-length computation overflows or underflows before normalization. Smallest maintainable improvement: scaled norm computation with an explicit zero case, reused by length and normalization. Add table/FFI regressions for finite huge/tiny axis-aligned vectors and ordinary `(3,4)`/zero values. Current ordinary-value regression fixtures miss these magnitudes.

## Known baseline and open numerical policy

`known_nominal/src/main.tua` confirms `local p: Vec2 = {x=1,y=2}` builds, contrary to docs/vectors.md's nominal guarantee. This is the explicitly known core nominal-assignment test failure in BASELINE.md; excluded from new counts/findings.json. Retain the existing failing regression and fix through shared nominal assignment semantics.

`edge_math` records equal-endpoint inverseLerp => NaN; equal-endpoint smoothstep => 1 in Lua 5.4 versus NaN in LuaJIT; reverse clamp => 10; `round(1,400)` => NaN. Docs specify neither valid endpoint ordering nor finite decimal-place bounds. These are design/contract questions rather than additional defects: decide whether to document preconditions or consistently reject/handle invalid ranges and unsupported precision. Colors are converted without clamping, matching documented conversion. No separate configuration enables these helpers; they are discovered per reference, which is why VECTORS-02 matters.
