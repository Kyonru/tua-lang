# ECS allocation probe

Run `lua benchmarks/ecs/query-allocation.lua path/to/tuaecs.lua` for each runtime.
The probe warms each query 100 times, then visits 12 entities in 10,000 invocations
with GC stopped. It checks visit counts, reports allocated KiB, and makes no
absolute timing assertion. Run CPU comparisons separately with GC running.
The existing `benchmarks/ecs-love` runner alternates before/after timing order.

On Lua 5.4.8 (macOS arm64), baseline `603d477d` versus cached bindings:

| Components | Before allocated KiB | After allocated KiB |
| --- | ---: | ---: |
| 1 | 703.250 | 0.000 |
| 3 / 4 | 1172.000 | 0.000 |
| 5 / 8 | 3593.875 | 1796.938 |

Wide queries retain a separate argument buffer per invocation for reentrancy.
These allocation numbers are interpreter-specific. The existing native CPU
benchmark (LÖVE 11.5, LuaJIT 2.1.1700008891, JIT disabled, 30 warm-up updates,
7 alternating samples of 20 updates) showed only 0–3% differences across its
five workloads, with matching checksums. This does not establish a whole-game
CPU or rendered frame-rate improvement. Integrated game measurements are kept
in the game's research report, outside this repository.
