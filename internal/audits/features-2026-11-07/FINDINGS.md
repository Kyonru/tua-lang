# Consolidated feature findings

65 confirmed defects: **8 High, 49 Medium, 8 Low**. All eight High findings have independent reproduction evidence. This is a representative feature audit, not a claim of universal coverage.

Each linked feature report supplies reproduction commands, expected/actual behavior, evidence, implementation references, test gaps and proposed regression coverage. Original finding IDs remain stable. Machine-readable index: [findings.json](findings.json).

## Confirmed defects

| ID | Severity | Finding and detailed evidence |
|---|---|---|
| STORES-03 | Medium | [Reentrant set returns a later state rather than its argument](stores/REPORT.md) |
| SIGNALS-01 | Medium | [`once` can invoke twice during nested emits](signals/REPORT.md) |
| SIGNALS-02 | Medium | [Statically invalid handlers compile and fail at runtime](signals/REPORT.md) |
| SIGNALS-03 | Medium | [Directly exported bus loses definition/reference identity](signals/REPORT.md) |
| SM-01 | Medium | [Event names can overwrite typed API fields without a diagnostic](state_machines/REPORT.md) |
| SM-02 | Medium | [A present nonliteral initial state becomes a fictional `none`](state_machines/REPORT.md) |
| SM-03 | Medium | [Cancelled before callback still executes leave side effects](state_machines/REPORT.md) |
| SM-04 | Medium | [Chaining an asynchronous transition from an after callback loses it](state_machines/REPORT.md) |
| ECS-01 | Medium | [get/remove erase component absence](ecs/REPORT.md) |
| ECS-02 | Medium | [World:get converts stored false to nil](ecs/REPORT.md) |
| ECS-03 | Medium | [Documented optional component schema is required](ecs/REPORT.md) |
| POOLS-01 | Medium | [Typed factory does not constrain reset or dispose callbacks](pools/REPORT.md) |
| POOLS-02 | Medium | [Reentrant reset corrupts ownership counts and free stack](pools/REPORT.md) |
| INPUT-01 | Medium | [Numeric spelling bypasses deadzone validation](input/REPORT.md) |
| INPUT-02 | Medium | [Statically empty controls are treated as unknown controls](input/REPORT.md) |
| AUDIO-01 | Medium | [Overlapping global and cue limits are not enforced together](audio/REPORT.md) |
| AUDIO-02 | Medium | [Bar transitions miss boundaries crossed by a long frame](audio/REPORT.md) |
| AUDIO-03 | Medium | [Looping music stops emitting beats after the first playback cycle](audio/REPORT.md) |
| AUDIO-04 | Medium | [Loop regions are ignored when a scene has no BPM](audio/REPORT.md) |
| AUDIO-05 | Low | [snapshot analysis state is not detached](audio/REPORT.md) |
| SPRITES-01 | High | [Zero duration lists hang playback](sprites/REPORT.md) |
| SPRITES-02 | Medium | [pauseAtStart stops on final frame](sprites/REPORT.md) |
| SPRITES-03 | Medium | [Loop callbacks lose multi-wrap counts](sprites/REPORT.md) |
| VECTORS-01 | Medium | [Chained constructor property reads escape builtin lowering](vectors/REPORT.md) |
| VECTORS-02 | Medium | [Game-math helper injection misses comments and bracket members](vectors/REPORT.md) |
| VECTORS-03 | Low | [Nonzero tiny vector divisors incorrectly raise zero-component error](vectors/REPORT.md) |
| VECTORS-04 | Low | [Normalization collapses finite large and small vectors to zero](vectors/REPORT.md) |
| CAMERA-01 | Medium | [Aliasing Camera skips runtime injection](camera/REPORT.md) |
| CAMERA-02 | Medium | [Rotated views escape world bounds](camera/REPORT.md) |
| CAMERA-03 | Low | [Pixel-perfect drawing and coordinate conversion disagree](camera/REPORT.md) |
| CAMERA-04 | Medium | [visibleBounds excludes part of the shaken view](camera/REPORT.md) |
| CAMERA-05 | Low | [Platformer preset contradicts documented deadzone proportions](camera/REPORT.md) |
| TWEEN-01 | Medium | [Callback cancellation can crash tween updates](tween/REPORT.md) |
| TWEEN-02 | Medium | [Shorthand after accepts invalid duration](tween/REPORT.md) |
| CLASS-01 | High | [Flattening constructor self references loses object identity](classes/REPORT.md) |
| TYPING-01 | High | [Dictionary dot writes bypass the value contract](typing/REPORT.md) |
| TYPING-02 | High | [Existing-local reassignment skips missing RHS slots](typing/REPORT.md) |
| TYPING-03 | Medium | [Short callbacks reject ignored trailing arguments](typing/REPORT.md) |
| TYPING-04 | Medium | [Optional record fields reject nil deletion](typing/REPORT.md) |
| FUNCTIONS-01 | Medium | [Return expressions skip call validation without matching result slots](functions/REPORT.md) |
| FUNCTIONS-02 | Medium | [Default expressions bypass declared parameter types](functions/REPORT.md) |
| FUNCTIONS-03 | Medium | [Explicit nil rejected for defaulted parameters](functions/REPORT.md) |
| MODULES-01 | Medium | [Inline require field extraction loses types](modules/REPORT.md) |
| MODULES-02 | Medium | [Imported ECS component loses payload schema](modules/REPORT.md) |
| MODULES-03 | Medium | [Declaration overlay disagrees across hover, signature and result inference](modules/REPORT.md) |
| LUA-INTEROP-01 | Medium | [Documented metatable colon call rejected](lua_interop/REPORT.md) |
| LUA-INTEROP-02 | Medium | [Lua exports discard overload call contracts](lua_interop/REPORT.md) |
| LOVE-01 | Medium | [Known void LOVE call chains pass checking and fail at runtime](love/REPORT.md) |
| ASSETS-01 | High | [Pipeline output-root change overwrites unowned source files](assets/REPORT.md) |
| ASSETS-02 | Medium | [Corrupted cached output bytes are materialized without rebuild](assets/REPORT.md) |
| SHADERS-01 | Medium | [Macro-defined uniform names trigger false missing-uniform errors](shaders/REPORT.md) |
| FMT-01 | High | [Formatting removes semantic whitespace on multiline long-string opening lines](formatter/REPORT.md) |
| LINTER-01 | High | [Safe unused-local fix deletes neighboring same-line statements](linter/REPORT.md) |
| TESTING-01 | Medium | [Structured reports hide backend startup diagnostics](testing/REPORT.md) |
| PROJECTS-01 | High | [Special-case cleanup deletes files without build ownership](projects/REPORT.md) |
| WATCH-01 | Medium | [dev retains stale output root after configuration reload](watch_reload/REPORT.md) |
| WATCH-02 | Medium | [New conf.lua is never tracked for restart after startup](watch_reload/REPORT.md) |
| DBG-01 | Medium | [Spaces in generated paths disable traceback remapping](debugging/REPORT.md) |
| EDITOR-01 | Medium | [Colon string completion enumerates receiver internals](editor/REPORT.md) |
| EDITOR-02 | Low | [Shader hover nests GLSL Markdown in a Tua fence](editor/REPORT.md) |
| API-MCP-01 | Medium | [Refresh reuses analysis revisions across changed snapshots](api_mcp/REPORT.md) |
| API-MCP-02 | Medium | [Project diagnostics omit new unsaved overlay files](api_mcp/REPORT.md) |
| API-MCP-03 | Low | [JSON-RPC envelope and notification handling is inconsistent](api_mcp/REPORT.md) |
| PACKAGING-01 | Medium | [Fused macOS app retains LOVE document-handler claim](packaging/REPORT.md) |
| SETUP-DOCS-01 | Low | [Setup README omits the shipped shader step](setup_docs/REPORT.md) |

## Capabilities and design questions

These six indexed opportunities are not included in the defect count. Additional opportunities and uncertainties appear in each report and the backlog.

| ID | Priority signal | Classification | Detail |
|---|---|---|---|
| ECS-GAP-01 | Medium | feature gap | [System callback schema does not flow into checking/hover](ecs/REPORT.md) |
| LOVE-02 | Low | feature gap | [Unannotated LOVE callback parameters remain any](love/REPORT.md) |
| LINTER-02 | Medium | design question | [Explicit import cleanup changes module initialization effects and order](linter/REPORT.md) |
| EDITOR-G01 | Low | feature gap | [Same-arity overload selection ignores literal argument](editor/REPORT.md) |
| EXPLORERS-01 | Low | feature gap | [Sidebar graph cache does not follow unsaved or filesystem changes](explorers/REPORT.md) |
| EXPLORERS-02 | Low | feature gap | [Project files omits Lua sources that Modules includes](explorers/REPORT.md) |

## Reconciliation and provenance

75 original records comprise 69 defect records, five capability gaps and one design question. Four duplicate defect records reduce the unique defect count to 65:

- STORES-01 and POOLS-03 → TYPING-03: trailing callback parameters.
- STORES-02 → TYPING-04: optional-field nil assignment.
- SHADERS-02 → EDITOR-02: nested Markdown fences.

Original reports are retained so each affected workflow has its own reproduction. MODULES-01/02 resolve earlier store/ECS import observations; these are not extra findings. AUDIO-01 was reviewed down to Medium: the demonstrated violation is bounded; parent-bus cap inheritance needs a documented policy. Camera and vector helper discovery share a proposed architectural direction but have distinct implementations and remain separate findings.

TYPING-02 is a reproduced residual of the earlier TYP-05 family, not a claim of a newly discovered issue. The raw typing records’ `known` flags on TYPING-03/04 refer to overlap within this audit. Two nominal-type test failures and the inline snapshot mismatch from release preparation remain separate baseline blockers, excluded from the 65.

See [independent reproductions](cross-checks/README.md), [evidence review](reviews/evidence-review.md), and [coverage review](reviews/coverage-review.md).
