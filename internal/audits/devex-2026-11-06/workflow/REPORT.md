# Workflow, protocol boundary, architecture and integration audit

Owner: lead agent. Scope: W01–W14 in the coverage matrix. All fixtures are disposable;
no production source or normal editor profile was changed.

## WF-01 — UTF-16 positions are treated as Unicode scalar columns

- **Category:** correctness / protocol / refactoring. **Severity:** High.
- **Confidence/status:** confirmed by stdio protocol, a second agent's independently
  named fixture, and actual VS Code edit application. No whole-server crash observed.
- **Impact:** emoji and other non-BMP characters before a symbol on the same line
  shift hover, definition, diagnostic, symbol and rename ranges. More such characters
  cause missing hover/definition and irrelevant completion. Rename can corrupt code.
- **Minimal source:** `local prefix="😀"; local score: number = 7; print(score)`.
  Request rename at the final `score` with UTF-16 positions, new name `points`.
- **Reproduce from repository root:**
  `node internal/audits/devex-2026-11-06/workflow/unicode.js`;
  `node internal/audits/devex-2026-11-06/hover/crosscheck-unicode.js`;
  `node internal/audits/devex-2026-11-06/workflow/editor-run.js` (local VS Code GUI).
- **Expected:** the declaration/use become `points`; ranges select exactly `score`.
  LSP defaults to UTF-16 when no other encoding is negotiated. See Microsoft's
  [Position contract](https://github.com/microsoft/vscode-languageserver-node/blob/main/types/src/main.ts).
- **Actual editor result:**
  `local prefix="😀"; localpointse: number = 7; printpointse)`.
  The single-emoji definition starts one character early. An eight-emoji prefix
  makes hover/definition null. ASCII and BMP `漢字` controls return correct ranges.
- **Evidence:** [unicode-evidence.json](unicode-evidence.json),
  [editor-evidence.json](editor-evidence.json), and
  [independent cross-check](../hover/unicode-crosscheck.json).
- **Source/root-cause evidence:** `crates/tua-lsp/src/server/protocol.rs:1518`
  (`lsp_range`) and `:1572` (`source_position_from_lsp`) merely subtract/add one.
  Core `source_text.rs:78` indexes Unicode scalar characters; parser source positions
  follow this convention. `server/capabilities.rs:39` does not negotiate another
  position encoding. This boundary cannot convert correctly without document text.
  Incremental-change handling already has a separate UTF-16 conversion in
  `server/handlers.rs:3780` and `:3826`, showing inconsistent paths rather than an
  inability to represent the document.
- **Why tests missed it:** UTF-16 tests exist in `server/tests/inline_completion.rs:290`
  and `:323`, and `model_completion.rs:41`; they cover the dedicated inline path,
  not ordinary hover/rename/edit serialization. Ordinary feature tests are largely
  ASCII. Source ranges merely being non-null does not verify selected text.
- **Smallest maintainable fix:** centralize document-aware input/output position
  conversion in the LSP boundary. Reuse it for all standard requests, nested edit
  ranges, diagnostics, semantic tokens, hints and resolve data. Do not change only
  rename or redefine all core positions ad hoc.
- **Regression:** table-driven ASCII/BMP/non-BMP/combining characters, CRLF and
  multiline cases. Apply returned rename/completion edits with UTF-16 indexing;
  assert exact output and parse it. Also assert every hover/diagnostic span lies
  within the original document and selects the intended token.

## WF-02 — Same-name modules resolve across unrelated workspace projects

- **Category:** correctness / project isolation. **Severity:** High.
- **Confidence/status:** confirmed in two independent actual LSP fixtures, including
  differently named importer files. The winning project differed across fixture layouts; ordering is an inferred cause, not an isolated permutation experiment.
- **Impact:** opening a second project changes the first project's hover types,
  definitions and diagnostics; changing its own dependency no longer changes the
  inferred export if another project's module was selected.
- **Minimal setup:** two configured projects, each with `source="src"` and
  `require("dep")`. Project A's `src/dep.lua` returns `{value=7}`; B's returns
  `{value="second"}`. Initialize one LSP with both workspace folders and open both
  importers. A assigns `dep.value` to `number`.
- **Reproduce:** `node internal/audits/devex-2026-11-06/workflow/lifecycle.js`;
  `node internal/audits/devex-2026-11-06/typing/verify-multi-root.js`.
- **Expected:** each importer uses its own project's configured runtime roots;
  A hovers `number`, B `string`; changes in A do not alter B.
- **Actual:** both hovers select the same project's export and module path. The
  losing project gets a false TL1003. The independent inventory/menu reproduction
  also navigates definition into the other project. Both projects pass their
  relevant single-root controls; this is not ambiguous dynamic Lua behavior.
- **Evidence:** [lifecycle-evidence.json](lifecycle-evidence.json), session
  `multi-root-isolation`; [independent cross-check](../typing/multi-root-crosscheck.json).
- **Likely root:** `server/analysis.rs:860` flattens all workspace source/output
  roots into one host. `module_graph/mod.rs:136` builds a global resolution context;
  `:625` uses `existing_candidates` for require without the importing project;
  `:865` scans every global root, `:693` chooses the first duplicate candidate.
  Relative imports have a distinct path, so do not assume every import form has
  the same symptom. The project model already knows containing workspaces; that
  identity must survive into require edges and cache keys.
- **Why tests missed it:** `server/tests/project_layout.rs:52` tests containing
  roots with distinct source directories and filenames, but never same-name
  exports with incompatible types. Workspace-symbol tests intentionally aggregate
  projects and therefore cannot establish semantic isolation.
- **Smallest maintainable fix:** resolve module names against the importer's
  owning workspace, retaining explicit cross-project/import-library mechanisms.
  Share that edge identity across checker, hover, completion, definitions and
  invalidation rather than adding per-feature filtering.
- **Regression:** two configured roots with the same require key and incompatible
  exports; swap root order, open order, importer names, Lua/Tua provider kinds;
  exercise hover/check/completion/definition and unsaved edits/deletion in each.

## Positive workflow evidence

[lifecycle.js](lifecycle.js) exercises actual notifications and pull requests:

- Single-root import hover/diagnostics change number → string after an unsaved
  dependency edit and return to disk number after closing that overlay.
- Watched disk edits change the type to boolean and produce the expected TL1003.
- Rename/delete invalidates the old module; updating the importer resolves the
  new name. Recreation restores the export; no server restart is necessary.
- Imported mismatch diagnostics may be deferred on the immediate fast pass after
  editing the importer. The added idle probe receives project diagnostics; this
  is the documented staging behavior, not recorded as a stale-diagnostic defect.
- A trailing-dot malformed edit retains member completion and recovers on a valid
  edit. Updating `tua.toml` globals removes the undefined-variable diagnostic.
- Independent stdio sessions do not share project facts; the confirmed isolation
  failure is multiple roots within one server.

[freshness.js](freshness.js) additionally sends 24 rapid unsaved edits; the final
hover and diagnostics agree with the final valid buffer. A CRLF document with an
emoji prefix accepts an incremental UTF-16 range edit at the intended literal
and diagnoses the new string/number mismatch. This confirms that incremental
synchronization has conversion logic even though standard query/output ranges
remain wrong. Push messages in this run omit document version, so no claim of
checking monotonic published version numbers is made.

## Cross-checks owned by other lanes

[workflow/crosscheck.js](crosscheck.js) independently confirms TYP-01 callback return
checking and TYP-02 imported chain checking with renamed functions/modules and Lua
runtime observations. Both invalid string contracts pass check/build and print
`number`; a directly annotated wrong return is correctly rejected.
It also confirms CMP-01 auto-import omission only on invoked completion context.
Actual VS Code 1.142.0 agrees: `createEntity` is missing from the completion provider
and the visible suggestion popup says **No suggestions** ([screenshot](editor-suggest.png)).
A control `obj.health` member completion succeeds. The DOM `offsetParent` heuristic
reported false for the fixed-position popup; visual inspection of the captured PNG,
not that heuristic, establishes visibility. These are client integration tests,
not merely simulated protocol clients.

## Latency measurements and limits

Run `node internal/audits/devex-2026-11-06/workflow/performance.js`.
See [performance-evidence.json](performance-evidence.json): three fresh processes
per bundled project, 30 warm and 30 post-edit samples per project, three incomplete
samples. Positions derive from exact buffers; expected imported member is checked;
representative response samples are saved. Initialization is timed separately.

These local stdio measurements are descriptive, not a regression comparison or
universal responsiveness claim. They do not measure keystroke-to-render latency,
large private projects, long-session memory, or performance on other platforms.
The same-process warm path is much faster than first-open/project rebuilding.
Performance work should retain result parity: the auto-import defect demonstrates
that a fast response can simply omit a promised capability.

| Project | Cold import median / max | Warm import median / p95 | Post-edit median / p95 | Warm hover median | Warm signature median |
|---|---:|---:|---:|---:|---:|
| features | 117.88 / 128.03 | 0.18 / 0.31 | 14.34 / 16.98 | 0.18 | 0.21 |
| starfall | 37.21 / 38.63 | 0.11 / 0.25 | 3.82 / 4.53 | 0.12 | 0.14 |
| love-full | 77.20 / 78.01 | 0.21 / 0.41 | 4.40 / 5.52 | 0.16 | 0.19 |

All values are milliseconds. All sampled expected members were returned and no sampled hover/signature was null.
