# M6 rich runtime integration

Implemented on `feature/tdialogue-rich-text`; not merged or published. This slice
connects typed schema-8 documents to player presentations and reveal timing. The
`.tdialogue` opt-in directive, compiler emission of rich fields, catalog markup
analysis and script-editor assistance remain pending; M6 is not complete.

## Contracts

Typed line/choice fields use `richText`; disabled reasons use `richReason`.
The corresponding plain fallback template must match exactly after stripping marks
and encoding literal percents. Raw templates are compiled and owned once during
construction. A serialized token IR is not exposed as a second public input format.
The Rust parser retains future compiler/tooling facts; a differential Lua/Rust
corpus checks accepted/rejected inputs, stripped output and token source ranges.

`resolveTemplate` returns unexpanded text and optional locale/plural metadata.
Selection, parsing, substitution and final grapheme validation happen before a
player transition or refresh commits. Substituted values cannot add marks or
commands. Legacy resolvers still return opaque final strings. Rich selections are
owned; caller/catalog mutation cannot modify an active presentation. Rich fields
without a resolver use scalar args or typed context bindings.

Resolved presentations expose plain text, grapheme-indexed style/rate runs,
pronunciation hints, ordered pauses and template-to-output byte origins. Empty
substitutions retain zero-length origins; a paired mark emptied by substitution
is rejected. Marks must align with final grapheme boundaries, including across
placeholder boundaries. Dynamic template origins do not fabricate source files.

Reveal validates and owns timing before replacing its view. It builds a bounded
cumulative deadline schedule once per presentation and binary-searches visibility
on updates. Plain snapshots retain the prior constant-rate path. Leading and
trailing pauses, restored nested rates, leftover frame time, instant finish,
reduced motion, large finite dt and stale timing identity are covered. The host
continues to report actual completion to anchor revealed events. Snapshot
`revealComplete` remains the existing forced-completion hint used on refreshes;
ordinary `update(..., true)` anchors events without changing that flag.

## Verification scope

The typed example is `examples/dialogue-rich`. Its actual emitted runtimes exercise
styles, origins, literal markup-looking values, localized callbacks, timing,
reduced motion and typed notifications. The maintained Lua oracle additionally
covers event IDs/expressions, choices, context refresh rollback, invalid UTF-8,
empty marked substitutions, oversized output, invalid nesting, nested rates,
reentrancy, cancel/dispose, extreme finite updates and owned snapshots.

A shared-core analysis fixture checks imported result types, field completion/hover,
invalid field assignments and invalid template callback return types. Examples use
an explicitly typed imported document and narrow `node.presentation` into a local;
those forms preserve current checker/editor inference. Broader deep optional-path
narrowing is not changed by this feature.

The formatter's long function-type alias wrapping lost a space on the first pass.
A separate fix and one-pass regression were committed as `59bb6750`; all formatter
tests passed. The integration must be formatted with a compiler containing that fix.

## SYSL-Text inspiration

The user's reference, [SYSL-Text](https://github.com/sysl-dev/SYSL-Text), demonstrates
per-textbox defaults, animated styles, inline images and character sounds. M10 now
records these as inspiration for the optional LÖVE presenter. Named style keys can
map to host effects; images require an explicit layout/plain-text fallback design.
Executable inline function tags are not part of Tua rich templates. This change
copies no SYSL-Text source or assets and adds no renderer dependency.

## Final validation

- The initial `make verify` passed the CLI/build-cache suites, then stopped at an
  old event-identity fixture that classified schema 8 as unsupported. The fixture
  now explicitly accepts schema 8 and rejects the next unsupported version.
- Re-ran the complete core and LSP suites: 2,145 core unit tests (one ignored),
  21 core integration tests and 508 LSP tests passed. The new emitted harness was
  rerun after simplifying a single-element loop flagged by Clippy.
- All remaining verification targets passed: workspace Clippy, version/config/model
  drift checks, release and AI tooling tests, model reference/negative evaluations,
  feature-example checks, extension checks, JSON and diff checks.
- Explicit Tua/Lua formatting checks, Rust formatting, strict documentation build,
  and the rich example check/build passed. The existing localization example and
  its maintained rich-template oracle passed with the final emitted runtimes.
- Final headless native verification passed in LÖVE 11.5 / LuaJIT 2.1.1700008891
  with JIT both enabled and disabled; standalone Lua 5.4.8 passed too. No installed
  editor UI, rendered typography or audio playback is claimed.

Final compiler: Tua 0.11.0, aarch64-apple-darwin debug, dirty base
`59bb6750a8467de2471395b625ed74318feee7c1`; executable SHA-256
`086a5288827f277dc570e79aa159f20e52175a470968273b8c83a02dd428aa52`.

Scheduling retains one deadline per grapheme (bounded by resolved text size).
Visibility lookup is logarithmic; unchanged-identity timing comparison is linear
in run/pause count. No CPU/allocation improvement is claimed without measurement.
