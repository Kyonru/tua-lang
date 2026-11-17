---
icon: lucide/unplug
---

# External API And MCP

Tua exposes its compiler analysis to trusted local tools without giving those
tools write access to the workspace. Both servers are explicitly started,
stdio-only processes and reuse one `AnalysisHost` for the session.

## JSON-RPC API

Start the newline-delimited JSON-RPC API in a Tua project:

```bash
tua api --stdio
tua api --stdio --workspace /path/to/project
```

Each request and response occupies one UTF-8 JSON line. Stdout is reserved for
protocol messages; diagnostics about loading the process are written to stderr.
The first request must negotiate protocol version `1`:

```json
{"jsonrpc":"2.0","id":1,"method":"tua.api/initialize","params":{"capabilities":{"sourceContext":true}}}
```

`capabilities.sourceContext` must be `true` before Tua returns model-ready
prefix or suffix excerpts. It also gates formatting and code-action previews
that may contain source text.

The session supports:

| Method | Result |
| --- | --- |
| `tua.api/projectSummary` | Roots, config summary, files, overlays, revision, and the running CLI path |
| `tua.api/languageContract` | The same compact contract embedded in model prompts |
| `tua.api/refresh` | Explicit disk rescan; semantic requests never rescan implicitly |
| `tua.api/setOverlay`, `clearOverlay` | In-memory document content for unsaved files |
| `tua.api/diagnostics` | Compiler diagnostics |
| `tua.api/semanticFacts` | Symbols and typed bindings |
| `tua.api/moduleGraph` | Static require edges and exports |
| `tua.api/references` | Project references at a zero-based position |
| `tua.api/assetMetadata` | LOVE asset inventory and reference metadata |
| `tua.api/completionContext` | Bounded FIM prompt, suffix, revision, fingerprint, local/function scope, and candidate limits |
| `tua.api/validateCompletion` | Normalized candidate or stable rejection reason |
| `tua.api/formatPreview` | Formatted text preview; never applied |
| `tua.api/codeActions` | Compiler-proven edit previews; never applied |

Send `tua.api/shutdown` to end the session. Paths may be workspace-relative or
absolute but cannot escape the selected workspace.

## MCP

Start the MCP adapter over the same internal session:

```bash
tua mcp --stdio
tua mcp --stdio --workspace /path/to/project
```

The adapter pins stable MCP protocol `2025-11-25` and implements
`initialize`, `tools/list`, and `tools/call`. Its tools are:

```text
tua_project_summary       tua_language_contract
tua_refresh               tua_diagnostics
tua_semantic_facts        tua_module_graph
tua_references            tua_asset_metadata
tua_completion_context    tua_validate_completion
tua_format_preview        tua_code_actions
```

Tool results contain compact text plus `structuredContent`. The MCP process
does not call a model, execute project Lua, open a socket, authenticate remote
clients, or apply edits.

### Agents using the VS Code extension

The extension advertises its bundled MCP server to supported VS Code agents
when a trusted local project is open. Each workspace folder gets its own
read-only analysis session. This registration does not connect external
terminal agents.

VS Code agents also get editor tools to build the active Tua project, run a
LÖVE game, start LÖVE debugging, and inspect recent Tua run output. At a
breakpoint, the debug-context tool can return a bounded stack and local
variables from the active Tua debug session. Build, run, and debug use the
same editor actions as the Command Palette; their tool calls report when a
task starts, and the context tool reports its later status. Debugging needs
LÖVE and the optional Lua debugger extension. These editor tools require a
trusted local workspace and are available only while the VS Code extension
is running. They are separate from the read-only MCP server.

For Codex in a terminal, run **Tua: Set Up Agent Tools for Codex** from the
Command Palette. The extension copies the bundled CLI to a stable path in its
user storage, verifies an MCP project-summary call, and registers the selected
workspace with Codex. The command runs only when invoked. If the Codex CLI is
not available to the extension, it copies the equivalent registration command
to the clipboard. Existing Codex entries with different commands are preserved.
The command shows the stable CLI path for terminal `check`, `build`, `test`,
and `dev` use; it does not change the shell's `PATH`. Start a new Codex
session after registration. Run the setup command again after an extension
update to refresh the copied CLI.
Terminal Codex can use that CLI path through its terminal tool to build, test,
and launch Tua projects. The MCP connection does not control a live VS Code
debug session; use the editor agent's debug-context tool for live stack and
variable information. The MCP project summary also returns `cliPath`, so an
agent can discover the executable path without guessing where the extension
installed it.

## Privacy And Trust

- The API and MCP servers read only configured source roots and type libraries,
  honor ignored paths, and never start from `tua lsp`.
- There is no HTTP, TCP, pipe, cloud, telemetry, or workspace-writing mode.
- Source excerpts require an explicit API capability. MCP source-returning
  tools are explicit tool calls by the parent client.
- Overlays exist only in memory. `refresh` reloads disk and reapplies them.
- Dynamic Lua behavior is never executed or guessed; unresolved facts remain
  dynamic or `any`.

The machine-readable language contract is also available as
[`llms.txt`](llms.txt).
