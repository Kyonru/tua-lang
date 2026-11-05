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

For a dedicated test directory, configure a project-relative root:

```toml
[test]
source = "tests"
```

Then create a spec ending in `_spec.tua`:

```tua title="tests/math_spec.tua"
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

With `[test] source = "tests"`, Tua discovers specs recursively under that
project-relative directory. Without it, discovery uses the normal `source` root. Both
`*_spec.tua` and `*_spec.lua` work; a custom Tua extension follows `extension`
in `tua.toml`. Declaration-only libraries and build/state directories are
excluded from test discovery.

An optional file or directory is **relative to the test discovery root**:

```sh
tua test math_spec.tua
tua test .
tua test --filter doubles
tua test --tags unit
```

`--filter` uses Busted's Lua-pattern name filter; `--tags` passes the requested
tags to Busted. Filtering happens after discovery, and Busted may report zero
executed tests when a filter matches nothing. The project is still compiled
together with the entire test root when selecting one spec, so errors in another Tua source can block
that run.

Use Busted's standard hooks, assertions, spies, and mocks inside specs. See
[the upstream reference](https://lunarmodules.github.io/busted/) for their APIs.
The command currently exposes executable, interpreter, name-filter, and tag
selection rather than forwarding every Busted CLI option.

## Discover Tests And Export Results

```sh
tua test --list
tua test --report test-results.json
```

`--list` returns versioned JSON with absolute file paths, source-root-relative
selectors, parse errors and statically discovered cases. It does not compile,
run Busted, execute specs or create test output. Literal, unshadowed
`describe`/`context` and `it`/`pending` registrations include full names and
one-based source positions. Dynamic names, escaped names and aliased registration
functions remain runnable at file level; discovery does not claim they are absent.

`--report PATH` preserves terminal output and writes JSON containing completion
and success flags, captured output, and passed/failed/errored/skipped cases.
Locations in native Tua and ordinary Lua specs refer to source files. User `print`
output cannot corrupt the JSON. A run that fails before Busted completes leaves
`completed: false` instead of stale results from a previous run. The destination's
parent directory must exist. Check both the command exit status and the report;
`--list` and `--report` are mutually exclusive.

## Run In VS Code Or CI

Save your files, then open VS Code's **Testing** view. Tua discovers configured
projects lazily using `tua test --list`; use **Tua: Refresh Unit Tests** to refresh
manually. Run a project, spec file, or literal named case from the tree or its
gutter icon. Dynamic registrations remain runnable through their spec file;
executed cases also appear in the results. Tests with the same full name in one
file run together because Busted filters by name.

The **Run with Busted** profile supports **continuous run**: opt in through the
Testing view to rerun the selected tests after saved source/config/asset changes.
Runs are serialized and refreshes coalesced. Generated output and `.tua/` changes
do not retrigger tests. Stop cancels the owned CLI/backend process group; abrupt
cancellation can leave an ignored temporary `.tua/test-*` directory.

Test Explorer requires a trusted workspace and an installed Busted backend.
`tua.test.bustedPath` and `tua.test.luaPath` select optional executables; empty
values use the CLI defaults. It uses saved files, not unsaved editor buffers.
Results show mapped failure locations and captured terminal output. No tests
run just because a file was edited or the tree refreshed.

**Tua: Run Unit Tests** remains available as a visible-terminal workflow.

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

Normal `tua build`, watch builds and `tua package` omit the configured test
directory, including its helpers and assets. They also omit `*_spec.tua` and
`*_spec.lua` under the production source root by default. Set
`[test] exclude_specs = false` only when those filenames are intentional
production modules; the dedicated test directory remains test-only.
Previously generated, compiler-owned spec output is removed on the next build.

`check`, `lint`, `fmt` and editor analysis include the dedicated test root.
During `tua test`, production modules keep their normal paths and dedicated test
files are emitted under `__tua_tests__/`; both roots are on Busted's module search
path, with production modules taking precedence. Keep `__tua_tests__` out of your
production source tree. Tests can import production modules and test helpers;
production code should not import test-only modules. Test assets are available
under `__tua_tests__/` relative to the temporary build directory.

Coverage UI and a CLI watch loop remain follow-up work. For runtime/gameplay checks, see [Debugging](debugging.md).
