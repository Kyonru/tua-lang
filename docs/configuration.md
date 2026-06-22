# Configuration

Tua projects use `tua.toml`.

## Default Config

```toml
source = "src"
out = "build"
target = "love11-luajit"
extension = "tua"
emit_maps = true

[diagnostics]
duplicate_keys = false
missing_requires = false
self_assignments = false
```

## Root Options

`source`
: Directory containing Tua source files.

`out`
: Directory where emitted Lua files are written.

`target`
: Runtime target label. The default is `love11-luajit`.

`extension`
: Source extension to discover. The default is `tua`.

`emit_maps`
: Reserved for source-map-style output. Current emitted Lua stays readable even
  without runtime metadata.

## Diagnostics

Optional lints are disabled by default.

```toml
[diagnostics]
duplicate_keys = true
missing_requires = true
self_assignments = true
```

`duplicate_keys`
: Reports repeated literal keys in table constructors.

`missing_requires`
: Reports `require("module")` calls that Tua cannot resolve from the current
  project layout.

`self_assignments`
: Reports assignments such as `player.hp = player.hp`.

## Workspace Resolution

The LSP chooses the nearest workspace folder or ancestor containing `tua.toml`
when resolving diagnostic settings. If no config is found, defaults are used.
