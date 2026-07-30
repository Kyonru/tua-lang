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
| `tua.api/projectSummary` | Roots, config summary, files, overlays, and revision |
| `tua.api/languageContract` | The same compact contract embedded in model prompts |
| `tua.api/refresh` | Explicit disk rescan; semantic requests never rescan implicitly |
| `tua.api/setOverlay`, `clearOverlay` | In-memory document content for unsaved files |
| `tua.api/diagnostics` | Compiler diagnostics |
| `tua.api/semanticFacts` | Symbols and typed bindings |
| `tua.api/moduleGraph` | Static require edges and exports |
| `tua.api/references` | Project references at a zero-based position |
| `tua.api/assetMetadata` | LOVE asset inventory and reference metadata |
| `tua.api/completionContext` | Bounded FIM prompt, suffix, revision, and fingerprint |
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
