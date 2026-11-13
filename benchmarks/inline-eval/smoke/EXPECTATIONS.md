# Smoke expectation provenance

The 2026-11-07 refresh accounts for changed completion-menu selection, not a
change to inline candidate generation or a parser relaxation.

A retained October 26 evaluator, `inline_eval-d1e716b87610a4c1`, reproduced every
previous expected total. Comparing its records to the current evaluator found
19 removed and 33 added backend records. All differences concern `grid.lua`:

- Cut scenarios on lines 20, 27 and 34, at the `Grid` prefix of method declarations:
  columns 12–14 change menu classification from closed to open, and column 11
  gains admitted suggestions. Exact columns differ by line; see the raw diff.
- Edit scenarios on line 31, columns 15 and 18, within
  `self.cells[y][x] = value`, gain menu-open recall suggestions.

The evaluator uses the first matching current-file completion as the simulated
selected item (`document_facts` / `menu_selection` in
`crates/tua-core/src/inline_completion/eval/mod.rs`). Recall candidates must extend
that item (`extends_selection`). Changes to ordinary completion therefore alter
both admission and the menu slice even when inline suggestions are unchanged.

**Control:** rerunning both executables with `--no-menu --records` produced
identical complete per-position record arrays and identical deterministic totals.
Thus all differences disappear when the menu-selection input is held absent.
No Lua/Tua fixture or inline algorithm modification was necessary. This control
also supersedes the initial investigation's parser-change hypothesis.

The executable hashes and added/removed records are retained in local audit
notes. They are not part of the tracked benchmark fixture.

Commands (use the retained historical executable when reproducing the comparison):

```sh
# Run each executable with and without --no-menu.
target/debug/examples/inline_eval-d1e716b87610a4c1 \
  --manifest benchmarks/inline-eval/manifest.toml --set smoke \
  --max-positions 0 --records --out /tmp/old-inline.json
target/debug/examples/inline_eval \
  --manifest benchmarks/inline-eval/manifest.toml --set smoke \
  --max-positions 0 --records --out /tmp/current-inline.json

# Refresh only after the per-position comparison and control above.
target/debug/examples/inline_eval \
  --manifest benchmarks/inline-eval/manifest.toml --set smoke \
  --max-positions 0 --write-expected benchmarks/inline-eval/smoke/expected.json
make inline-eval-smoke
```

This refresh accepts observed deterministic menu behavior. It does not claim
that every recalled suggestion is desirable or that aggregate quality improved:
some admitted candidates add incorrect characters. Evaluation metrics retain
those costs. The historical executable is a local investigation artifact, not a
new build dependency or a binary committed to the repository.
