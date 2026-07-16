---
icon: lucide/unplug
---

# External API Bridge Design

Tua may eventually expose a local API session for tools that need structured
project facts without pretending to be an editor. This page is a design record,
not an implemented feature. No bridge starts today unless a future command or
preview setting explicitly starts it.

## Goals

- Give trusted local tools read-only access to the same project facts used by
  the CLI and LSP: diagnostics, asset metadata, module facts, semantic facts, and
  cache/profile summaries.
- Keep the bridge separate from the core LSP path. `tua lsp` must remain fully
  functional when no bridge exists, fails, or is disabled.
- Reuse `tua-core` `AnalysisHost` and `AnalysisSnapshot` queries instead of
  adding parallel semantic inference.
- Make security boundaries explicit before implementation: local-only transport,
  explicit startup, workspace trust, authentication, and source-data limits.

## Non-Goals

- No remote server mode, cloud sync, telemetry, or internet-facing endpoint.
- No execution of project Lua, `require(...)`, LOVE callbacks, or extension runtime
  code.
- No write/refactor API in the first bridge. Editor writes should keep using
  standard LSP workspace edits and code actions.
- No replacement for LSP. Editors should continue using LSP for hover,
  completion, diagnostics, navigation, formatting, and code actions.
- No source-content export by default. Tools can request ranges, paths, types,
  diagnostics, and metadata without receiving full file text.

## Transport

The bridge should support two local-only transports when a concrete consumer
exists:

- `stdio`: a parent process starts `tua api --stdio` and talks JSON-RPC over the
  child process pipes. This is the safest first implementation because process
  ownership is explicit and no socket is left behind.
- `local pipe/socket`: a parent process starts `tua api --pipe <path-or-name>`
  with an owner-only Unix domain socket or Windows named pipe. TCP is out of
  scope for the first bridge; if it is ever needed, it must be loopback-only,
  token-authenticated, and preview-gated.

The server must never auto-start from `tua lsp`. A VS Code command or future tool
may start it only when the user opts in, for example through a command-palette
action guarded by `tua.preview.enabled`.

## Authentication And Trust

- `stdio` sessions inherit trust from the process that launched them. The parent
  chose the executable and owns the pipes.
- Pipe/socket sessions require a randomly generated bearer token for the first
  `tua.api/initialize` request. The token is passed out-of-band by the launcher,
  never logged, and invalidated when the process exits.
- Socket files must be created under an owner-only runtime directory. On
  platforms that expose peer credentials, the bridge should reject connections
  from a different user.
- VS Code must require workspace trust before launching a workspace-local Tua
  binary or a bridge session that can read workspace files.
- Terminal usage such as `tua api --stdio` counts as explicit user consent, but
  the bridge still remains local-only and read-only by default.
- The bridge respects `tua.toml`, editor settings passed at initialization,
  configured source roots, and ignored paths. Ignored paths are not indexed or
  returned.

## Request Model

The bridge should use versioned JSON-RPC requests under the `tua.api/` namespace.
Request and response shapes should be stable enough for a consumer but are not
part of the language syntax contract until documented as stable.

| Request | Purpose | Notes |
| --- | --- | --- |
| `tua.api/initialize` | Starts a session with workspace roots, settings, client name, requested capabilities, and authentication token when required. | Returns protocol version, Tua version, session id, supported requests, trust mode, and initial snapshot revision. |
| `tua.api/shutdown` | Ends the session and removes pipe/socket resources. | Also sent automatically on process exit where possible. |
| `tua.api/projectSummary` | Returns roots, config source, enabled feature flags, indexed file counts, and current snapshot revision. | No source text. |
| `tua.api/diagnostics` | Returns diagnostics for a file, workspace root, or full project scope. | Uses the same diagnostic codes and pipeline as `tua check` and LSP diagnostics. |
| `tua.api/assetMetadata` | Returns known LOVE asset facts for a path, prefix, or loader context. | Includes status, kind, inferred LOVE type, extension, and cheap metadata such as dimensions when already available. No file bytes. |
| `tua.api/semanticFacts` | Returns symbol/type facts for a URI and position or a known symbol id. | May include kind, display type, documentation summary, definition location, type-definition location, module/export origin, and reference count. |
| `tua.api/moduleGraph` | Returns static module edges and exported symbols for indexed files. | Dynamic `require(...)` expressions are reported as dynamic/unknown rather than guessed. |
| `tua.api/references` | Returns project references for a URI and position. | Reuses the same reference/index data as `textDocument/references`. |
| `tua.api/cacheStats` | Returns analysis cache counters and snapshot revision metadata. | Mirrors the existing `tua/cacheStats` LSP request where possible. |

Future requests must be added only when a real consumer needs them. Broad
catch-all requests such as “dump the whole snapshot” should be avoided because
they make privacy, performance, and compatibility harder to control.

## Lifecycle

1. A user or trusted tool explicitly starts the bridge process.
2. The bridge initializes an `AnalysisHost` with provided workspace roots,
   settings, and optional open-document overlays.
3. The first request authenticates the session and returns supported
   capabilities.
4. Read-only requests take immutable `AnalysisSnapshot` values and return
   structured JSON data.
5. File, configuration, and open-document changes increment the host revision.
   A future long-running session may either accept explicit change notifications
   from its launcher or own file watchers; watcher ownership should be designed
   per consumer.
6. Idle sessions shut down after a configurable timeout unless the launcher opts
   into a persistent session.
7. Shutdown removes local pipe/socket resources and drops in-memory project
   state.

If the bridge crashes, the LSP and CLI remain unaffected. If the LSP crashes,
the bridge remains unaffected unless both were started by the same supervising
tool and that tool decides to stop both.

## Data Exposure Rules

- Default responses return paths, ranges, type strings, docs, diagnostics,
  symbol ids, metadata, and counts.
- Full source text is never included by default. Any future source-content
  request must require an explicit `includeSource` capability and should be
  limited to a specific file/range.
- Diagnostics and semantic facts may include snippets only when the existing
  diagnostic formatting path already includes them for the same caller.
- Asset metadata never returns raw file bytes.
- Dynamic Lua facts are represented as `unknown`, `dynamic`, or `any`; the
  bridge should not invent precision that the checker/LSP cannot prove.

## Implementation Slices

Implementation should wait for a concrete consumer. When that exists, keep the
work split into small commits:

1. Add shared request/response DTOs and tests around `tua-core` snapshot queries.
2. Add a `tua api --stdio` prototype in `tua-cli`, disabled unless invoked
   directly.
3. Add authentication and local pipe/socket transport only after stdio has a
   consumer.
4. Add an opt-in VS Code command or connector integration behind
   `tua.preview.enabled` only if editor-side tooling needs it.
5. Add profiling and cache limits after request shapes are stable.

The acceptance bar for the first implementation is: no bridge starts by default,
authentication and trust are documented and tested, LSP behavior is unchanged,
and every bridge request routes through existing `AnalysisHost` or
`AnalysisSnapshot` queries.
