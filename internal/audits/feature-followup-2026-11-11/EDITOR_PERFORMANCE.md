# Editor and formatter latency

Current source revision: `ab1b69c506f962ab239e015d2db4c38764fd592a`; extension version 0.7.0; macOS arm64; VS Code 1.142.0; release server `target/release/tua`. A disposable 113-file Tua project and a JavaScript control were opened in an isolated VS Code development host. The [runner](visible-latency-run.js) and [host suite](visible-latency-suite.js) use CDP text insertion and a renderer MutationObserver to time from immediately before inserting `.` until the actual suggestion widget displays the expected `health` and `name` entries. The suite also times `document.save()` with `editor.formatOnSave=true` and verifies the resulting source. `editor.quickSuggestionsDelay` was 10 ms.

```sh
node internal/audits/feature-followup-2026-11-11/visible-latency-run.js
```

Latest host run, 12 repeated samples except where stated:

| Scenario | Observed latency |
| --- | ---: |
| Tua first typed-dot to visible suggestions | 296.3 ms |
| Tua warm typed-dot to visible suggestions | p50 161.3 ms; p95 171.4 ms |
| Tua manual Trigger Suggest to visible suggestions | p50 109.8 ms; p95 113.3 ms |
| JavaScript typed-dot control | p50 108.7 ms; p95 361.7 ms, driven by a cold JS-service sample; remaining samples were near 107–114 ms |
| Tua after switching to another imported source, then back | 165.0 ms; 156.3 ms |
| Format-on-save, two-line source | p50 12.7 ms; p95 17.8 ms |
| Format-on-save, 1,001-line source | five saves: 32.5, 18.3, 15.6, 19.0, 22.7 ms |

An earlier 103-file host run observed first Tua suggestion 230.2 ms, warm p50 161.7 ms/p95 171.5 ms, manual p50 109.7 ms/p95 113.2 ms, and two-line save p50 9.6 ms/p95 48.1 ms (first save 48.1 ms). Together these show a repeatable roughly 150–180 ms visible warm suggestion interval for the original imported-table case, while formatter saves stayed below 50 ms in these synthetic files. The unannotated two-hop class case below did not show a suggestion menu. This is an observed experience gap against the roughly 109 ms manual/JavaScript control in the same host, **not** a proven regression against an earlier Tua version.

## Imported class through factories

The host also opened a [class chain](fixtures/crossfile): `Agent` → typed `factory.spawn` → consumer. The expected `health`, `name`, and `takeDamage` entries were visible in 110.5 ms on first trigger, p50 107.5 ms and p95 110.8 ms across 12 warm triggers. Keyboard-invoked hover on `agent` displayed `local agent: Agent` in p50 9.6 ms and p95 71.1 ms across eight samples. Pointer-triggered hover appeared in 320.3–325.2 ms over three samples, including the editor hover delay. One protocol-only hover took 2.9 ms.

An unannotated `service.current()` forwarding function one file deeper is a [confirmed editor finding](FINDINGS.md): LSP completion returned no members; the installed widget stayed hidden over ten seconds, though visible hover displayed `local agent: AgentInstance` in 7.4–69.3 ms across five samples. Explicitly importing `AgentInstance` and annotating `service.current(): AgentInstance` restored the three visible suggestions (115.9 ms first, 105.4–144.7 ms over five warm triggers). [Raw host measurements](visible-latency.json) include all samples and the deep-case widget observation.

This follow-up's protocol-only `project-100` warm completion p95 was 0.47 ms, consistent with the prior audit's 0.43 ms. That number measures a different boundary; it does not explain or replace the visible 160 ms interval. The extra time can include VS Code trigger scheduling, client transport, semantic/editor work, and rendering. A trace that timestamps trigger, client send, server response, widget creation, and final label paint is needed before assigning the delay to a component.

Limits: synthetic source and modules, 12 warm samples, automated CDP insertion rather than a physical keyboard, no real game workspace, and no previous-release visible-menu baseline. The JavaScript control's cold outlier illustrates why p95 from 12 samples should not be treated as a stable SLA. The host ran during a separate LOVE inspector soak, so system contention may affect absolute timings; the earlier run without that soak showed similar warm Tua values. Server-only profile results and inspector UI results are recorded in [VERIFICATION.md](VERIFICATION.md).

## November 12 Starfall recheck

The later [Starfall qualification](NOV12_QUALIFICATION.md#visible-completion-comparison) adds an actual bundled game workflow and comparison to the existing 0.7.0 VSIX. Warm typed-dot to visible labels was 124.6 ms median in the current host and 121.4 ms in the earlier package, eleven samples each. Opt-in current-extension middleware measured 3.0 ms median from trigger to provider entry, 17.4 ms inside the provider, and 102.9 ms from provider return to expected visible labels. The last interval includes VS Code scheduling and renderer work; precise wire and paint timestamps remain unmeasured. The Starfall run also caught, and the product fix resolved, missing returned-local numeric module fields in completion.
