# New finding from the follow-up

## FUP-01 — A second inferred factory hop loses class member completion

**Resolution (November 12, 2026): fixed in `d58f3718`.** Exported
function signatures now carry imported nominal instance identity through the
forwarding module. Project completion refreshes the class shape from the current
snapshot and defers the standalone fast path when another open file has an
unsaved change. A core regression covers the three-module chain; an LSP test
covers hover, completion, and an unsaved class-field edit. The original stdio
probe now returns `health`, `name`, and `takeDamage` for `service.current()`;
an actual VS Code host displayed all three suggestions in 141 ms in one
synthetic sample. The same 100-module profile measured warm completion p95 at
0.46 ms with the fix and 0.50 ms with the pre-fix packaged server. These are
protocol and synthetic-host measurements, not a real game-session latency
guarantee. `make verify` passed with `RUST_MIN_STACK=33554432` (the unchanged
baseline's Rot runtime test overflows the default test-thread stack on this
host), as did `make lsp-features-smoke`. The newly packaged 0.7.0 VSIX passed
both installed real-server suites. The original finding and its pre-fix
evidence follow below.

**Category:** editor correctness / cross-file type provenance. **Severity:** Medium. **Confidence:** High for the observed symptom; Medium for the inferred root cause. **Verification:** CLI check/build/Lua runtime, direct stdio LSP, and actual VS Code host.

The [seven-file fixture](fixtures/crossfile) defines `Agent` in `classes/agent.tua`, exports its instance type, and defines `factory.spawn(): AgentInstance`. A consumer of `factory.spawn()` gets `health`, `name`, and `takeDamage` completion. Another module, `service.tua`, returns `factory.spawn("pilot")` from an **inferred** `current()` function. In the consumer of `service.current()`, hover still says `local agent: AgentInstance`, and `agent.health` checks and runs, but completion after `agent.` returns **zero LSP items**. The installed editor's automatic suggestion widget stayed hidden over a ten-second observation. This creates a particularly confusing state: the type is displayed and a known member is accepted, yet the editor cannot suggest the members.

### Reproduce

```sh
TUA_REPO="$PWD"
cd internal/audits/feature-followup-2026-11-11/fixtures/crossfile
"$TUA_REPO/target/release/tua" check
"$TUA_REPO/target/release/tua" build
LUA_PATH='build/?.lua;build/?/init.lua;;' lua build/main.lua
cd "$TUA_REPO"
python3 internal/audits/feature-followup-2026-11-11/crossfile-lsp-probe.py internal/audits/feature-followup-2026-11-11/fixtures/crossfile
node internal/audits/feature-followup-2026-11-11/visible-latency-run.js
```

The CLI checked and built all seven files; Lua printed `100` and an instance table. The stdio probe returned:

```text
textDocument/completion []
textDocument/hover local agent: AgentInstance
bare completion []
direct constructor ["health", "name", "takeDamage"]
factory return ["health", "name", "takeDamage"]
service return []
annotated second hop ["health", "name", "takeDamage"]
```

The fixture's `service_annotated.tua` declares `current(): AgentInstance` and imports that type explicitly. Its consumer gets all three members in LSP and in the visible VS Code widget. This is a workable interim pattern for developers, but the unannotated function's hover should not advertise a useful class type that completion cannot resolve.

### Affected surfaces and timing

In the 113-file installed-host workload, direct one-hop factory completion reached the **visible** expected entries in 110.5 ms cold, 107.5 ms warm p50, 110.8 ms warm p95 (12 samples). The annotated second-hop case reached them in 115.9 ms cold and 105.4–144.7 ms over five warm samples. A keyboard-invoked hover on the one-hop local appeared in 9.6 ms p50 and 71.1 ms p95 over eight samples; the first sample was 71.1 ms. On the failing unannotated second hop, hover displayed `local agent: AgentInstance` (five visible samples, 7.4–69.3 ms), while typed-dot suggestions never displayed. These are host timings from CDP insertion or Show Hover command to DOM content, not physical-key latency. Three pointer-triggered hover samples took 320.3–325.2 ms including VS Code’s hover delay. The protocol-only hover for the one-hop local took 2.9 ms on one request.

### Likely cause and regression coverage

This appears to be a provenance mismatch between exported inferred return types and the consumer's alias resolver. The return is preserved as the printed name `AgentInstance`, but the second module does not import that alias and its export may lack enough origin information for member lookup. [Export inference](../../../crates/tua-core/src/module_index/mod.rs) walks function returns; `resolve_export_aliases_inner` at lines 1424–1451 intentionally preserves some non-class aliases. [Completion](../../../crates/tua-core/src/completion/mod.rs) at lines 2033–2065 resolves the receiver and collects fields, while `collect_table_fields_inner` at lines 2644–2687 emits none when an alias remains unresolved. This is an inferred root cause; a trace of the exact exported `Type` at each hop is still needed before choosing a fix.

The current exported-class tests cover direct imports and members, and generic tests cover imported class applications, but this exact **class → annotated factory → unannotated forwarding function → consumer** chain is absent. Add a project snapshot regression that asserts check, hover, completion, signature/definition where applicable, and the same after an unsaved edit to the defining class. Test both the unannotated and explicitly annotated forwarding variants. Preserve canonical class/alias identity through module export inference so the checker and editor use the same resolved type rather than adding a completion-only fallback.
