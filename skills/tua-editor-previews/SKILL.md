---
name: tua-editor-previews
description: Design, audit, or change Tua VS Code feature previews, explorers, graphs, workbenches, and information views. Use for sprite, state-machine, behavior-tree, signal, module, ECS, pool, audio, asset, inspector, or generated-Lua UI; not for ordinary language completion.
---

# Tua Editor Previews

Make each view answer a game developer's question using facts Tua can actually
provide. The compiler or runtime owns data; the extension presents it.

## Choose The View's Job

- Read the relevant feature page, `docs/lsp.md`, source fixture, provider,
  and webview before changing labels or layout. State the developer's question
  and next source action. Avoid interchangeable graph dashboards.
- Keep static source structure separate from opt-in sampled runtime state.
  An authored transition is not an observed transition. Mark conservative
  warnings and unresolved assets honestly.
- Prefer a domain arrangement: animation timing and frame order, state routes
  and edge ownership, behavior execution order, ECS access, pool ownership,
  signal producers and listeners, or audio cue and bus relationships.

## Preserve Editor Behavior

- Use compiler-backed facts and shared navigation, refresh, and message
  handling. Never reparse Tua or resolve assets inside a webview. Keep source
  links valid after an edit, dependency change, and panel reveal.
- Check populated, empty, loading, stale, error, narrow-pane, and large-data
  states in a real VS Code host when available. Use theme tokens, keyboard
  focus, screen-reader labels, high contrast, and reduced motion. Pan and zoom
  must not hide source navigation or make edges ambiguous.
- Hidden views may defer visual work only if they catch up on reveal. Record
  request time separately from visible render time; do not suppress useful
  updates to improve a benchmark.

## Verify

- Compare the displayed facts with the same source fixture's compiler or LSP
  response. Check links, filtering, refresh, keyboard controls, and large
  graphs with meaningful regressions. Use native host screenshots where
  available; report protocol-only verification explicitly.
- Update `docs/lsp.md` and the corresponding feature guide when the view's
  claims or user workflow change.
