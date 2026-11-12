# View inventory and task fit

Status key: **H** = rendered in current VS Code host with synthetic data; **R** = current actual-server host path; **I** = source/tests inspected; **P** = prior host evidence only. A listed status does not imply every state/theme/scale was exercised. All static compiler views describe declarations, not live game state.

| View | Primary question → next action | Data and tested state | Assessment and remaining work |
| --- | --- | --- | --- |
| Sprite animation | Which frame, duration, origin, or bounds is wrong? → play/step, inspect source rectangle, change sprite declaration. | `tua/inspect` sprite data; H populated/decode, paused playback, narrow, retry, keyboard focus. | Frame sequence, coordinates, and source rectangle earn the space. Real asset variants and compiler/image parity remain open. |
| State machine | Which event moves from one state to another? → select route and open declaration. | `tua/inspect` static machine; H populated, selection restore, double-click source, narrow. | Directed topology and event inspector fit the task. The view does not claim live active state. Large cycles and zoom remain open. |
| Behavior tree | Which branch owns an action and in what order? → expand/select node, open source. | `tua/inspect` static tree; H populated, collapse, order labels, focus, source, narrow. | Hierarchy and sequence numbers convey execution order. Computed branches are labeled as runtime-resolved; live path needs a separate runtime contract. |
| Signals | Who declares, emits, listens, or cleans an event? → filter role, inspect payload/schema conflict, open call or create a typed listener. | `tua/signalGraph`; H populated/empty/narrow/refresh; R populated and source navigation. | Roles and payload are useful; large graph/conflict host fidelity needs a task study. The colored edges alone are insufficient, but the call ledger supplies exact sites. |
| Modules | What pulls a file into the build, and what does it export? → use Roots/Why included/dependents, open source. | `tua/moduleGraph`; H empty/narrow/refresh; R sidebar filesystem replay; existing populated gallery inspected. | PV-01 fixed a hidden filter. Large graph and compiler parity remain unqualified. |
| ECS | Which systems read/write a component? → inspect schema/access and open system use. | `tua/ecsGraph`; H populated/empty/narrow/refresh; R populated and source navigation. | Access labels answer a domain question. Larger read/write matrices and performance insights need data-fidelity checks. |
| Object pools | Where is an item acquired or released? → follow lifecycle sites and item type. | `tua/objectPoolGraph`; H populated/empty/narrow/refresh; R populated and source navigation. | Lifecycle is more relevant than generic edges. A single acquire is not proof of a leak; insight wording needs real-workload evaluation. |
| Audio Workbench | Which cue/scene/layer plays and how does a transition sound? → audition, inspect authored timing, open source/asset. | `tua/audioGraph`; H cue/scene populated, navigation, transition message, error, narrow. | Recent domain-specific workbench distinguishes authored timing from audition; actual audible LOVE host and latency remain unverified here. |
| Assets | Which asset is this, what metadata is known, where is it referenced? → native preview/open reference/rename. | `tua/assetInventory` + VS Code native editors; I current source/unit tests, P installed sidebar replay. | Native editor reuse is appropriate. Test each media type, decoding failures, references, and focus return in installed host. |
| Runtime Inspector | What changed in this running game? → select player/object, pause, inspect value and creation site. | Live runtime socket/tree; I current source/unit tests, P 600-second synthetic soak with 19 reconnects. | Static views should link here only when live data exists. Long real-game use, truncation comprehension, and high-zoom tree navigation remain open. |
| Server Health | Is the server busy, stale, or unmeasured? → inspect queues/cache/latency and copy JSON for diagnosis. | `tua/health`; H empty/narrow and current unit tests. | PV-02 corrected false zero measurements. A populated slow-project trace is needed to judge causality and any timeline design. |
| Generated Lua | What code will run and where did it come from? → open generated Lua and jump between mapped source positions. | Build output + source maps in native text editors; I current command source, existing integration tests. | Native text editor supports diff/search. Unsaved-source rebuild and two-way mapping need a focused representative fixture. |
| Explain Type | Why did this value resolve to this type/`any`? → choose inference step and open its source. | `tua/explainType` in VS Code Quick Pick; I current source. | Step provenance is more useful than a generic type dashboard. Test stale response, keyboard/focus, and cross-file steps in installed host. |
| Offline LÖVE API reference | What is the API contract? → search/read docs and return to code. | Bundled HTML webview; I command/content source. | Command exists; current pass did not verify search, links, accessibility, or focus return in host. |
| Offline annotation guide | What tags can I write? → open Markdown preview and copy a documented form. | Bundled Markdown/VS Code preview; I command/content source. | Native Markdown is suitable; command discovery and return-to-editor path need installed-host check. |
| Stores | What is a live store value? → inspect Stores group in Runtime Inspector. | Runtime inspector tree; I. | No dedicated static Store preview command found. A static dependency view is a capability question, not a redesign of an existing view. |

The current webview suite also verifies webview theme-class switching to light and both high-contrast variants after an error state, but it does **not** compare every populated view in every theme. Reduced-motion and screen-reader announcements for every view were inspected at CSS/DOM level only, not tested with assistive technology. Zoom, truly large datasets, and stale-response recovery are covered unevenly; `preview-refresh.test.js` exercises out-of-order responses for six project webviews.

## State coverage ledger

`H` means exercised now in the VS Code webview host; `R` means exercised now with the real server; `U` means current unit/integration test; `P` means prior documented evidence; `I` means source inspected; `—` means not verified in this pass. A combined cell lists the strongest relevant checks. Light and high-contrast theme switching was exercised once on a webview error page, not separately for every row.

| Surface | Populated | Empty | Loading/error | Stale/edit | Narrow | Large | Source action | A11y/theme/zoom |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Sprite | H | I | H | I | H | — | I | H focus; I theme/motion; — zoom/reader |
| State machine | H | I | I | H selection | H | — | H message | H focus; I theme; — zoom/reader |
| Behavior tree | H | I | I | H selection | H | — | H message | H focus; I theme; — zoom/reader |
| Signal | H/R | H | I | H/U | H | — | H/R | H focus; I theme; — zoom/reader |
| Module | P | H | I | H/U/R sidebar | H | — | I | I keyboard/theme; — zoom/reader |
| ECS | H/R | H | I | H/U | H | — | H/R | H focus; I theme; — zoom/reader |
| Pools | H/R | H | I | H/U | H | — | H/R | H focus; I theme; — zoom/reader |
| Audio | H | I | H | H/U | H | — | H message | H focus; I theme; — zoom/reader |
| Assets/native | P | U | I | U/P | — | — | U | — current host |
| Runtime inspector | P | U | U | P soak | — | P synthetic | U | — current host |
| Server health | U | H | I | U | H | — | n/a | I theme; — zoom/reader |
| Generated Lua | U | n/a | U | U | n/a | — | U | native editor |
| Explain Type | U | U | U | U | n/a | — | U | native Quick Pick; — reader |
| LÖVE reference | I | n/a | I | n/a | — | n/a | — | — current host |
| Annotation guide | I | n/a | I | n/a | — | n/a | — | native Markdown; — current host |
| Stores static | n/a | n/a | n/a | n/a | n/a | n/a | n/a | no dedicated static view |
