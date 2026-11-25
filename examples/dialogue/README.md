# Dialogue examples

Choose the smallest independently runnable scenario you need. Each subdirectory
has its own `tua.toml`; run `tua check` and `tua build` **inside that directory**.
Headless examples run with `lua main.lua` from their `build/` directory. Native
examples require LÖVE 11.5 and run with `love build` from the scenario directory.

| Scenario | Start here for |
| --- | --- |
| [showcase](showcase/README.md) | Three LÖVE presentations, controls, queues and a headless oracle |
| [basics](basics/README.md) | Lines, choices, shared characters and timed events |
| [context](context/README.md) | Typed facts and placeholder bindings |
| [conditions](conditions/README.md) | Conditional choices and local state |
| [localization](localization/README.md) | Catalogs and per-player languages |
| [expressions](expressions/README.md) | Character expressions and media descriptions |
| [events](events/README.md) | Typed host events |
| [event-identity](event-identity/README.md) | Stable event identity |
| [actions](actions/README.md) | Explicit host actions, waiting and cancellation |
| [rich](rich/README.md) | Rich templates and safe substitution |
| [reveal](reveal/README.md) | Grapheme reveal and reveal timing |
| [audio](audio/README.md) | Optional chatter and recorded voice adapter (LÖVE) |
| [checkpoint](checkpoint/README.md) | Save/resume and bounded history |
| [durable](durable/README.md) | Durable action recovery (headless and LÖVE) |

The scenarios do not import one another. The compiler includes runtime helpers
only when referenced. `Dialogue` and explicit `require("tuadialogue")` retain their
public entry point; internal runtime dependencies now live under `tuadialogue/`.
No `?/init.lua` package-search entry is required. A project-provided
`tuadialogue.lua` still replaces the built-in runtime.
