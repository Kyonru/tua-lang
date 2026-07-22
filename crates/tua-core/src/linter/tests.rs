use crate::{
    check::check_parsed_source_with_asset_roots_diagnostics_jit_and_ecs_config, check_tua_source,
    check_tua_source_with_diagnostics_config, parse_tua_source, DiagnosticCode, DiagnosticSeverity,
    DiagnosticsConfig, EcsConfig, JitConfig,
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

    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_DUPLICATE_TABLE_KEY),
        2,
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
fn reports_duplicate_function_parameters_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function move(player, player)
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
        Some(DiagnosticCode::LINT_DUPLICATE_PARAMETER)
    );
    assert!(diagnostics[0]
        .message
        .contains("duplicate function parameter `player`"));
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
    assert!(diagnostics[0]
        .message
        .contains("duplicate if condition; first seen at"));
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
fn checker_backed_function_lints_do_not_duplicate_type_diagnostics() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local function damage(amount: number): string
  return amount
end

damage()
damage("bad")
"#,
        &DiagnosticsConfig {
            wrong_argument_count: true,
            parameter_type_mismatches: true,
            return_type_mismatches: true,
            ..diagnostics_without_standard_rules()
        },
    )
    .expect_err("checker-backed function diagnostics should report");

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::TYPE_ARGUMENT_COUNT),
        1
    );
    assert_eq!(count_code(&diagnostics, DiagnosticCode::TYPE_MISMATCH), 2);
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_WRONG_ARGUMENT_COUNT),
        0
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_PARAMETER_TYPE_MISMATCH),
        0
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_RETURN_TYPE_MISMATCH),
        0
    );
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
    assert!(diagnostics[0]
        .message
        .contains("undefined variable `missing`"));
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
