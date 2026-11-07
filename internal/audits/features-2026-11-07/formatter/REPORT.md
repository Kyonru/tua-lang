# Formatter audit — 2026-11-07

One **High confirmed defect**: formatting changes multiline long-string values by removing trailing whitespace from their opening line. CLI and all three LSP formatting entry points reproduce it. No product code changed.

## Scope and evidence

Candidate baseline: revision `9d490dcd299bd25218c9fa4adc83e080e617453c`, existing release changes preserved; absolute `target/release/tua` 0.6.0 from the shared baseline. Runtime: `/opt/homebrew/bin/lua` 5.4.8. No cargo rebuild or full suite was run; the shared baseline reports 389 passing LSP tests. Tested stdio LSP directly, not desktop VS Code UI. LuaJIT/LOVE runtime equivalence was not separately exercised.

From repository root, exact reproducible harness commands:

```sh
python3 internal/audits/features-2026-11-07/formatter/probe.py
python3 internal/audits/features-2026-11-07/formatter/minimal-probe.py
node internal/audits/features-2026-11-07/formatter/lsp-probe.js
```

Harnesses save each fixture's `tua.toml`, before/after Tua, emitted Lua and structured commands/stdout/stderr/exit status. `cli-evidence.json` contains 11 CLI cases; `minimal-evidence.json` contains three isolated runtime cases; `lsp-evidence.json` contains eight actual stdio-LSP cases. LSP requests include document, range and on-type formatting, then formatting the resulting document again. The harness uses UTF-16 JavaScript offsets to apply protocol edits.

| Surface | Result / limits |
| --- | --- |
| CLI fmt and non-mutating fmt --check | Unformatted valid inputs rejected by check, formatted inputs accepted. Incomplete input rejected and unchanged. Invalid indent_width=0 reported as configuration error. |
| Document LSP / CLI parity | Matching output in all eight exercised cases; already formatted documents produce `[]`; incomplete document produces `null`. |
| Range / on-type | Local first-line edits exercised, including confirmed defect; second-line Unicode/CRLF selection also inspected. Incomplete source produces `null`. |
| Nested and inline else/elseif | Known previously fixed shape exercised successfully; subsequent statement returns to top-level indentation. |
| Classes / typed signatures / default arguments | Build and runtime unchanged (`4`, `5`), idempotent output. |
| Quotes / comments / long strings / escaped multiline strings | Forced single quotes and escaped multiline text work; long-string opening-line defect below. Interior blank lines and trailing spaces preserved in control. |
| CRLF / Unicode / options | Preserve CRLF, tabs and final_newline=false exercised together. Emoji retained. Project layout overrides generic LSP tabSize=8/insertSpaces=false. BOM removed as documented. |
| Width / idempotence | Width=20 wraps comma-separated tables/calls, repeat formatting stable. Heterogeneous table fixture has expected checker union-index error before and after; emitted runtime tested using --emit-on-error, not counted as a clean type-check pass. |
| Runtime comparison | Nine parseable CLI fixtures executed before/after; eight unchanged and one corruption. Three dedicated controls isolate opening-line whitespace and CRLF. |
| Inspected | docs/formatter.md; shared formatter implementation/tests; LSP formatting tests; ROADMAP.md and release/changelog formatter mentions. |
| Not covered | Exhaustive Unicode/escape/config combinations, randomized parsing, desktop format-on-save integration, LuaJIT runtime. |

No style-preference findings: documented annotation punctuation preservation, preferred (not hard) width, unsupported disable directives/path/stdin modes and parseability requirement are accepted scope, not defects.

## FMT-01 — Formatting deletes semantic trailing whitespace on multiline string opening lines

**Severity:** High. **Category:** confirmed defect. **Status:** reproduced through CLI/runtime and document/range/on-type LSP; independently reproduced by lead. **Known:** new in this audit; separate from the known inline middle-branch indentation fix and not identified in inspected roadmap/prior release summaries.

Minimal source (`minimal/before.tua`; there are two spaces after `a`):

```lua
local s = [[a  
b]]
print(#s)
```

Run `python3 internal/audits/features-2026-11-07/formatter/minimal-probe.py` from repository root. For each isolated project, the script runs the absolute release binary's `build`, `/opt/homebrew/bin/lua build/main.lua`, `fmt`, `build`, and the same Lua command. Both builds and formatting exit 0.

Expected: print `5` before and after formatting, because the two spaces are literal data. Actual: `5` before, `3` after; formatter rewrites the first line to `local s = [[a`. This can silently corrupt embedded source, templates or other whitespace-sensitive data during format-on-save.

`minimal-crlf` repeats the same `5 → 3` with CRLF and `line_ending="preserve"`. `interior-control` moves `a  ` to the next line and remains `5 → 5`. The larger `strings` case reproduces the defect with `[=[` delimiters and forced quote style while preserving subsequent literal lines. All eight LSP document results match CLI; the minimal LSP range/on-type requests return edits removing the literal spaces as well. Repeat formatting is stable, demonstrating that idempotence alone does not imply semantic safety.

Root cause: `crates/tua-core/src/formatter.rs:729` calls `line.trim()` before recognizing that the line opens a multiline literal; `:1187` additionally calls `output.trim_end()` in spacing normalization. The literal-preserving branch at `:612` only applies when multiline state was already active on entry. `:643` updates multiline state after formatting that first line. This shared implementation explains the CLI/LSP agreement.

Contract: `docs/formatter.md` promises string/comment contents are never reformatted, including nonblank long-bracket-string lines. Ordinary Lua string semantics independently require preservation.

Existing blind spot: `crates/tua-core/src/formatter/tests.rs:728` and `:749` put the opening `[[` on a line without literal content; they cover interior blank/code-like lines but not whitespace after content on the opening line. UTF-8 preservation test at `:784` uses the same opener-only shape. LSP tests cover formatting edits/parse errors, not this value-preservation boundary.

Smallest maintainable improvement: make leading/trailing whitespace normalization aware of literal/comment spans across line boundaries in the shared formatter. Preserve bytes belonging to literals on the opening line; retain normal trimming of surrounding code. Audit both the input trim and final normalized-output trim rather than fixing only one surface.

Regression: before/after literal equality for `[[a  \nb]]` and `[=[a\t \nb]=]`, LF/CRLF, interior-line controls, and document/range/on-type edits. Require clean build, unchanged runtime length/content, and idempotence.

Evidence: `minimal-evidence.json`, `minimal/{before,after}.{tua,lua}`, `minimal-crlf/`, `interior-control/`, `cli-evidence.json` (strings), `lsp-evidence.json` (minimal and strings). Independent lead reproduction: `../cross-checks/formatter-string.json`.
