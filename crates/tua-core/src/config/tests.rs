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
            emit: EmitConfig {
                annotation_mode: AnnotationMode::Off,
            },
            format: FormatConfig::default(),
            jit: JitConfig::default(),
            ecs: EcsConfig::default(),
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
[emit]
annotation_mode = "luacats"

[format]
indent_width = 4
line_width = 80
quote_style = "preserve"
final_newline = false

[jit]
enabled = true
variable_type_drift = false
table_shape_drift = true
hot_loop_hints = false

[ecs]
enabled = true
backend = "tiny-ecs"

[diagnostics]
duplicate_keys = true
missing_requires = true
self_assignments = true
doc_annotations = true
undefined_variables = true
unused_locals = true
unused_parameters = true
use_before_definition = true
duplicate_parameters = true
wrong_argument_count = true
bad_requires = true
impossible_boolean_expressions = true
duplicate_if_conditions = true
duplicate_binary_operands = true
float_equality = true
const_assignments = true
parameter_type_mismatches = true
return_type_mismatches = true
assignment_arity = true
local_declaration_arity = true
unused_assignments = true
binary_operator_type_mismatches = true
uncalled_local_functions = true
formatting_lints = true
inline_ignores = true
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line", "tua-ignore-file"]
inline_ignore_max_per_file = 4
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
            emit: EmitConfig {
                annotation_mode: AnnotationMode::Luacats,
            },
            format: FormatConfig {
                indent_width: 4,
                line_width: 80,
                quote_style: QuoteStyle::Preserve,
                final_newline: false,
            },
            jit: JitConfig {
                enabled: true,
                variable_type_drift: false,
                table_shape_drift: true,
                hot_loop_hints: false,
            },
            ecs: EcsConfig {
                enabled: true,
                backend: "tiny-ecs".to_owned(),
            },
            diagnostics: DiagnosticsConfig {
                duplicate_keys: true,
                missing_requires: true,
                self_assignments: true,
                doc_annotations: true,
                undefined_variables: true,
                unused_locals: true,
                unused_parameters: true,
                use_before_definition: true,
                duplicate_parameters: true,
                wrong_argument_count: true,
                bad_requires: true,
                impossible_boolean_expressions: true,
                duplicate_if_conditions: true,
                duplicate_binary_operands: true,
                float_equality: true,
                const_assignments: true,
                parameter_type_mismatches: true,
                return_type_mismatches: true,
                assignment_arity: true,
                local_declaration_arity: true,
                unused_assignments: true,
                binary_operator_type_mismatches: true,
                uncalled_local_functions: true,
                formatting_lints: true,
                inline_ignores: true,
                inline_ignore_scopes: vec![
                    "tua-ignore-line".to_owned(),
                    "tua-ignore-next-line".to_owned(),
                    "tua-ignore-file".to_owned(),
                ],
                inline_ignore_max_per_file: 4,
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
    assert_eq!(
        config.emit,
        EmitConfig {
            annotation_mode: AnnotationMode::Off,
        }
    );
    assert_eq!(config.jit, JitConfig::default());
    assert_eq!(config.format, FormatConfig::default());
    assert_eq!(config.diagnostics, DiagnosticsConfig::default());
}

#[test]
fn parses_format_section_with_partial_defaults() {
    let config = parse_config(
        r#"
[format]
line_width = 120
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.format,
        FormatConfig {
            line_width: 120,
            ..FormatConfig::default()
        }
    );
    assert_eq!(
        config.format.options(),
        FormatOptions {
            line_width: 120,
            ..FormatOptions::default()
        }
    );
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
    assert!(!config.diagnostics.duplicate_parameters);
    assert!(!config.diagnostics.wrong_argument_count);
    assert!(!config.diagnostics.bad_requires);
    assert!(!config.diagnostics.impossible_boolean_expressions);
    assert!(!config.diagnostics.duplicate_if_conditions);
    assert!(!config.diagnostics.duplicate_binary_operands);
    assert!(!config.diagnostics.float_equality);
    assert!(!config.diagnostics.const_assignments);
    assert!(!config.diagnostics.parameter_type_mismatches);
    assert!(!config.diagnostics.return_type_mismatches);
    assert!(!config.diagnostics.assignment_arity);
    assert!(!config.diagnostics.local_declaration_arity);
    assert!(!config.diagnostics.unused_assignments);
    assert!(!config.diagnostics.binary_operator_type_mismatches);
    assert!(!config.diagnostics.uncalled_local_functions);
    assert!(config.diagnostics.formatting_lints);
    assert!(!config.diagnostics.inline_ignores);
    assert_eq!(
        config.diagnostics.inline_ignore_scopes,
        vec![
            "tua-ignore-line".to_owned(),
            "tua-ignore-next-line".to_owned()
        ]
    );
    assert_eq!(config.diagnostics.inline_ignore_max_per_file, 0);
    assert!(config.diagnostics.globals.is_empty());
}

#[test]
fn parses_jit_section_with_partial_defaults() {
    let config = parse_config(
        r#"
[jit]
enabled = true
table_shape_drift = false
"#,
    )
    .expect("config should parse");

    assert!(config.jit.enabled);
    assert!(config.jit.variable_type_drift);
    assert!(!config.jit.table_shape_drift);
    assert!(config.jit.hot_loop_hints);
}

#[test]
fn parses_ecs_section_with_partial_defaults() {
    let config = parse_config(
        r#"
[ecs]
enabled = true
"#,
    )
    .expect("config should parse");

    assert!(config.ecs.enabled);
    assert_eq!(config.ecs.backend, "tiny-ecs");
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
fn rejects_invalid_annotation_mode() {
    let error = parse_config("[emit]\nannotation_mode = \"strict\"\n").unwrap_err();

    assert_eq!(error.line, 2);
    assert!(error.message.contains("expected annotation mode"));
}

#[test]
fn parses_emit_section_with_defaults() {
    let config = parse_config(
        r#"
[emit]
annotation_mode = "luacats"
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.emit,
        EmitConfig {
            annotation_mode: AnnotationMode::Luacats,
        }
    );
    assert_eq!(config.source, PathBuf::from("src"));
    assert_eq!(config.emit.annotation_mode, AnnotationMode::Luacats);
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
