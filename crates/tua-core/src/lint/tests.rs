use crate::{
    check_tua_source, check_tua_source_with_diagnostics_config, DiagnosticCode, DiagnosticSeverity,
    DiagnosticsConfig,
};

#[test]
fn configured_lints_are_disabled_by_default() {
    check_tua_source(
        "src/main.tua",
        concat!(
            r#"
local config = { hp = 10, hp = 20 }
local hp = 10
hp = hp
local Missing = require("definitely_missing_lint_module")
local cramped={1,2,3}"#,
            "   \n",
        ),
    )
    .expect("lint-only issues should be disabled by default");
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
        },
    )
    .expect_err("bad require lint should report diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::LINT_BAD_REQUIRE));
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
fn reports_formatting_lints_when_enabled() {
    let diagnostics = check_tua_source_with_diagnostics_config(
        "src/main.tua",
        "local hp=10   \n\tlocal values = {1,2,3}\nif hp > 0  then  print( hp ) end\n",
        &DiagnosticsConfig {
            formatting_lints: true,
            ..DiagnosticsConfig::default()
        },
    )
    .expect_err("formatting lints should report diagnostics");

    assert_eq!(
        count_code(
            &diagnostics,
            DiagnosticCode::LINT_FORMAT_TRAILING_WHITESPACE
        ),
        1
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FORMAT_INDENTATION),
        1
    );
    assert_eq!(
        count_code(&diagnostics, DiagnosticCode::LINT_FORMAT_KEYWORD_SPACING),
        2
    );
    assert!(
        count_code(&diagnostics, DiagnosticCode::LINT_FORMAT_TOKEN_SPACING) >= 5,
        "{diagnostics:?}"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning));
}

#[test]
fn formatting_lints_accept_baseline_style() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local values = { 1, 2, 3 }

if values[1] > 0 then
  print(values[1])
end
"#,
        &DiagnosticsConfig {
            formatting_lints: true,
            ..DiagnosticsConfig::default()
        },
    )
    .expect("baseline style should not report formatting diagnostics");
}

#[test]
fn formatting_lints_ignore_strings_and_comments() {
    check_tua_source_with_diagnostics_config(
        "src/main.tua",
        r#"
local text = "a,b"
local block = [[
  this , keeps spacing
]]
-- if value  then  print( value )
"#,
        &DiagnosticsConfig {
            formatting_lints: true,
            ..DiagnosticsConfig::default()
        },
    )
    .expect("strings and comments should not report formatting diagnostics");
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
        },
    )
    .expect_err("unused symbol lints should report diagnostics");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_LOCAL)
            && diagnostic.message.contains("unused local `unused`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::LINT_UNUSED_PARAMETER)
            && diagnostic
                .message
                .contains("unused parameter `unused_param`")
    }));
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
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
            ..DiagnosticsConfig::default()
        },
    )
    .expect("known globals and dynamic module fields should not warn");
}

fn count_code(diagnostics: &[crate::Diagnostic], code: DiagnosticCode) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(code))
        .count()
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
