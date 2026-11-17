# Tua tools for agents

Tua's MCP server can return diagnostics, typed symbols, module relationships,
references, asset metadata, and the language contract. Its tools read project
state; they do not build, run, debug, or edit your game.

VS Code agents can discover the server from the extension in a trusted local
workspace. They can also use Tua editor tools to build, run a LÖVE game, start
debugging, and inspect recent output. At a breakpoint, the debug-context tool
can read a bounded stack and local variables. These actions use the editor's
existing Tua tasks and debugger, and need a trusted local project.

For terminal agents, run **Tua: Set Up Agent Tools** from the Command Palette. It
lets you choose a client, copies the bundled CLI to stable extension storage,
and checks an MCP tool call. It copies settings for the selected client to the
clipboard when needed. Merge them into your existing settings; setup does not
edit those files. Use the displayed CLI path for `check`, `build`, `test`, or
`dev`. Run setup again after updating the extension to refresh that copy. The
MCP project summary returns `cliPath` so a connected agent can find the
executable without shell `PATH` changes. Live debug context is available
through the editor agent tool.
