# M6 rich authoring qualification

This slice completes the authoring side of the rich runtime on
`feature/tdialogue-rich-text`. It is not merged or published.

## Authored contract

Place `@text rich` immediately after `@dialogue 1`, allowing blank lines and
ordinary comments. Duplicate, malformed or late opt-ins fail. Plain scripts
retain their existing parsing and version selection.

Rich lines, choices and disabled reasons are parsed by the bounded shared parser.
Lines permit pauses and scoped speeds; choices/reasons permit presentation styles
and pronunciation metadata only. The compiler emits schema 8, retaining both
the raw template and its matching plain fallback. Source diagnostics convert
UTF-8 offsets to source columns; the LSP adapts these to UTF-16 editor ranges.

Catalog analysis checks every static plural form against the source field kind
and checks interpolation names in plain fallback templates. Metadata does not
become interpolation input. Dynamic templates and final substituted grapheme
boundaries are still validated at runtime. Extraction is read-only and keeps
raw rich templates in `text`, adding `format: "rich"` and `plainTemplate`;
existing plain entries are unchanged.

Shared editor queries recover unfinished marks for name completion and hover.
Choices/reasons do not suggest timing marks. Escaped brackets and quoted
pronunciation brackets remain literal. Unsaved opt-in/catalog changes refresh
analysis. TextMate supplies highlighting; paired closing-tag insertion and full
structural authoring assistance are not claimed by this slice.

## Qualification coverage

- Parser tests cover opt-in placement, plain compatibility, field restrictions,
  raw/fallback preservation, emission and Unicode diagnostic columns.
- Catalog tests cover malformed plural forms, invalid timing, valid translated
  styles/metadata and removal of rich source status.
- Core/LSP tests cover incomplete marks, existing brackets, Unicode hover ranges,
  UTF-16 adaptation and unsaved opt-in removal.
- CLI integration checks/builds the example, extracts rich translation entries
  and executes actual emitted Lua when an interpreter is available.
- `examples/dialogue/rich` now runs an authored script and a typed document.
  The existing Rust/Lua parity corpus and runtime lifecycle oracle retain M6's
  Unicode, placeholder isolation, localization, skip and timing coverage.

See [runtime qualification](M6-runtime.md) and
[reveal qualification](M6-reveal.md) for the earlier acceptance slices.

## Host limits

Native checks use headless LÖVE and do not establish rendered font coverage,
bidirectional shaping, audible speech quality or installed VS Code behavior.
Those require M10 presenter/consuming-game qualification. No game files, renderer
dependencies, release artifacts or generated Lua are committed by this slice.

## Validation results

- `make verify` passed: workspace tests (including 2,149 core unit tests and
  509 LSP tests), Clippy with warnings denied, model reference/negative controls,
  feature mappings, extension checks (292 tests), grammar, metadata and whitespace.
  The repository's existing ignored tests remain ignored.
- After the catalog metadata refinement and completion assertion, reran the
  focused rich suite: 18 core tests plus emitted-runtime integration passed.
  The final rebuilt CLI's rich authoring/extraction/emitted-Lua test passed.
  Clippy and the later gate stages included these refinements.
- Final `tua fmt`, `tua fmt --check`, `tua check` and `tua build` passed for
  `examples/dialogue/rich`. No authored Lua assets changed in this slice.
- `make docs-build llms-drift-check diff-check` passed.
- Final emitted example passed under Lua 5.4.8. LÖVE 11.5 with LuaJIT
  2.1.1700008891 passed with JIT explicitly on and off: authored/typed example,
  maintained rich lifecycle oracle and an actual `I18n.dialogueTemplate` adapter
  check with locale fallback and literal markup-looking arguments.

Compiler provenance: Tua 0.11.0, debug `aarch64-apple-darwin`, based on
`a387c76435848950cf9a3a7aec4e39b008ab22ee` with this slice's working changes.
Final executable SHA-256:
`0e2dc2a3bdd6d69932f2f19bf7b3a0dfae8149bba8ec37a7e6ed5922df88263c`.
No standalone Lua 5.1 interpreter was available; LuaJIT coverage used native LÖVE.
Installed-editor interaction and graphical/audio presentation were not tested.
