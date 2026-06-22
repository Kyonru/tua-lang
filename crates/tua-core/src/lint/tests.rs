use crate::{
    check_tua_source, check_tua_source_with_diagnostics_config, DiagnosticCode, DiagnosticSeverity,
    DiagnosticsConfig,
};

#[test]
fn configured_lints_are_disabled_by_default() {
    check_tua_source(
        "src/main.tua",
        r#"
local config = { hp = 10, hp = 20 }
local hp = 10
hp = hp
local Missing = require("definitely_missing_lint_module")
"#,
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
