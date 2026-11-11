# Large previews and native asset follow-up

At revision `ab1b69c506f962ab239e015d2db4c38764fd592a`, a disposable VS Code 1.142.0 development host on macOS arm64 loaded the production preview templates. Run:

```sh
node internal/audits/feature-followup-2026-11-11/preview-stress-run.js
```

The [fixture script](preview-stress-suite.js) builds 500 events with 1,500 call sites and 500 modules with 499 dependency edges. These are intentionally generated view models; they stress rendering and interaction but do not independently verify a 500-module compiler response. The native asset is the tracked 192×16 `player.png` fixture opened through the extension's `tua.assets.preview` command.

| Host check | Observed result |
| --- | --- |
| Signal graph | 500 events / 1 file / 1,500 calls rendered in 81 ms from assigning webview HTML to populated DOM. Filtering to `event_499` took 1.4 ms in the renderer, left one event, and a call-row source action posted `openLocation`. |
| Module graph | 500 modules / 499 dependencies rendered in 85 ms. Filtering to `module_499` took 4.9 ms and retained two nodes because the preceding module's dependency also matches; double-click posted `openLocation`. |
| Native asset | `tua.assets.preview` returned the file URI; active editor tab was `player.png` at that URI. The [host capture](native-asset.png) visibly shows the image in VS Code's Image Preview, 192×16, 228 B. |
| Renderer exceptions | Zero in the graph webview and workbench CDP sessions. |

One first harness run asserted that module filtering should leave one node; it failed because the filter intentionally matches dependency names as well as module names. The corrected assertion expects two and the next two host runs passed. The initial native screenshot was taken before VS Code painted the image; a subsequent capture after 750 ms shows it. Neither harness correction is a product finding.

The observed render times include host/webview startup and IPC for this one synthetic workload, so they are useful as local qualification rather than a benchmark across machines. Long-term interaction, 1,000+ nodes, large real projects, accessibility navigation, and actual project graph fidelity remain separate qualifications. The existing [preview audit](../feature-and-preview-2026-11-11/PREVIEWS.md) covers smaller populated, empty, error, narrow, and refresh states.
