# Follow-up priorities

The current working-tree fixes and permanent tests are described in the lane reports. This backlog separates remaining qualification from unsupported language design. Effort is an engineering estimate, not a promise.

| Priority | Work | Category / effort | Dependency and acceptance criteria |
|---|---|---|---|
| 1 | Extend the three completed game trials to larger class/event-driven games | Qualification / medium | Preserve source, exact commands, actual LSP responses and runtime smoke. Current two-module record-based trials do not establish class/event ergonomics at scale. See `games/`. |
| 2 | Qualify installed-editor long sessions and larger real projects | Performance / medium | Measure cold/warm completion, unsaved import invalidation, hover, format-on-save and memory over sustained edits. Report key-to-menu separately from protocol latency, with repeatable workloads and machine metadata. |
| 3 | Cross-platform test process and editor lifecycle qualification | Robustness / medium | Exercise cancellation, child trees, workspace changes during tests and report limits on Windows/Linux/macOS. Existing Unix force-kill and output-cap tests are not Windows evidence. |
| 4 | Additional escaped-literal consumers | Correctness investigation / small | Asset/Sprite collection now shares the Lua decoder. Create runtime-equivalent escaped module/signal name fixtures before claiming other raw-token consumers are defects. Reuse the shared decoder if confirmed. |
| 5 | Complete native GUI and full-game visual qualification | Qualification / medium | Existing screenshot hashes and targeted completion/rename checks do not establish every popup, overload, game port or visual behavior. Exercise selected native games and editor surfaces with explicit scenarios. |
| 6 | Completion for keys requiring brackets and richer annotation authoring | Usability / medium | Define insertion behavior for dot/colon contexts, preserve receiver and suffix, validate emitted edits. Keep label/signature documentation from canonical semantic facts. |
| 7 | Exact default-expression display and overload ranking | Capability / medium | Specify stable display and ranking semantics; share signature metadata across completion/hover/help and test nullable versus optional parameters. |
| 8 | Protocols/generics, dynamic service/mixin modeling and class initialization policy | Design / large | Requires language design and compatibility decisions. Do not infer support from successful class-polymorphism fixtures. |

Implementation should preserve shared declaration identity, source-coordinate conversion and project ownership. Prefer extending the existing semantic contracts and permanent regression suites over feature-specific parsing or line scans. The game trials confirmed a formatter defect, now fixed with permanent regression tests. Their remaining capability ideas are distinct from that correctness fix.

Game-trial capability observations: a CLI project-selection option could reduce directory-changing boilerplate; combined type/runtime import ergonomics and clearer no-return function labels deserve evaluation. None was established as a broken documented contract.
