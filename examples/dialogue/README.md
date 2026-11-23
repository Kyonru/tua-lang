# Headless dialogue example

From this directory, use the feature-branch compiler:

```sh
/path/to/tua build
cd build
lua main.lua
```

The explicit `require("tuadialogue")` carries a checked constructor/method contract
and includes the runtime only when used. No LÖVE host is needed. Replace `print`
with a renderer consuming owned snapshots. The snapshot node is optional; guard it before reading. Present nodes always have
`text` and `options`, using an empty string/array when not applicable. `start()` selects the
entry node; `start("label")` selects a named node. Choices use stable IDs.

## Authoring frontend harness

`intro.tdialogue` exercises the shared version-one parser/emitter. Automatic
project asset discovery is pending, so it deliberately lives outside `src`.
From the repository root, compile it using the development harness:

```sh
cargo run -q -p tua-core --example dialogue_compile -- examples/dialogue/intro.tdialogue > /tmp/intro-dialogue.lua
```

Then, from this example directory, build the runtime and execute the data:

```sh
../../target/debug/tua build
lua asset_host.lua /tmp/intro-dialogue.lua
```

The compiler emits ordinary data with no runtime parsing or execution of text.
The host runs the same line/choice/end player as the structured example. Generated
output stays outside source control. See `docs/dialogue.md` for the supported
syntax and escaping. Build/watch integration, localization, asynchronous commands
and `.tdialogue` editor support remain pending. Reveal and temporary-message
policies belong to the host.
