---
name: tua-ai-bridge
description: Maintain Tua's model language contract, completion prompts, local JSON-RPC API, or MCP adapter. Use when changing AI-facing facts, tool schemas, source-context behavior, or model integration; not for ordinary LSP completion alone.
---

# Tua AI Bridge

Give an assistant current compiler facts without making a second language
engine or granting it implicit access to source and execution.

## Contract Ownership

- Edit `docs/llms.txt`, then run `make generate-llms`; root `llms.txt` is the
  generated copy embedded by `tua-core`. Check the focused feature docs and
  runnable examples before adding an API claim. `ROADMAP.md` is not evidence
  of implementation.
- The contract guides both model prompts and `tua.api/languageContract`/
  `tua_language_contract`. Prefer concrete, concise syntax and negative
  boundaries: Lua 5.1 output, gradual `any`, erased types, supported generics,
  and opt-in game helpers. Do not suggest TypeScript, Luau, or invented APIs.
- `benchmarks/llm-language/cards/` contains the compact offline evaluation
  prompts, while `tasks.js` contains reference answers and negative controls.
  Keep these aligned with the documented language; a benchmark must not reward
  avoiding supported syntax. Run `make llm-language-eval-test` after changing
  them and distinguish its compiler-oracle checks from model quality results.
- Keep API/MCP semantic facts from the shared analysis snapshot. The CLI owns
  newline-delimited JSON-RPC/MCP framing; adapters must not rescan or infer a
  different type model. See `docs/external-api-bridge.md` for current methods.

## Trust And Freshness

- These transports are explicitly started, local, read-only analysis sessions.
  Keep workspace path confinement, bounded responses, overlay precedence,
  explicit disk refresh, and protocol-version negotiation.
- Source excerpts and source-bearing previews require the documented
  `sourceContext` capability or an explicit MCP tool call. Do not make a model
  request, run project Lua, apply edits, or start a network listener merely
  because a client asks for facts.

## Verify

- Run `make llms-drift-check` and focused core language-contract/prompt tests.
  Exercise `tua api --stdio` and `tua mcp --stdio` tests when transport or
  privacy behavior changes. Compare an unsaved overlay and a disk refresh so
  returned facts cannot silently become stale.
- Distinguish a compact model prompt from user documentation; update the
  owning docs page when the product contract changes.
