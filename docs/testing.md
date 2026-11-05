---
icon: lucide/flask-conical
---

# Testing Tua

Use `tua test` to check and compile your project, run its unit tests with
[Busted](https://lunarmodules.github.io/busted/), and report failures against
Tua source. Tests use normal Busted functions and Lua assertions; Tua adds the
build and source-map integration, not a new test syntax.

Pure functions, data transformations, and classes are easiest to test. Code
that calls LOVE needs test doubles or a separate game/runtime test; Busted runs
under a Lua interpreter and does not create a LOVE window or graphics context.

## Install The Backend

Install Lua/LuaJIT, LuaRocks, and Busted for the interpreter you plan to use:

```sh
luarocks install busted
busted --version
```

Busted and its dependencies are external tools, not bundled into Tua or the
VSIX. Tua does not download them when tests run. Ensure the terminal or VS Code
process can find `busted` on `PATH`, or select the executable explicitly:

```sh
tua test --busted /path/to/busted
```

`TUA_BUSTED` also selects the executable; `--busted` takes precedence. A bare
name is resolved on `PATH`; a relative executable path such as `./tools/busted`
is resolved from the project root. To choose another installed Lua runtime:

```sh
tua test --lua luajit
```

Busted and any Lua modules or native dependencies must be installed for that
interpreter. This integration is exercised with Busted 2.3.0.

## Write A Typed Spec

Create a module under the configured source root (normally `src/`):

```tua title="src/mathlib.tua"
local function double(value: number): number
  return value * 2
end

return { double = double }
```

Then create a spec ending in `_spec.tua`:

```tua title="src/spec/math_spec.tua"
local mathlib = require("mathlib")

describe("mathlib #unit", function()
  it("doubles a number", function()
    local result: number = mathlib.double(3)
    assert.equals(6, result)
  end)
end)
```

Run from the project folder containing `tua.toml`:

```sh
tua test
```

Busted prints its test summary. Assertion failures, runtime errors, compiler
errors, and an unavailable backend make the command fail with a nonzero exit
status. No discovered spec files is also an error. Tua never runs stale output
when the current test build has errors.

`describe`, `it`, hooks, and other Busted globals are supplied to the test build.
For ordinary `tua check` and editor diagnostics, add the globals you use to your
existing configuration:

```toml title="tua.toml"
[diagnostics]
globals = ["describe", "it", "before_each", "after_each", "setup", "teardown", "pending"]
```

Merge these names with any existing `globals` list. This permits the names; it
does not add runtime functions or full Busted-specific signature inference.
Use ordinary Tua types in the functions and classes under test.

## Select Tests

By default, Tua discovers `_spec` files recursively under `source`. Both
`*_spec.tua` and `*_spec.lua` work; a custom Tua extension follows `extension`
in `tua.toml`. Declaration-only libraries and build/state directories are
excluded from test discovery.

An optional file or directory is **relative to the configured source root**:

```sh
tua test spec/math_spec.tua
tua test spec
tua test --filter doubles
tua test --tags unit
```

`--filter` uses Busted's Lua-pattern name filter; `--tags` passes the requested
tags to Busted. Filtering happens after discovery, and Busted may report zero
executed tests when a filter matches nothing. The project is still compiled
as a whole when selecting one spec, so errors in another Tua source can block
that run.

Use Busted's standard hooks, assertions, spies, and mocks inside specs. See
[the upstream reference](https://lunarmodules.github.io/busted/) for their APIs.
The command currently exposes executable, interpreter, name-filter, and tag
selection rather than forwarding every Busted CLI option.

## Run In VS Code Or CI

Save your files, then run **Tua: Run Unit Tests** in a trusted workspace. It
runs the selected Tua server's `test` command in a visible terminal. Busted must be available to the editor process.

For CI, install the intended Lua interpreter and Busted, then run:

```sh
tua check
tua fmt --check
tua test
```

`check` validates the project, `fmt --check` verifies formatting, and `test`
executes assertions. One does not replace the others. A complete example is
available in [`examples/unit-testing`](https://github.com/Kyonru/tua-lang/tree/main/examples/unit-testing).

## Output And Runtime Boundaries

Each run builds fresh Lua and source maps into a unique directory under `.tua/`.
It does not overwrite the normal `out` directory. It uses the normal build
pipeline, including configured asset pipelines and bundled helper runtimes.
Test output is removed after the backend finishes or a handled error occurs.
An abruptly terminated process can leave a temporary directory behind.

Busted's working directory is the temporary build root, with that root on its
Lua module search path. Source-tree assets are copied there. Relative file I/O
therefore addresses the built project, not the original repository root. The
project-root `.busted` file is not automatically loaded from that directory.

The terminal report is collected until Busted exits. Tua maps its plain-text
failure headers and verbose Lua traceback locations through compiler source maps,
even if normal builds set `emit_maps = false`. Plain Lua specs retain Lua locations.

Specs are regular project sources: ordinary `tua build` and `tua package` do not
yet exclude them. Keep unit tests in a separate test project when distribution
must omit test code. A separate test root, package exclusions, coverage UI,
watch-mode testing, and VS Code Test Explorer discovery are not implemented by
this command. For runtime/gameplay checks, see [Debugging](debugging.md).
