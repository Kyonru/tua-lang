# Native unit testing

Install Busted for the Lua interpreter you intend to test with:

```sh
luarocks install busted
```

From this folder, run:

```sh
tua test
tua test counter_spec.tua
tua test --filter independent
tua test --tags unit
```

The example has three passing tests: two typed class tests and one plain Lua
spec. Change `assert.equals(13, ...)` to `assert.equals(14, ...)` to see a failure
mapped back to the original Tua line, then restore it.

Tests run in plain Lua through Busted, without LOVE. The runner builds into a
fresh directory under `.tua/`, leaves `build/` untouched, and cleans its temporary
output on normal completion. `tua check` and the editor use the listed diagnostic
globals; `tua test` also supplies Busted globals to its build automatically.

Use **Tua: Run Unit Tests** in VS Code, or see
[Testing Tua](../../docs/testing.md) for setup, executable selection, and limits.

The dedicated `tests/` directory is included in checking and formatting, but
omitted from production builds and packages. Specs import `counter` from `src/`.
