use super::*;
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("tua-{name}-{}-{unique}", process::id()));

        fs::create_dir_all(&path).expect("failed to create temp dir");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn default_config_matches_spec() {
    assert_eq!(
        ProjectConfig::default(),
        ProjectConfig {
            source: PathBuf::from("src"),
            out: PathBuf::from("build"),
            target: "love11-luajit".to_owned(),
            extension: "tua".to_owned(),
            emit_maps: true,
            diagnostics: DiagnosticsConfig::default(),
        }
    );
}

#[test]
fn parses_present_config() {
    let config = parse_config(
        r#"
source = "game"
out = "dist"
target = "love11-luajit"
extension = "tl"
emit_maps = false

[diagnostics]
duplicate_keys = true
missing_requires = true
self_assignments = true
doc_annotations = true
undefined_variables = true
unused_locals = true
unused_parameters = true
use_before_definition = true
globals = ["Game", "World"]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config,
        ProjectConfig {
            source: PathBuf::from("game"),
            out: PathBuf::from("dist"),
            target: "love11-luajit".to_owned(),
            extension: "tl".to_owned(),
            emit_maps: false,
            diagnostics: DiagnosticsConfig {
                duplicate_keys: true,
                missing_requires: true,
                self_assignments: true,
                doc_annotations: true,
                undefined_variables: true,
                unused_locals: true,
                unused_parameters: true,
                use_before_definition: true,
                globals: vec!["Game".to_owned(), "World".to_owned()],
            },
        }
    );
}

#[test]
fn parses_partial_config_with_defaults() {
    let config = parse_config(
        r#"
source = "game"
emit_maps = false
"#,
    )
    .expect("config should parse");

    assert_eq!(config.source, PathBuf::from("game"));
    assert_eq!(config.out, PathBuf::from("build"));
    assert_eq!(config.target, "love11-luajit");
    assert_eq!(config.extension, "tua");
    assert!(!config.emit_maps);
    assert_eq!(config.diagnostics, DiagnosticsConfig::default());
}

#[test]
fn parses_diagnostics_section_with_partial_defaults() {
    let config = parse_config(
        r#"
[diagnostics]
duplicate_keys = true
"#,
    )
    .expect("config should parse");

    assert!(config.diagnostics.duplicate_keys);
    assert!(!config.diagnostics.missing_requires);
    assert!(!config.diagnostics.self_assignments);
    assert!(!config.diagnostics.doc_annotations);
    assert!(!config.diagnostics.undefined_variables);
    assert!(!config.diagnostics.unused_locals);
    assert!(!config.diagnostics.unused_parameters);
    assert!(!config.diagnostics.use_before_definition);
    assert!(config.diagnostics.globals.is_empty());
}

#[test]
fn load_missing_config_uses_defaults() {
    let temp = TempDir::new("missing-config");
    let loaded = load_config_from_dir(temp.path()).expect("missing config should load");

    assert!(!loaded.found);
    assert_eq!(loaded.path, temp.path().join(CONFIG_FILE));
    assert_eq!(loaded.config, ProjectConfig::default());
}

#[test]
fn load_present_config_from_directory() {
    let temp = TempDir::new("present-config");
    fs::write(temp.path().join(CONFIG_FILE), "source = \"game\"\n")
        .expect("failed to write config");

    let loaded = load_config_from_dir(temp.path()).expect("present config should load");

    assert!(loaded.found);
    assert_eq!(loaded.config.source, PathBuf::from("game"));
    assert_eq!(loaded.config.out, PathBuf::from("build"));
}

#[test]
fn rejects_unknown_config_keys() {
    let error = parse_config("mode = \"strict\"\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert!(error.message.contains("unknown config key"));
}

#[test]
fn rejects_invalid_boolean_values() {
    let error = parse_config("emit_maps = yes\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert!(error.message.contains("expected boolean"));
}

#[test]
fn rejects_unknown_diagnostics_keys() {
    let error = parse_config("[diagnostics]\nmissing_requirez = true\n").unwrap_err();

    assert_eq!(error.line, 2);
    assert!(error.message.contains("unknown diagnostics config key"));
}

#[test]
fn rejects_unknown_config_sections() {
    let error = parse_config("[lint]\nduplicate_keys = true\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert!(error.message.contains("unknown config section"));
}

#[test]
fn config_parse_errors_convert_to_diagnostics() {
    let path = PathBuf::from("tua.toml");
    let source = parse_config("emit_maps = yes\n").unwrap_err();
    let error = ConfigError::Parse { path, source };
    let diagnostic = error.to_diagnostic();

    assert_eq!(
        diagnostic.format_cli(),
        "tua.toml:1:1 error TL3001: expected boolean value"
    );
}

#[test]
fn strips_comments_outside_strings() {
    let config = parse_config(
        r##"
source = "game#client" # inline comment
# full-line comment
"##,
    )
    .expect("config should parse");

    assert_eq!(config.source, PathBuf::from("game#client"));
}

#[test]
fn init_creates_default_config_and_src_directory() {
    let temp = TempDir::new("init");
    let result = init_project(temp.path()).expect("init should succeed");

    assert!(result.config_created);
    assert!(result.source_created);
    assert_eq!(
        fs::read_to_string(temp.path().join(CONFIG_FILE)).expect("failed to read config"),
        DEFAULT_CONFIG_TEXT
    );
    assert!(temp.path().join("src").is_dir());
}

#[test]
fn init_does_not_overwrite_existing_config() {
    let temp = TempDir::new("init-existing");
    let config_path = temp.path().join(CONFIG_FILE);
    let existing = "source = \"custom\"\n";

    fs::write(&config_path, existing).expect("failed to write existing config");

    let result = init_project(temp.path()).expect("init should succeed");

    assert!(!result.config_created);
    assert_eq!(
        fs::read_to_string(config_path).expect("failed to read config"),
        existing
    );
    assert!(temp.path().join("src").is_dir());
}
