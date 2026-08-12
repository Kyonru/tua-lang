---
icon: lucide/package
---

# Asset Pipelines

Tua can run opt-in external asset importers before `check`, `build`, and
`watch`. Importers remain standalone tools; Tua provides content-addressed
caching, safe generated-output ownership, structured diagnostics, and normal
type checking for generated `.tua` and `.d.tua` modules.

Use a pipeline when source assets need deterministic conversion before the game
can consume them—for example Tiled or LDtk maps, Aseprite metadata, dialogue
databases, or a custom level format.

## Configure A Pipeline

Each `[[asset_pipelines]]` entry owns one generated directory below the
configured `source` root:

```toml title="tua.toml"
[[asset_pipelines]]
name = "world-maps"
inputs = [
  "maps/**/*.ldtk",
  "tools/import_maps.py",
]
command = ["python3", "tools/import_maps.py"]
output = "src/generated/world-maps"
version = "1"
```

| Field | Meaning |
| --- | --- |
| `name` | Stable cache identity. Use letters, digits, `.`, `-`, or `_`. |
| `inputs` | Project-relative files, directories, or `*`, `?`, and `**` globs. Every matched file contributes to the cache key. |
| `command` | Executable followed by arguments. Tua invokes it from the project root. |
| `output` | Dedicated generated directory strictly below `source`. Pipeline outputs are materialized here. |
| `version` | Optional importer/schema version. Change it when behavior changes without a corresponding input-file change. |

Include the importer script or executable in `inputs`. Tua hashes a relative
first command executable when it is a file, but it cannot hash an interpreter
or executable found through `PATH`. Listing scripts in `inputs` makes tool
changes invalidate the cache consistently.

Pipeline output directories may not overlap. Tua also refuses path traversal,
symbolic-link output paths, duplicate manifest outputs, missing declared files,
and replacement of a source file the pipeline does not already own.

## Importer Environment

Tua starts the command with these environment variables:

| Variable | Value |
| --- | --- |
| `TUA_ASSET_PIPELINE_NAME` | Configured pipeline name. |
| `TUA_ASSET_PIPELINE_PROJECT_ROOT` | Absolute project root. |
| `TUA_ASSET_PIPELINE_INPUTS` | JSON array of sorted absolute input paths. |
| `TUA_ASSET_PIPELINE_OUTPUT_DIR` | Staging directory where the importer must write generated files. |
| `TUA_ASSET_PIPELINE_MANIFEST` | Path where the importer must write its JSON manifest. |
| `TUA_ASSET_PIPELINE_CACHE_KEY` | Deterministic content key for this invocation. |

Arguments can also use `{project}`, `{output}`, `{manifest}`, and `{cache_key}`.
An argument that is exactly `{inputs}` expands to one argument per input file:

```toml
command = [
  "tools/import-world",
  "--output", "{output}",
  "--manifest", "{manifest}",
  "{inputs}",
]
```

Environment variables are the more robust interface when an importer needs all
of the metadata.

## Manifest Protocol

The importer must write UTF-8 JSON using manifest version 1:

```json
{
  "version": 1,
  "outputs": [
    "rooms.tua",
    "types.d.tua",
    "images/player.png"
  ],
  "diagnostics": [
    {
      "severity": "warning",
      "message": "animation marker has no matching frame",
      "path": "maps/world.ldtk",
      "line": 1,
      "column": 1
    }
  ]
}
```

Every entry in `outputs` is relative to `TUA_ASSET_PIPELINE_OUTPUT_DIR`. It must
name a regular file written during that invocation. Tua copies those files to
the configured pipeline `output` directory only after validating the complete
manifest.

Diagnostic fields are:

- `severity`: `error`, `warning`, `info`, or `hint`;
- `message`: required user-facing text;
- `path`: optional project-relative source asset;
- `line` and `column`: optional one-based positions, defaulting to 1;
- `end_line` and `end_column`: optional one-based range end;
- `code`: optional numeric Tua diagnostic code; otherwise Tua uses `TL2011`.

Importer diagnostics retain their asset path and range in CLI output. A
non-zero importer exit is always an error; Tua also includes manifest
diagnostics if the failed importer managed to write a valid manifest.

## Typed Generated Modules

Generated source joins the normal project source tree before discovery and
analysis. The importer does not need a special type protocol beyond emitting
valid Tua:

```tua title="generated/types.d.tua"
export type Room = {
  name: string,
  spawnX: number,
  spawnY: number,
}
```

```tua title="src/world.tua"
import type { Room } from "./generated/world-maps/types"

local function enter(room: Room)
end
```

Declaration-only `.d.tua` files are checked and indexed but produce no Lua.
Generated runtime `.tua`, ordinary `.lua`, images, audio, and other declared
files continue through the regular build pipeline. This keeps importer-specific
schema logic outside the compiler while preserving completion, hover,
navigation, and type checking in game code.

## Cache Behavior

Cache entries live under:

```text
.tua/cache/asset-pipelines/<pipeline-name>/<content-key>/
```

The key includes the pipeline protocol, name, configured version, output,
command arguments, sorted input paths, and every input file's bytes. A relative
first command executable is also hashed when it exists as a file.

On a cache hit, Tua validates the manifest and every cached output before
materializing them. Corrupt or incomplete entries are discarded and rebuilt.
When a new manifest removes a previously owned output, Tua removes that stale
materialized file. Old content-key directories are retained so switching input
versions can reuse them; cache garbage collection is intentionally left to the
user or CI environment for now.

Do not commit `.tua/cache`. Generated source directories may be ignored or
committed according to the project's reproducibility policy, but Tua will not
overwrite a different, unowned file already present there.

## Command Behavior

- `tua check` runs or restores pipelines before checking generated modules.
- `tua build` runs or restores pipelines before discovery, checking, and
  emission.
- `tua watch` watches pipeline input roots in addition to source and declaration
  roots. An input content change reruns the pipeline and starts a fresh warm
  build session over the new generated files.
- `tua lint` does not execute external tools.
- The language server does not execute external commands. It indexes generated
  files materialized by `check`, `build`, or `watch`, and editor file watching
  observes subsequent changes.

## Security And Reproducibility

An asset pipeline command is arbitrary local code with the same permissions as
Tua. Only enable commands from a project you trust. Tua constrains where declared
outputs are materialized, but it cannot sandbox the importer process itself.

For deterministic builds:

1. pin the importer or interpreter version in the development environment;
2. include importer scripts, schemas, and configuration files in `inputs`;
3. avoid timestamps and machine-specific absolute paths in generated content;
4. sort source records before writing outputs and diagnostics;
5. bump the pipeline `version` when hidden behavior or protocol assumptions
   change.
