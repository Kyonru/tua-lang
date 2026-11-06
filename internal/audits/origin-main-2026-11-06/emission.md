# Emission, normalization and hot-reload audit

Scope: origin/main..HEAD at d7aed2e, plus fixes in the current working tree. Inspected changed parser normalization/model/LuaDoc collection, class lowering/provenance, builtin helper lowering, lowering operations and hot-reload implementation. No unsupported language feature was introduced. Other lanes own checker/editor inference and CLI orchestration.

## EMIT-01 — typed function header trivia panics emission (High, fixed)

Prior audit CMP-02 independently established `local function f(x: number) -- hello\n return x\nend` panics `check` and diagnostic workers. The emitter rendered the closing-parenthesis token together with its trivia and passed it to `TokenReference::symbol`, which rejects a comment as part of a symbol. The return-annotation branch also discarded comments.

The fix preserves the unchanged parenthesis token when no return annotation exists, and transfers annotation trailing trivia structurally when erasing a return type. Full Moon's `Node::tokens()` visits contained delimiters before contents: `.last()` is not the lexical last token of tuples/records. A focused type-token helper handles those structured endings without reparsing rendered text. Regression covers ordinary, suppression, anonymous/block, CRLF, scalar-return and tuple-return header comments, asserting exact output and Lua 5.1 parse acceptance.

Changed files: `crates/tua-core/src/emit/{mod,tests}.rs`. Reproduce: `RUST_MIN_STACK=16777216 cargo test -p tua-core function_header_comments_survive_annotation_erasure`. Owning emitter suite: `RUST_MIN_STACK=16777216 cargo test -p tua-core emit::tests -- --test-threads=1`: **89 passed**. An initial default-stack run aborted in existing anonymous nested-default test; rerun uses the repository's established larger-stack setting. Log: `target/origin-main-audit/emitter-tests.log`.

## EMIT-02 — private constructor helpers capture replaced class tables (Medium, fixed)

Minimal Lua shape: `local Actor={}; local function make() return setmetatable({},Actor) end; function Actor.new() return make() end; return {Actor=Actor}`. After reload, the exported constructor captures only `make`; the old rebinder inspected only direct upvalues and left private helper `Actor` pointing to the newly loaded table. New instances then fail `getmetatable(instance)==preservedActor` and stop receiving later method updates. The existing direct constructor fixture missed the indirection.

Reproduced before fixing by adapting `vendor/tests/hotreload_test.lua` into `target/origin-main-audit/private-helper-hotreload.lua`; Lua 5.4 failed `reloaded constructors retain class identity`. The runtime now visits function-valued upvalues using the same visited set. It deliberately does not traverse arbitrary upvalue tables such as `_ENV` or imported state. The regression includes mutually recursive private helpers so cycles cannot recurse forever, and checks old/new instances across successive reloads.

Changed files: `vendor/runtime/tuahotreload.lua`, `vendor/tests/hotreload_test.lua`. Verification: `LUA_PATH='vendor/runtime/?.lua;;' lua vendor/tests/hotreload_test.lua`: **exit 0**, including shader/error-recovery/data-preservation controls. LuaJIT/actual LOVE runtime unavailable in shell; no runtime parity claim beyond Lua 5.4 and existing Lua 5.1 syntax validation.

## Remaining boundaries

Class source provenance is covered by existing emitter tests (method body, following multiline/non-ASCII declarations, generated default guards). Editor expression-coordinate defects are owned by the editor lane. Class initialization order remains documented behavior; no redesign attempted. Helper/default lowering and compound assignment controls pass the emitter suite. Full workspace/gates and CLI source-map runtime integration are lead-owned; this lane does not claim their execution.

Additional normalization verification: `RUST_MIN_STACK=16777216 cargo test -p tua-core parser::tests -- --test-threads=1`: **80 passed** (`target/origin-main-audit/parser-tests.log`).

## Integrated runtime qualification update

The lead subsequently located LOVE 11.5 at `/Applications/love.app/Contents/MacOS/love` and executed the runtime oracle under its **LuaJIT 2.1.1700008891**. I independently read the saved output: `tween_test`, `camera_test`, `hotreload_test` (including the new private-helper cycle regression), and `baton_test` all passed. Durable copied evidence: `runtime-luajit-evidence.log` in this directory; original log `/tmp/tua-runtime-luajit-audit.log`. This supersedes the earlier shell-PATH limitation for the tested hot-reload runtime, but does not imply full-game visual qualification.

## Integrated handoff

The checkpoints above preserve investigation history, including corrected fixture mistakes and intermediate failures. [Final integrated verification](VERIFICATION.md) is authoritative for the finished working tree, rebuilt-server replay and test VSIX; earlier counts and intermediate binaries are not final package provenance.
