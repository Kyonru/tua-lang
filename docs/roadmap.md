# Roadmap

The canonical roadmap lives at the repository root:

- [ROADMAP.md](https://github.com/Kyonru/tua-lang/blob/main/ROADMAP.md)

It is organized into commit-sized slices. Each roadmap item has a proposed
commit title, scope, and acceptance criteria.

## Current Focus

The v1 implementation covers the compiler, CLI, examples, VS Code extension,
and a broad language-server surface.

Recent roadmap work adds planned parity phases for:

- LuaHelper-style annotation and lint depth;
- rust-analyzer-inspired project analysis and editor workflows;
- project-wide references, rename, imports, and code actions;
- richer hover, signature, completion, semantic tokens, and docs.

## Working Rule

Each commit should leave the repository buildable and tested. Behavior changes
should include focused core or LSP tests in the same commit.
