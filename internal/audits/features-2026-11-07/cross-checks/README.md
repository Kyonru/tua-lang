# Independent lead verification

The feature agent's reproduction was rerun by the lead. These are protocol,
CLI or runtime checks, not inferred from reading the implementation. Fake
user data was created only in disposable audit projects.

| Finding | Independent evidence | Result |
|---|---|---|
| ASSETS-01 | asset-ownership.json | Existing fake user content overwritten after output-root change; both checks exit 0 |
| PROJECTS-01 | build-ownership.json | Handwritten sibling Lua file deleted; build exits 0 |
| FMT-01 | formatter-string.json | build/run 5, format, build/run 3 |
| LINTER-01 | linter.js, linter.json | Fresh actual LSP fix-all deletes print; observable stdout lost |
| CLASS-01 | class-self.json | Self-identity assertion fails; local-capture control passes |
| TYPING-01 | dictionary-assignment.json | Invalid dot mutation passes checking then crashes on arithmetic |
| TYPING-02 | missing-assignment-slot.json | Missing RHS passes checking then crashes on nil concatenation; known family |
| SPRITES-01 | sprite-zero-duration.log | Actual LOVE instruction guard aborts nonterminating update; timer stays zero |
| AUDIO-01–05 | audio-runtime.txt | Deterministic Source-double probes reproduced; no audible/device claim |
| SIGNALS-01 | first-domains.json | Emitted Lua invokes once listener twice |
| SM-01 | state-machine-contract.json | Normal path passes; reserved event checks clean then crashes |
| SM-03/04 | first-domains.json | Cancelled transition invokes leave; nested pending async transition lost |
| STORES-01 / TYPING-03 | store-callback.json | One-parameter callback wrongly rejected in isolated project |
| POOLS-02 | pools-runtime.txt | Negative active count and duplicate checkout reproduced |
| ECS-01 | ecs-absent-components.json | get/remove check clean; both crash on nil member access |
| VECTORS-01 | vector-constructor.json | Chained constructor checks clean; emitted global vec2 is nil |

The initial store direct-file CLI invocation in first-domains.json was a harness
mistake (CLI expects a project), superseded by store-callback.json. It is not
counted as a product finding. Other feature agents' cross-checks are recorded
in their reports and in FOLLOWUPS.md.
