# Headless dialogue example

`src/intro.tdialogue` is authored separately from the host in `src/main.tua`.
From this directory, use the feature-branch compiler:

```sh
/path/to/tua check
/path/to/tua build
cd build
lua main.lua
```

`require("intro")` imports a typed dialogue document. The explicit
`require("tuadialogue")` includes the player runtime only when used. No LÖVE
host is needed. Replace `print` with a renderer consuming owned snapshots;
guard the optional snapshot node before reading it. Choices use stable text IDs.

`tua watch` rebuilds changed scripts. Invalid edits report authored locations and
keep the previous valid output; text IDs must be unique across project scripts.
See `docs/dialogue.md` for syntax and escaping. Localization, asynchronous commands
and dedicated `.tdialogue` editor support remain pending. Reveal and temporary
message policies belong to the host.

For frontend development, the standalone harness also remains available from the
repository root:

```sh
cargo run -q -p tua-core --example dialogue_compile -- examples/dialogue/src/intro.tdialogue > /tmp/intro-dialogue.lua
```

After building, `lua asset_host.lua /tmp/intro-dialogue.lua` from this directory
executes that data using the same bundled player. Generated output stays outside
source control.
