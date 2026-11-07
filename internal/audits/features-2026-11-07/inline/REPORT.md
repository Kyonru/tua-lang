# Inline completion feature audit — 2026-11-07

No new confirmed defects in this bounded pass. Deterministic completion and model-context/validation contracts worked through the release binary's actual stdio LSP. The focused extension suite passed 25/25. Real Ollama generation is blocked: `/usr/local/bin/ollama` exists, but `http://127.0.0.1:11434/api/tags` refused connection. No server was started, model downloaded, or normal editor profile changed.

## Reproduction and evidence

Run from repository root:

```sh
node internal/audits/features-2026-11-07/inline/probe.js
node --test editors/vscode/test/unit/inline-completion.test.js
command -v ollama
curl --max-time 2 -sS http://127.0.0.1:11434/api/tags
```

`probe.js` launches the absolute `target/release/tua` executable (0.6.0), writes isolated fixture sources, and captures exact request parameters and responses in `protocol-results.json`. It records 34 operations including initialization and seven endpoint-policy checks. `probe.log` records successful completion; `extension-tests.log` contains all 25 test names and outcomes; `ollama-availability.log` preserves the unavailable-server evidence. Node built-in tests use mocked VS Code APIs, not an editor host. Protocol fixture files intentionally include incomplete code.

## Coverage matrix

| Surface | Status | Observed result |
|---|---|---|
| Default disabled gate | Exercised: LSP + extension | Empty local result; disabled provider does not contact LSP |
| Enable local completion | Exercised: real LSP | Current-file continuation `score`, zero-width range at cursor |
| Standard inline request | Exercised: real LSP | Same recall result; semantic `whi` produces a snippet insertion |
| Lua file recall | Exercised: real LSP | Same current-file continuation as Tua |
| Source switches | Exercised: real LSP | Disabling semantic/current-file/statistical returns no suggestion |
| Empty, comment, string, assignment contexts | Exercised: real LSP | Local source abstains |
| Selected menu item and UTF-16 | Exercised: real LSP + extension | `player.name)` replaces characters 12–15 after emoji; equivalent items suppressed in extension tests |
| Stale document version / fingerprint / edit | Exercised: real LSP + extension | No local suggestion or model rejection with `stale` |
| Local before model / unsupported local server | Exercised: focused extension tests | Local success wins; empty or method-not-found permits model-context fallback |
| Cancellation / superseded request / timeout | Exercised: focused extension tests | Linked work cancelled; late cancellation does not affect newer request; timeout and transient failures do not trigger model work |
| Model context and bounds | Exercised: real LSP | Local lane 8 lines/1024 bytes; function lane 64 lines/8192 bytes; excessive context request capped at 32768 bytes |
| Model validation | Exercised: real LSP | `1` accepted for number; wrong type, Markdown, empty output and foreign syntax rejected; FIM tokens removed |
| Comment/string model context | Exercised: real LSP | Null context |
| Loopback endpoint policy | Exercised: exported production helper | localhost, 127/8 and IPv6 loopback accepted; HTTPS, remote hostname, credentials and spoofed loopback suffix rejected |
| Source-free counters / toggle isolation | Exercised: focused extension tests | Numeric allowlisted counters and migration, batching/reset; deterministic toggle changes its workspace setting only |
| Real model completion / missing-model response / warm command | Blocked | No running Ollama server; no generation-quality or warm-model claim |
| Actual editor acceptance / cursor event integration | Inspected only | Provider event hooks exist; no desktop editor session used |
| Statistical project fallback | Not independently exercised | Shared baseline LSP suite covers warm adoption; this pass does not claim evaluation quality |

## Findings and remaining work

No new finding IDs; `findings.json` is empty. The prior release blocker is still applicable: `target/release-prep-0.6.0/README.md:62` records the existing `inline-eval-smoke` snapshot mismatch. This pass neither reran that aggregate gate nor changed its expectations. `ROADMAP.md:17` already calls for independent real-project deterministic-inline evidence; this small controlled-fixture pass does not satisfy that broader quality evaluation.

The highest-value remaining verification is a consented, already-running local-model run through the actual extension: warm, generate, reject unsafe output, edit/cursor cancellation, and accept one valid insertion. This is a coverage limitation, not evidence of a defect. Existing tests provide good deterministic arbitration and cancellation coverage, but cannot establish real model output quality or desktop behavior.

Implementation inspected: `editors/vscode/src/editor/inline-completion.js` (provider arbitration and cancellation), `editors/vscode/src/editor/ollama-client.js` (endpoint and HTTP transport), `crates/tua-lsp/src/server/handlers.rs:2035` (context conversion/version checks), and the inline/model LSP tests. Owning contract: `docs/lsp.md:244–330`.
