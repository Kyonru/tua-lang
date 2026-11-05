---
icon: lucide/flask-conical
---

# Examples

The repository includes examples for language features and LOVE workflows.
Clone or download the repository first. Run the `make` commands below from its
root; they build Tua from source and require Rust/Cargo and Make.

If you already have the standalone CLI, enter an example folder containing
`tua.toml`, then run `tua check` and `tua build`. For a LOVE example, run
`love build` after building, or `tua dev` for hot reload. You can also open that
folder in VS Code and use **Tua: Run LOVE Project** without building the compiler.

| Start with | When you want |
| --- | --- |
| `love-minimal` | The smallest LOVE build/run example. |
| `features` | Focused syntax and typing samples; this is a tour, not one game. |
| `starfall` | A playable game with scenes, input, camera, and persistence. |
| `love-tua` | A game combining typed systems with existing Lua libraries. |
| `audio-system` | Audio buses, cues, music scenes, and analysis metadata. |
| `unit-testing` | Typed class and plain Lua specs run through Busted. |
| `linter` | Intentionally failing examples of diagnostics. |

## `examples/features`

A buildable tour of the language and LSP features.

```bash
make build-features
```

The feature tour opts out of the default standard lint set because several
files demonstrate intentionally incomplete or suspicious patterns. Use the
dedicated `examples/linter` fixture to exercise those rules.

The feature tour includes placeholder assets under `src/assets/` so static LOVE
asset validation, asset-path completion, and asset metadata hovers can run
without depending on real art or audio files.

It also includes focused modules for the Baton-backed `input({...})` helper,
state machines, stores, Signals, ECS, object pools, sprite sheets, vectors, and
Lua interoperability. `src/polymorphism.tua` demonstrates cross-module native
inheritance and typed adapters for unrelated gameplay and UI objects; it also
serves as a runnable companion to the [migration guidance](lua-interop.md#migrate-singleton-tables-and-mixins).

## `examples/linter`

An intentionally failing diagnostic fixture with one named example for every
currently emitted standard lint rule.

```bash
cargo build -p tua-cli
cd examples/linter
../../target/debug/tua lint
```

The lint command exits with status `1` by design. With standard rules enabled
by default, `tua check` reports the same fixture warnings.

## `examples/love-minimal`

A small LOVE project for basic compile-and-run validation.

```bash
make build-love-minimal
make run-love-minimal
```

!!! info "LOVE is required only for run targets"
    Build targets work without LOVE. `make run-love-minimal` and
    `make run-love-full`, `make run-love-tua`, and `make run-audio-system`
    require a local LOVE installation.

## `examples/love-full`

A larger LOVE conversion example with Lua interop.

```bash
make build-love-full
make run-love-full
```

The run target uses a smoke mode suitable for local validation.

## `examples/love-tua`

A small playable bullet hell that combines Tua's ECS, object pool, store, state
machine, and input helpers with a hand-written Lua projectile-pattern module
and declaration-typed [feel.lua](https://github.com/Kyonru/feel.lua) feedback.

```bash
make check-love-tua
make run-love-tua
```

The `.tua` and `.lua` sources live together at the example root, including the
tracked Feel runtime and declarations under `lib/feel`. Tua indexes the
declarations without emitting them, copies runtime Lua unchanged, and writes
the runnable project under `build/`.

## `examples/starfall`

A small complete game: collect stars and dodge meteors in a world larger than
the screen, with title, play, and game-over scenes and a saved high score. It
uses the built-in [`Camera`](camera.md) and [`tween`](tweening.md) modules with
`input`, `stateMachine`, `objectPool`, and `vec2`.

```bash
make run-starfall
# Alternatively, with the standalone CLI:
cd examples/starfall
tua dev
# After closing the game:
tua package
```

`tua dev` plays it with [hot reload](hot-reload.md), and `tua package` writes
`dist/starfall.love` (see [Packaging Games](packaging.md)). `make
smoke-starfall` plays a scripted round and quits, which exercises the whole
game in LOVE without input.

## `examples/audio-system`

A self-contained LOVE audio project demonstrating typed buses, sound-effect
cues, layered music scenes, beat and bar events, and cached waveform and tempo
analysis. The checked-in generated catalog lets editor features work from a
fresh checkout; builds rerun the built-in `audio-analysis` pipeline when its
inputs or options change.

```bash
make build-audio-system
make run-audio-system
```

The synthetic `sample*.wav` fixtures deliberately include an ambiguous tempo,
so `TL2012` may report a low-confidence analysis warning. The example authors
the intended BPM for that scene and remains safe to build and run.

## `examples/project-model`

A focused fixture for configured source/output roots, project-relative module
resolution, and mixed `.tua` compilation plus `.lua` copying.

```bash
make build-project-model
```

## `examples/unit-testing`

A headless unit-test project with a typed class, an exported instance alias,
and mixed Tua/Lua specs. Install Busted, then run from the repository root:

```bash
cd examples/unit-testing
tua test
```

See [Testing Tua](testing.md). The example needs Busted and Lua, not LOVE.
