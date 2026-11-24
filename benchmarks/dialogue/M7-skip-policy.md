# M7 host advance and skip policy

This slice completes M7 on the feature branch. It adds optional typed host code
and documentation; the core player, handle API, compiler and authored syntax are
unchanged. No merge or publication is included.

## Contract

Finish-text, line-advance and conversation-cancellation remain separate host
commands. The audio example finishes silently while keeping the recording; line
advance stops that recording. Choices require explicit selection, and advance
cannot bypass an authored wait or action.

The `dialogue-actions` example binds one skip policy to one player. A handler
registers its current generation/visit/node with `begin(request, cleanup)` to
explicitly permit cancellation, or omits cleanup to protect non-cancellable work.
Conversation skip refuses unstarted, untracked, non-cancellable or already-settled
actions and suspended players. Settled results must be consumed through the normal
`runAction` path. Accepted skip uses player cancellation, never a manufactured
result or a jump to an action target. An old visit's capability cannot authorize
cancelling fresh work after restart.

The existing runtime detaches and invalidates a handle before cleanup. Errors stay
observable through the handle's cancellation diagnostics. A reentrant restart wins;
late completion cannot revive the old visit. Direct lifecycle disposal still
invalidates dialogue even if external work cannot stop. This optional policy does
not roll back external effects or turn a host cancellation claim into a guarantee.

## Verification

- The action example passes shared formatting, type checking and build with the
  rebuilt compiler. Its emitted Lua runs under Lua 5.4.8.
- Emitted action, expression and rich-text integrations pass. Skip regressions
  cover refusal reasons, one-time cleanup, late settlement, concurrent owners,
  untracked work, cleanup failure, reentrant restart, wait cancellation and stale
  capabilities after restart.
- The complete core suite passes: 2,156 unit tests, one existing ignored test,
  plus integrations. A first direct invocation overflowed the default test-thread
  stack; rerunning with the repository's `RUST_MIN_STACK=16777216` setting passed.
  Core all-target Clippy passes. The final added stale-capability regression also
  passes in the focused emitted-action test and both native modes.
- Headless LÖVE 11.5 / LuaJIT 2.1.1700008891 passes JIT-on/off runs of emitted
  examples and action/wait/skip lifecycle oracles. Graphics/audio are disabled;
  audible and rendered behavior remains covered by the preceding audio slice.
- Documentation build, JSON checks, Rust formatting, explicit Lua fixture
  formatting and whitespace checks pass.

No installed-editor UI or performance claim is made. The full workspace/extension
verification gate was not repeated for this host-example/docs/test-only change.
No standalone Lua 5.1 or LuaJIT executable was available; native LÖVE supplies
LuaJIT coverage.

Compiler: Tua 0.11.0, debug, `aarch64-apple-darwin`, revision
`b1de7a74b0525a7233e3be5780f8a086d6ad4c72` plus this slice's working changes. SHA-256:
`bfebd3759d28e0ce5c3b1a43114f774718f46c981687d731f266346b3b8fa6b9`.
