# Native unit testing with Busted

The initial integration adds `tua test` and **Tua: Run Unit Tests**. It uses the
normal project build, module resolution, asset pipelines, runtime helpers and
traceback mapper. Busted remains an installed external dependency. There is no
new test syntax or parallel compiler pipeline.

## Exercised behavior

- Real Busted 2.3.0 executes a typed module spec, exits successfully for passing
  assertions, and returns failure for a deliberately wrong assertion. Both its
  failure header and verbose traceback are remapped through emitted source maps.
- Source maps are produced for tests even with `emit_maps = false`.
- Missing executables produce installation guidance. Empty discovery, escaping
  selections, unsafe state-directory symlinks and compile errors fail before
  backend execution. Handled exits remove temporary output.
- A project with `source = "."` ignores normal output and abandoned `.tua`
  directories. Existing production output remains intact.
- `examples/unit-testing` exercises imported class types and mixed Tua/Lua
  specs: three passing tests, individual-file selection, name filtering and tags.
- The extension uses the existing project/task machinery and refuses execution
  in an untrusted workspace. Files must be saved before running the command.

The integration test explicitly skips real Busted execution when it is not
installed; the local verification used the installed backend. CI environments
that require this integration should install Busted before running CLI tests.

## Boundaries and follow-ups

Tests are currently regular source files and remain part of ordinary builds and
packages. A dedicated test root and distribution exclusions belong in the next
iteration, alongside Test Explorer, watch mode, coverage and richer declarations.
The runner buffers its report until Busted exits and exposes a small set of
backend options. It does not load the project-root `.busted` file automatically.

Headless Lua tests do not establish LOVE graphics/audio behavior. No graphical
VS Code or LOVE session was exercised for this change. These limitations are
also recorded in `docs/testing.md` and the roadmap.

While writing the example, an imported alias used only in a nested callback
annotation produced a spurious TL4054 unused-type warning. A top-level typed
factory avoids it in the example; nested annotation usage tracking remains a
separate semantic follow-up.
