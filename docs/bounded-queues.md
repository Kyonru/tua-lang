# Bounded Queues

`BoundedQueue` supplies an opt-in, typed FIFO in Tua. Referencing it injects
`local BoundedQueue = require("tuaboundedqueue")` into emitted Lua and bundles its
Lua 5.1-compatible runtime. Unused or locally shadowed references do not include
the runtime. Explicit imports remain supported, including for ordinary Lua. A project can supply a
compatible module with the same name, following ordinary runtime replacement
rules. The queue does not own, copy, pool, or dispose its payloads.

```tua
type DamageEvent = { target: number, amount: number }
local initial: DamageEvent[] = {}
local pending = BoundedQueue.new(128, initial)

if not pending.push({ target = 7, amount = 12 }) then
  print("queue full")
end
pending.drain(32, function(event)
  print(event.target, event.amount)
end)
```

The dense initial array supplies the item type and is copied into the queue.
Annotate an empty initial array; an untyped `{}` cannot infer a useful payload
contract. Constructor arguments, payloads, nullable pops, and callback fields
use the shared generic checker and editor queries, including module re-exports.

| Operation (dot syntax) | Contract |
| --- | --- |
| `BoundedQueue.new(capacity, initial)` | Positive finite integer capacity; initial length must fit. |
| `push(value)` | Returns `true`; returns `false` and increments `rejected()` if full. Never evicts old entries. |
| `pop()` | Removes the oldest entry, or returns `nil` when empty. Releases the stored reference. |
| `count()`, `capacity()`, `rejected()` | Pending count, fixed capacity, and overflow count since creation/clear. |
| `clear()` | Drops all references, resets pending/overflow counts, and invalidates an active drain. |
| `drain(limit, visit)` | Visits at most `limit` entries present at its start; returns the count visited. |

`nil` is not a valid payload; `false` is. Capacities and drain limits must be
finite integers; drain limits can be zero. Sparse initial arrays are unsupported.
Popping or pushing does not shift the remaining entries. Popped slots are cleared;
`clear()` replaces storage, so capacity is a bound rather than a preallocation
promise. Payload allocations remain the caller's responsibility.

A drain defers entries pushed by its visitor. Nested drains on the same queue
return zero. A visitor can pop existing entries; newly pushed entries still wait
until the next drain. Clearing and repopulating during a callback stops that drain
before it touches the new generation. A throwing callback leaves the consumed
item removed, preserves pending entries, releases the drain guard, and rethrows.
Callbacks must not yield across this Lua 5.1 `xpcall` boundary. Application-specific
root budgets, ECS liveness checks, reset rules, and tracing remain in the game.

Use a queue for bounded FIFO work such as effects or commands. An array is still
appropriate for indexed data, sorting, or very small collections. Measure the
actual workload before claiming a frame-time benefit. See
`benchmarks/bounded-queue.lua` for separate CPU and GC-stopped allocation probes,
and [the example](https://github.com/Kyonru/tua-lang/blob/main/examples/features/src/bounded_queue.tua).
