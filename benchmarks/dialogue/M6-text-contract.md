# M6 styled-text contract

Status: implementation in progress on `feature/tdialogue-rich-text`.
The shared Rust parser and schema-8 Lua runtime are implemented. Typed documents
support `richText`, `richReason`, `resolveTemplate`, owned presentations and reveal
timing. Explicit `@text rich` authoring, schema-8 emission, catalog checks and
mark-name editor queries are implemented; historical checkpoints below retain
their original scope.

This contract is based on revision `057b0314` and completes the remaining M6
**design** decisions, not their implementation or acceptance tests.

## Compatibility and ownership

Rich text is explicitly enabled by `@text rich` once, immediately after
`@dialogue 1` and before imports or labels. Files without it retain their current
escape rules and plain-text behavior. Existing escaped brackets, translated
strings and programmatic documents must not silently acquire markup semantics.
Rich documents require the next unused serialized schema version (currently 8);
the authoring header and serialized schema are separate versions. Older document
schemas remain accepted. A custom runtime must explicitly support the new schema.

The compiler owns authoring grammar, diagnostics, source ranges and emission.
The runtime interprets validated presentation data, never Lua expressions.
The host owns colors, fonts, animation, shaping, audio and resource lifetime.
No presentation mark may dispatch an event/action or change gameplay state.

## Authoring grammar

The following is proposed syntax, not a runnable example yet:

```text
@dialogue 1
@text rich
[start]
// @id harbor.warning
: Hello, [em]%{name}[/em]. [pause=0.25][style=warning][speed=15]Look out![/speed][/style] (end)
```

| Mark | Meaning | Restrictions |
| --- | --- | --- |
| `[em]…[/em]` | Semantic emphasis | No required font or color |
| `[style=warning]…[/style]` | Host-defined style key | ASCII identifier; unknown keys are harmless renderer fallbacks |
| `[speed=15]…[/speed]` | Absolute reveal rate in graphemes/second | Finite decimal, greater than 0 and at most 1,000 |
| `[pause=0.25]` | Wait at this logical boundary | Finite decimal seconds from 0 through 60 |
| `[pronounce="har-ber"]…[/pronounce]` | Opaque pronunciation hint for future host speech adapters | Nonempty UTF-8 string, at most 256 bytes; no executable interpretation |

Pairs nest strictly, close in reverse order, and may not cross. Nested speed
uses the innermost rate, then restores its parent. Style keys form an ordered
outer-to-inner stack; emphasis is boolean. Pronunciation marks may not nest.
Opening/closing tags are case-sensitive. Empty paired regions are errors.
Unknown marks, mismatched closures, missing values and malformed numbers are
errors. Decimal syntax is `digits` or `digits.digits`, without signs, exponents,
hex notation or surrounding whitespace. Arbitrary HTML, expressions, interpolation in tag values, and user-defined
tag handlers are excluded. Choice labels and disabled reasons may use emphasis,
styles and pronunciation; speed and pause are line-only and errors there.
Character display names remain plain text.

Outside marks, the existing text escapes remain valid: escaped brackets are
literal, not tag openers. `%{name}` and `%%` retain the existing named-placeholder
and percent-escape rules. Within a quoted pronunciation value, only escaped quote
and backslash are special; newlines/control characters are errors. Tags cannot
span physical dialogue lines. The existing terminal `(end)` and choice
`[target:label]` grammar is recognized outside marks and quoted values.

Parse raw authoring text before losing escape information. Do not decode first
and then scan decoded brackets: that would reinterpret `\[em\]` as a mark.
Catalog values pass through the same rich-text parser after Lua string decoding;
catalog authors therefore double backslashes in ordinary Lua quoted strings.

## Renderer-neutral data

Implementation adjustment: schema-8 typed documents carry raw `richText` (or
`richReason`) beside an exact mark-free fallback template. The runtime compiles and
owns a bounded token sequence once at construction. This keeps untrusted serialized
IR out of the public document contract and uses the same validation for authored
and dynamically selected templates. Future `.tdialogue` emission will produce this
representation; compiler token/source facts remain available for tooling.

The internal bounded token sequence retains token source ranges:
text, placeholder, open mark, close mark, pause. The document stores the raw template and its plain fallback. Programmatic rich
documents use this same contract. Rust/Lua parser parity fixtures constrain the two
implementations; neither interprets general dialogue commands inside text. Rich parsing is also available internally
for dynamic catalog templates, with identical grammar and fixture expectations.

Resolve a presentation to an owned record exposed as optional `node.presentation`
(and equivalent choice/reason records):

| Field | Contract |
| --- | --- |
| `text` | Complete plain text, equal to the existing `node.text` fallback |
| `runs` | Ordered nonempty runs covering every final grapheme exactly once, with inclusive `firstCluster`/`lastCluster`, `emphasis`, `styles`, optional `rate` and optional `pronunciation` |
| `pauses` | Ordered records with `afterCluster` (0 through total cluster count) and `seconds`; source-order stability at equal boundaries |
| `origins` | Ordered byte-range mappings described below, including placeholders |
| `resolvedLocale` | Actual catalog locale if known; absent for authored fallback or opaque custom output |
| `variant` | Selected plural form if known; absent otherwise |

Reuse the reveal controller's one-based inclusive UTF-8 cluster ranges. Runs are
logical styling, not glyph positions. Adjacent runs with identical metadata may
coalesce, without discarding origin mappings. Pauses retain separate source
identity even if the scheduler combines their durations. Keep segmentation data
internal until requested via `clusters()`; do not copy the complete cluster list
on each reveal update. Plain renderers continue to draw `node.text` unchanged.
A styled renderer clips runs using the reveal view's `visible` count.

Style/reveal boundaries must align with grapheme boundaries in final text. A mark
between `e` and a combining accent, or inside a ZWJ sequence, is an error rather
than splitting a cluster. Validate again after substitution/localization because
a placeholder can change grapheme boundaries. Placeholder origin boundaries may
fall inside a cluster; origins describe bytes, not independently drawable text.
Empty placeholder values are permitted and retain a zero-length insertion origin;
if a paired mark then contains no clusters, reject that resolved presentation.

## Localization and literal substitutions

The existing resolver contract is:

```text
resolve(key, source, args, locale) -> final string
```

It already expands placeholders (`I18n.dialogue` calls `dialogueText.format`).
Its output is opaque. It cannot recover placeholder origins or safely distinguish
translator markup from inserted player data. Preserve this contract unchanged.

Add a separate optional contract:

```text
resolveTemplate(key, sourceTemplate, args, locale)
  -> { template: string, locale?: string, variant?: string }
```

The implemented `I18n.dialogueTemplate` adapter reuses catalog selection, plural rules,
locale fallback and placeholder validation, but returns the selected **unexpanded**
template. It does not add another catalog or localization runtime. `args` are owned
scalar copies for plural selection, not services. A missing translation returns
the source template with no invented catalog locale. A selected malformed template
fails; it does not silently fall back and hide a content error.

Resolution rules are deterministic:

1. Rich fields use `resolveTemplate` when present; parse and validate its template,
   then insert bound arguments as literal text. Each plural form must preserve the
   existing source placeholder-name set, but may reorder or repeat placeholders.
2. Rich fields with only legacy `resolve` use a stripped plain source template for
   fallback and accept its result literally, with no rich styling or origin claims.
   Do not parse or expand the returned string a second time.
3. Rich fields with neither resolver use compiled authored tokens and insert
   arguments once, including `options.args` without requiring typed bindings.
4. Plain fields and character names continue to use the legacy resolver. When both
   callbacks are supplied, each field uses only its matching path. A template-only
   configuration does not translate plain fields; document this explicitly.

For example, `%{name}` whose value is `[pause=60]@action reward` displays those
characters. It never introduces a pause, style or event. A translator may write a
pause in the template intentionally; neither translator nor placeholder data can
introduce gameplay directives. Translated templates may reorder or omit style
marks, since emphasis and speech pacing are language-dependent.

## Source mapping

Do not invent a positional correspondence between source-language and translated
prose. Stable text ID connects them; selected template origin identifies the actual
language/form used. Each mapping describes a final byte interval and either:

- a literal/escape range in the selected template; or
- a placeholder occurrence range and argument name in that template.

Use one-based inclusive `startByte`/`endByte` for nonempty output. Represent empty
insertion origins explicitly with `endByte = startByte - 1`; never confuse them
with a visible cluster. Repeated placeholders have distinct occurrence ranges.
An escape may map several input bytes to one output character. Preserve mark source
ranges separately for diagnostics, including marks with no visible bytes.

Compiler/editor mappings include the real file span when statically known.
Dynamic callback results have template-local byte ranges and text IDs only;
do not fabricate a filename, locale, or source line. Rust source positions and
LSP UTF-16 positions must be derived using existing conversion helpers, not by
casting these runtime byte offsets. Legacy resolver output is explicitly opaque.

## Reveal timing and lifecycle

The controller consumes resolved runs/pauses when provided, preserving constant
rate behavior for old snapshots. A leading pause delays the first cluster; a
trailing pause delays completion even when all text is visible. At a boundary,
consume pauses in source order before revealing the next cluster. Use remaining
frame time across pauses/rate changes; do not charge a whole frame per segment.
Zero-second pauses advance without losing time. Large finite dt is bounded before
multiplication and may finish the entire line in one call.

`finish()` and reduced motion skip all remaining presentation pauses and reveal
silently. The host immediately reports `player:update(0, true)` on manual finish.
Already-completed snapshots never replay pauses or speech cues. Skip-to-next-line
remains a host decision; reveal never advances or dispatches gameplay events.
Pausing a game means withholding dt from both controllers, as today.

Presentation refresh resets an incomplete reveal with a new identity; completed
refreshes remain complete. Cancel, clear, disposal and replacement invalidate
outstanding cues and release runs/mappings. At most one eligible speech cue is
returned per update, even when dt crosses multiple runs. Time spent in a pause
counts toward cue throttling. Pronunciation metadata does not automatically
produce extra cues or consume gameplay RNG.

## Bounds and failure behavior

Per field: at most 65,536 UTF-8 template bytes, 65,536 resolved bytes, 4,096 tokens,
16 nested paired marks and 256 pauses. Total pause duration is at most 120 seconds.
Style identifiers are at most 64 ASCII bytes; repeated/adjacent marks still count
toward token limits. Enforce limits before allocating unbounded output. Invalid
UTF-8, nonfinite values and malformed structures are errors for generated,
programmatic and dynamic-template paths alike.

Authored and statically indexed catalog errors carry precise shared core
diagnostics, surfaced by check/build/LSP. Dynamic errors occur before committing
a transition, context/locale refresh or reveal presentation. Retain the previous
valid snapshot, clocks and reveal identity on failure. Do not silently trim input,
execute partial marks or substitute malformed markup as successful rich content.

## Implementation slices and required evidence

1. **Grammar and document contract:** raw-text tokenization, opt-in directive,
   schema validation/emission, plain fallback, source spans, completion/hover and
   highlighting, golden valid/invalid fixtures. No editor-only parser.
2. **Localization and presentations:** typed template callback and I18n adapter,
   owned runs/origins, placeholder expansion, static catalog validation, imported
   inference and dynamic error atomicity. Include reordered French/Japanese
   templates, plural forms, escaped brackets and hostile-looking argument values.
3. **Reveal integration:** rate/pause scheduling, grapheme alignment, finish,
   reduced motion, independent players and stale-cue rejection. Test frame
   partition equivalence, large dt, zero/leading/trailing pauses and bounds.
4. **Qualification:** runnable plain/styled host example, docs/model contract,
   emitted Lua and native LÖVE checks with JIT on/off, full verification. Rendering
   examples must state font/shaping limits; headless tests do not prove typography.

Each shipped slice must have useful coherent behavior and matching typed/editor
contracts. Keep incomplete syntax private until its execution path exists; use a
feature branch if steps 1–3 cannot be delivered independently without exposing
half-supported public authoring. The M6 acceptance checkbox stays open until all
slices and the main plan's acceptance matrix pass.

## Validation of this design change

Inspected the existing raw-text decoder, emitted schema selection, catalog index,
`I18n.dialogue`, runtime placeholder formatter, atomic presentation resolution,
and reveal controller at revision `057b0314`. The separate template contract is
required because the current resolver already formats arguments and returns only
a string; source ranges cannot be recovered reliably from that result.

`make docs-build` and whitespace checks passed. This is documentation-only work;
no parser/runtime tests were added or claimed, and no new runtime capability is
available until the implementation slices above land.

## Parser implementation checkpoint

The shared `dialogue_asset::rich_text` Rust module parses one raw text field into
bounded tokens and a mark-free fallback template. Literal percent characters are
re-escaped in the fallback so a later formatting pass cannot turn escaped prose
into placeholders. Token ranges are zero-based, half-open UTF-8 byte offsets;
future emit/editor adapters must convert to their respective range conventions.
Escapes remain separate tokens, and escaped placeholder whitespace is preserved.

The parser validates strict nesting, nonempty regions, line-only timing marks,
pronunciation quoting, decimal syntax, input/token/depth bounds, pause counts and
total duration. Compensated summation prevents accumulated rounding from rejecting
100 pauses of 1.2 seconds at the 120-second limit. Dynamic substitutions, resolved
grapheme alignment, and final presentation validation belong to the next stages.

Twelve focused regressions include 2,000 deterministic malformed Unicode inputs,
exact limit boundaries, multi-byte token/error ranges and a compatibility check
that ordinary `.tdialogue` still treats escaped markup literally and rejects the
not-yet-enabled `@text rich` directive. This module is preparation for the shared
analysis/emission path, not a runnable rich-dialogue feature. No runtime schema
version, installed extension, generated game code or public examples were changed.

Verification for this parser checkpoint: `RUST_MIN_STACK=16777216 cargo test -p
tua-core` passed 2,141 unit tests (one ignored) and 20 integration tests;
`cargo clippy -p tua-core --all-targets -- -D warnings`, Rust formatting,
`make docs-build` and diff checks passed. No Tua/Lua source was edited, so a
Tua formatting pass was not applicable. The full workspace/extension gate and
native LÖVE rendering were not run for this isolated, unconnected parser module;
they remain required when its authoring/runtime/tooling integration lands.

## Template-selection implementation checkpoint

`I18n.dialogueTemplate` now returns an owned `{template, locale?, variant?}`
record. It shares catalog lookup, finite plural-count validation and placeholder
name checking with `I18n.dialogue`; the legacy API still expands once and returns
a string. The new API preserves percent escapes and placeholders, permits absent
non-count arguments, reports the actual selected locale/form, and does not expose
catalog tables or change global locale. Source fallback omits locale/variant.

The localization example and emitted-runtime oracle cover parent/configured/source
fallback, missing plural forms, escaped placeholders, literal markup-like argument
values, independent result ownership and errors shared by both APIs. A cross-module
analysis fixture covers result inference, field completion/hover and invalid argument
or result types. The selector does not parse rich markup; player `resolveTemplate`,
rich presentation records, emission and reveal integration remain pending.

Native checks passed in headless LÖVE 11.5 / LuaJIT 2.1.1700008891 with JIT both
on and off, using compiler-emitted modules; Lua 5.4 also passed the example/oracle.
Compiler: Tua 0.11.0, aarch64-apple-darwin debug, dirty base
`ad4944df8457278456a813d355069f5513f8d83c`, executable SHA-256
`8245a294c0bc0640276f62590b065bc7716ff1b7dda27385f1bae77967e8ce63`.
These are execution checks, not installed-editor, audio or typography validation.

The complete `make verify` gate passed: 2,142 core unit tests (one ignored),
508 language-server tests, CLI/emitted-runtime and build-cache suites, workspace
Clippy, model-contract checks and extension checks. Strict documentation build,
explicit Lua formatting, typed example formatting/check/build, and diff checks
also passed. The example retains its intentional missing-French-translation warnings.
