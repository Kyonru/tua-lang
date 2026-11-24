# Typed dialogue notifications

This headless example shares a `Dialogue.events` registry through a re-export,
combines typed and legacy notifications, and composes the same registry with typed
context. The host uses typed readers; it does not grant rewards automatically.

```sh
../../target/debug/tua fmt --check
../../target/debug/tua check
../../target/debug/tua build
cd build
lua main.lua
```

Use the paired schema-5 compiler/runtime. No LÖVE, signal bus, filesystem save slot
or renderer is needed. See [the dialogue guide](../../docs/dialogue.md#declare-typed-notification-payloads)
for ownership, payload bounds, lifecycle and compatibility rules.
