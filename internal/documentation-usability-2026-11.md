# Documentation usability pass — November 2026

This pass improves the learning path and daily-use documentation without changing
compiler or extension behavior. The public navigation now contains 37 Markdown
pages. Existing feature/API reference material is retained where it already
answers the task; this is not a claim that every paragraph needed rewriting.

## Findings And Changes

| Finding | Change | Commit |
| --- | --- | --- |
| The first README example required understanding several optional game systems at once. | Start with a small typed LOVE program and direct links to learning and troubleshooting. | `0df8696` |
| Lua runtime assumptions were scattered or implicit. | Add Lua basics covering truthiness, one-based sequences, table identity, missing keys, multiple returns, and colon calls. | `0df8696` |
| Class imports and initialization were buried in the language reference. | Give classes their own guide with sections for instance type imports, inheritance, documentation, constructor requirements, and initialization order. Preserve `language.md#classes` as an entry point. | `0df8696` |
| Choosing runtime imports versus type-only imports required searching several long pages. | Add a two-file module guide, path examples, declaration-only modules, loading semantics, plain-Lua execution, and simple assertion-based testing. | `0df8696` |
| CLI examples assumed a source checkout, while the quickstart emphasized experimental completions before formatting. | Use installed `tua` commands, explain CLI versus bundled server, add a command/task table and format-on-save settings, and link optional completion settings separately. | `0df8696` |
| Editor and runtime troubleshooting lacked one entry point. | Add symptom-based guidance for suggestions, hover, formatting, imports, class initialization, reloads, assets, and source-mapped errors. | `0df8696` |
| Setup/navigation did not expose all common user journeys. | Update homepage, learning path, configuration entry points, examples, extension README, support links, and site navigation. Fix debugging callout indentation. | `0df8696` |
| Audio onboarding immediately required generated metadata; sprites omitted image prerequisites. | Add a one-sound audio example without an analysis pipeline and explain the sprite image size and zero-based grid coordinates. | `11da6f6` |
| The release page mixed downloads with maintainer publication instructions. | Add upgrade/version-selection guidance and move the maintainer procedure to `internal/releasing.md`, linked from contributor docs. Preserve the release-process anchor. | `6972648` |

Comments and annotation behavior remains owned by `docs/comments.md` and the
LuaDoc reference. The new navigation makes those pages discoverable without
creating another conflicting tag list. Unsupported generics, protocols, and
class features remain explicitly unsupported; this pass does not turn roadmap
ideas into documentation promises.

## Checks

Evidence and temporary snippet projects are under `target/documentation-audit/`.

- `bash scripts/docs.sh build --clean --strict`: passes with no warnings.
  An incremental build initially emitted false missing-page warnings for files
  that existed; a clean build and independent rendered-link check confirmed the
  final output.
- Rendered content: 4,988 local link occurrences across the authored site pages
  resolve to existing files and anchors. This includes navigation and repeated
  links. A separate check of 43 Markdown sources found all 265 relative file
  links present.
- New Lua-basics, module, and class examples pass `tua check` and `tua build`.
  The module and class outputs also execute successfully with local Lua.
- The README, quickstart, and new one-sound audio examples pass check/build.
  Audio checking uses the repository's WAV fixture at the documented path;
  no interactive audio playback is claimed.
- The bundled feature example passes `make check-example build-example
  EXAMPLE=examples/features` with the release compiler: 33 checked files and
  41 built files. Generated example output has no tracked diff.
- CLI usage was compared with local `tua --help`; new command-palette labels
  were compared with the extension manifest. Import-path restrictions and
  check-versus-lint pipeline effects were checked against implementation.
- `make llms-drift-check config-schema-drift-check version-drift-check
  json-check diff-check extension-screenshots-check`: passes.
  Screenshots are the existing verified capture set; no extension UI changed.

## Boundaries

No full compiler test rerun or new VSIX is needed for these documentation-only
changes. The focused examples exercise the changed instructional code. External
website availability, every existing API example, and native graphical gameplay
were not revalidated in this pass.

The generated theme's `404.html` has a skip link to a missing `__skip` anchor;
that theme-owned error page is excluded from the authored-content link check.
All authored content pages pass. A theme/template accessibility fix is separate
from the guide changes.

The site follows the repository's `main` branch; the release guide now explains
that installed releases can lag these docs and points readers to the matching
tag's documentation. These changes are local commits, not a site publication.
