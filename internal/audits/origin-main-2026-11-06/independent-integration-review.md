# Independent integration review

Reviewed the uncommitted integration diff against HEAD, with additional origin/main..HEAD inspection of configuration/schema/template changes, annotation catalog/generator, LOVE override generator wiring, language/classes/configuration documentation and CI/test mapping. Applied architecture, language-auditor, test-auditor, documentation-auditor and review guidance. This is a targeted independent review, not proof every one of the 203 changed files is exhaustively tested.

## Findings and reconciliation

1. **Noisy-test result loss at CLI/extension boundary (Medium, addressed by lead).** CLI retains 8 MiB per backend stream, then prints both plus metadata. Extension formerly rejected combined logs above 16 MiB, so a valid structured-result run could still fail. Lead added explicit `truncateOutput` only to report-backed Test Explorer runs. Discovery JSON remains fail-closed. Independent review confirms continued stream draining, preserved exit code and cancellation state. `node --test editors/vscode/test/unit/testing.test.js`: **6 passed, 0 skipped**, including an actual SIGTERM-resistant child and 17 MiB child with exit code 7. Log `target/origin-main-audit/testing-independent.log`.
2. **Method-hover whitespace layout (Medium, confirmed/fixed by editor owner).** Initial HOV-01 correction inspected exactly one character before the method name. Valid `newCanvas(...): getWidth()` therefore bypassed the identity path. Reported to owner, who independently reproduced it and changed detection to shared receiver extraction across whitespace/newlines, with layout regressions. Owner independently reports the expanded whitespace/newline regression passed; current-binary replay follows.
3. **Quoted member key trivia suspicion rejected.** Asked whether Identifier+Eof lexer filtering admits leading/trailing trivia. Owner independently ran the lexer and confirmed whitespace/comments produce additional tokens, so the filter already rejects them. Defensive exact-spelling check and tests were added; this is not a new confirmed defect.
4. **Schema Unicode suspicion rejected.** Tool-rendered diff suggested a double-escaped LOVE description. Direct JSON decode returns `LÖVE`; no schema correction is warranted.

## Architecture review

Standard UTF-16 conversion stays at the protocol boundary, preserves core scalar resolve-data coordinates, and uses synchronized document text / immutable snapshot text for cross-file results. Custom graph serializers were independently inspected after the lead's extension: signal call/insertion spans, asset references, module exports/inclusion steps/edges, ECS component/system/query uses, pool operations and audio systems select the matching owning file. Inspector uses its requested document; type explanation steps select their own path when available. No generic JSON walker or request-time filesystem read was introduced.

Module isolation is implemented in the shared module graph rather than only hover: owning workspaces supply require roots and declaration-library namespaces. The existing explicit-root builder remains available for callers without project models. The authoritative missing-edge guard prevents reintroducing cross-project fallback through type resolution. The final integration added and passed an explicit outside-root shared declaration-library fixture alongside conflicting per-project overlays and a workspace without an overlay.

Callable fixes use lexical source identity and shared call-prefix/value-list helpers; editor class constructor recovery removes the whitespace-dependent source scan. Tests distinguish correctness from undocumented abstraction features. Current public prose does not claim protocols/generics/abstract classes were implemented.

## Documentation and generator checks

- `node scripts/generate-annotation-guide.js --check`: passed. The extension's `npm run check` invokes it; trusted CI invokes extension checks. Supported tags explicitly distinguish metadata-only features.
- `node scripts/generate-config-schema.js --check`: passed, **58 TOML settings** synchronized. `test.source` relative-root/output-overlap checks match documented isolation; dedicated and in-source spec exclusion defaults match schema/template.
- LOVE argument corrections are centralized in `scripts/love-api-overrides.lua` and consumed by primary/signature/extra catalog generators. Pinned catalog drift runs in trusted CI; no new upstream/runtime overload truth is inferred merely from generator agreement.
- New typing examples explain final-call multiple returns, parenthesized scalar calls and missing nil slots; optional/default signature docs distinguish required nilable parameters. New hot-reload prose limits preservation of private state; test docs now distinguish discovery JSON from report-backed terminal logs.
- Runtime qualification updated separately in `emission.md`: installed LOVE 11.5's LuaJIT 2.1 ran the four runtime oracles successfully; durable raw log copied to `runtime-luajit-evidence.log`.

Final current-binary replay and broad integration gates are coordinated by the lead. Original devex-audit artifacts remain unchanged.

## Current release-binary replay

`node internal/audits/origin-main-2026-11-06/independent-hover-replay.js` completed successfully against release SHA-256 `f8a80336d0859172bc56ec201ef322c679d600794597d984651f47884970a270`.

**8/8 cases passed:** HOV-01 void method and value-method whitespace/newline layouts; HOV-02 nested same-name function contract; HOV-03 arithmetic expression after a native class; HOV-04 default-parameter optional marker; CMP-02 ordinary typed-header comment; WF-01 eight-emoji hover and returned rename edits applied exactly to source. Every case also returned CLI check status 0 and no LSP error diagnostic. Fresh raw request/response/source evidence: `independent-hover-replay.json`. Original archived evidence was not overwritten. This replay does not cover all other 18 original findings; other lanes and lead own their corresponding replays.
