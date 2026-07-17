---
icon: lucide/clipboard-list
---

# LuaHelper Code Matrix

This matrix maps LuaHelper check identifiers, warning settings, and annotation
tags to Tua's current core and LSP behavior. It is an implementation inventory,
not a promise that Tua will clone LuaHelper one-for-one.

LuaHelper's source uses numeric `CheckError*` constants, while Tua exposes stable
`TLxxxx` diagnostic codes. The rows below map behavior, not code names.

## Sources

| Key | Source |
| --- | --- |
| `LH-ERR` | [`luahelper-lsp/langserver/check/common/err_info.go`](https://github.com/Tencent/LuaHelper/blob/master/luahelper-lsp/langserver/check/common/err_info.go) |
| `LH-WARN` | [`luahelper-vscode/package.json`](https://github.com/Tencent/LuaHelper/blob/master/luahelper-vscode/package.json) |
| `LH-ANN` | [`luahelper-lsp/langserver/check/annotation/annotatelexer/annotate_token.go`](https://github.com/Tencent/LuaHelper/blob/master/luahelper-lsp/langserver/check/annotation/annotatelexer/annotate_token.go) |
| `LH-ANN-INFO` | [`luahelper-lsp/langserver/check/common/annotate_info.go`](https://github.com/Tencent/LuaHelper/blob/master/luahelper-lsp/langserver/check/common/annotate_info.go) |

## Status Legend

- **implemented**: Tua core reports/checks it and `tua-lsp` surfaces it.
- **core-only**: core behavior exists, but no editor-specific feature is mapped.
- **lsp-only**: editor behavior exists, but there is no matching core diagnostic.
- **partial**: some common cases work, but LuaHelper has broader behavior.
- **missing**: not implemented yet and mapped to a roadmap item.
- **out of scope**: intentionally not planned for Tua's LuaJIT/LOVE focus.

## Diagnostic And Check Codes

| LuaHelper code/tag | Source | Tua core checker/lint | `tua-lsp` editor surface | State | Commit / migration notes |
| --- | --- | --- | --- | --- | --- |
| `CheckErrorSyntax = 1` | `LH-ERR` | `TL0001` parse diagnostics. | Published diagnostics. | implemented | Landed in parser phases 7-8. |
| `CheckErrorNoDefine = 2` | `LH-ERR`, `luahelper.Warn.CheckNoDefine` in `LH-WARN` | `TL4004` undefined variables when `undefined_variables` is enabled. | Published diagnostics; respects config/global allow-list. | implemented | Phase 36. |
| `CheckErrorCycleDefine = 3` | `LH-ERR`, `luahelper.Warn.CheckAfterDefine` in `LH-WARN` | `TL4020` require cycles and `TL4007` use-before-local where provable. | Published diagnostics after project indexing/watched changes. | partial | Load-order/global-cycle proof remains narrower; see 59, 60, 62. |
| `CheckErrorLocalNoUse = 4` | `LH-ERR`, `luahelper.Warn.CheckLocalNoUse` in `LH-WARN` | `TL4005` unused locals. | Published diagnostics and remove-unused-local quick fix. | implemented | Phases 36 and 44. |
| `CheckErrorTableDuplicateKey = 5` | `LH-ERR`, `luahelper.Warn.CheckTableDuplicateKey` in `LH-WARN` | `TL4001` duplicate literal table keys. | Published diagnostics and duplicate-key rename quick fix. | implemented | Phases 36, 44. |
| `CheckErrorNoFile = 6` | `LH-ERR`, `luahelper.Warn.CheckReferNoFile` in `LH-WARN` | `TL4002` missing require, `TL4010` bad require, `TL2004` missing LOVE asset. | Diagnostics, document links for resolved requires, create-missing-require quick fix. | implemented | Phases 37, 44, 48.6, 59. |
| `CheckErrorAssignParamNum = 7` | `LH-ERR`, `luahelper.Warn.CheckAssignParamNum` in `LH-WARN` | `TL4027` assignment arity when `assignment_arity` is enabled and value count is statically known. | Published diagnostics; unknown multi-return and vararg sources stay permissive. | implemented | Phase 70.24. |
| `CheckErrorLocalParamNum = 8` | `LH-ERR`, `luahelper.Warn.CheckLocalDefineParamNum` in `LH-WARN` | `TL4028` local declaration arity when `local_declaration_arity` is enabled and initializer count is statically known. | Published diagnostics. | implemented | Phase 70.24. |
| `CheckErrorGotoLabel = 9` | `LH-ERR`, `luahelper.Warn.CheckGotoLable` in `LH-WARN` | No `goto` support goal for Lua 5.1/LuaJIT output. | No LSP diagnostic. | out of scope | Tua targets LuaJIT/LOVE and does not plan Lua 5.2+ `goto` analysis in v1. |
| `CheckErrorCallParam = 10` | `LH-ERR`, `luahelper.Warn.CheckFuncParam` in `LH-WARN` | `TL1004` / `TL4009` wrong argument count. | Diagnostics plus signature help for known calls. | implemented | Phases 16, 37, 41. |
| `CheckErrorImportVar = 11` | `LH-ERR`, `luahelper.Warn.CheckImportModuleVar` in `LH-WARN` | Module fields and exports are checked for `require(...)` and static `import(...)` bindings. Arbitrary custom import functions are not modeled. | Hover/completion/definition/references for indexed module exports. | partial | Owner: project model/module graph for any future configurable import helpers. See 55, 59, 61. |
| `CheckErrorNotIfVar = 12` | `LH-ERR`, `luahelper.Warn.CheckIfNotVar` in `LH-WARN` | Some impossible comparisons and flow-sensitive narrowing exist; no dedicated "used after negative guard" diagnostic. | Hover/checking use narrowed facts in supported guards. | partial | Owner: type flow. Planned expansion in 62. |
| `CheckErrorDuplicateParam = 13` | `LH-ERR`, `luahelper.Warn.CheckFunctionDuplicateParam` in `LH-WARN` | `TL4008` duplicate function parameters. | Published diagnostics. | implemented | Phase 37. |
| `CheckErrorDuplicateExp = 14` | `LH-ERR`, `luahelper.Warn.CheckBinaryExpressionDuplicate` in `LH-WARN` | `TL4013` duplicate binary operands. | Published diagnostics. | implemented | Phase 37. |
| `CheckErrorOrAlwaysTrue = 15` | `LH-ERR`, `luahelper.Warn.CheckErrorOrAlwaysTrue` in `LH-WARN` | `TL4011` covers some impossible boolean expressions. | Published diagnostics. | partial | More boolean identities belong in 37/62 fixture expansion. |
| `CheckErrorAndAlwaysFalse = 16` | `LH-ERR`, `luahelper.Warn.CheckErrorAndAlwaysFalse` in `LH-WARN` | `TL4011` covers some impossible boolean expressions. | Published diagnostics. | partial | More boolean identities belong in 37/62 fixture expansion. |
| `CheckErrorNoUseAssign = 17` | `LH-ERR`, `luahelper.Warn.CheckNoUseAssign` in `LH-WARN` | `TL4029` unused assignments when `unused_assignments` is enabled; `TL4005` and `TL4003` still cover adjacent declaration/self-assignment cases. | Published diagnostics; remove-unused-local and remove-self-assignment quick fixes remain separate. | implemented | Phase 70.24. |
| `CheckErrorAnnotate = 18` | `LH-ERR`, `luahelper.Warn.CheckAnnotateType` in `LH-WARN` | `TL0003` malformed doc annotations when `doc_annotations` is enabled. | Diagnostics, annotation completions, hover/signature docs for supported tags. | implemented | Phases 33, 34, 38, 40, 41, 70.10. |
| `CheckErrorDuplicateIf = 19` | `LH-ERR`, `luahelper.Warn.CheckDuplicateIf` in `LH-WARN` | `TL4012` duplicate `if`/`elseif` conditions. | Published diagnostics. | implemented | Phase 37. |
| `CheckErrorSelfAssign = 20` | `LH-ERR`, `luahelper.Warn.CheckSelfAssign` in `LH-WARN` | `TL4003` self-assignment. | Published diagnostics and remove-self-assignment quick fix. | implemented | Phases 36, 44. |
| `CheckErrorFloatEq = 21` | `LH-ERR`, `luahelper.Warn.CheckFloatEq` in `LH-WARN` | `TL4014` float equality. | Published diagnostics. | implemented | Phase 37. |
| `CheckErrorClassField = 22` | `LH-ERR` | Table-shape field checks exist for known Tua/LuaDoc types; LuaHelper class-field behavior is broader. | Completion/hover/definition for known fields. | partial | Owner: LuaDoc types/metatable facts. Planned in 70.10, 70.11, 70.13. |
| `CheckErrorConstAssign = 23` | `LH-ERR` | `TL4015` const assignment when const metadata is known. | Published diagnostics. | implemented | Phase 37. |
| `CheckErrorCallParamType = 24` | `LH-ERR` | `TL1003` / `TL4016` function argument type mismatch. | Diagnostics plus signature help/completions for literals such as LOVE modes. | implemented | Phases 15, 16, 37, 41, 48.5. |
| `CheckErrorFuncRetErr = 25` | `LH-ERR` | `TL1003` / `TL4017` function return type mismatch. | Published diagnostics; hover/inlay use inferred returns. | implemented | Phases 16, 37, 61. |
| `CheckErrorAssignType = 26` | `LH-ERR` | Annotated locals, stable inferred `.tua` local contracts, compound assignments, and known table fields check via `TL1003`; `any`, unknown expressions, and unannotated `.lua` locals remain gradual. | Published diagnostics; hover and completion retain the declaration contract while flow narrowing remains local to guarded blocks. | implemented | Phases 15, 61, 62. |
| `CheckErrorBinopType = 27` | `LH-ERR` | `TL4030` binary operator type mismatch when `binary_operator_type_mismatches` is enabled and both operand types are statically known. | Published diagnostics; dynamic/`any` operands remain permissive. | implemented | Phase 70.24. |
| `CheckErrorLocFuncNotCall = 28` | `LH-ERR` | `TL4031` local function referenced but never directly called when `uncalled_local_functions` is enabled. | Published diagnostics. | implemented | Phase 70.24. |
| `CheckErrorEnumValue = 29` | `LH-ERR` | Tua enum sugar rejects duplicate enum members; LuaDoc `---@enum` is currently shallow metadata. | Diagnostics for Tua enum syntax; completions from literal unions. | partial | Owner: LuaDoc annotation lowering. Planned in 70.10. |

## LuaHelper Warning Settings

LuaHelper exposes many check constants through VS Code settings. Tua's closest
equivalent is `[diagnostics]` in `tua.toml` plus `tua.diagnostics.*` editor
overrides.

| LuaHelper setting | Maps to | Tua config / LSP setting | State |
| --- | --- | --- | --- |
| `luahelper.Warn.AllEnable` | Global warning master toggle. | No single master toggle; individual `[diagnostics]` keys are explicit. | intentionally divergent |
| `luahelper.Warn.CheckSyntax` | `CheckErrorSyntax`. | Always-on syntax diagnostics. | implemented |
| `luahelper.Warn.CheckNoDefine` | `CheckErrorNoDefine`. | `undefined_variables` / `tua.diagnostics.undefinedVariables`. | implemented |
| `luahelper.Warn.CheckAfterDefine` | `CheckErrorCycleDefine`. | `use_before_definition`, `missing_requires`, require-cycle diagnostics. | partial |
| `luahelper.Warn.CheckLocalNoUse` | `CheckErrorLocalNoUse`. | `unused_locals` / `tua.diagnostics.unusedLocals`. | implemented |
| `luahelper.Warn.CheckTableDuplicateKey` | `CheckErrorTableDuplicateKey`. | `duplicate_keys` / `tua.diagnostics.duplicateKeys`. | implemented |
| `luahelper.Warn.CheckReferNoFile` | `CheckErrorNoFile`. | `missing_requires`, `bad_requires`, asset diagnostics. | implemented |
| `luahelper.Warn.CheckAssignParamNum` | `CheckErrorAssignParamNum`. | `assignment_arity` / `tua.diagnostics.assignmentArity`. | implemented |
| `luahelper.Warn.CheckLocalDefineParamNum` | `CheckErrorLocalParamNum`. | `local_declaration_arity` / `tua.diagnostics.localDeclarationArity`. | implemented |
| `luahelper.Warn.CheckGotoLable` | `CheckErrorGotoLabel`. | No Tua setting. | out of scope |
| `luahelper.Warn.CheckFuncParam` | `CheckErrorCallParam`. | `wrong_argument_count` / `tua.diagnostics.wrongArgumentCount`. | implemented |
| `luahelper.Warn.CheckImportModuleVar` | `CheckErrorImportVar`. | Require graph and module export indexing. | partial |
| `luahelper.Warn.CheckIfNotVar` | `CheckErrorNotIfVar`. | Flow-sensitive narrowing diagnostics. | partial, 62 |
| `luahelper.Warn.CheckFunctionDuplicateParam` | `CheckErrorDuplicateParam`. | `duplicate_parameters` / `tua.diagnostics.duplicateParameters`. | implemented |
| `luahelper.Warn.CheckBinaryExpressionDuplicate` | `CheckErrorDuplicateExp`. | `duplicate_binary_operands` / `tua.diagnostics.duplicateBinaryOperands`. | implemented |
| `luahelper.Warn.CheckErrorOrAlwaysTrue` | `CheckErrorOrAlwaysTrue`. | `impossible_boolean_expressions`. | partial |
| `luahelper.Warn.CheckErrorAndAlwaysFalse` | `CheckErrorAndAlwaysFalse`. | `impossible_boolean_expressions`. | partial |
| `luahelper.Warn.CheckNoUseAssign` | `CheckErrorNoUseAssign`. | `unused_assignments` / `tua.diagnostics.unusedAssignments`. | implemented |
| `luahelper.Warn.CheckAnnotateType` | `CheckErrorAnnotate`. | `doc_annotations` / `tua.diagnostics.docAnnotations`. | implemented |
| `luahelper.Warn.CheckDuplicateIf` | `CheckErrorDuplicateIf`. | `duplicate_if_conditions` / `tua.diagnostics.duplicateIfConditions`. | implemented |
| `luahelper.Warn.CheckSelfAssign` | `CheckErrorSelfAssign`. | `self_assignments` / `tua.diagnostics.selfAssignments`. | implemented |
| `luahelper.Warn.CheckFloatEq` | `CheckErrorFloatEq`. | `float_equality` / `tua.diagnostics.floatEquality`. | implemented |

## Annotation And Documentation Tags

LuaHelper's annotation lexer recognizes the tags and keywords below. Tua also
parses several LuaLS/LuaCATS tags that LuaHelper's lexer does not name directly;
those live in [Lua Interop](lua-interop.md).

| LuaHelper tag/type keyword | Source | Tua core behavior | `tua-lsp` editor surface | State | Commit / migration notes |
| --- | --- | --- | --- | --- | --- |
| `---@class` | `LH-ANN`, `LH-ANN-INFO` | Parses and lowers class/table shape aliases. | Hover, completion, signature docs, semantic tokens. | implemented | Phases 33, 34, 40, 47. |
| `---@field` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower fields including optional fields and literal unions. | Field completion/hover/definition where shape is known. | implemented | Phases 33, 34, 70.10. |
| `---@alias` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower aliases, literal unions, arrays, dictionaries, function types. | Hover/completion/semantic tokens. | implemented | Phases 33, 34, 70.10. |
| `---@type` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower local/global type docs where statically attachable. | Hover/inlay/diagnostics where attached. | implemented | Phases 33, 34. |
| `---@param` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower function parameter types and docs. | Signature help and parameter documentation. | implemented | Phases 33, 34, 41. |
| `---@return` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower single and multi-return annotation data. | Hover/signature help for known functions. | implemented | Phases 33, 34, 41, 61. |
| `---@generic` | `LH-ANN`, `LH-ANN-INFO` | Parsed and preserved with spans; no full generic solver. | Semantic/annotation awareness only. | partial | Owner: types. Generics remain deferred; 70.10 tracks shallow compatibility. |
| `---@overload` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower overload function signatures attached to known functions. | Signature help lists overloads, selects by argument position, and call diagnostics check matching overloads. | implemented | Phase 70.25. |
| `---@vararg` | `LH-ANN`, `LH-ANN-INFO` | Parses/lower typed varargs for annotated `...` functions. | Signature help shows `...: T`; call diagnostics check extra arguments. | implemented | Phase 70.25. |
| `---@enum start` / `---@enum end` | `LH-ANN`, `LH-ANN-INFO` | Tua has enum sugar and shallow `---@enum`; LuaHelper block enum semantics are not fully lowered. | Literal-union completions work when lowered from Tua/LuaCATS aliases. | partial | Owner: LuaDoc lowering. Planned in 70.10. |
| `const` modifier | `LH-ANN` | Const metadata can drive `TL4015` where known. | Published diagnostics. | implemented | Phase 37. |
| `public` / `protected` / `private` | `LH-ANN` | Visibility can be parsed where present but does not enforce access rules. | May appear as metadata; no access diagnostics. | intentionally shallow | Tua avoids object-visibility semantics for v1 unless a LOVE/library use case proves it. |
| `fun(...)` type | `LH-ANN` | Function type forms lower to the same semantic representation as native Tua `(parameters) -> returns` callback types, including names and typed varargs. | Hover, completion details, checking, module exports, and signature help where attached. | implemented | Phases 13, 70.10. Native function types completed 2026-07-16. |
| `table<...>` type | `LH-ANN` | Key-value table forms are parsed/lowered to table/container types. | Hover/completion where shape is known. | implemented | Phase 70.10. |
| `identifier` type keyword | `LH-ANN` | Treated as `string` in LuaDoc type positions; Tua does not model a separate name-like type. | Hover/signature/checking display and enforce it as `string`. | intentionally shallow | Phase 70.25. This keeps common LuaHelper stubs useful without adding a new erased primitive. |

## Code Actions And Quick Fix Families

The fetched LuaHelper tree does not expose a dedicated LSP code-action or quick
fix source file by that name. Its VS Code package contributes formatter and
debugger commands, while diagnostics are driven by the check constants above.

Tua currently has conservative code actions for:

- create missing require target (`TL4002` / related missing-require diagnostics);
- add missing local annotation from inferred type;
- remove unused local (`TL4005`);
- rename duplicate table key (`TL4001`);
- remove self-assignment (`TL4003`);
- remove trailing whitespace (`TL4021`);
- rename close LOVE callback typos (`TL5006`);
- apply the LOVE dot-method colon fix (`TL4018`);
- create missing core LOVE callbacks, organize/sort imports, remove unused
  static import bindings, and fix all safe diagnostics in a file through source
  actions.

Any future LuaHelper-style quick fix should be added through the analysis
snapshot assist framework from roadmap 66, with exact workspace-edit tests.

## Core-Only And LSP-Only Rows

This inventory currently does not identify a LuaHelper `CheckError*` row that is
implemented in Tua core but unavailable through `tua-lsp`: diagnostics flow
through the shared snapshot before publication.

It also does not identify a LuaHelper diagnostic/code row that is implemented
only in `tua-lsp` without core support. Tua has LSP-only presentation features
such as semantic tokens and document symbols, but those are protocol features,
not LuaHelper diagnostic codes.

## Missing Rows By Owner

| Owner | Missing/partial LuaHelper rows | Follow-up |
| --- | --- | --- |
| `lint/types` | broader inferred assignment-type changes beyond annotated/known assignments | 61, 62 |
| `types/flow` | negative-guard use diagnostics and broader boolean identities | 62 |
| `project-model/module` | configurable non-`require` custom import helper modeling | Optional future config work after 55/59/61; static relative Tua `import("./...")` is built in, while arbitrary import functions stay dynamic. |
| `types/lua-doc` | LuaHelper enum blocks and class-field precision | 70.10, 70.11, 70.13 |

## Maintenance Rules

- Add a row when Tua implements a new LuaHelper-equivalent diagnostic, annotation,
  quick fix, or editor feature.
- Move a row to **implemented** only when both core behavior and LSP surfacing
  have tests.
- Keep source links pinned to LuaHelper files or docs that actually define the
  behavior.
- Prefer a Tua-specific non-goal over a vague "missing" row when LuaHelper
  behavior does not fit LuaJIT/LOVE source-to-source tooling.
