# Skeptical review of the hover/signature lane

Reviewer: completion audit agent. Revision remains `d7aed2e`. No product changes.

Reviewed `hover/REPORT.md`, all H coverage rows, the raw broad/focused/provenance
responses, class controls, and relevant semantic/signature formatting code.
Independently ran five additional source variants with
`node internal/audits/devex-2026-11-06/completion/review-hover.js`.
The exact requests/responses and CLI outcomes are in `review-hover-evidence.json`.

## Verdict

The four confirmed hover findings are supported. No severity downgrade or
withdrawal is needed. Keep the distinction between observed failures and inferred
root causes, and retain the current separate gap classification for overload
ranking, class signature documentation and incomplete annotation hover.

## HOV-01: method identity on chains — confirmed independently

The captured Canvas `setFilter` hover position is inside the method name, not an
argument or punctuation. Its null response contrasts with populated signature
help, completion and a builtin definition. The return-valued `getWidth` example
returns the whole expression result, not the method identity. Thus this is not a
void-return modeling mistake.

My additional valid source:

```tua
local height=love.graphics.newFont(14):getHeight()
print(height)
```

Hover inside `getHeight` gives `expression: number` spanning the full constructor
chain, while signature help returns `function Font:getHeight(): number` with
Font documentation. CLI check passes. This reproduces on another LOVE type and
with no native class syntax. Expected method documentation is justified by the
existing documented chained-hover contract.

The recommended shared resolved-member query is credible. The exact reason for
fallthrough is still inferred from source inspection; no fix was applied to
prove that one branch alone cures all receivers. The report already labels this
appropriately. Keep HOV-01 separate from class provenance.

## HOV-02: local function binding identity — evidence holds

`shadow-signature` raw requests target the inner `use("inner")` and outer `use(1)`
correctly. The inner hover, signature and checker all select the number contract;
this cannot be explained by dynamic Lua permissiveness or a malformed fixture.
The name-only first-match lookup at `TypeEnvironment::function` is an actual
implementation fact. Since the checker also rejects the valid inner call,
"all surfaces agree" would not be a correctness oracle here.

The typing lane's independent different-name fixture is a suitable second
verification. Future fixes should not simply reverse list order: that would
break the outer call. Scope/HIR declaration identity remains the right fix
boundary. Nothing in the reviewed evidence warrants changing High severity.

## HOV-03: normalized class provenance — confirmed, but preserve scope limits

Raw arithmetic controls show a useful number hover without a class prefix and
null with one. My additional `Hero` class (health field, read method, no explicit
constructor) followed by `local result = 50 + 3` reproduces null hover on `+` with
successful CLI checking. Therefore arithmetic hover loss does not depend on
Counter names, constructor parameters, or a class initialization warning.

The stronger `Counter.new(1):read()` response is objectively wrong: boolean[] is
inconsistent with the declared number method and its range ends at line 7,
character 1 although the input has six code lines plus a trailing empty line.
The class source is ASCII, ruling out the separate UTF-16 failure.

`SemanticModel::type_of_expression_at_position` traverses raw AST spans with the
provided source position, supporting the provenance hypothesis. Nonetheless the
**empty completion and null signature in the explicit-init Counter fixture are
not proven to share that cause**. The report says this, and the integrated
backlog should continue to do so. No-explicit-init class controls work. Do not
report all class methods/constructors as broken. Completion independently
preserved the failing fixture but has not established another constructor root
cause; wording that it "owns further constructor-binding evidence" should be
read as a cross-check, not an independently completed causal investigation.

## HOV-04: optionality presentation — confirmed with a discriminating control

I tested three distinct `scale` contracts with otherwise identical code:

- `value: number | nil`: `scale()` rejected with TL1004 (one required argument).
- `value?: number`: `scale()` accepted.
- `value: number = 2`: `scale()` accepted.

The first two produce **identical hover and signature labels**,
`function scale(value: number | nil): number`. The defaulted case displays
`function scale(value: number): number`, also lacking optionality. This proves
loss of the omission contract, not merely a stylistic preference for showing
default values. It also validates the proposed required-nilable regression
control. The fix should communicate optionality; reproducing the exact default
expression is a separate enhancement and is not required to correct the defect.

## Active-parameter claims and gap classification

I checked raw positions and responses in the broad evidence:

- inner function second argument -> activeParameter 1;
- outer arguments after the nested call -> 1 and 2;
- cursor immediately after nested `)` but before outer comma -> outer parameter 0;
- a preceding comment containing comma/parenthesis -> next outer parameter 1;
- following table argument -> 1, within that table -> 0;
- following function argument -> 1; nested math.max callback argument uses its
  vararg slot 0;
- third/fourth arguments of a one-fixed-plus-vararg function clamp to slot 1.

These observations support H06's positive coverage. They do not enumerate all
malformed syntax or higher-order nesting; the report explicitly avoids that
claim. Native vararg type loss remains owned by typing, not an active-index bug.

The same-arity overload example keeping the first signature agrees with the
current documented arity-oriented selection. Calling this HOV-G01 rather than a
confirmed defect is correct. HOV-G02's type-only class signature metadata is a
useful missing capability; existing hover documentation succeeds. HOV-G03
likewise separates malformed/incomplete tags from valid annotation support.

## Coverage/evidence qualifications

All H rows have relevant executed evidence. `H09` includes protocol Markdown
inspection; it does **not** establish visual Markdown rendering in VS Code, and
the row/report correctly says so. H06 and H05 are representative boundary sets,
not every nesting/overload in the catalog. Focused CLI cases disable lint rules;
"no diagnostics" in those records must not be promoted to "no default-lint
warnings" in the integrated report (e.g. the explicit-init Counter fixture has
warnings when lint rules are enabled).

The tests were run externally rather than added to the product suite, as the
user requested. Source-file/line references checked in this review align with
the baseline. The reviewer found no harness offset error in the reported hover
findings. One initial review-only helper queried an argument instead of its
function token; it was corrected before the saved review evidence. This does not
affect the original finding evidence.

## Completion-lane wording check

The completion report distinguishes:

- **CMP-01:** invoked context1 is broken; context omitted returns imports;
  independently confirmed through the lead's actual VS Code provider/menu path.
  I tightened its impact wording to avoid claiming all automatic typing requests
  fail, since trigger-character requests can take another path.
- **CMP-06:** explicit false snippet capability still receives snippet payloads;
  this is protocol evidence for clients without support, not a VS Code UI failure.
- **CMP-03–05:** insertion effects are verified from raw LSP edits and CLI checks;
  the script's limited placeholder expansion is identified as a simulation, not
  a full VS Code snippet engine. Non-snippet path/key failures do not depend on it.
- Function snippet `filterText: fn` for prefix `fun` remains an unverified client
  filtering hypothesis and is not included in the six confirmed completion defects.

No owner's hover report or product files were edited during this review.
