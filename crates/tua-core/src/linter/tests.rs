use crate::{
    check::check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config,
    check_parsed_source_with_diagnostics_config, check_tua_source,
    check_tua_source_with_diagnostics_config, parse_lua_source, parse_tua_source, DiagnosticCode,
    DiagnosticSeverity, DiagnosticsConfig, EcsConfig, JitConfig,
};

#[test]
fn standard_lints_are_enabled_by_default() {
    let diagnostics = check_tua_source(
        "src/main.tua",
        r#"
local config = { hp = 10, hp = 20 }
local hp = 10
hp = hp
local Missing = require("definitely_missing_lint_module")
local spaced = { 1, 2, 3 }
"#,
    )
    .expect_err("standard lint rules should be enabled by default");

    for code in [
        DiagnosticCode::LINT_DUPLICATE_TABLE_KEY,
        DiagnosticCode::LINT_UNUSED_LOCAL,
        DiagnosticCode::LINT_UNUSED_IMPORT,
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(code)),
            "expected {code} in {diagnostics:#?}"
        );
    }
}

#[test]
fn formatting_is_not_a_lint() {
    check_tua_source("src/main.tua", "return 1   \n")
        .expect("formatting differences belong to the formatter");
}

#[test]
fn builtin_global_write_reports_direct_target_writes_with_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
math = {}
function print() end
table += 1
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_BUILTIN_GLOBAL_WRITE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "built-in global `math` is overwritten here; consider using a local or changing the name",
            "built-in global `print` is overwritten here; consider using a local or changing the name",
            "built-in global `table` is overwritten here; consider using a local or changing the name",
        ]
    );
}

#[test]
fn builtin_global_write_resolves_local_function_bindings_and_ignores_fields() {
    let diagnostics = check_standard_rules_source(
        r#"
local math
function math() end
math()

local function replace(print)
  function print() end
  print()
end
replace(function() end)

table.custom = 3
function string.custom() end
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_BUILTIN_GLOBAL_WRITE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn builtin_global_write_uses_the_luajit_love_target_catalog() {
    let diagnostics = check_standard_rules_source(
        r#"
_ENV = {}
warn = function() end
jit = {}
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_BUILTIN_GLOBAL_WRITE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "built-in global `jit` is overwritten here; consider using a local or changing the name"
        ]
    );
}

#[test]
fn placeholder_read_reports_local_and_global_reads_with_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
do
  local _ = 5
  print(_)
  _ = 6
end

_ = 5
print(_)
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_PLACEHOLDER_READ))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "placeholder value `_` is read here; consider using a named variable",
            "placeholder value `_` is read here; consider using a named variable",
        ]
    );
}

#[test]
fn placeholder_read_covers_parameters_loops_fields_closures_and_compound_assignments() {
    let diagnostics = check_standard_rules_source(
        r#"
local function consume(_)
  local read = function()
    return _
  end
  _ += 1
  _.field = read()
  return _
end

for _ = 1, 1 do
  print(_)
end

for _, value in ipairs({ 1 }) do
  print(_, value)
end

consume({ field = 1 })
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_PLACEHOLDER_READ),
        6,
        "{diagnostics:#?}"
    );
}

#[test]
fn placeholder_writes_do_not_report_placeholder_or_global_local_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
local function discardLocal()
  local _ = print("discarded")
  _ = print("discarded again")
end

local function discardGlobal()
  _ = print("discarded globally")
end

discardLocal()
discardGlobal()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_PLACEHOLDER_READ),
        0,
        "{diagnostics:#?}"
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn standard_rules_report_symbol_rules() {
    let diagnostics = check_standard_rules_source(
        r#"
local Outer = 1
do
  local Outer = 2
  print(Outer)
end

local uninitialized
print(uninitialized)

local Feel = require("feel")
local _ = 1
print(_)
print = function() end
print(Missing)

local function createGlobal()
  ProjectOnly = 1
  return ProjectOnly
end
"#,
    );

    for code in [
        DiagnosticCode::LINT_LOCAL_SHADOW,
        DiagnosticCode::LINT_UNINITIALIZED_LOCAL,
        DiagnosticCode::LINT_UNUSED_IMPORT,
        DiagnosticCode::LINT_PLACEHOLDER_READ,
        DiagnosticCode::LINT_BUILTIN_GLOBAL_WRITE,
        DiagnosticCode::LINT_UNDEFINED_VARIABLE,
        DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL,
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(code)),
            "expected {code} in {diagnostics:#?}"
        );
    }
}

#[test]
fn uninitialized_local_tracks_initializer_arity_and_compound_reads() {
    let diagnostics = check_standard_rules_source(
        "local first, missing = 1\nprint(missing)\nlocal total\ntotal += 1\nprint(total)\n",
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNINITIALIZED_LOCAL))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 7));
    assert_eq!(
        diagnostics[0].message,
        "local `missing` defined at line 1 may be read before it is initialized; initialize with `nil` to silence"
    );
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (4, 1));
    assert_eq!(
        diagnostics[1].message,
        "local `total` defined at line 3 may be read before it is initialized; initialize with `nil` to silence"
    );
}

#[test]
fn uninitialized_local_allows_final_calls_and_varargs_only_to_fill_missing_values() {
    let diagnostics = check_standard_rules_source(
        r#"local function pair()
  return 1, 2
end

local first, second = pair()
print(first, second)

local function consume(...)
  local head, tail = ...
  print(head, tail)

  local wrapped, missing = (pair())
  print(wrapped, missing)
end

consume(1, 2)
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNINITIALIZED_LOCAL))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (13, 18));
    assert_eq!(
        diagnostics[0].message,
        "local `missing` defined at line 12 may be read before it is initialized; initialize with `nil` to silence"
    );
}

#[test]
fn uninitialized_local_uses_forward_definite_assignment() {
    let diagnostics = check_standard_rules_source(
        r#"local outer
do
  local outer = 1
  print(outer)
end
print(outer)

local later
print(later)
later = nil

local conditional
if ready then
  conditional = 1
end
print(conditional)

local complete
if ready then
  complete = 1
else
  complete = 2
end
print(complete)

local loopOnly
while ready do
  loopOnly = 1
end
print(loopOnly)

local repeated
repeat
  repeated = 1
until ready
print(repeated)

local captured
local function initializeCaptured()
  captured = 1
end
print(captured)

local function reachableBranches()
  local result
  if ready then
    return
  else
    result = 1
  end
  print(result)
end
reachableBranches()

local callback
function callback() end
callback()
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNINITIALIZED_LOCAL))
        .collect::<Vec<_>>();

    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 5, "{diagnostics:#?}");
    for name in ["outer", "later", "conditional", "loopOnly", "captured"] {
        assert!(
            messages
                .iter()
                .any(|message| message.contains(&format!("`{name}`"))),
            "expected `{name}` in {diagnostics:#?}"
        );
    }
    for name in ["complete", "repeated", "result", "callback"] {
        assert!(
            messages
                .iter()
                .all(|message| !message.contains(&format!("`{name}`"))),
            "did not expect `{name}` in {diagnostics:#?}"
        );
    }
}

#[test]
fn reports_dynamic_require_bindings_as_unused_imports_and_reads_arguments() {
    let diagnostics = check_standard_rules_source(
        r#"
local moduleName = "dynamic"
local Dynamic = require(moduleName)
local marker = "marker"
local Extra = require("extra", marker)
"#,
    );
    let mut messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_IMPORT))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    messages.sort_unstable();

    assert_eq!(
        messages,
        vec![
            "import `Dynamic` is never used; prefix with `_` to silence",
            "import `Extra` is never used; prefix with `_` to silence",
        ]
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_LOCAL)
            && (diagnostic.message.contains("`moduleName`")
                || diagnostic.message.contains("`marker`"))
    }));
}

#[test]
fn shadowed_require_calls_are_ordinary_local_initializers() {
    let diagnostics = check_standard_rules_source(
        r#"
local function require(name)
  return { name = name }
end

local Package = require("pkg")
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNUSED_IMPORT),
        0,
        "{diagnostics:#?}"
    );
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_FUNCTION)
            && diagnostic.message.contains("`require`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_LOCAL)
            && diagnostic.message.contains("`Package`")
    }));
}

#[test]
fn import_uses_include_types_nested_closures_and_underscore_bindings() {
    let diagnostics = check_standard_rules_source(
        r#"
local Types = require("types")
local Nested = require("nested")
local _Intentional = require("intentional")

local value: Types.Player = nil
local function readNested()
  return Nested
end

print(value, readNested())
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNUSED_IMPORT),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn local_shadow_reports_same_function_redeclarations_and_parameters() {
    let diagnostics = check_standard_rules_source(
        r#"
local value = 1
print(value)
local value = 2
print(value)

local function increment(amount)
  local amount = amount + 1
  return amount
end

print(increment(1))
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_LOCAL_SHADOW),
        2,
        "{diagnostics:#?}"
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_LOCAL_SHADOW)
            && diagnostic.message.contains("declaration at 2:7")
    }));
}

#[test]
fn local_shadow_reports_nested_loop_indices() {
    let diagnostics = check_standard_rules_source(
        r#"
local function iterate()
  for i = 1, 10 do
    for i = 1, 10 do
      print(i)
    end
  end
end

iterate()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_LOCAL_SHADOW),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn local_shadow_ignores_inter_function_shadowing() {
    let diagnostics = check_standard_rules_source(
        r#"
local value = 1

local function inner()
  local value = 2
  return value
end

print(value, inner())
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_LOCAL_SHADOW),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn local_shadow_reports_user_globals_regardless_of_source_order() {
    let diagnostics = check_standard_rules_source(
        r#"
do
  local Project = {}
  print(Project)
end

Project = {}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_LOCAL_SHADOW))
        .expect("a used local should shadow a user global referenced later in the file");

    assert!(
        diagnostic.message.contains("global used at 7:1"),
        "{diagnostics:#?}"
    );
}

#[test]
fn local_shadow_ignores_unused_duplicate_builtin_and_configured_bindings() {
    let diagnostics = check_standard_rules_source(
        r#"
local outer = 1
do
  local outer = 2
end
print(outer)

local duplicate, duplicate = 1, 2
print(duplicate)

local function repeated() end
local function repeated() end
repeated()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_LOCAL_SHADOW),
        0,
        "{diagnostics:#?}"
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_DUPLICATE_LOCAL),
        1,
        "{diagnostics:#?}"
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_DUPLICATE_FUNCTION),
        1,
        "{diagnostics:#?}"
    );

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local math = math
local ConfiguredGlobal = ConfiguredGlobal
print(math, ConfiguredGlobal)
"#,
        &DiagnosticsConfig {
            rules: true,
            globals: vec!["ConfiguredGlobal".to_owned()],
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("builtin and configured globals should remain safe local names");
}

#[test]
fn standard_rules_report_control_flow_and_declaration_rules() {
    let diagnostics = check_standard_rules_source(
        r#"
local a, a = 1, 2

local function choose(value)
  if value then
    return 1
  end
end

local function duplicate() end
local function duplicate() end

if true then
  return
else
  return
end
print("never")
"#,
    );

    for code in [
        DiagnosticCode::LINT_DUPLICATE_LOCAL,
        DiagnosticCode::LINT_IMPLICIT_RETURN,
        DiagnosticCode::LINT_DUPLICATE_FUNCTION,
        DiagnosticCode::LINT_UNREACHABLE_CODE,
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(code)),
            "expected {code} in {diagnostics:#?}"
        );
    }
}

#[test]
fn duplicate_function_keeps_the_original_definition_for_every_repeat() {
    let diagnostics = check_standard_rules_source(
        r#"function globalName() end
function globalName() end
function globalName() end

local function localName() end
local function localName() end
"#,
    );
    let mut diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_FUNCTION))
        .collect::<Vec<_>>();
    diagnostics.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));

    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 10));
    assert_eq!(
        diagnostics[0].message,
        "function `globalName` is defined more than once in this block; first definition is at 1:10"
    );
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (3, 10));
    assert_eq!(
        diagnostics[1].message,
        "function `globalName` is defined more than once in this block; first definition is at 1:10"
    );
    assert_eq!((diagnostics[2].line, diagnostics[2].column), (6, 16));
    assert_eq!(
        diagnostics[2].message,
        "function `localName` is defined more than once in this block; first definition is at 5:16"
    );
}

#[test]
fn duplicate_function_respects_branch_blocks_and_qualified_method_names() {
    let diagnostics = check_standard_rules_source(
        r#"if condition then
  function branch() end
  function branch() end
else
  function branch() end
end

local object = {}
function object.method() end
function object:method() end

do
  local function nested() end
end
do
  local function nested() end
end
"#,
    );
    let mut diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_FUNCTION))
        .collect::<Vec<_>>();
    diagnostics.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));

    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (3, 12));
    assert_eq!(
        diagnostics[0].message,
        "function `branch` is defined more than once in this block; first definition is at 2:12"
    );
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (10, 10));
    assert_eq!(
        diagnostics[1].message,
        "function `object.method` is defined more than once in this block; first definition is at 9:10"
    );
}

#[test]
fn duplicate_local_reports_locals_parameters_and_implicit_self_with_origins() {
    let diagnostics = check_standard_rules_source(
        r#"function foo(a, b, a)
  return a, b
end

local object = {}
function object:method(self)
  return self
end

local x, y, x = 1, 2, 3

local function multiline(
  first,
  second,
  first
)
  return first, second
end

local callback = function(item, item)
  return item
end

return foo, object, x, y, multiline, callback
"#,
    );
    let mut duplicates = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_LOCAL))
        .collect::<Vec<_>>();
    duplicates.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));

    assert_eq!(duplicates.len(), 5, "{diagnostics:#?}");
    assert_eq!(
        duplicates
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "function parameter `a` already defined on column 14",
            "function parameter `self` already defined implicitly",
            "variable `x` already defined on column 7",
            "function parameter `first` already defined on line 13",
            "function parameter `item` already defined on column 27",
        ]
    );
    assert_eq!(
        duplicates
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect::<Vec<_>>(),
        vec![(1, 20), (6, 24), (10, 13), (15, 3), (20, 33)]
    );
}

#[test]
fn duplicate_local_accepts_repeated_exact_placeholders() {
    let diagnostics = check_standard_rules_source(
        r#"local _, _, _ = 1, 2, 3

local function discard(_, _, _)
end

return discard
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_DUPLICATE_LOCAL),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn implicit_return_reports_named_and_anonymous_fallthrough_locations() {
    let diagnostics = check_standard_rules_source(
        r#"local function find(items, expected)
  for key, value in pairs(items) do
    if key == expected then
      return value
    end
  end
end

local anonymous = function(flag)
  if flag then
    return nil
  end
end

return find, anonymous
"#,
    );
    let implicit_returns = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_IMPLICIT_RETURN))
        .collect::<Vec<_>>();

    assert_eq!(implicit_returns.len(), 2, "{diagnostics:#?}");
    assert_eq!(
        implicit_returns[0].message,
        "function `find` can implicitly return no values even though there is an explicit return at line 4; add an explicit return to silence"
    );
    assert_eq!(
        (implicit_returns[0].line, implicit_returns[0].column),
        (6, 6)
    );
    assert_eq!(
        implicit_returns[1].message,
        "function can implicitly return no values even though there is an explicit return at line 11; add an explicit return to silence"
    );
    assert_eq!(
        (implicit_returns[1].line, implicit_returns[1].column),
        (12, 6)
    );
}

#[test]
fn implicit_return_understands_guaranteed_and_infinite_loops() {
    let diagnostics = check_standard_rules_source(
        r#"local function repeatReturns(ready)
  repeat
    return 1
  until ready()
end

local function whileForever(ready)
  while true do
    if ready() then
      return 2
    end
  end
end

local function repeatForever(ready)
  repeat
    if ready() then
      return 3
    end
  until false
end

local function whileBreaks(ready, stop)
  while true do
    if ready() then
      return 4
    end
    if stop() then
      break
    end
  end
end

local function repeatBreaks(ready, stop)
  repeat
    if ready() then
      return 5
    end
    if stop() then
      break
    end
  until false
end

local function nestedGuaranteedLoop(flag, ready)
  if flag then
    repeat
      return 6
    until ready()
  else
    do
      while true do
        if ready() then
          return 7
        end
      end
    end
  end
end

local function nestedLoopBreak(ready)
  while true do
    repeat
      break
    until false
    if ready() then
      return 8
    end
  end
end

return repeatReturns, whileForever, repeatForever, whileBreaks, repeatBreaks, nestedGuaranteedLoop, nestedLoopBreak
"#,
    );
    let implicit_returns = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_IMPLICIT_RETURN))
        .collect::<Vec<_>>();

    assert_eq!(implicit_returns.len(), 2, "{diagnostics:#?}");
    assert_eq!(
        implicit_returns[0].message,
        "function `whileBreaks` can implicitly return no values even though there is an explicit return at line 26; add an explicit return to silence"
    );
    assert_eq!(implicit_returns[0].line, 31);
    assert_eq!(
        implicit_returns[1].message,
        "function `repeatBreaks` can implicitly return no values even though there is an explicit return at line 37; add an explicit return to silence"
    );
    assert_eq!(implicit_returns[1].line, 42);
}

#[test]
fn implicit_return_accepts_explicit_empty_and_terminating_builtin_paths() {
    let diagnostics = check_standard_rules_source(
        r#"local function bare(flag)
  if flag then
    return 1
  else
    return
  end
end

local function stopped(flag)
  if flag then
    return 2
  end
  error("stopped")
end

local function asserted(flag)
  if flag then
    return 3
  end
  assert(false, "stopped")
end

return bare, stopped, asserted
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_IMPLICIT_RETURN),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn unreachable_code_recognizes_resolved_error_and_assert_calls() {
    let diagnostics = check_standard_rules_source(
        r#"
local function cbrt(value)
  if value >= 0 then
    return value ^ (1 / 3)
  else
    error("cbrt expects a non-negative argument")
  end
  return 0
end

local function stop(flag)
  if flag then
    error("stopped")
  else
    assert(false, "stopped")
  end
  return 0
end

return cbrt, stop
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNREACHABLE_CODE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "unreachable code (previous statement always returns or errors)",
            "unreachable code (previous statement always errors)",
        ]
    );
}

#[test]
fn unreachable_code_does_not_treat_shadowed_terminators_as_builtins() {
    let diagnostics = check_standard_rules_source(
        r#"
local function error(message)
  print(message)
end

local function assert(value)
  return value
end

local function run(flag)
  if flag then
    error("keep going")
  else
    assert(false)
  end
  return 0
end

return run
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNREACHABLE_CODE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn unreachable_code_does_not_treat_replaced_terminators_as_builtins() {
    let assigned = check_standard_rules_source(
        r#"
error = print
assert = print

local function run()
  error("keep going")
  assert(false)
  return 0
end

return run
"#,
    );
    assert_eq!(
        count_code(&assigned, DiagnosticCode::LINT_UNREACHABLE_CODE),
        0,
        "{assigned:#?}"
    );

    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local function run()
  error("keep going")
  assert(false)
  return 0
end

return run
"#,
    )
    .expect("source should parse");
    let configured = check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        &parsed,
        &[std::path::PathBuf::from("src")],
        &DiagnosticsConfig {
            rules: true,
            globals: vec!["error".to_owned(), "assert".to_owned()],
            ..diagnostics_without_standard_rules()
        },
        &JitConfig::default(),
        &EcsConfig::default(),
    )
    .err()
    .unwrap_or_default();
    assert_eq!(
        count_code(&configured, DiagnosticCode::LINT_UNREACHABLE_CODE),
        0,
        "{configured:#?}"
    );
}

#[test]
fn unreachable_code_propagates_past_an_already_unreachable_tail() {
    let diagnostics = check_standard_rules_source(
        r#"
local function stop(flag)
  if flag then
    do return 1 end
    print("dead")
  else
    return 2
  end
  print("also dead")
end

return stop
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNREACHABLE_CODE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "unreachable code (previous statement always returns)",
            "unreachable code (previous statement always returns)",
        ]
    );
}

#[test]
fn unreachable_code_groups_each_region_and_identifies_breaks() {
    let diagnostics = check_standard_rules_source(
        r#"
local function scan(flag)
  while flag do
    do break end
    print("dead one")
    print("dead two")
  end
end

return scan
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNREACHABLE_CODE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["unreachable code (previous statement always breaks)"]
    );
}

#[test]
fn unreachable_code_allows_a_trailing_return_after_a_direct_error() {
    let diagnostics = check_standard_rules_source(
        r#"
local function stop()
  error("stopped")
  return 0
end

return stop
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNREACHABLE_CODE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn standard_rules_report_expression_and_library_rules() {
    let diagnostics = check_standard_rules_source(
        r#"
local kind = type(value) == "numbre"
local result = ready and false or fallback
local comparison = 1 < value < 3
local negated = not value == fallback
local huge = 9007199254740993
local formatted = string.format("%Q", value)
table.insert(items, #items + 1, value)
table.remove(items, 0)

for index = 3, 1 do
  print(index)
end

for index = #items, 1 do
  print(index)
end
"#,
    );

    for code in [
        DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE,
        DiagnosticCode::LINT_MISLEADING_AND_OR,
        DiagnosticCode::LINT_COMPARISON_PRECEDENCE,
        DiagnosticCode::LINT_INTEGER_PRECISION,
        DiagnosticCode::LINT_FORMAT_STRING,
        DiagnosticCode::LINT_TABLE_OPERATION,
        DiagnosticCode::LINT_FOR_RANGE,
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(code)),
            "expected {code} in {diagnostics:#?}"
        );
    }
}

#[test]
fn comparison_precedence_reports_operator_specific_messages_and_expression_ranges() {
    let cases = [
        (
            "not a == b",
            "`not x == y` is equivalent to `(not x) == y`; consider using `x ~= y`, or add parentheses to silence",
        ),
        (
            "not a ~= b",
            "`not x ~= y` is equivalent to `(not x) ~= y`; consider using `x == y`, or add parentheses to silence",
        ),
        (
            "not a < b",
            "`not x < y` is equivalent to `(not x) < y`; add parentheses to silence",
        ),
        (
            "not a <= b",
            "`not x <= y` is equivalent to `(not x) <= y`; add parentheses to silence",
        ),
        (
            "not a > b",
            "`not x > y` is equivalent to `(not x) > y`; add parentheses to silence",
        ),
        (
            "not a >= b",
            "`not x >= y` is equivalent to `(not x) >= y`; add parentheses to silence",
        ),
        (
            "a < b < c",
            "`x < y < z` is equivalent to `(x < y) < z`; did you mean `x < y and y < z`?",
        ),
        (
            "a <= b == c",
            "`x <= y == z` is equivalent to `(x <= y) == z`; add parentheses to silence",
        ),
    ];

    for (expression, expected_message) in cases {
        let source = format!(
            "local a, b, c = true, true, true\nlocal result = {expression}\nprint(result)\n"
        );
        let diagnostics = check_standard_rules_source(&source)
            .into_iter()
            .filter(|diagnostic| {
                diagnostic.code == Some(DiagnosticCode::LINT_COMPARISON_PRECEDENCE)
            })
            .collect::<Vec<_>>();

        assert_eq!(diagnostics.len(), 1, "{expression}: {diagnostics:#?}");
        assert_eq!(diagnostics[0].message, expected_message, "{expression}");
        assert_eq!(
            (
                diagnostics[0].line,
                diagnostics[0].column,
                diagnostics[0].end_line,
                diagnostics[0].end_column,
            ),
            (2, 16, 2, 16 + expression.chars().count()),
            "{expression}: {diagnostics:#?}"
        );
    }
}

#[test]
fn comparison_precedence_accepts_symmetric_negation_and_explicit_parentheses() {
    let diagnostics = check_standard_rules_source(
        r#"local a, b, c = true, true, true
local values = {
  not a == not b,
  not a ~= not b,
  not a <= not b,
  not (a == b),
  (not a) == b,
  not (a ~= b),
  (not a) ~= b,
  not (a <= b),
  (not a) <= b,
  (a <= b) == c,
  a <= (b == c),
}
print(values)
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_COMPARISON_PRECEDENCE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn misleading_and_or_identifies_the_alternative_and_reports_the_expression() {
    let diagnostics = check_standard_rules_source(
        r#"local condition = true
local outer = true
local fallback = 42
local other = 0
local falseResult = condition and false or fallback
local nilResult = condition and nil or fallback
local nested = outer and (condition and false or fallback) or other
print(falseResult, nilResult, nested)
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MISLEADING_AND_OR))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (
                diagnostic.line,
                diagnostic.column,
                diagnostic.end_line,
                diagnostic.end_column,
                diagnostic.message.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                5,
                21,
                5,
                52,
                "`and`-`or` expression always selects the fallback because its first alternative is `false`; use an `if` statement assignment to preserve `false`",
            ),
            (
                6,
                19,
                6,
                48,
                "`and`-`or` expression always selects the fallback because its first alternative is `nil`; use an `if` statement assignment to preserve `nil`",
            ),
            (
                7,
                27,
                7,
                58,
                "`and`-`or` expression always selects the fallback because its first alternative is `false`; use an `if` statement assignment to preserve `false`",
            ),
        ]
    );
}

#[test]
fn misleading_and_or_accepts_truthy_and_parenthesized_alternatives() {
    let diagnostics = check_standard_rules_source(
        r#"local condition = true
local fallback = 42
local truth = condition and true or fallback
local zero = condition and 0 or fallback
local groupedLeft = (condition and false) or fallback
local groupedAlternative = condition and (false) or fallback
print(truth, zero, groupedLeft, groupedAlternative)
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_MISLEADING_AND_OR),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn table_operations_report_indices_and_multireturns_exactly() {
    let diagnostics = check_standard_rules_source(
        r#"
local items = { 1, 2 }
local other = { 1 }

local function pair()
  return 1, 42
end

table.insert(items, #items, 42)
table.insert(items, (#items), 42)
table.insert(items, #(items), 42)
table.insert(items, #items + 1, 42)
table.insert(items, #other + 1, 42)
table.insert(items, 0, 42)
table.remove(items, 0)
table.remove(items, #items - 1)
table.insert(items, pair())
table.insert(items, (pair()))
table.insert(items, string.find("hello", "h"))
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_TABLE_OPERATION))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "table.insert will insert before the last element, which is likely a bug; omit the index to append or wrap it in parentheses to silence",
            "table.insert will insert before the last element, which is likely a bug; omit the index to append or wrap it in parentheses to silence",
            "table.insert will append the value; omit the index for efficiency",
            "table.insert uses index 0 but arrays are 1-based; did you mean 1 instead?",
            "table.remove uses index 0 but arrays are 1-based; did you mean 1 instead?",
            "table.remove will remove the value before the last element, which is likely a bug; omit the index to remove the last or wrap it in parentheses to silence",
            "table.insert may change behavior when the final call returns more than one value; wrap the second argument in parentheses to keep only its first result",
            "table.insert may change behavior when the final call returns more than one value; wrap the second argument in parentheses to keep only its first result",
        ],
        "{diagnostics:#?}"
    );
}

#[test]
fn vector_length_reports_hash_on_builtin_vec2_values() {
    let diagnostics = check_standard_rules_source(
        r#"
local v = vec2(3, 4)
local player = { pos = vec2(1, 2) }
print(#v, #player.pos, #vec2(5, 12), #-v, #(v + v), v.length)
"#,
    );
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_VECTOR_LENGTH))
        .collect::<Vec<_>>();
    let messages = warnings
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "`#v` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `v.length` instead",
            "`#player.pos` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `player.pos.length` instead",
            "`#vec2(5, 12)` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `vec2(5, 12).length` instead",
            "`#-v` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `(-v).length` instead",
            "`#(v + v)` is 0 when LuaJIT's JIT is off, because vectors are then plain tables; use `(v + v).length` instead",
        ],
        "{diagnostics:#?}"
    );
    assert!(warnings
        .iter()
        .all(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Warning));
    assert_eq!(
        (warnings[0].line, warnings[0].column, warnings[0].end_column),
        (4, 7, 9),
        "the warning should cover the whole `#v` expression"
    );
}

#[test]
fn vector_length_ignores_arrays_strings_and_project_vec2_aliases() {
    let diagnostics = check_standard_rules_source(
        r#"
type Vec2 = { x: number, y: number }

local p: Vec2 = { x = 1, y = 2 }
local list: number[] = { p.x, p.y }
print(#list, #"text")
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::LINT_VECTOR_LENGTH)),
        "{diagnostics:#?}"
    );
}

#[test]
fn table_operations_require_resolved_lua_builtins() {
    let diagnostics = check_standard_rules_source(
        r#"
local items = { 1 }
local record = { name = "Ada" }

table.insert(items, 0, 42)
ipairs(record)

do
  local table = {
    insert = function() end,
    remove = function() end,
  }
  table.insert(items, 0, 42)
  table.remove(items, 0)
end

local function customIterator(ipairs)
  ipairs(record)
end
customIterator(function() end)
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_TABLE_OPERATION))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "table.insert uses index 0 but arrays are 1-based; did you mean 1 instead?",
            "using 'ipairs' on a table without an array part is likely a bug",
        ],
        "{diagnostics:#?}"
    );
}

#[test]
fn table_operations_use_shared_table_types_conservatively() {
    let diagnostics = check_standard_rules_source(
        r#"
local record = { data = { 1, 2, 3 } }
local stringDictionary: { [string]: number } = {}
local numberDictionary: { [number]: number } = {}
local empty = {}
local array = { 1, 2 }
type Named = { name: string }
local uncertain: { [number]: number } | Named = {}
local dynamic: any = record

print(#record)
ipairs(record)
print(#stringDictionary)
ipairs(stringDictionary)

print(#numberDictionary)
ipairs(numberDictionary)
print(#empty)
ipairs(empty)
print(#array)
ipairs(array)
print(#uncertain)
ipairs(uncertain)
print(#dynamic)
ipairs(dynamic)

local withLength = { name = "Ada" }
setmetatable(withLength, {
  __len = function(): number return 1 end,
})
print(#withLength)

local withIpairs = { name = "Ada" }
setmetatable(withIpairs, {
  __ipairs = function() end,
})
ipairs(withIpairs)
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_TABLE_OPERATION))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "using '#' on a table without an array part is likely a bug",
            "using 'ipairs' on a table without an array part is likely a bug",
            "using '#' on a table with string keys is likely a bug",
            "using 'ipairs' on a table with string keys is likely a bug",
        ],
        "{diagnostics:#?}"
    );
}

#[test]
fn format_string_reports_luajit_and_lua51_errors_exactly() {
    let diagnostics = check_standard_rules_source(
        r#"
local subject = "12"
local _ = string.format("%")
local _ = string.format("%100d", 1)
local _ = string.match(subject, "(%d)%2")
local _ = string.match(subject, "((a)%1)")
local _ = string.match(subject, "%q")
local _ = string.match(subject, "%bx")
local _ = string.match(subject, "%foo")
local _ = string.match(subject, "(%d")
local _ = string.match(subject, "[%1]")
"#,
    );
    let format_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FORMAT_STRING))
        .collect::<Vec<_>>();

    assert_eq!(
        format_diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "invalid format string: unfinished format specifier",
            "invalid format string: format width or precision exceeds two digits",
            "invalid match pattern: capture reference `%2` does not exist",
            "invalid match pattern: capture reference `%1` must refer to a closed capture",
            "invalid match pattern: unknown character class `%q`",
            "invalid match pattern: balanced match `%b` requires two delimiter characters",
            "invalid match pattern: frontier pattern `%f` must be followed by a character class",
            "invalid match pattern: capture is missing a closing `)`",
            "invalid match pattern: character classes cannot contain capture references",
        ]
    );
    assert_eq!(
        format_diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        (3..=11).collect::<Vec<_>>()
    );
}

#[test]
fn format_string_accepts_valid_luajit_and_lua51_formats_and_patterns() {
    let diagnostics = check_standard_rules_source(
        r#"
local subject = "A12"
local _ = string.format("hello %+10d %.02f %% %a %p", 4, 5, 6, subject)
local _ = string.format("%------d", 1)
local _ = string.match(subject, "[A-Z]+(%d)%1")
local _ = string.match(subject, "%b()")
local _ = string.match(subject, "%f[%a]")
local _ = string.gsub(subject, "(%d)", "%0%1%%")
local _ = string.gsub(subject, "(%d)", "%x")
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FORMAT_STRING),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn format_string_supports_literal_format_methods_and_resolved_builtins() {
    let diagnostics = check_standard_rules_source(
        r#"
local _ = string.format("%")
local _ = ("%d %"):format(1, 2)

do
  local string = {
    format = function(_format) return "custom" end,
    match = function(_subject, _pattern) return "custom" end,
  }
  local _ = string.format("%")
  local _ = string.match("value", "[")
end
"#,
    );
    let format_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FORMAT_STRING))
        .collect::<Vec<_>>();

    assert_eq!(format_diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(format_diagnostics[0].line, 2);
    assert_eq!(format_diagnostics[1].line, 3);
    assert!(format_diagnostics.iter().all(
        |diagnostic| diagnostic.message == "invalid format string: unfinished format specifier"
    ));
}

#[test]
fn format_string_honors_plain_find_and_validates_gsub_replacements() {
    let diagnostics = check_standard_rules_source(
        r#"
local subject = "12"
local dynamicPlain = choosePlainMode()
local _ = string.find(subject, "[", 1, true)
local _ = string.find(subject, "[", 1, false)
local _ = string.find(subject, "[", 1, dynamicPlain)
local _ = string.gsub(subject, "(%d)", "%2")
local _ = string.gsub(subject, "(%d)", "%1")
local _ = string.gsub(subject, "(%d)", "%")
local _ = subject:match("[")
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FORMAT_STRING))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "invalid match pattern: character class is missing a closing `]`",
            "invalid match replacement: capture reference `%2` does not exist",
            "invalid match replacement: unfinished `%` replacement",
        ]
    );
}

#[test]
fn format_string_leaves_os_date_formats_to_host_strftime() {
    let diagnostics = check_standard_rules_source(
        r#"
local _ = os.date("%e %F %R %T")
local _ = os.date("%Q")
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FORMAT_STRING),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn for_range_reports_downward_and_array_ranges_with_exact_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
local items = {}

for index = #items, 1 do
  print(index)
end

for index = #items, 0 do
  print(index)
end

for index = 8, 1 do
  print(index)
end

return items
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FOR_RANGE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "range counts from a length down to 1 but has no negative step",
            "range counts from a length down to 0; use a negative step and consider changing 0 to 1 because arrays start at 1",
            "range counts downward but has no negative step",
        ]
    );
}

#[test]
fn for_range_computes_the_implicit_step_endpoint_from_both_bounds() {
    let diagnostics = check_standard_rules_source(
        r#"
for index = 1.3, 7.5 do
  print(index)
end

for index = 1.5, 8 do
  print(index)
end
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FOR_RANGE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "range ends at 7.3 instead of 7.5; specify an explicit step",
            "range ends at 7.5 instead of 8; specify an explicit step",
        ]
    );
}

#[test]
fn for_range_keeps_aligned_dynamic_and_explicit_step_ranges_quiet() {
    let diagnostics = check_standard_rules_source(
        r#"
local function iterate(start)
  for index = 1.5, 8.5 do
    print(index)
  end

  for index = start, 8.5 do
    print(index)
  end

  for index = 8, 1, -1 do
    print(index)
  end

  for index = 1.3, 7.5, 1 do
    print(index)
  end
end

return iterate
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FOR_RANGE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn for_range_parses_hexadecimal_integer_bounds_with_luajit_semantics() {
    let diagnostics = check_standard_rules_source(
        r#"
for index = 0x8, 0x1 do
  print(index)
end
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_FOR_RANGE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec!["range counts downward but has no negative step"]
    );
}

#[test]
fn standard_rules_comment_directive_rule_ignores_long_strings_and_comments() {
    let diagnostics = check_standard_rules_source(
        r#"
local text = [[
--!inside-string
]]

--[[
--!inside-comment
]]

--!outside
return text
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_COMMENT_DIRECTIVE),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn comment_directive_accepts_the_tua_generated_module_contract() {
    let diagnostics = check_standard_rules_source(
        r#"--!tua generated
return { 1, 2, 3 }
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_COMMENT_DIRECTIVE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn comment_directive_points_luau_nolint_to_exact_tua_file_suppression() {
    let diagnostics = check_standard_rules_source(
        r#"--!nolint MisleadingAndOr
return true
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_COMMENT_DIRECTIVE))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        diagnostics[0].message,
        "Luau comment directive `--!nolint` is not supported by Tua; use `-- tua-ignore-file TLxxxx` as the first nonblank line"
    );
}

#[test]
fn standard_rules_unknown_type_rule_accepts_luajit_cdata() {
    let diagnostics = check_standard_rules_source(
        r#"
local value = dynamic()
return type(value) == "cdata"
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn unknown_type_validates_luajit_type_names_with_target_specific_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
local function inspect(value)
  local upper = type(value) == "String"
  local luauOnly = "vector" ~= type(value)
  return upper, luauOnly
end

return inspect
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "unknown LuaJIT runtime type name `String`; did you mean `string`?",
            "unknown LuaJIT runtime type name `vector`; expected a value returned by the built-in `type(...)`",
        ]
    );
}

#[test]
fn unknown_type_accepts_every_luajit_type_result() {
    let diagnostics = check_standard_rules_source(
        r#"
local function inspect(value)
  return type(value) == "nil"
    or type(value) == "boolean"
    or type(value) == "userdata"
    or type(value) == "number"
    or type(value) == "string"
    or type(value) == "table"
    or type(value) == "function"
    or type(value) == "thread"
    or type(value) == "cdata"
end

return inspect
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn unknown_type_requires_the_resolved_luajit_type_builtin() {
    let diagnostics = check_standard_rules_source(
        r#"
local function inspect(value)
  local function type(_value)
    return "String"
  end

  local shadowed = type(value) == "String"
  local function typeof(_value)
    return "String"
  end
  local custom = typeof(value) == "String"
  return shadowed, custom
end

type = function(_value)
  return "String"
end
local reassigned = type(1) == "String"

return inspect, reassigned
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE),
        0,
        "{diagnostics:#?}"
    );

    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
return type(1) == "String"
"#,
        &DiagnosticsConfig {
            rules: true,
            globals: vec!["type".to_owned()],
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("a configured `type` function should not be treated as the LuaJIT builtin");
}

#[test]
fn unknown_type_validates_known_love_object_type_apis() {
    let diagnostics = check_standard_rules_source(
        r#"
local function inspect(canvas: Canvas, dynamic: any)
  local typo = canvas:type() == "Canavs"
  local concrete = canvas:type() == "Canvas"
  local parent = canvas:typeOf("Drawable")
  local misspelled = canvas:typeOf("Drawble")
  local shorthand = canvas:typeOf "image"
  local unknownReceiver = dynamic:typeOf("Missing")
  return typo, concrete, parent, misspelled, shorthand, unknownReceiver
end

return inspect
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "unknown LOVE object type name `Canavs`; expected a LOVE 11.x object type name",
            "unknown LOVE object type name `Drawble`; expected a LOVE 11.x object type name",
            "unknown LOVE object type name `image`; did you mean `Image`?",
        ]
    );
}

#[test]
fn unknown_type_does_not_confuse_user_aliases_with_love_objects() {
    let diagnostics = check_standard_rules_source(
        r#"
type Canvas = string

local function inspect(value: Canvas)
  return value:typeOf("Missing")
end

return inspect
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn unknown_type_leaves_named_runtime_checks_to_the_is_checker() {
    let diagnostics = check_standard_rules_source(
        r#"
type Record = {
  value: string,
}

local function inspect(value: any)
  if value is Record then
    return true
  end
  return false
end

return inspect
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNKNOWN_RUNTIME_TYPE),
        0,
        "{diagnostics:#?}"
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn standard_rules_table_literal_rule_normalizes_numeric_and_implicit_keys() {
    let diagnostics = check_standard_rules_source(
        r#"
local values = {
  "implicit",
  [1.0] = "explicit",
  [2] = "first",
  [2.0] = "second",
}
return values
"#,
    );
    let table_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY))
        .collect::<Vec<_>>();

    assert_eq!(table_diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(table_diagnostics[0].line, 4);
    assert_eq!(table_diagnostics[0].column, 4);
    assert_eq!(
        table_diagnostics[0].message,
        "table key `1.0` duplicates list entry 1"
    );
    assert_eq!(table_diagnostics[1].line, 6);
    assert_eq!(table_diagnostics[1].column, 4);
    assert_eq!(
        table_diagnostics[1].message,
        "table key `2.0` is a duplicate; previously defined at line 5"
    );
}

#[test]
fn table_literal_rule_uses_runtime_string_and_signed_number_keys() {
    let diagnostics = check_standard_rules_source(
        r#"
local values = {
  a = "name",
  ["\97"] = "decimal escape",
  [-1] = "integer",
  [-1.0] = "float",
}
return values
"#,
    );
    let table_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY))
        .collect::<Vec<_>>();

    assert_eq!(table_diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(table_diagnostics[0].line, 4);
    assert_eq!(table_diagnostics[0].column, 4);
    assert_eq!(
        table_diagnostics[0].message,
        r"table key `\97` is a duplicate; previously defined at line 3"
    );
    assert_eq!(table_diagnostics[1].line, 6);
    assert_eq!(table_diagnostics[1].column, 4);
    assert_eq!(
        table_diagnostics[1].message,
        "table key `-1.0` is a duplicate; previously defined at line 5"
    );
}

#[test]
fn table_literal_rule_reports_duplicate_table_type_fields() {
    let diagnostics = check_standard_rules_source(
        r#"
type Shape = {
  first: number,
  first: string,
}
"#,
    );
    let table_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY))
        .collect::<Vec<_>>();

    assert_eq!(table_diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(table_diagnostics[0].line, 4);
    assert_eq!(table_diagnostics[0].column, 3);
    assert_eq!(
        table_diagnostics[0].message,
        "table type field `first` is a duplicate; previously defined at line 3"
    );
}

#[test]
fn duplicate_keys_option_includes_list_entries_and_table_type_fields() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local values = {
  "implicit",
  [1] = "explicit",
}
type Shape = {
  value: number,
  value: string,
}
return values
"#,
        &DiagnosticsConfig {
            duplicate_keys: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("the dedicated duplicate-key option should report every table collision");
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "table key `1` duplicates list entry 1",
            "table type field `value` is a duplicate; previously defined at line 7",
        ],
        "{diagnostics:#?}"
    );
}

#[test]
fn standard_rules_report_statement_layout_and_comment_directives() {
    let diagnostics = check_standard_rules_source(
        r#"--!mystery
local first = 1 local second = 2
local total = first +
second
"#,
    );

    for code in [
        DiagnosticCode::LINT_COMMENT_DIRECTIVE,
        DiagnosticCode::LINT_SAME_LINE_STATEMENT,
        DiagnosticCode::LINT_MULTI_LINE_STATEMENT,
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(code)),
            "expected {code} in {diagnostics:#?}"
        );
    }
}

#[test]
fn same_line_statement_counts_unseparated_pairs_in_nested_blocks() {
    let diagnostics = check_standard_rules_source(
        r#"
if true then local first = 1 local second = 2 print(first, second) end
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_SAME_LINE_STATEMENT),
        2,
        "{diagnostics:#?}"
    );
}

#[test]
fn same_line_statement_accepts_semicolon_separators() {
    let diagnostics = check_standard_rules_source(
        r#"
if true then local first = 1; local second = 2; print(first, second) end
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_SAME_LINE_STATEMENT),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn multi_line_statement_checks_nested_calls_and_accepts_indentation() {
    let diagnostics = check_standard_rules_source(
        r#"
print(math.max(1,
math.min(2, 3)))

print(math.max(1,
  math.min(2, 3)))
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_MULTI_LINE_STATEMENT),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn multi_line_statement_checks_return_expressions() {
    let diagnostics = check_standard_rules_source(
        r#"
return math.max(1,
math.min(2, 3))
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_MULTI_LINE_STATEMENT),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn multi_line_statement_stays_within_nested_statement_and_table_boundaries() {
    let diagnostics = check_standard_rules_source(
        r#"
if true then
print("block indentation belongs to the formatter")
end

local values = {
answer = 42,
}

repeat
print("repeat condition indentation stays exempt")
until true

if true then
  print(math.max(1,
math.min(2, 3)))
end

return values
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_MULTI_LINE_STATEMENT),
        1,
        "{diagnostics:#?}"
    );
}

#[test]
fn multi_line_statement_uses_statement_columns_and_identifier_nodes() {
    let diagnostics = check_standard_rules_source(
        r#"
local first = 1; print(math.max(1,
  2))

local value = first +
endless
local other = first +
elsewhere
return value, other
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_MULTI_LINE_STATEMENT),
        3,
        "{diagnostics:#?}"
    );
}

#[test]
fn reports_enabled_lints() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local config = {
  hp = 10,
  hp = 20,
  ["name"] = "first",
  name = "second",
}

local hp = 10
hp = hp

local Missing = require("definitely_missing_lint_module")
"#,
        &DiagnosticsConfig {
            duplicate_keys: true,
            missing_requires: true,
            self_assignments: true,
            doc_annotations: false,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("enabled lints should report diagnostics");

    assert_eq!(diagnostics.len(), 4);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning));
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_TABLE_KEY)
            })
            .count(),
        2
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_SELF_ASSIGNMENT)
            && diagnostic.message.contains("self-assignment of `hp`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_MISSING_REQUIRE)
            && diagnostic
                .message
                .contains("require file not found: `definitely_missing_lint_module`")
    }));
}

#[test]
fn duplicate_parameter_toggle_emits_duplicate_local_diagnostic() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"local function move(player, player)
  return player
end
"#,
        &DiagnosticsConfig {
            duplicate_parameters: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("duplicate parameters should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_DUPLICATE_LOCAL)
    );
    assert_eq!(
        diagnostics[0].message,
        "function parameter `player` already defined on column 21"
    );
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 29));
}

#[test]
fn reports_bad_require_files_with_dedicated_toggle() {
    let project = TempProject::new("bad-require");

    project.write("tua.toml", "source = \"src\"\n");

    let main_path = project.path.join("src/main.tua");
    let diagnostics = check_tua_source_with_diagnostics_config(
        &main_path,
        r#"local Missing = require("definitely_missing_bad_require_module")
"#,
        &DiagnosticsConfig {
            bad_requires: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("bad require lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_BAD_REQUIRE));
    assert!(diagnostics[0]
        .message
        .contains("bad require file: `definitely_missing_bad_require_module`"));
}

#[test]
fn bad_require_lint_does_not_duplicate_missing_require_lint() {
    let project = TempProject::new("bad-require-no-duplicate");

    project.write("tua.toml", "source = \"src\"\n");

    let main_path = project.path.join("src/main.tua");
    let diagnostics = check_tua_source_with_diagnostics_config(
        &main_path,
        r#"local Missing = require("definitely_missing_bad_require_module")
"#,
        &DiagnosticsConfig {
            missing_requires: true,
            bad_requires: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("bad require lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_BAD_REQUIRE));
}

#[test]
fn static_import_binding_is_not_an_undefined_variable() {
    let project = TempProject::new("static-import-not-undefined");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/player.tua", "return {}\n");

    let main_path = project.path.join("src/main.tua");
    let diagnostics = check_tua_source_with_diagnostics_config(
        &main_path,
        r#"local Player = import("./player")

return Player
"#,
        &diagnostics_without_standard_rules(),
    );

    assert_eq!(diagnostics, Ok(()));
}

#[test]
fn unresolved_static_import_binding_is_not_an_undefined_variable_when_unchecked() {
    let project = TempProject::new("unresolved-import-not-undefined");

    project.write("tua.toml", "source = \"src\"\n");

    let main_path = project.path.join("src/main.tua");
    let diagnostics = check_tua_source_with_diagnostics_config(
        &main_path,
        r#"local Player = import("player")

return Player
"#,
        &diagnostics_without_standard_rules(),
    );

    assert_eq!(diagnostics, Ok(()));
}

#[test]
fn reports_impossible_boolean_expressions_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local ready = true

if ready and false then
end

if true or ready then
end

if not false then
end
"#,
        &DiagnosticsConfig {
            impossible_boolean_expressions: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("impossible boolean lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_IMPOSSIBLE_BOOLEAN),
        3
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_IMPOSSIBLE_BOOLEAN)
            && diagnostic.severity == DiagnosticSeverity::Warning
    }));
}

#[test]
fn reports_duplicate_if_conditions_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local hp = 10

if hp > 0 then
elseif hp > 0 then
end
"#,
        &DiagnosticsConfig {
            duplicate_if_conditions: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("duplicate if condition lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_DUPLICATE_IF_CONDITION)
    );
    assert_eq!(
        diagnostics[0].message,
        "condition has already been checked on line 4"
    );
}

#[test]
fn reports_duplicate_binary_operands_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local hp = 10
local same = hp - hp

if hp == hp then
end
"#,
        &DiagnosticsConfig {
            duplicate_binary_operands: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("duplicate binary operand lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND),
        2
    );
}

#[test]
fn duplicate_condition_flattens_boolean_chains_and_reports_origins() {
    let diagnostics = check_standard_rules_source(
        r#"local a = true
local b = false
local self = { _adorns = {} }
local normID1 = 1
local chain = a and b and a
local grouped = (a or b) or (a)
assert(self._adorns[normID1] and self._adorns[normID1])

if a then
elseif b then
elseif (a) then
end

print(chain, grouped)
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                Some(
                    DiagnosticCode::LINT_DUPLICATE_IF_CONDITION
                        | DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND
                )
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 4, "{diagnostics:#?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (
                diagnostic.code,
                diagnostic.line,
                diagnostic.column,
                diagnostic.message.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                Some(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND),
                5,
                27,
                "condition has already been checked on column 15",
            ),
            (
                Some(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND),
                6,
                30,
                "condition has already been checked on column 18",
            ),
            (
                Some(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND),
                7,
                34,
                "condition has already been checked on column 8",
            ),
            (
                Some(DiagnosticCode::LINT_DUPLICATE_IF_CONDITION),
                11,
                8,
                "condition has already been checked on line 9",
            ),
        ]
    );
}

#[test]
fn duplicate_condition_compares_runtime_literals_without_collapsing_strings() {
    let diagnostics = check_standard_rules_source(
        r#"local value = 1
local text = "a b"
local numeric = value == 1 and value == 1.0
local distinct = text == "a  b" and text == "a b"
local escaped = text == "\97" and text == "a"
print(numeric, distinct, escaped)
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(
        diagnostics[0].message,
        "condition has already been checked on column 17"
    );
    assert_eq!(diagnostics[1].line, 5);
    assert_eq!(
        diagnostics[1].message,
        "condition has already been checked on column 17"
    );
}

#[test]
fn duplicate_condition_suppresses_and_or_ternary_idioms() {
    let diagnostics = check_standard_rules_source(
        r#"local a = true
local b = false
local c = true
local ternary = a and a or b
local groupedTernary = (a and a) or b
local longerChain = (a and b and a) or c
local nestedAlternative = a and a or (a and a)
print(ternary, groupedTernary, longerChain, nestedAlternative)
"#,
    );
    let diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DUPLICATE_BINARY_OPERAND))
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(
        diagnostics[0].message,
        "condition has already been checked on column 22"
    );
    assert_eq!(diagnostics[1].line, 7);
}

#[test]
fn reports_float_equality_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local value = 0

if value == 0.5 then
end

if 1e-3 ~= value then
end
"#,
        &DiagnosticsConfig {
            float_equality: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("float equality lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FLOAT_EQUALITY),
        2
    );
}

#[test]
fn reports_const_assignments_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local MAX_HP = 10
MAX_HP = 20
"#,
        &DiagnosticsConfig {
            const_assignments: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("const assignment lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_CONST_ASSIGNMENT)
    );
    assert!(diagnostics[0]
        .message
        .contains("assignment to const-like name `MAX_HP`"));
}

#[test]
fn checker_function_contracts_are_always_type_diagnostics() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function damage(amount: number): string
  return amount
end

damage()
damage("bad")
"#,
        &diagnostics_without_standard_rules(),
    )
    .expect_err("checker-backed function diagnostics should report");

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::TYPE_ARGUMENT_COUNT),
        1
    );
    assert_eq!(count_code(&diagnostics, DiagnosticCode::TYPE_MISMATCH), 2);
}

#[test]
fn missing_require_lint_accepts_resolved_project_modules() {
    let project = TempProject::new("resolved-require");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/player.tua", "return { hp = 10 }\n");

    let main_path = project.path.join("src/main.tua");

    check_tua_source_with_diagnostics_config(
        &main_path,
        r#"local Player = require("player")
"#,
        &DiagnosticsConfig {
            missing_requires: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("resolved project requires should not warn");
}

#[test]
fn reports_undefined_variables_independently() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local value = missing + math.sin(1)
love.graphics.print(value)
ConfiguredGlobal()
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            globals: vec!["ConfiguredGlobal".to_owned()],
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("undefined variable lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(diagnostics[0]
        .message
        .contains("undefined variable `missing`"));
}

#[test]
fn undefined_variables_remain_warnings_in_lua_sources() {
    let parsed = parse_lua_source("src/main.script", "return missing\n".to_owned())
        .expect("Lua source should parse");
    let diagnostics = check_parsed_source_with_diagnostics_config(
        &parsed,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("undefined Lua variable should report a diagnostic");

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
    );
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn accepts_globals_defined_later_in_the_same_file() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function readLater()
  return LaterValue + LaterFunction()
end

SelfInitialized = SelfInitialized or {}
LaterValue = 40

function LaterFunction()
  return 2
end

return readLater(), SelfInitialized
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("global definitions should apply across the current file");
}

#[test]
fn later_local_declarations_do_not_define_globals() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function readLater()
  return LaterLocal
end

local LaterLocal = 1
return readLater(), LaterLocal
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("a later local is not in an earlier function's lexical scope");

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
    );
}

#[test]
fn forward_global_reads_count_for_global_used_as_local() {
    let diagnostics = check_standard_rules_source(
        r#"
local function readShared()
  return Shared
end

local function writeShared()
  Shared = 1
end

readShared()
writeShared()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNDEFINED_VARIABLE),
        0,
        "{diagnostics:#?}"
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn global_used_as_local_tracks_the_common_enclosing_function() {
    let diagnostics = check_standard_rules_source(
        r#"
local function outer()
  local function writeShared()
    Shared = 1
  end

  local function readShared()
    return Shared
  end

  writeShared()
  return readShared()
end

outer()
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL))
        .expect("a global shared only by nested closures can be local to their parent");

    assert!(
        diagnostic.message.contains("one enclosing function"),
        "{diagnostics:#?}"
    );
}

#[test]
fn global_used_as_local_reports_independent_dominated_writes() {
    let diagnostics = check_standard_rules_source(
        r#"
local function writeFirst()
  Scratch = 1
  return Scratch
end

local function writeSecond()
  Scratch = 2
  return Scratch
end

writeFirst()
writeSecond()
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL))
        .expect("a global only read after independent writes can become local");

    assert!(
        diagnostic
            .message
            .contains("never read before being written"),
        "{diagnostics:#?}"
    );
}

#[test]
fn global_used_as_local_respects_conditional_writes() {
    let diagnostics = check_standard_rules_source(
        r#"
local function maybeWrite(flag)
  if flag then
    Shared = 1
  end
  return Shared
end

local function writeShared()
  Shared = 2
end

maybeWrite(true)
writeShared()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn global_used_as_local_ignores_module_scope_definitions() {
    let diagnostics = check_standard_rules_source(
        r#"
Shared = 1

local function readShared()
  return Shared
end

readShared()
"#,
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_GLOBAL_USED_AS_LOCAL),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn compound_assignment_does_not_define_an_unknown_global() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        "MissingScore += 1\n",
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("compound assignment should read its target before writing it");

    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
    );
}

#[test]
fn accepts_builtin_ecs_module_for_undefined_variable_lint() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
return { Position = Position, world = world }
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("built-in ecs module should not be reported as undefined");
}

#[test]
fn accepts_builtin_object_pool_for_undefined_variable_lint() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"local pool = objectPool({
  create = function()
    return {}
  end,
})
return pool
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("built-in objectPool should not be reported as undefined");
}

#[test]
fn reports_undefined_function_calls_in_local_initializers() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local b = atun("")
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("undefined function call should report a diagnostic");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNDEFINED_VARIABLE)
            && diagnostic.message.contains("undefined variable `atun`")
    }));
}

#[test]
fn accepts_luajit_bit_global_for_undefined_variable_lint() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
if bit then
  local mask = bit.bor(1, 2)
  print(mask)
end
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("LuaJIT bit library should be a known builtin global");
}

#[test]
fn reports_unused_locals_and_parameters_independently() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local used = 1
print(used)

local unused = 2
local _ignored = 3

local function test(used_param, unused_param, _ignored_param)
  print(used_param)
end

test(1, 2, 3)
"#,
        &DiagnosticsConfig {
            unused_locals: true,
            unused_parameters: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("unused symbol lints should report diagnostics");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_LOCAL)
            && diagnostic.message.contains("unused local `unused`")
            && diagnostic.message.contains("prefix with `_` to silence")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_PARAMETER)
            && diagnostic
                .message
                .contains("unused parameter `unused_param`")
    }));
}

#[test]
fn reports_unused_local_global_and_nested_functions() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
function unusedGlobal()
end

local function unusedLocal()
end

local function outer()
  local function nestedUnused()
  end
  return 1
end

function usedGlobal()
end

local function usedLocal()
end

local function _unusedLocal()
end

function _unusedGlobal()
end

print(outer(), usedGlobal(), usedLocal())
"#,
        &DiagnosticsConfig {
            rules: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("unused functions should report diagnostics");
    let mut messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_FUNCTION))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    messages.sort_unstable();

    assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
    assert_eq!(
        messages,
        vec![
            "function `nestedUnused` is never used; prefix with `_` to silence",
            "function `unusedGlobal` is never used; prefix with `_` to silence",
            "function `unusedLocal` is never used; prefix with `_` to silence",
        ]
    );
}

#[test]
fn counts_default_parameter_and_nested_closure_reads_as_local_uses() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local fallback = 10
local captured = 20

local function pick(value = fallback)
  return function()
    return value + captured
  end
end

print(pick()())
"#,
        &DiagnosticsConfig {
            unused_locals: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("default expressions and nested closures should count as reads");
}

#[test]
fn excludes_loop_bindings_from_unused_local_diagnostics() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local items = { 1, 2, 3 }
for key, value in pairs(items) do
  print(value)
end

for index = 1, 3 do
  print("tick")
end

local explicit = 1
"#,
        &DiagnosticsConfig {
            unused_locals: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("the explicit unused local should still be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_UNUSED_LOCAL));
    assert!(diagnostics[0].message.contains("unused local `explicit`"));
}

#[test]
fn reports_write_only_locals_as_unused() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local write_only = 1
write_only = 2
"#,
        &DiagnosticsConfig {
            unused_locals: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("a write-only local should be unused");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_UNUSED_LOCAL));
    assert!(diagnostics[0]
        .message
        .contains("prefix with `_` to silence"));
}

#[test]
fn compound_assignments_read_rhs_parameters_for_symbol_lints() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
type Player = {
  hp: number,
}

local function damage(player: Player, amount: number)
  player.hp -= amount
end

local player: Player = { hp = 10 }
damage(player, 2)
"#,
        &DiagnosticsConfig {
            unused_parameters: true,
            unused_locals: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("compound assignment operands should count as reads");
}

#[test]
fn colon_methods_have_implicit_self_for_symbol_lints() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local Account = {}

function Account:deposit(amount: number)
  print(self, amount)
end
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            unused_parameters: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("colon method bodies should have an implicit self parameter");
}

#[test]
fn shadowing_and_nested_scopes_track_the_right_binding() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local value = 1

do
  local value = 2
  print(value)
end

print(value)
"#,
        &DiagnosticsConfig {
            unused_locals: true,
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("shadowed locals should each count their own uses");
}

#[test]
fn reports_use_before_local_definition_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
print(score)
local score = 1
print(score)
"#,
        &DiagnosticsConfig {
            use_before_definition: true,
            undefined_variables: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("use-before-definition should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(DiagnosticCode::LINT_USE_BEFORE_DEFINITION)
    );
    assert!(diagnostics[0]
        .message
        .contains("use of local `score` before its definition"));
}

#[test]
fn dynamic_required_modules_and_known_globals_remain_permissive() {
    let project = TempProject::new("symbol-globals");

    project.write("tua.toml", "source = \"src\"\n");
    project.write("src/player.lua", "return {}\n");

    let main_path = project.path.join("src/main.tua");

    check_tua_source_with_diagnostics_config(
        &main_path,
        r#"
local Player = require("player")
Player.unknown_export()
math.sin(1)
love.graphics.print("ok")
ConfiguredGlobal()
_G.DynamicGlobal()
"#,
        &DiagnosticsConfig {
            undefined_variables: true,
            globals: vec!["ConfiguredGlobal".to_owned()],
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("known globals and dynamic module fields should not warn");
}

#[test]
fn reports_unbalanced_assignments_with_exact_guidance() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local a, b, c = 1, 2
local d = 1, 2
a, b = 1
d = 1, 2, 3
"#,
        &DiagnosticsConfig {
            assignment_arity: true,
            local_declaration_arity: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("arity lints should report diagnostics");

    let local_messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_LOCAL_DECLARATION_ARITY))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    let assignment_messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_ASSIGNMENT_ARITY))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        local_messages,
        vec![
            concat!(
                "assigning 2 values to 3 variables initializes extra variables with nil; ",
                "add `nil` to the value list to silence"
            ),
            "assigning 2 values to 1 variable leaves some values unused",
        ]
    );
    assert_eq!(
        assignment_messages,
        vec![
            concat!(
                "assigning 1 value to 2 variables initializes extra variables with nil; ",
                "add `nil` to the value list to silence"
            ),
            "assigning 3 values to 1 variable leaves some values unused",
        ]
    );
}

#[test]
fn unbalanced_assignment_accepts_multivalue_tails_and_explicit_nil() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function pair()
  return 1, 2
end

local function collect(...)
  local first = pair()
  local a, b, c = pair()
  local d, e, f = dynamic()
  local g, h, i = ...
  local ok, result, extra = pcall(pair), nil

  first = pair()
  a, b, c = pair()
  d, e, f = dynamic()
  g, h, i = ...
  ok, result, extra = pcall(pair), nil

  return first, a, b, c, d, e, f, g, h, i, ok, result, extra
end

return collect
"#,
        &DiagnosticsConfig {
            assignment_arity: true,
            local_declaration_arity: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("calls, varargs, and explicit nil tails should remain permissive");
}

#[test]
fn recursive_alias_does_not_crash_unbalanced_assignment_lint() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
type Loop = Loop
local value: Loop = dynamic()
local first, second = value()
return first, second
"#,
        &DiagnosticsConfig {
            assignment_arity: true,
            local_declaration_arity: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .err()
    .unwrap_or_default();

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_LOCAL_DECLARATION_ARITY),
        0,
        "{diagnostics:#?}"
    );
}

#[test]
fn deprecated_api_reports_resolved_functions_and_members_with_guidance() {
    let diagnostics = check_standard_rules_source(
        r#"
---@deprecated use newApi
local function oldApi() end
oldApi()

local function runShadow()
  local oldApi = function() end
  oldApi()
end
runShadow()

local api = {}
---@deprecated use api.new
function api.old() end
api.old()
local saved = api.old
print(saved)

---@deprecated use api.newMethod
function api:oldMethod() end
api:oldMethod()

---@deprecated
local function retired() end
retired()

---@deprecated use newGlobal
function oldGlobal() end
oldGlobal()
"#,
    );
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DEPRECATED_API))
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        messages,
        vec![
            "API `oldApi` is deprecated; use newApi",
            "API `api.old` is deprecated; use api.new",
            "API `api.old` is deprecated; use api.new",
            "API `api.oldMethod` is deprecated; use api.newMethod",
            "API `retired` is deprecated",
            "API `oldGlobal` is deprecated; use newGlobal",
        ],
        "{diagnostics:#?}"
    );
}

#[test]
fn reports_unused_assignments_when_local_is_never_read() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local unused = 1
unused = 2

local used = 1
used = 2
print(used)
"#,
        &DiagnosticsConfig {
            unused_assignments: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("unused assignment lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNUSED_ASSIGNMENT),
        1
    );
    assert!(diagnostics[0]
        .message
        .contains("assignment to local `unused` is never read"));
}

#[test]
fn reports_binary_operator_type_mismatches_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local badMath = 1 + "hp"
local badCompare = true < 2
local okConcat = "hp" .. 10
local unknown = dynamic + 1
"#,
        &DiagnosticsConfig {
            binary_operator_type_mismatches: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("binary operator type lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_BINARY_OPERATOR_TYPE),
        2
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator `+` cannot be applied to number and string")
    }));
}

#[test]
fn accepts_valid_ordering_operators_when_binary_operator_lint_is_enabled() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local hp: number = 10
local name: string = "Ada"
local healthy = hp > 0
local high = hp >= 9
local sorted = name <= "Mina"
"#,
        &DiagnosticsConfig {
            binary_operator_type_mismatches: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect("valid number and string ordering operators should not warn");
}

#[test]
fn reports_referenced_local_functions_that_are_never_directly_called() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function callback()
end

local function called()
end

print(callback)
called()
"#,
        &DiagnosticsConfig {
            uncalled_local_functions: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("uncalled local function lint should report diagnostics");

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_UNCALLED_LOCAL_FUNCTION),
        1
    );
    assert!(diagnostics[0]
        .message
        .contains("local function `callback` is referenced but never directly called"));
}

#[test]
fn accepts_local_functions_exported_from_module_returns() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/module.tua",
        r#"
local function direct()
end

local function shorthand()
end

local function conditional()
end

local function assignedField()
end

local function privateCallback()
end

local internal = { callback = privateCallback }
print(internal)

local module = { conditional = conditional }
module.assignedField = assignedField

if featureFlag then
  return {
    direct = direct,
    nested = { shorthand },
  }
end

return module
"#,
        &DiagnosticsConfig {
            uncalled_local_functions: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("only the private callback should remain uncalled");

    let uncalled = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_UNCALLED_LOCAL_FUNCTION))
        .collect::<Vec<_>>();

    assert_eq!(uncalled.len(), 1, "{diagnostics:#?}");
    assert!(uncalled[0].message.contains("`privateCallback`"));
}

#[test]
fn jit_lints_are_disabled_by_default() {
    check_tua_source(
        "src/main.tua",
        r#"
local player = { hp = 10 }
player.name = "late"

for i = 1, 3 do
  player[i] = i
end
"#,
    )
    .expect("JIT guidance should be opt-in");
}

#[test]
fn reports_jit_type_and_table_shape_drift_when_enabled() {
    let diagnostics = check_jit_source(
        r#"
local value = 1
value = "shape drift"

local player = { hp = 10 }
player.name = "late"
player.hp = "wrong"

local dynamic: any = 1
dynamic = "allowed"
"#,
        JitConfig {
            enabled: true,
            ..JitConfig::default()
        },
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_JIT_TYPE_DRIFT),
        0
    );
    assert_eq!(count_code(&diagnostics, DiagnosticCode::TYPE_MISMATCH), 2);
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_JIT_TABLE_SHAPE_DRIFT),
        2
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("type mismatch: expected number, found string")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("late field insertion `player.name`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("table field type drift for `player.hp`")
    }));
}

#[test]
fn reports_jit_dynamic_table_writes_inside_loops_when_enabled() {
    let diagnostics = check_jit_source(
        r#"
local values = {}

for i = 1, 3 do
  values[i] = i
  values.fixed = i
end
"#,
        JitConfig {
            enabled: true,
            ..JitConfig::default()
        },
    );

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_JIT_HOT_LOOP_DYNAMIC_KEY),
        1
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == DiagnosticSeverity::Hint
            && diagnostic
                .message
                .contains("hot loop uses a dynamic table write")
    }));
}

#[test]
fn jit_sub_rules_can_be_disabled_individually() {
    let diagnostics = check_jit_source(
        r#"
local player = { hp = 10 }
player.name = "late"

for i = 1, 3 do
  player[i] = i
end
"#,
        JitConfig {
            enabled: true,
            variable_type_drift: false,
            table_shape_drift: false,
            hot_loop_hints: false,
        },
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn class_lints_report_constructor_paths_virtual_init_calls_and_redundant_fields() {
    let diagnostics = check_standard_rules_source(
        r#"open class Base {
  value: number | nil

  init() {
  }

  configure() {
  }
}

open class Child extends Base {
  value: number | nil

  init(skip: boolean) {
    if not skip then
      super()
    end
    -- VirtualCallInInit (TL4058)
    self:configure()
  }
}
"#,
    );

    let parent_constructor = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_CLASS_PARENT_CONSTRUCTOR))
        .expect("a constructor path that skips super should report TL4057");
    assert_eq!(
        parent_constructor.message,
        "constructor for class `Child` can return without calling parent constructor `Base`; call `super(...)` on every returning path"
    );
    assert_eq!(
        (
            parent_constructor.line,
            parent_constructor.column,
            parent_constructor.end_line,
            parent_constructor.end_column,
        ),
        (14, 3, 14, 7)
    );

    let virtual_call = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_CLASS_VIRTUAL_CALL_IN_INIT))
        .expect("an overridable self call during init should report TL4058");
    assert_eq!(
        virtual_call.message,
        "overridable method `configure` is called during `Child.init`; a subclass override can observe incomplete state, so mark the method `final` or move the call out of `init`"
    );
    assert_eq!(
        (
            virtual_call.line,
            virtual_call.column,
            virtual_call.end_line,
            virtual_call.end_column,
        ),
        (19, 5, 19, 21)
    );

    let redundant_field = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::LINT_CLASS_REDUNDANT_INHERITED_FIELD)
        })
        .expect("a type-only inherited field redeclaration should report TL4059");
    assert_eq!(
        redundant_field.message,
        "field `value` in class `Child` redundantly redeclares the inherited field from class `Base`; remove this declaration"
    );
    assert_eq!(
        (
            redundant_field.line,
            redundant_field.column,
            redundant_field.end_line,
            redundant_field.end_column,
        ),
        (12, 3, 12, 8)
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| {
                matches!(
                    diagnostic.code,
                    Some(
                        DiagnosticCode::LINT_CLASS_PARENT_CONSTRUCTOR
                            | DiagnosticCode::LINT_CLASS_VIRTUAL_CALL_IN_INIT
                            | DiagnosticCode::LINT_CLASS_REDUNDANT_INHERITED_FIELD
                    )
                )
            })
            .count(),
        3,
        "{diagnostics:#?}"
    );
}

#[test]
fn class_lints_accept_complete_parent_paths_final_calls_and_deferred_callbacks() {
    let diagnostics = check_standard_rules_source(
        r#"open class Base {
  value: number | nil

  init() {
  }

  final stable() {
  }

  configure() {
  }
}

open class Safe extends Base {
  init(early: boolean) {
    if early then
      super()
      return
    end

    super()
    self:stable()

    local callback = function()
      self:overridable()
    end
    print(callback)
  }

  overridable() {
  }
}

open class Automatic extends Base {
}

open class Customized extends Base {
  value: number | nil = 1

  init() {
    super()
  }
}

open class ForwardChild extends ForwardBase {
  init() {
    -- Forward parents use the same source call identity.
    super()
  }
}

open class ForwardBase {
  init() {
  }
}

open class ShadowedSelf extends Base {
  init() {
    super()
    local self = { configure = function() end }
    self.configure()
  }
}
"#,
    );

    for code in [
        DiagnosticCode::LINT_CLASS_PARENT_CONSTRUCTOR,
        DiagnosticCode::LINT_CLASS_VIRTUAL_CALL_IN_INIT,
        DiagnosticCode::LINT_CLASS_REDUNDANT_INHERITED_FIELD,
    ] {
        assert_eq!(count_code(&diagnostics, code), 0, "{diagnostics:#?}");
    }
}

#[test]
fn disabling_standard_rules_keeps_class_contract_errors_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"open class Base {
  value: number

  init() {
    self.value = 1
  }

  update() {
  }
}

open class Child extends Base {
  value: number

  init() {
    self:update()
  }
}
"#,
        &diagnostics_without_standard_rules(),
    )
    .expect_err("required class fields should remain correctness errors");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_CLASS)),
        "{diagnostics:#?}"
    );
    for code in [
        DiagnosticCode::LINT_CLASS_PARENT_CONSTRUCTOR,
        DiagnosticCode::LINT_CLASS_VIRTUAL_CALL_IN_INIT,
        DiagnosticCode::LINT_CLASS_REDUNDANT_INHERITED_FIELD,
    ] {
        assert_eq!(count_code(&diagnostics, code), 0, "{diagnostics:#?}");
    }
}

#[test]
fn exact_codes_suppress_class_lints() {
    check_tua_source(
        "src/main.tua",
        r#"-- tua-ignore-file TL4057 TL4058 TL4059
open class Base {
  value: number | nil

  init() {
  }

  update() {
  }
}

open class Child extends Base {
  value: number | nil

  init(skip: boolean) {
    if not skip then
      super()
    end
    self:update()
  }
}
"#,
    )
    .expect("each class lint should support exact-code suppression");
}

fn count_code(diagnostics: &[crate::Diagnostic], code: DiagnosticCode) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(code))
        .count()
}

fn check_jit_source(source: &str, jit_config: JitConfig) -> Vec<crate::Diagnostic> {
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");

    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        &parsed,
        &[std::path::PathBuf::from("src")],
        &diagnostics_without_standard_rules(),
        &jit_config,
        &EcsConfig::default(),
    )
    .err()
    .unwrap_or_default()
}

fn check_standard_rules_source(source: &str) -> Vec<crate::Diagnostic> {
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");

    check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config(
        &parsed,
        &[std::path::PathBuf::from("src")],
        &DiagnosticsConfig {
            rules: true,
            ..diagnostics_without_standard_rules()
        },
        &JitConfig::default(),
        &EcsConfig::default(),
    )
    .err()
    .unwrap_or_default()
}

fn diagnostics_without_standard_rules() -> DiagnosticsConfig {
    // Dedicated-toggle tests isolate one optional rule from the standard set.
    DiagnosticsConfig {
        rules: false,
        ..DiagnosticsConfig::default()
    }
}

struct TempProject {
    path: std::path::PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("tua-lint-{name}-{}-{unique}", std::process::id()));

        std::fs::create_dir_all(&path).expect("temp project should be created");

        Self { path }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path.join(relative);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directory should be created");
        }

        std::fs::write(path, contents).expect("fixture should be written");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
