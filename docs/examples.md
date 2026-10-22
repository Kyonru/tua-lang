---
icon: lucide/flask-conical
---

# Examples

The repository includes examples for language features and LOVE workflows.

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
Lua interoperability.

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
cd examples/starfall && tua dev
cd examples/starfall && tua package
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
