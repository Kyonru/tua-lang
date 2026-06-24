# features

Buildable feature tour for tua language and editor behavior.

```sh
cargo run -p tua-cli -- check
cargo run -p tua-cli -- build
```

From the repository root:

```sh
make check-example EXAMPLE=examples/features
make build-features
```

## Files

- `src/types.tua` covers primitive types, `any`, aliases, table shapes,
  optional fields, unions, and annotated locals.
- `src/functions.tua` covers annotated function parameters and returns,
  inferred return hovers, call checking, and table-field mutation.
- `src/compound.tua` covers `+=`, `-=`, `*=`, `/=`, and `%=` lowering for
  locals and field chains.
- `src/inference.tua` covers local type inlay hints, inferred function return
  hovers, parameter-name hints, field-chain inference from annotated locals, and
  the permissive `any` boundary.
- `src/modules.tua` covers normal Lua `require(...)` usage, plain Lua module
  interop, and cross-file go to definition for required module exports.
- `src/navigation.tua` is a compact same-file sample for go to definition,
  references, rename, document symbols, inlay hints, and semantic highlighting.
- `src/assets.tua` covers typed LÖVE asset loader helpers such as
  `image "assets/player.png"` and `sound "assets/jump.wav"`, plus LOVE object
  method completion/signature behavior such as preferring `player:setFilter()`
  over `player.setFilter()`.
- `src/assets/` contains placeholder files used by static asset validation.
- `src/main.tua` ties the feature modules together while keeping `require`
  calls unchanged in emitted Lua.
- `src/legacy.lua` is intentionally plain Lua. Builds copy it unchanged into
  `build/`, while the LSP can still infer simple returned module exports from
  it.
