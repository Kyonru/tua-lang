# Project configuration, initialization, and build audit

One confirmed high-severity defect: special-case cleanup bypasses output ownership and can delete handwritten Lua source during an officially supported in-place build. Ordinary separate-output cleanup, initialization, custom roots, mixed modules, emission switches and error reporting passed the sampled workflows. No product files changed.

## Evidence and reproduction

Run from repository root:

```sh
python3 internal/audits/features-2026-11-07/projects/probe.py
python3 internal/audits/features-2026-11-07/projects/ownership_probe.py
python3 internal/audits/features-2026-11-07/projects/extra_probe.py
```

`probe.py` initial run used fresh fixture directories; it is a stateful scenario harness, so rerunning over its old fixtures changes initial-state results. `ownership_probe.py` restores all relevant inputs and is independently repeatable. `raw.json`, `ownership.json`, and `extra.json` preserve exact absolute commands, exit statuses, stdout/stderr, and file inventories/content. `commands.log` and `ownership.log` capture the initial command transcript. Binary: repository `target/release/tua`, baseline 0.6.0; Lua `/opt/homebrew/bin/lua` 5.4.8. No cargo suites were rerun.

## Coverage matrix

| Area | Exercised / inspected | Result |
| --- | --- | --- |
| Init | Empty/default layout, existing .gitignore, second init, symlinked src | Creates runnable starter; preserves files and entries; rejects symlink before writes |
| Config | No config, defaults, custom source/out/main/extension, typo, invalid enum, nonexistent source | Defaults/custom layout work; precise TL3001 suggestion/enum errors; missing source fails TL2002 |
| Mixed project | Tua boot entry + Lua helper + text asset under custom root | Build succeeds; mirrored boot and main alias; Lua prints `mixed-ok`; asset bytes preserved |
| Modules | Conflicting `mod.tua` and `mod.lua` | Check uses Tua numeric export; build warns/skips Lua; runtime prints 42 |
| Main collision | boot.tua configured main plus src/main.tua | Build rejects duplicate output with TL2003 |
| Exclusions | Broken `_spec`, explicit exclude_specs=false, dedicated test root, .git metadata, nested build output | Defaults omit specs from build but check diagnoses them (documented); opt-in specs fail build; dedicated helpers/assets excluded; no recursive output copy |
| Emission | Source maps on→off, LuaCATS, type error + syntax error + valid file | Maps removed; typed comment emitted; default blocks; emit-on-error writes lowerable files and returns nonzero |
| Ownership | Removed source, manually modified stale Lua, untracked output | Normal manifest cleanup removes unchanged stale output and preserves modified/untracked files |
| In-place | Ordinary repeat build; declaration plus handwritten sibling | Ordinary build works with collision warning; declaration cleanup silently deletes source (PROJECTS-01) |
| Editor | Config schema implementation/docs inspected only | No LSP/editor interaction in this lane |

## PROJECTS-01 — Special-case cleanup deletes files without build ownership

**Severity:** high. **Category:** confirmed defect. **Status:** reproduced. **Known:** not found in ROADMAP.md or reviewed prior audit reports; distinct from ASSETS-01 asset-pipeline materialization overwrites.

Fresh disposable reproduction (all paths relative to its directory):

`tua.toml`:
```toml
source="src"
out="src"
```

`src/types.d.tua`:
```tua
export type Point = {x: number}
```

Handwritten `src/types.d.lua`:
```lua
return { important = "handwritten source" }
```

Run `/Users/kyonru/Documents/projects/Experiments/tlua/target/release/tua build` with that directory as cwd. Expected: declaration is checked without runtime emission; handwritten Lua source remains untouched. Actual: exit 0, stdout `built 0 file(s)\n`, empty stderr, and `src/types.d.lua` is deleted. Full exact bytes and before/after states: `ownership.json`, first entry. The lead independently reproduced the deletion in a fresh disposable fixture; see `../cross-checks/build-ownership.json`.

Two related triggers demonstrate the same missing ownership boundary:

- With separate roots, unrecorded `build/types.d.lua` is deleted merely because `src/types.d.tua` exists (second entry).
- With `emit_maps=false`, unrecorded/manually maintained `build/main.lua.map` and `.map.json` are deleted (third entry).

The public contract in `docs/cli.md` says in-place builds are supported, overlapping trees do not perform stale-output cleanup, and unrecorded/manually modified files are preserved. The first reproduction destroys a project input, so this is more than a documentation discrepancy.

Root cause: `crates/tua-cli/src/build/mod.rs:304` calls `remove_declaration_outputs`, whose `:614` unconditionally calls `remove_output_if_exists`; maps-disabled emission invokes the same cleanup via `:365`/`:723`. `remove_output_if_exists` at `:735` checks symlinks/existence but neither source overlap nor ownership. The manifest's fingerprint preservation (`build/ownership.rs:90`) and overlap check (`build/mod.rs:421`) run later and cannot protect these files. These triggers are one defect because they use the same ownership-bypassing removal path, though each requires regression coverage.

Existing test blind spot: `declaration_modules_are_checked_but_never_emitted` (`crates/tua-cli/tests/cli.rs:1359`) explicitly creates an unrecorded `build/types.d.lua` and expects deletion; `build_respects_disabled_source_maps` (`:1323`) likewise expects deletion of unrecorded maps. The normal manifest cleanup test (`:485`) verifies an unrelated untracked text file. No test combines declaration cleanup with in-place handwritten Lua.

Smallest maintainable improvement: route obsolete declaration/map cleanup through the common ownership reconciliation policy. Never delete an overlapping source-tree file based only on an output filename. Preserve unrecorded and modified separate outputs, and remove only previously recorded, unchanged generated artifacts. Update old tests to seed genuine owned output rather than labeling arbitrary bytes stale.

Regression: run the exact three triggers above; assert source/unrecorded bytes remain. Also cover genuine previously generated maps/declaration output disappearing after their owning configuration/source changes, while modified versions remain. Include a source==out declaration test with no manifest and require a no-mutation success or explicit collision diagnostic.

## Limits and non-findings

`target="potato"` is accepted, consistent with docs calling it a runtime target label rather than a validated target enum. `extension=".tua"` selects no source files and copies `.tua` as assets; bare `extension="tl"` works. This may merit configuration guidance/validation, but no unsupported contract was invented. A missing configured main is not diagnosed and regular mirrored main can still exist; whether to require an entrypoint for library-only builds needs a product decision, not counted here.

Check deliberately includes specs that production builds exclude; the initial mixed fixture check error is expected under `docs/testing.md:252-259`, not a defect. Source maps were checked for file presence and deletion, not mapping accuracy. No watch, package, editor initialization command, multi-root LSP, platform-specific filesystem, or actual LOVE launch was exercised in this lane. Broader import/declaration semantics belong to the language/module lanes. No blockers prevented the bounded CLI audit.
