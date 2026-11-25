# Save and resume a run

A small persistent score example using a typed Save contract, a store and an optional
persistence binding. The runnable version enables autosave and demonstrates explicit
checkpointing, historical schema validation and deliberate recovery decisions.

## Run

Use a current Tua compiler and LÖVE 11.5. From the repository root:

```sh
cd examples/love-save
tua check
tua build
love build
```

## Controls

- Space earns one point. Changes autosave after one quiet second.
- S saves a checkpoint immediately. Escape quits and flushes pending valid progress.
- D opens reset confirmation. Enter deletes both save copies and starts over;
  Escape cancels. Other input and autosave are suspended while that prompt is open.
- R accepts a recovered valid snapshot only when the other save copy is damaged.
  That snapshot may be older; this decision is never automatic.

Close and reopen the example to resume. The stable identity remains `tua-save-example`;
normal runs use LÖVE's save directory, not the repository. Do not reuse this identity
for your own game. Existing valid v2 saves remain compatible.

## Expected behavior

`profile.tua` creates a fresh contract for each session. `session.tua` loads before
creating the progress store and persistence binding. UI notices live outside that
store, so changing a message does not dirty the save. `main.tua` owns input and drawing.

The v2 record contains score and the existing `musicOn` preference; this sample does
not play music. Historical v1 data must match `{ points: number }` before migration
converts it to `{ score, musicOn = true }`. Loading alone never rewrites files;
the binding's next write can persist a validated migration.

Missing saves start fresh. Failed loads freeze progress editing and writes. A
recovered valid copy is displayed only with its warning. Unavailable progress is
not presented as a legitimate zero-score run. Unsupported future versions remain
blocked, with reset requiring explicit confirmation. A failed reset keeps its
prompt and error visible so it can be cancelled or retried. A partial deletion is
not undone by cancelling; saving remains blocked after a reset failure.

The binding uses a one-second debounce and ten-second maximum interval. S forces
an explicit checkpoint; quit flushes only dirty data. A failed write keeps in-memory
progress and reports the error; S can retry. Quitting with a reset prompt open does
not confirm deletion or force a pending write.

## Try a change

- Change debounce from 1 to 2 and observe the longer quiet interval before autosave.
- Earn points, save, quit, and reopen. Notice which data persists and which UI notices do not.
- Open reset confirmation and cancel it; the score and files remain intact.

## Verification

The persistence harness builds typed source, then loads its emitted session in two
separate LÖVE processes sharing a fresh UUID-based test identity. It does not use or
modify `tua-save-example`. Use a new output directory each time:

```sh
python3 tests/verify.py --tua ../../target/release/tua \
  --love /Applications/love.app/Contents/MacOS/love \
  --output /tmp/tua-save-persistence
```

It checks process restart, missing saves, autosave, UI isolation, quit flushing,
valid migration, invalid historical data, future versions, corrupt envelopes,
explicit older-copy recovery, reset confirmation/failure and failed-write retry.
Successful runs remove only their own test save identity. Failed runs leave isolated
fixtures for inspection and never delete another game's files.

From the repository root, capture the actual UI (requires ffmpeg for video):

```sh
python3 scripts/love_replay.py --project examples/love-save \
  --tua target/release/tua --love /Applications/love.app/Contents/MacOS/love \
  --scenario examples/love-save/tests/replay.lua --frames 130 --fps 30 --seed 71 \
  --output /tmp/tua-save-ui --video
```

Both harnesses use actual LÖVE filesystem/data APIs. These are functional tests,
not storage-performance or power-loss durability measurements. The formatter checks
typed source and the authored Lua tests separately; generated output is not committed.

## Next steps

Follow the [walkthrough](../../docs/tutorials/save-and-resume.md). Consult
[save contracts](../../docs/save-contracts.md) for validation and recovery guarantees.
Consequential inventory actions and cloud synchronization need additional design;
they are not implied by this score example.
