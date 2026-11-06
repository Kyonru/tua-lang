# Consolidated developer-experience findings

This is the integrated index of **18 confirmed defects: 8 High, 10 Medium,
0 Critical, 0 Low**. All 18 have reproduction evidence; all eight High findings
were independently cross-checked. Confidence in the observed behavior is high;
root causes are source-backed explanations, not validated patches.

High means broad loss of type safety, ordinary-workflow analysis failure, wrong
symbol/project identity, destructive edits, or loss of core editor assistance.
Medium means a narrower correctness or presentation failure with a practical
workaround. Severity is impact, not implementation size. No security exploit or
complete-server termination is claimed.

The linked lane reports contain each finding's exact minimal source, reproduction
commands, expected/actual results, raw evidence, source file/line references,
existing coverage gaps, recommended shared fix and regression test. Read them
alongside [verification instructions](VERIFICATION.md), not as hypotheses from
source inspection alone. IDs remain stable across all reports.

| ID | Severity | Finding and detailed report | Observed impact | Cross-check |
|---|---|---|---|---|
| WF-01 | High | [Non-BMP text corrupts LSP positions and rename edits](workflow/REPORT.md) | VS Code applies a rename that damages surrounding source; ASCII/BMP controls work. | Independent protocol and actual VS Code |
| CMP-02 | High | [Typed function header comments panic analysis](completion/REPORT.md) | Ordinary trailing comments panic CLI/check and an LSP diagnostic worker; a suppression action can create this source. Server recovery was observed. | Independent CLI and LSP comment controls |
| WF-02 | High | [Same-name modules leak across workspace projects](workflow/REPORT.md) | Two individually valid projects resolve a foreign module, producing wrong types, definitions and false diagnostics. | Independent differently named projects plus standalone controls |
| HOV-02 | High | [Shadowed local functions resolve the outer contract](hover/REPORT.md) | Checker, hover and signature help can agree on the wrong declaration and reject valid inner calls. | Independent same-name function fixtures |
| TYP-01 | High | [Contextual callback return contracts are skipped](typing/REPORT.md) | Invalid callback returns are accepted on common direct-call and typed-initializer paths while consumers are shown the declared result type. | Independent fixture and emitted-Lua runtime oracle |
| TYP-02 | High | [Call-result property chains bypass type contracts](typing/REPORT.md) | A number property after a call accepts a string assignment even when hover knows the number type. | Independent fixture and emitted-Lua runtime oracle |
| CMP-07 | High | [Native instance inference depends on assignment whitespace](completion/REPORT.md) | Removing spaces around = loses the class receiver in hover, completion and signature help. Checker rejection still works. | Independent Tank constructor whitespace controls |
| CMP-01 | High | [Invoked identifier completion bypasses autoimports](completion/REPORT.md) | Explicitly invoked completion omits a project symbol available through other request contexts. | Independent protocol and actual VS Code suggestion UI |
| CMP-03 | Medium | [Completion replacement ignores right-hand syntax](completion/REPORT.md) | Accepting a candidate can retain a token suffix or duplicate an existing call. | Returned edits applied and checked; independent suffix fixture |
| CMP-04 | Medium | [Asset paths are inserted without quote escaping](completion/REPORT.md) | A filename containing the active string delimiter creates invalid source. | Applied edits and independent opposite-quote fixture |
| CMP-05 | Medium | [Quoted record keys become invalid dot accesses](completion/REPORT.md) | Reserved or non-identifier keys are inserted as bare members. | Applied edits and independent reserved-key fixture |
| CMP-06 | Medium | [Snippet capability negotiation is ignored](completion/REPORT.md) | Clients explicitly declining snippets still receive snippet format and placeholders; this is not a VS Code-specific defect. | Actual protocol requests with snippetSupport false |
| TYP-03 | Medium | [Native vararg annotations lose their element type](typing/REPORT.md) | Wrong trailing arguments are accepted and useful type information disappears from editor surfaces. | Native and LuaDoc controls across checker and editor |
| TYP-04 | Medium | [Final multi-return arguments are counted syntactically](typing/REPORT.md) | Valid Lua-style consume(pair()) calls are rejected for missing arguments. | CLI/LSP plus Lua runtime oracle |
| TYP-05 | Medium | [Definitely absent assignment slots infer any](typing/REPORT.md) | Known nil assignment slots silently lose their useful type and permit incompatible contracts. | CLI/LSP plus Lua runtime oracle |
| HOV-01 | Medium | [Method-token hover loses call-chain identity](hover/REPORT.md) | LOVE method hover is absent or shows the entire expression result despite usable completion/signature/definition data. | Independent Font and Canvas chain fixtures |
| HOV-03 | Medium | [Class normalization breaks expression hover coordinates](hover/REPORT.md) | Expression hover can disappear, report an unrelated type, or return a range beyond the document. | Independent native-class prefix and arithmetic controls |
| HOV-04 | Medium | [Optional parameters are displayed as required](hover/REPORT.md) | Optional/default parameters look required; optional forms merely display nilability despite different accepted call arity. | Independent optional versus required-nilable controls |

## Observed behavior with unresolved attribution

**U-01 — Direct native constructor chains:** completion and signature help on
`Counter.new(...):read(...)` fail across the recorded class variants, while a
spaced local binding works. The observation is reproducible; whether it shares
CMP-07's underlying missing constructor facts is not established. It is kept
outside the 18-defect count to avoid counting potentially overlapping roots.
See [class follow-up](completion/CLASS-FOLLOWUP.md). The initial hypothesis that
explicit `init` caused the behavior was disproved by whitespace controls.

## Consolidation and independent review

- **Identity:** WF-02 is project identity; HOV-02 is lexical declaration identity.
  Their cross-surface agreement on a wrong answer makes surface-only testing weak.
- **Semantic propagation:** TYP-01/02, CMP-07 and U-01 need canonical callable,
  constructor and expression facts. They have distinct reproductions; no claim
  is made that one small change would resolve all of them.
- **Callable metadata:** TYP-03 and HOV-04 concern vararg and optional metadata.
  TYP-04/05 concern Lua value-list semantics, a related implementation family.
- **Coordinates:** WF-01's UTF-16 protocol boundary and HOV-03's normalized AST
  provenance are separate layers and need different tests.
- **Completion:** CMP-01 concerns routing; CMP-03/04/05 concern edit construction;
  CMP-06 concerns client capabilities. Candidate presence alone tests none of
  insertion validity, protocol negotiation or routing parity.
- **Trivia:** the ordinary-comment panic and suppression-generated panic are
  one finding, CMP-02. The server survives the observed worker panic.
- **Documentation:** HOV-01 is method identity loss; incomplete annotation help,
  method signature documentation and type-aware overload ranking are separate
  capability/usability notes rather than inflated defect counts.

Independent reviews and their raw observations are retained in
[typing review](typing/REVIEW-completion.md),
[hover review](hover/REVIEW.md),
[completion review](completion/REVIEW-hover.md), and
[class whitespace review](typing/REVIEW-class-whitespace.md).
Review corrected a mismatched standalone module control, rejected a root-order
claim not experimentally established, and replaced an unsuitable DOM visibility
heuristic with the actual screenshot. Corrections are recorded in
[the verification log](VERIFICATION.md).

## Gaps, design questions and performance

The [prioritized backlog](BACKLOG.md) separately tracks documented narrowing
limits, nullable callable syntax, closure effects, overload selection, annotation
authoring, same-line callback recovery, colon candidate relevance and longer-term
abstraction design. Unsupported generics/protocols are not called regressions.
An unverified snippet-filtering suspicion is explicitly excluded from findings.

Measured cold completion medians were approximately 37–118 ms across three
bundled projects; warm protocol calls were much faster. These are current-host
round trips, not a historical regression comparison or keystroke-to-menu timing.
See [workflow measurements and limitations](workflow/REPORT.md).
