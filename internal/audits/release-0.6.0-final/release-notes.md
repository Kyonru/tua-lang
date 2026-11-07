# Tua 0.6.0

Tua 0.6.0 improves everyday editing, cross-file types, Lua output, and testing for
Lua and LOVE projects. It also includes the feature-audit fixes across game
runtimes, build ownership, formatting, and editor assistance. The CLI, language
server, and VS Code extension share this version.

## Highlights

- Run typed and Lua specs with Busted, including native editor gutter buttons,
  dedicated test roots, mapped results, cancellation, and optional LuaCov coverage.
- Get consistent assistance for imported classes, chained expressions, callbacks,
  event payloads, declaration overlays, and literal overloads.
- Preserve Unicode edits, lexical symbol identity, module isolation, typed-header
  comments, and constructor identity through Lua emission and hot reload.
- Protect handwritten build outputs and preserve literal whitespace and adjacent
  statements during formatting and safe quick fixes.
- Repair reentrant game helpers, animation durations, audio timing, camera bounds,
  and vector arithmetic; refresh workspace sidebars as project files change.
- Browse LOVE and annotation documentation offline, with extension dependency
  security updates and expanded language, migration, and testing guides.

## Upgrade notes

Update the extension and any custom CLI/server together. Busted must be installed
in the selected Lua environment; coverage also requires LuaCov. Production builds
exclude dedicated test roots and `_spec` files by default; use
`test.exclude_specs = false` for intentional production spec filenames.

Optional record reads and potentially absent ECS components retain `nil`.
Initialize required state and guard delayed resources or missing components.
Stricter default, callback, and call checking can reveal previously missed errors.
`TL4077` warns about field defaults after direct assignments in `init`; class
initialization order is unchanged. Explicit import organization/removal may change
initialization effects or order and remains excluded from safe fix-all. Review
project diagnostics before applying automated edits.

See the [full changelog](https://github.com/Kyonru/tua-lang/blob/v0.6.0/CHANGELOG.md) for every change in this release.

Download the CLI archive or platform-specific VS Code extension below. Checksums are available in `SHA256SUMS`.
