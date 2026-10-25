# Inspector validation — Apple M3 Max, 2026-10-25

Validated on macOS arm64, 16 logical CPUs, native LÖVE and VSCodium. Production
changes: `aade9ce` (controls), `62eaae6` (reconnect), `fbaa9df` (performance).
These results support local development use on this machine. They are not a
multi-hour endurance test or cross-platform production certification.

## Ten-minute soak and reconnects

A no-shim process ran beside an inspected heavy fixture: 2,000 entities,
48 stores × 32 fields, 128 signal emissions per frame, and short-lived stores.
Both ran for 600 seconds. Forced TCP disconnects occurred every 30 seconds.

- 1,126 valid snapshots, 19 successful reconnects, approximately 1.12 seconds
  recovery each, no recorded errors. Explicit Disable stopped reconnection.
- Retained Lua heap growth: **24.6 KiB active**, **−0.2 KiB no-shim**, comparing
  the first/last three full-GC samples after the first 30 seconds.
- Node post-GC heap range: **476.5 KiB** across the remaining samples. This is a
  range, not a measured leak rate; no unbounded growth appeared in this window.
- Live intervals of 1,000/200/500 ms delivered **1/5/2 snapshots per second**.
  Pause, refresh while paused, resume, and disable also passed.
- 786 snapshots were explicitly truncated by production sampling budgets.
  Truncation is expected for this workload; incomplete data was not mistaken for
  a disconnected/cheap inspector.

The soak includes Lua and Node retained heaps, not longitudinal process RSS or
renderer memory. It used the pre-optimization tree presenter; the native tree
comparison below separately validates the optimized presenter.

[Raw soak samples](results/2026-10-25-soak.json).

## Snapshot spikes

Three alternating before/after heavy-active pairs, 3 seconds warmup and 8 seconds
measured per case. Only the snapshot implementation differs. Both use current
transport and collection controls, default 500 ms frequency, and no rendering.
The no-rendering soak ran concurrently, so these are paired same-host measurements
rather than an idle-machine absolute minimum; all cases passed the host-load guard.

| Measure | Before | Optimized |
| --- | ---: | ---: |
| Median frame-work p99 | 4.029 ms | 2.354 ms |
| Range of frame-work p99 | 4.015–4.611 ms | 2.183–2.916 ms |
| Median mean frame work | 0.579 ms | 0.480 ms |
| Median retained Lua heap | 968.1 KiB | 1,006.5 KiB |

Median p99 fell **42%**, at a median retained-heap cost of **38.4 KiB**. This uses
bounded escaped-key caching, cached immutable object metadata, and a reusable
20-slot signal history. Serialization budgets remain unchanged. Frame work
includes game fixture updates, snapshot serialization, GC, and scheduling; it
is not an isolated serializer timer. The earlier 4.75 ms result came from a
separate run; the paired baseline above is the fair comparison.

[Raw paired runs, source hashes, host loads, and frame arrays](results/2026-10-25-encoding-pairs.json).

## Native expanded inspector tree

Actual isolated VSCodium window, 16 stores × 128 expanded fields (2,048 value
rows), updates every 500 ms, 15 seconds paused and 15 seconds live per version.
Navigation timing measures CDP key dispatch through an observed selection change.
The native tree virtualizes offscreen rows; all objects were expanded.

| Measure, live updates | Before | Optimized |
| --- | ---: | ---: |
| Tree construction p99* | 23.03 ms | 2.53 ms |
| Extension-host timer lag p99 | 11.26 ms | 2.35 ms |
| Keyboard response median | 8.68 ms | 4.61 ms |
| Keyboard response p99 | 24.31 ms | 20.35 ms |
| Maximum keyboard response | 25.59 ms | 89.14 ms |

*Tree construction includes initial expansion and refresh requests. Reusing an
Intl.Collator and avoiding duplicate store-tree construction reduced host work.
Both runs had zero captured renderer JavaScript errors. The optimized run still
had a native compositor latency warning and an 89 ms input outlier; paused input
p99 was 39.72 ms versus 15.17 ms before. One run per version establishes the host
work reduction, not a universal improvement in all input-latency percentiles.
The soak ran concurrently; broader interactive/long-session testing remains useful.

[Before](results/2026-10-25-tree-before.json) · [After](results/2026-10-25-tree-after.json).

## Rendered zombie game

Nine fresh-process runs in temporary source copies: three per condition, rotating
order. Visible 960×540 window, vsync off, deterministic aim and five shots/second,
fixed 60 Hz simulation, 180 warmup + 600 measured frames. Sleep pacing targets
60 FPS; scheduler oversleep explains the observed approximately 58.6 FPS. Work
measurements exclude pacing sleep and include rendering/present and inspector work.
The soak overlapped the beginning of this batch only.

| Median across three runs | No shim | All categories | Stores only |
| --- | ---: | ---: | ---: |
| Actual paced FPS | 58.64 | 58.62 | 58.70 |
| Mean frame work | 1.375 ms | 1.654 ms | 1.532 ms |
| Frame-work p99 | 3.486 ms | 4.934 ms | 4.727 ms |
| Frames over 16.67 ms, all 1,800 measured | 1 | 2 | 0 |

All-category mean work rose **0.279 ms**. Its p99 ranged 2.713–6.606 ms across
runs; no-shim p99 ranged 3.158–4.223 ms. Similar paced FPS does not mean zero cost.
This game's inspected runtime objects are stores, so stores-only is not a strong
category-savings workload. Category tests separately verify disabled constructors
remain unwrapped. Larger combat scenes, other GPUs/platforms, uncapped FPS, and
multiple simultaneous games remain outside this result.

[Raw rendered frame samples](results/2026-10-25-game.json).

## Controls and release isolation

The gear picker configures categories and 100–10,000 ms frequency. Frequency
applies live; category changes apply on the next launch. Empty categories skip
the shim entirely. Existing wrappers remain until a running game exits.

All 150 unit tests and the extension check suite passed. Native integration
passed two-player collection, pause/refresh/resume, independent exit, cleanup,
and inspector-free packaging. Benchmarks and the shim stay extension-owned;
compiler and release game output receive no instrumentation.

See [reproduction instructions](README.md#extended-validation).
