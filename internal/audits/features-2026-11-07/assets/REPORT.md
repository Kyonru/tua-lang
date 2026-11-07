# Assets and pipelines audit — 2026-11-07

Two confirmed defects: one high (generated-output ownership can overwrite user files after an output-root change), one medium (cached file corruption is accepted). Ordinary helper/native asset editor behavior passed the representative probes. Product files were not edited. Skills applied: tua-audit-lead, tua-language-auditor, tua-asset-metadata.

Baseline: release `target/release/tua` 0.6.0 at revision recorded in ../BASELINE.md; all probes use that absolute binary. Python 3 importers, Node actual stdio LSP, Lua 5.4.8. Tests were bounded; no Cargo rebuild or external services.

| Surface | Executed / inspected | Outcome |
|---|---|---|
| `source=src`, `out=build`, helper/native PNG | CLI build, copied PNG, actual LSP | Passed |
| Missing PNG, audio-as-image, unknown extension | CLI check | Correct TL2004, TL2006, TL2005 errors |
| Lexically shadowed `image` | CLI emit/check + LSP hover/completion | Ordinary string/function behavior, no asset validation |
| Helper/native path hover | Actual LSP | Valid image, Image type, .png, 1x1 dimensions |
| Kind-filtered and unfinished path completion | Actual LSP | PNG suggested, wav/xyz excluded |
| Asset file rename | Actual workspace/willRenameFiles | Two helper/native reference edits |
| Inventory | Actual tua/assetInventory | Source inventory returned; output duplicate excluded |
| Generated declaration discovery / omission from emitted Lua | check/build | Checked; no types.d.lua emitted |
| Pipeline cache hit and input edit invalidation | Importer run counter | 1 after two identical checks, 2 after input edit |
| Importer nonzero exit, duplicate output, absent declared output | CLI | Actionable TL2011 failures |
| Root-change ownership and cached byte corruption | CLI | Defects below |
| Runtime emitted calls | Lua with bounded LOVE stub | Correct helper/native calls and shadowed function |
| Actual LOVE headless image decode | Attempted actual app executable | Process aborted (status -6), no output; runtime qualification blocked, not a product finding |
| Timeout/stderr limits, watch, symlink guards | Source/existing tests inspected | Not rerun |
| Multi-workspace assets, GPU textures, binary format matrix | Not exercised | Coverage limit |

Commands from repository root: `python3 internal/audits/features-2026-11-07/assets/probe.py` (initial fresh fixture run; preserves final states), `node internal/audits/features-2026-11-07/assets/lsp-probe.js`, `python3 internal/audits/features-2026-11-07/assets/edge-probe.py`. For repeatable isolated ownership reproduction use the dedicated script below with a nonexistent root. `cli-evidence.json`, `lsp-evidence.json`, `edge-evidence.json`, `ownership-evidence.json`, and `lua-runtime.log` preserve raw outcomes. Fixtures include generated output and cache state under this audit directory.

## ASSETS-01 — Changing pipeline output root transfers ownership to unrelated source files

**High, confirmed defect, new.** A routine `output` configuration edit silently overwrites a different user-authored file. Docs promise refusal to replace source files the pipeline does not already own (`docs/asset-pipelines.md:62`).

Exact reproduction:

```
python3 internal/audits/features-2026-11-07/assets/ownership-repro.py /absolute/new/disposable/root /absolute/repo/target/release/tua
```

The script configures pipeline `world`, runs `check` to generate `src/generated/a/data.txt`, creates `src/generated/b/data.txt` containing `USER AUTHORED CONTENT`, then changes only output from `src/generated/a` to `src/generated/b` and runs `check`. Expected: reject overwrite of unowned `b/data.txt`. Actual: both checks exit 0; file becomes `GENERATED CONTENT`. Evidence preserves before/after strings and prior ownership state. Lead independently reproduced this in `../cross-checks/asset-ownership.json`.

Root cause: `MaterializedState` stores relative filenames alone (`crates/tua-cli/src/asset_pipeline.rs:78`); state lookup is keyed by pipeline name, and materialization reads the old set (`:1411-1423`) without its original root. The unowned-source check at `:1467` treats the same relative name in the new root as previously owned. Stale cleanup also applies old names under the new root (`:1426-1440`), an adjacent risk not separately counted.

Test blind spot: CLI cache/invalidation regression (`crates/tua-cli/tests/cli.rs:1420`) never changes `output`; existing stale-output symlink regression does not exercise root identity. Smallest improvement: persist normalized owning output root with state, scope every ownership/deletion decision to that root, and handle legacy state conservatively. Do not infer ownership of a newly configured directory from relative names. Regression: same-name user file survives root change; command reports an unowned collision; old-root cleanup never deletes new-root files.

## ASSETS-02 — Corrupted cached output bytes are accepted and copied to source

**Medium, confirmed defect against documented cache-integrity behavior, new.** Docs say corrupt cache entries are discarded and rebuilt (`docs/asset-pipelines.md:173-174`).

Reproduction is the `pipeline-corrupt-cache` stage of `probe.py`: after normal generation and cache use, replace the cache entry's regular `files/data.txt` with `CORRUPT`, leave manifest intact, run `tua check`. Expected: invalidate/reexecute importer and recover original `second` bytes. Actual: check exits 0; materialized file becomes `CORRUPT`; run counter remains 3 (third execution came from preceding output-root change). Raw entry path and output are in `cli-evidence.json`. The input cache key cannot detect changes to generated output bytes.

Root cause: `read_valid_cached_manifest` (`crates/tua-cli/src/asset_pipeline.rs:1299-1321`) checks version, safe path and regular-file existence only. It records/verifies no output digest. `materialize_outputs` copies the accepted bytes at `:1486`. Binary generated assets can therefore change silently without a source-input change. The probe uses text payload solely for unambiguous evidence; no hostile actor assumption is necessary.

Test blind spot: `asset_pipeline_caches_typed_generated_modules_and_invalidates_by_content` covers input edits, not cached-output edits. Smallest improvement: persist output size/digest metadata outside the importer-supplied manifest and verify on hits; invalid entries should use the existing rebuild path. Alternatively explicitly narrow the documented integrity guarantee, but that leaves corrupted asset delivery unresolved. Regression: truncate/change a regular cached payload while keeping manifest present; next check reruns importer and restores original bytes.

## Missing options, intentional behavior and remaining coverage

- Pipeline cache garbage collection is explicitly left to user/CI; an inspect/prune command is a usability idea, not a defect. No additional importer framework is necessary for these fixes.
- LSP intentionally does not execute importers; generated files must first be materialized with check/build/watch. This restriction is documented and was not classified as a gap.
- Dynamic path expressions cannot be proven and are intentionally not equivalent to literal asset references. Shadowed loader paths correctly avoid native metadata.
- Existing configurable output, timeout and manifest/stderr bounds are present. No missing mandatory option found in this sample.
- Source ownership and output ownership need different treatment: normal asset index excludes generated build copies, while pipeline materialized ownership requires persistent root identity.
- Prior `internal/audits/origin-main-2026-11-06/asset-escape-followup.md` concerns escaping/rename, not either finding here. ROADMAP.md:25 generally calls for asset-root hardening; neither concrete pipeline issue was found in prior audit text or roadmap. No skill update needed for this audit.
