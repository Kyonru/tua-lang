# linter

Intentionally broken Tua sources demonstrating every rule in the standard
lint set. This is a diagnostic fixture, not a buildable example project.

From the repository root:

```sh
cargo build -p tua-cli
cd examples/linter
../../target/debug/tua lint
```

The command is expected to print warnings and exit with status `1`. Each source
comment names the rule and diagnostic code exercised immediately below it.

There is deliberately no `tua.toml`: the default `src` layout is sufficient,
and omitting a project file keeps this intentionally failing fixture out of the
buildable example and benchmark suites.

`DeprecatedGlobal` is omitted because it requires a host runtime catalog that
Tua does not currently provide. `DeprecatedApi` is demonstrated through
explicit `---@deprecated` project metadata.
