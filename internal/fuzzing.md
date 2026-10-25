# Fuzzing

Tua fuzzes project-controlled source and configuration without running project
code. The scheduled trusted workflow exercises three targets:

- `source_pipeline` sends arbitrary bytes through Tua and Lua parsing, then
  checks, formats, and emits every input that parses as Tua. It asserts formatter
  idempotence, successful reparsing of formatted source, and valid emitted Lua;
- `project_config` parses arbitrary project configuration text;
- `protocol_and_paths` exercises bounded LSP framing and path confinement.

The CLI suite also runs a deterministic arithmetic/compound-assignment corpus
against a native Rust oracle and the selected Lua runtime (pinned LuaJIT in CI).

The hosted workflow runs each target for five minutes every week. It is
scheduled and manually dispatchable only; untrusted pull requests never run
repository code.

To reproduce a crash or run a longer local campaign, install the pinned tool
with a nightly Rust toolchain:

```sh
cargo install cargo-fuzz --locked --version 0.13.2
cargo +nightly fuzz run source_pipeline -- -max_len=65536
cargo +nightly fuzz run project_config -- -max_len=65536
```

Crash artifacts are written below `fuzz/artifacts/<target>/`. Minimize and turn
every confirmed defect into a focused regression test in the owning crate
before deleting the artifact.
