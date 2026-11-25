# A pause menu and game flow

A small title → playing ↔ paused example using a typed state machine, independent
input players and separate gameplay/UI tween groups. A moving marker makes pause
behavior visible without requiring assets or a larger game.

## Run

Use a current Tua compiler and LÖVE 11.5. From the repository root:

```sh
cd examples/pause-input
tua check
tua build
love build
```

## Controls

| Screen | Controls |
| --- | --- |
| Title | Enter / Space starts; Escape quits |
| Playing | Space fires once per press; Escape pauses; R restarts; Backspace returns to title |
| Paused | Enter / Space / Escape resumes; R restarts; Backspace returns to title |

## Expected behavior

`state.tua` owns a `stateMachine` with title, playing and paused states. The renderer
reads the machine instead of maintaining a second paused flag. All input instances
are polled every update, including on title, while paused and while unfocused.
A held start/resume key therefore does not become a fresh gameplay fire press.

UI time and its tween group advance while gameplay time and marker movement stop.
The pause caption fades in using UI time. Title, restart, pause and resume transitions
consume their update: input cannot both change screens and fire. Gameplay resumes
with the next update's dt; there is no accumulated catch-up.

Focus loss pauses a playing session. Refocus does not resume automatically, and
input cannot resume or restart the game while unfocused. Reset stops the current
menu tween, replaces both tween groups and clears run counters, but retains input
instances so held keys keep their edge history. Repeated transitions do not create
listeners. Returning to title freezes the old run; starting again resets it.

The existing optional practice-mode regression remains: U grants an upgrade only
when the model was explicitly reset with `practice = true`. The interactive title
starts normal mode. This gate is not part of the tutorial's required menu setup.

## Try a change

- Change the pause-caption fade from 0.15 to 0.5 seconds: it still animates while paused.
- Add a new named event to the literal state-machine declaration, then inspect its preview.
- Hold Space while starting/resuming: no shot fires until you release and press again.

## Verification

```sh
tua fmt --check
tua check --deny-warnings
tua build
```

From the repository root, use fresh output directories and adjust executable paths:

```sh
python3 scripts/love_replay.py --project examples/pause-input \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario-module scenario --frames 60 --fps 30 --seed 71 \
  --output /tmp/tua-pause-regression

python3 scripts/love_replay.py --project examples/pause-input \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario-module flow_scenario --frames 130 --fps 30 --seed 71 \
  --output /tmp/tua-game-flow --video
```

The first scenario preserves paused movement, advancing UI, held keys across resume,
no catch-up, practice gating and simultaneous pause/fire checks. It explicitly starts
playing in setup because normal launch now begins at the title. The second covers
title/start, explicit resume, focus handling, restart, returning to title, input
ownership and 100 repeated transition/reset cycles. Scenario modules run only when
explicitly loaded by the runner. Video capture requires ffmpeg and does not measure FPS.

## Next steps

Read the [walkthrough](../../docs/tutorials/game-flow.md), then consult
[state machines](../../docs/state-machines.md), [input](../../docs/input.md) and
[tweening](../../docs/tweening.md). These are host policies composed from ordinary
helpers, not a required game framework.
