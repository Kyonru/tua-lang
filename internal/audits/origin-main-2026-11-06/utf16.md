# Standard LSP UTF-16 boundary correction

Prior audit WF-01 proved source corruption when applying rename after non-BMP text; eight emoji also destroyed ordinary hover/completion queries. Core source columns count Unicode scalar values, whereas LSP defaults to UTF-16 code units. The old transport converted only zero/one-based indexing.

The standard document request now retains its synchronized source and converts client cursors/ranges at the boundary. Standard serializers receive source text explicitly. Cross-file navigation, references, rename, actions, workspace symbols and call hierarchy obtain target text from the immutable analysis snapshot, including unopened providers; no request-time filesystem reads were introduced. Completion, code-lens and call-hierarchy opaque resolve data retain their existing scalar representation so round-trips are not double-converted. UTF-16 is now explicitly advertised.

Updated surfaces: hover; completion replacement and resolved additional edits; definition/type-definition/implementation/references; prepare-rename/rename and file-rename edits; code-action diagnostics and edits; document links/highlights/symbols; selection/folding/inlay ranges; code lenses and reference commands; call hierarchy; full/range/on-type formatting; ordinary and shader diagnostics; semantic-token start/length encoding before delta calculation. Semantic-token conversion indexes lines once per batch. Full-document formatting also had an independent scalar final-column counter, corrected here.

Configuration has a pre-existing UTF-16 contract (`tua-core/src/config/parser.rs` and the LSP TOML editor), so it explicitly bypasses scalar conversion. Existing inline-completion source-aware encoding was moved into the shared protocol helper. No generic recursive JSON walker is used.

Malformed numeric positions now use checked conversion/addition; empty documents cannot reach the previous `u64::MAX + 1` panic. Surrogate-interior cursors return no position. Whole-document assist ranges with oversized end lines clamp to EOF; an initial strict range implementation rejected four existing code-action fixtures using line 999, caught by the full LSP suite and corrected.

Permanent regression file: `crates/tua-lsp/src/server/tests/unicode.rs`. It exercises ASCII/Japanese/one and eight emoji, actual returned rename edits applied to source, hover and definitions, completion edits, diagnostic ranges, closed-provider cross-file navigation/rename, semantic-token delta/range consistency, final-line formatting and malformed-position boundaries. Shared test position helpers now derive UTF-16 columns from the actual byte offset and handle needles crossing newlines.

## Explicit limits

The lead extended the same source-aware boundary to signal/module/ECS/object-pool/audio graphs, inspector/type-explanation responses and model-completion context/rejection payloads. Graph nodes select their own source path; explanation steps select the referenced file when available. Additional protocol regressions compare ASCII/non-BMP graph/asset/explanation offsets and model context/validation round trips. No actual VS Code GUI rerun is claimed by this lane; permanent tests exercise real request handlers/protocol payloads. Existing custom config spans intentionally remain UTF-16. Root cause fixes do not imply unsupported editor features become available.

## Verification

Initial two permanent protocol tests passed before broader controls were added. Full owning LSP suite is rerun after the final changes; final result is recorded in the integration report and `target/origin-main-audit/lsp-tests.log`. The earlier full run found the four oversized-range regressions above plus one independently changed completion candidate expectation, handed to its owner for reconciliation. No test failure is counted as a pass.

Final standard-lane run: `RUST_MIN_STACK=16777216 cargo test -p tua-lsp --lib -- --test-threads=1`: **383 passed, 1 failed**. All six Unicode tests and all configuration/shader/navigation/formatting/inline/source-action tests passed. Remaining failure: `completion_and_local_hover_use_project_types_in_incomplete_documents` expects synthetic `type` among `[hp,name,type]`, while the parallel candidate-filtering fix returns `[hp,name]`; handed to lead/editor owner. Lead is extending the custom graph/inspector serializers and owns the subsequent integration rerun, so the custom-limit inventory above describes this lane's boundary rather than the eventual integrated result.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.
