# Alchemy game developer-experience notes

## Verified workflow

- `tua fmt --check`: `checked formatting for 4 file(s)`, exit 0 after applying `tua fmt` to the trial source.
- `tua check`: `checked 4 file(s)`, exit 0 after using `rawget` for a potentially absent tile.
- `tua build`: `built 4 file(s)`, exit 0.
- Lua 5.4 headless gameplay smoke: `alchemy smoke passed`, exit 0. It exercises module loading, cross-file generic values, protocol dispatch, combinations, duplicate discovery, mouse and keyboard callbacks, and drawing against a graphics stub.
- `python3 lsp_probe.py` before completion fix: hover on `name` returned `string | nil`; hover on `selected:get` returned `() -> string`; completion after `selected:` returned `get` and `value`.
- `python3 lsp_probe.py` after the fix: the same hovers remained correct; completion after `selected:` returned only `get`.
- LOVE 11.5 is installed, but a bounded run using `SDL_VIDEODRIVER=dummy` exited `-6` with `Unable to create OpenGL window`. Native rendering was not verified.

## Confirmed defect: colon completion offered a data field

At `src/main.tua:39`, immediately after the colon in `selected:get()`, LSP completion offered `value` (kind `Field`) alongside `get` (kind `Function`). Inserting `value` there forms the invalid call `selected:value()`. The LSP response had a replacement range on line 38 from UTF-16 characters 25–28. This is a medium severity suggestion-quality defect. It was absent from existing tests because the colon completion tests only asserted that a method existed, without asserting that non-callable fields were absent.

The shared core completion pipeline now retains only callable candidates for colon access, including current-file and project-aware paths, while dot completion still offers both. Regression tests were added in `crates/tua-core/src/completion/tests.rs` and `crates/tua-lsp/src/server/tests/completion.rs`; an existing state-machine test was adjusted to expect its `current` data field after a dot. `docs/lsp.md` now states the distinction. Focused tests and both completion suites passed; the parent audit lead will run the broader gate.

A second core regression confirms that a function-valued table field remains available after a colon when it explicitly accepts the receiver. This keeps the filter from hiding valid Lua callback patterns.

## Learning friction: dense array indexing

The first implementation used `local name = elements[index]` followed by `if name ~= nil` for a click-derived index. `tua check` warned `TL1005: comparison is always true: string and nil do not overlap` at `src/main.tua:56`, although Lua may return `nil` for an absent index. This is consistent with the documented dense-array contract in `docs/language.md` (ordinary indexed reads have type `T`; `rawget` gives `T | nil`). Switching to `rawget(elements, index)` removed the warning, and LSP hover confirmed `local name: string | nil`. This is a discoverability issue, not a checker regression; a short diagnostic suggestion or array-indexing example in the learning docs could help game input handlers.

## Positive observations

- A generic class constructed in one module retained `string` through an imported instance alias, allowing `selected:get()` to hover as `() -> string`.
- The protocol-typed `RecipeBook` dispatched at runtime and type-checked across files without requiring a shared superclass.
- The generic history helper inferred `T` from a call using a string value and `string[]` destination; no type arguments were needed at the call.
- The emitted Lua remained readable and executed unchanged under Lua 5.4 with small LOVE stubs.

## Scope limits

The LSP probe checks protocol responses, not VS Code menu rendering or keyboard trigger behavior. The Lua smoke uses stubbed LOVE calls and does not exercise actual OpenGL rendering or LuaJIT. The game is intentionally small; this is evidence from two editing paths, not a universal 0.7.0 quality claim.
