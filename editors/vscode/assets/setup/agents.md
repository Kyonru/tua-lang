# Tua tools for agents

Tua's MCP server can return diagnostics, typed symbols, module relationships,
references, asset metadata, and the language contract. Its tools read project
state; they do not build, run, debug, or edit your game.

VS Code agents can discover the server from the extension in a trusted local
workspace. For Codex in a terminal, use **Tua: Set Up Agent Tools for Codex**.
The command copies the bundled CLI to Tua's stable user storage, checks an MCP
tool call, and registers this workspace with Codex. It does not change your
terminal's `PATH`. Use the displayed CLI path for `check`, `build`, `test`,
or `dev` in a terminal.
Run the setup command again after updating the extension to refresh that copy.
