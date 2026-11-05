---
icon: lucide/life-buoy
---

# Troubleshooting

Start with the exact failing action: checking, building, editor suggestions,
formatting, or running the game. A successful build does not prove a runtime
asset exists or that the selected editor server matches your terminal CLI.

## Check Your Setup

1. Open the project folder containing `tua.toml`.
2. Confirm the source file's VS Code language mode is **Tua**.
3. Check the Tua language status item for the server path, version, and workspace.
4. Run **Tua: Focus Output** and read the first relevant error.

The bundled editor server is not installed on your shell `PATH`. If `tua` is
not found in a terminal, install the [standalone CLI](installation.md#standalone-cli-release)
or use the extension commands. `tua --version` reports the terminal binary's
version; it does not identify a separately selected editor server.

## Suggestions Or Hover Are Missing Or Slow

- Try a known expression such as `love.graphics.`. If nothing works, check server
  status and output before changing types.
- If only one value loses its members, run **Tua: Explain Type / Why Is This Any?**.
  A dynamic `any` cannot supply the same members as a known class or table shape.
- A method chain can continue only when the preceding call returns a value.
  For example, `Text:set(...)` returns no receiver; use a separate `text:getWidth()`
  call instead of `text:set(...):getWidth()`.
- Run **Tua: Show Language Server Health** to inspect queues and warming state.
  Note whether the delay happens on first open, after every edit, after changing
  an import, or only while saving. Cold and warm requests exercise different work.
- If testing optional inline completion, distinguish normal menu completion,
  deterministic inline suggestions, and Ollama suggestions. They have separate
  settings and workloads; see [Language Server](lsp.md#completion-hover-and-signature-help).
- After replacing a development server binary, run **Tua: Restart Language Server**.
  Repeated restarts make every test cold and do not fix a persistent inference issue.

For a performance report, include the file and approximate size, exact expression,
cursor position, action, and observed delay. Hover text, inlay hints, selection
highlights, and semantic coloring are separate features; identify which one is
wrong. A short reproduction is more actionable than “the editor is slow.”

## Formatting Does Not Run

Use **Format Document** first to distinguish a formatter problem from a save
setting. Select `SolenodonteLabs.tua` as the formatter for `[tua]` and enable
`editor.formatOnSave`; see the [settings example](formatter.md#format-in-vs-code).

Fix syntax errors before formatting. Type errors and lint warnings are different
from parse errors. Check `[format]` in `tua.toml` and any `tua.format.*` editor
overrides. Generic editor tab-size settings do not replace project formatting
rules. `line_width` is a wrapping preference, not a hard maximum.

`tua fmt --check` only reports differences; `tua fmt` applies them. Neither
command offers a path or stdin mode. For more details, see [Formatter](formatter.md).

## An Import Is Missing Or Has Type `any`

For default roots, `src/combat/enemy.tua` is `require("combat.enemy")`.
Check path capitalization, source roots, excluded paths, and whether the module
returns the expected API. A type-only module has no runtime output.

If the constructor works but an instance annotation does not, export and import
an instance type alias; a runtime local is not an automatic type-only import.
See [Modules And Imports](modules.md) and [Classes](classes.md).

For third-party Lua, provide the runtime module and, when needed, supported
LuaDoc or configured [type libraries](configuration.md#declaration-only-type-libraries).
Types do not install a missing runtime dependency. The editor indexes generated
asset modules only after their pipeline has materialized them; run `tua check`
or `tua build` to run configured pipelines.

## LOVE Does Not Start Or Finds No Assets

Tua does not include the LOVE runtime. Install LOVE and set `tua.love.command`
when the executable is not discovered. For `tua dev`, use `--love PATH` or
`TUA_LOVE`; see [Hot Reload](hot-reload.md).

Build first and launch the configured output folder, normally `build/`, rather
than `src/`. It needs a root `main.lua`. Assets referenced by the game must be
present at the expected paths in that output. Tua copies source-tree assets;
it does not automatically ship arbitrary files outside the source tree.

Read build errors before debugging the game. `tua build --emit-on-error` still
returns failure and can skip files; it is not evidence that the output is a
complete playable build.

## A Class Or Reload Behaves Unexpectedly

- Declare instance fields and initialize fields that cannot be `nil`.
- Within each class, field defaults run after `init` and can overwrite its
  assignments. Declare a field without a default when `init` computes it.
- Use `instance:method(...)` so the receiver survives a move across modules.
- Constructors do not rerun for existing objects after hot reload. Captured
  functions and inlined final calls can retain an old implementation.

See [Classes](classes.md) and [Hot Reload](hot-reload.md) for the full rules.

## A Runtime Error Points To Generated Lua

Keep source maps enabled, rebuild, and use **Tua: Debug LOVE Project**, generated
Lua navigation, or `tua traceback --maps build` with a traceback on stdin.
Do not edit generated Lua to repair the source. See [Debugging](debugging.md)
and the [traceback command](cli.md#traceback).

## Report A Reproducible Problem

Run **Tua: Copy Debug Info**. Include the extension/server versions, operating
system, sanitized `tua.toml`, minimal source, exact command or editor action,
and expected versus actual behavior. For cross-file issues, include both the
exporter and consumer. Remove private paths, credentials, and code you cannot share.

Protocol traces can help: temporarily set `tua.trace.server` to `messages` or
`verbose` and `tua.log.verbosity` to `debug` or `trace`. Turn verbose tracing off
again before comparing normal typing performance.

Use the repository's [support and bug-report guidance](https://github.com/Kyonru/tua-lang/blob/main/SUPPORT.md).
Report suspected security vulnerabilities through the private process in
[SECURITY.md](https://github.com/Kyonru/tua-lang/blob/main/SECURITY.md).
