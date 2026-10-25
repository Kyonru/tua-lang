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
            main: PathBuf::from("main.tua"),
            target: "love11-luajit".to_owned(),
            extension: "tua".to_owned(),
            emit_maps: true,
            type_libraries: Vec::new(),
            asset_pipelines: Vec::new(),
            emit: EmitConfig {
                annotation_mode: AnnotationMode::Off,
            },
            format: FormatConfig::default(),
            jit: JitConfig::default(),
            ecs: EcsConfig::default(),
            shaders: ShadersConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
        }
    );

    assert_eq!(
        parse_config(DEFAULT_CONFIG_TEXT).expect("default config text should parse"),
        ProjectConfig::default()
    );
}

#[test]
fn parses_all_formatter_quote_styles() {
    for (value, expected) in [
        ("preserve", QuoteStyle::Preserve),
        ("double", QuoteStyle::Double),
        ("single", QuoteStyle::Single),
    ] {
        let config = parse_config(&format!("[format]\nquote_style = {value:?}\n"))
            .expect("quote style should parse");
        assert_eq!(config.format.quote_style, expected);
    }
}

#[test]
fn parses_formatter_indent_and_line_ending_styles() {
    let config = parse_config("[format]\nindent_style = \"tabs\"\nline_ending = \"preserve\"\n")
        .expect("formatter styles should parse");

    assert_eq!(config.format.indent_style, IndentStyle::Tabs);
    assert_eq!(config.format.line_ending, LineEnding::Preserve);
}

#[test]
fn rejects_zero_formatter_widths() {
    for setting in ["indent_width", "line_width"] {
        let error = parse_config(&format!("[format]\n{setting} = 0\n"))
            .expect_err("zero formatter widths should be rejected");
        assert!(
            error.message.contains("greater than or equal to 1"),
            "{}",
            error.message
        );
    }
}

#[test]
fn parses_declaration_only_type_libraries() {
    let config = parse_config(
        r#"
[[type_libraries]]
path = "lib/feel/types"
declaration_prefix = "feel"
runtime_prefix = "lib.feel"
"#,
    )
    .expect("type library config should parse");

    assert_eq!(
        config.type_libraries,
        vec![TypeLibraryConfig {
            path: PathBuf::from("lib/feel/types"),
            declaration_prefix: "feel".to_owned(),
            runtime_prefix: "lib.feel".to_owned(),
        }]
    );
}

#[test]
fn parses_asset_pipelines() {
    let config = parse_config(
        r#"
[[asset_pipelines]]
name = "world-art"
inputs = ["maps/**/*.json", "tools/import.py"]
command = ["python3", "tools/import.py"]
output = "src/generated/world-art"
version = "3"
"#,
    )
    .expect("asset pipeline config should parse");

    assert_eq!(
        config.asset_pipelines,
        vec![AssetPipelineConfig {
            name: "world-art".to_owned(),
            inputs: vec!["maps/**/*.json".to_owned(), "tools/import.py".to_owned(),],
            command: vec!["python3".to_owned(), "tools/import.py".to_owned()],
            builtin: None,
            output: PathBuf::from("src/generated/world-art"),
            version: "3".to_owned(),
            timeout_ms: 60_000,
            max_manifest_bytes: 1_048_576,
            max_stderr_bytes: 65_536,
            options: std::collections::BTreeMap::new(),
        }]
    );
}

#[test]
fn parses_asset_pipeline_resource_limits() {
    let config = parse_config(
        r#"
[[asset_pipelines]]
name = "world-art"
inputs = ["maps/world.json"]
command = ["tools/import"]
output = "src/generated/world-art"
timeout_ms = 2500
max_manifest_bytes = 4096
max_stderr_bytes = 2048
"#,
    )
    .expect("asset pipeline resource limits should parse");

    let pipeline = &config.asset_pipelines[0];
    assert_eq!(pipeline.timeout_ms, 2500);
    assert_eq!(pipeline.max_manifest_bytes, 4096);
    assert_eq!(pipeline.max_stderr_bytes, 2048);
}

#[test]
fn parses_builtin_audio_analysis_pipeline_options() {
    let config = parse_config(
        r#"
[[asset_pipelines]]
name = "audio"
builtin = "audio-analysis"
inputs = ["assets/**/*.ogg"]
output = "src/generated/audio"

[asset_pipelines.options]
envelope_hz = 20
tempo_min = 60
"#,
    )
    .expect("built-in audio analysis config should parse");
    let pipeline = &config.asset_pipelines[0];
    assert_eq!(pipeline.builtin.as_deref(), Some("audio-analysis"));
    assert!(pipeline.command.is_empty());
    assert_eq!(pipeline.options["envelope_hz"].as_i64(), Some(20));
}

#[test]
fn parses_present_config() {
    let config = parse_config(
        r#"
source = "game"
out = "dist"
main = "love/boot.tua"
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
rules = true
duplicate_keys = true
missing_requires = true
self_assignments = true
doc_annotations = true
undefined_variables = true
unused_locals = true
unused_parameters = true
use_before_definition = true
duplicate_parameters = true
bad_requires = true
impossible_boolean_expressions = true
duplicate_if_conditions = true
duplicate_binary_operands = true
float_equality = true
const_assignments = true
assignment_arity = true
local_declaration_arity = true
unused_assignments = true
binary_operator_type_mismatches = true
uncalled_local_functions = true
max_cyclomatic_complexity = 12
divide_by_zero = true
almost_swapped = true
mixed_tables = true
unknown_record_fields = true
inline_ignores = true
inline_ignore_scopes = ["tua-ignore-line", "tua-ignore-next-line", "tua-ignore-file"]
inline_ignore_max_per_file = 4
globals = ["Game", "World"]
severity = { TL1003 = "warning", TL4035 = "allow" }
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config,
        ProjectConfig {
            source: PathBuf::from("game"),
            out: PathBuf::from("dist"),
            main: PathBuf::from("love/boot.tua"),
            target: "love11-luajit".to_owned(),
            extension: "tl".to_owned(),
            emit_maps: false,
            type_libraries: Vec::new(),
            asset_pipelines: Vec::new(),
            emit: EmitConfig {
                annotation_mode: AnnotationMode::Luacats,
            },
            format: FormatConfig {
                indent_style: IndentStyle::Spaces,
                indent_width: 4,
                line_width: 80,
                quote_style: QuoteStyle::Preserve,
                line_ending: LineEnding::Lf,
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
            shaders: ShadersConfig::default(),
            diagnostics: DiagnosticsConfig {
                rules: true,
                duplicate_keys: true,
                missing_requires: true,
                self_assignments: true,
                doc_annotations: true,
                undefined_variables: true,
                unused_locals: true,
                unused_parameters: true,
                use_before_definition: true,
                duplicate_parameters: true,
                bad_requires: true,
                impossible_boolean_expressions: true,
                duplicate_if_conditions: true,
                duplicate_binary_operands: true,
                float_equality: true,
                const_assignments: true,
                assignment_arity: true,
                local_declaration_arity: true,
                unused_assignments: true,
                binary_operator_type_mismatches: true,
                uncalled_local_functions: true,
                max_cyclomatic_complexity: 12,
                divide_by_zero: true,
                almost_swapped: true,
                mixed_tables: true,
                unknown_record_fields: true,
                inline_ignores: true,
                inline_ignore_scopes: vec![
                    "tua-ignore-line".to_owned(),
                    "tua-ignore-next-line".to_owned(),
                    "tua-ignore-file".to_owned(),
                ],
                inline_ignore_max_per_file: 4,
                globals: vec!["Game".to_owned(), "World".to_owned()],
                severity: BTreeMap::from([
                    (DiagnosticCode::TYPE_MISMATCH, DiagnosticLevel::Warning),
                    (DiagnosticCode::LINT_LOCAL_SHADOW, DiagnosticLevel::Allow),
                ]),
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
    assert_eq!(config.main, PathBuf::from("main.tua"));
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
    assert!(config.diagnostics.missing_requires);
    assert!(!config.diagnostics.self_assignments);
    assert!(!config.diagnostics.doc_annotations);
    assert!(!config.diagnostics.undefined_variables);
    assert!(!config.diagnostics.unused_locals);
    assert!(config.diagnostics.rules);
    assert!(!config.diagnostics.unused_parameters);
    assert!(!config.diagnostics.use_before_definition);
    assert!(!config.diagnostics.duplicate_parameters);
    assert!(!config.diagnostics.bad_requires);
    assert!(!config.diagnostics.impossible_boolean_expressions);
    assert!(!config.diagnostics.duplicate_if_conditions);
    assert!(!config.diagnostics.duplicate_binary_operands);
    assert!(!config.diagnostics.float_equality);
    assert!(!config.diagnostics.const_assignments);
    assert!(!config.diagnostics.assignment_arity);
    assert!(!config.diagnostics.local_declaration_arity);
    assert!(!config.diagnostics.unused_assignments);
    assert!(!config.diagnostics.binary_operator_type_mismatches);
    assert!(!config.diagnostics.uncalled_local_functions);
    assert_eq!(config.diagnostics.max_cyclomatic_complexity, 0);
    assert!(!config.diagnostics.divide_by_zero);
    assert!(!config.diagnostics.almost_swapped);
    assert!(!config.diagnostics.mixed_tables);
    assert!(!config.diagnostics.unknown_record_fields);
    assert!(config.diagnostics.inline_ignores);
    assert_eq!(
        config.diagnostics.inline_ignore_scopes,
        vec![
            "tua-ignore-line".to_owned(),
            "tua-ignore-next-line".to_owned(),
            "tua-ignore-file".to_owned()
        ]
    );
    assert_eq!(config.diagnostics.inline_ignore_max_per_file, 0);
    assert!(config.diagnostics.globals.is_empty());
}

#[test]
fn strict_lints_leave_noisy_project_policy_rules_opt_in() {
    let mut diagnostics = DiagnosticsConfig::default();

    diagnostics.enable_all_lints();

    assert!(!diagnostics.uncalled_local_functions);
    assert_eq!(diagnostics.max_cyclomatic_complexity, 0);
    assert!(!diagnostics.divide_by_zero);
    assert!(!diagnostics.almost_swapped);
    assert!(!diagnostics.mixed_tables);
    assert!(!diagnostics.unknown_record_fields);
    assert!(diagnostics.self_assignments);
    assert!(diagnostics.undefined_variables);
}

#[test]
fn parses_exact_diagnostic_severity_policy() {
    let config = parse_config(
        r#"
[diagnostics.severity]
TL1003 = "warning"
TL4035 = "allow"
TL4045 = "error"
"#,
    )
    .expect("severity policy should parse");

    assert_eq!(
        config.diagnostics.severity,
        BTreeMap::from([
            (DiagnosticCode::TYPE_MISMATCH, DiagnosticLevel::Warning),
            (DiagnosticCode::LINT_LOCAL_SHADOW, DiagnosticLevel::Allow),
            (
                DiagnosticCode::LINT_UNINITIALIZED_LOCAL,
                DiagnosticLevel::Error,
            ),
        ])
    );
}

#[test]
fn rejects_unknown_diagnostic_severity_codes_and_levels() {
    let unknown_code = parse_config(
        r#"
[diagnostics.severity]
TL4999 = "warning"
"#,
    )
    .expect_err("unknown code should fail");
    assert!(unknown_code
        .message
        .contains("unknown diagnostic severity code"));

    let unknown_level = parse_config(
        r#"
[diagnostics.severity]
TL1003 = "fatal"
"#,
    )
    .expect_err("unknown level should fail");
    assert!(unknown_level.message.contains("unknown variant `fatal`"));
}

#[test]
fn removed_checker_lint_switches_have_actionable_migrations() {
    for (setting, replacement) in [
        ("wrong_argument_count", "TL1004"),
        ("parameter_type_mismatches", "TL1003"),
        ("return_type_mismatches", "TL1003"),
    ] {
        let source = format!("[diagnostics]\n{setting} = true\n");
        let error = parse_config(&source).expect_err("removed setting should fail explicitly");

        assert!(error.message.contains("was removed"), "{}", error.message);
        assert!(error.message.contains(replacement), "{}", error.message);
    }
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
    assert_eq!(config.ecs.backend, "tua");
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
    let error = parse_config("sorce = \"game\"\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert_eq!(error.column, 1);
    assert_eq!(error.end_line, 1);
    assert_eq!(error.end_column, 6);
    assert!(error.message.contains("unknown config key"));
    assert!(error.message.contains("did you mean `source`"));
}

#[test]
fn rejects_invalid_boolean_values() {
    let error = parse_config("emit_maps = yes\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert_eq!(error.column, 13);
    assert_eq!(error.end_line, 1);
    assert_eq!(error.end_column, 16);
    assert!(error.message.contains("must be quoted"));
}

#[test]
fn malformed_config_dependency_panics_become_parse_errors() {
    let input = String::from_utf8_lossy(&[
        61, 61, 91, 34, 10, 10, 123, 46, 0, 0, 0, 0, 0, 0, 255, 255, 61, 255, 61,
    ]);

    let error = parse_config(&input).expect_err("malformed config should be rejected");

    assert_eq!(error.line, 1);
    assert_eq!(error.column, 1);
    assert!(error.message.contains("malformed input"));
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
    assert_eq!(error.column, 1);
    assert_eq!(error.end_column, 17);
    assert!(error.message.contains("unknown diagnostics config key"));
    assert!(error.message.contains("did you mean `missing_requires`"));
}

#[test]
fn rejects_unknown_config_sections() {
    let error = parse_config("[formt]\nline_width = 80\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert_eq!(error.column, 2);
    assert_eq!(error.end_column, 7);
    assert!(error.message.contains("unknown config section"));
    assert!(error.message.contains("did you mean `[format]`"));
}

#[test]
fn config_parse_errors_convert_to_diagnostics() {
    let path = PathBuf::from("tua.toml");
    let source = parse_config("emit_maps = yes\n").unwrap_err();
    let error = ConfigError::Parse { path, source };
    let diagnostic = error.to_diagnostic();

    assert_eq!(
        diagnostic.format_cli(),
        "tua.toml:1:13 error TL3001: string values must be quoted, expected literal string"
    );
    assert_eq!(diagnostic.end_line, 1);
    assert_eq!(diagnostic.end_column, 16);
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
fn parses_standard_toml_strings_arrays_and_dotted_keys() {
    let config = parse_config(
        r#"
source = 'game'
format.line_width = 120
diagnostics.inline_ignore_scopes = [
  "tua-ignore-line",
  "scope,with,commas",
]
diagnostics.globals = ["Game\u0041"]
"#,
    )
    .expect("standards-compliant TOML should parse");

    assert_eq!(config.source, PathBuf::from("game"));
    assert_eq!(config.format.line_width, 120);
    assert_eq!(
        config.diagnostics.inline_ignore_scopes,
        vec!["tua-ignore-line", "scope,with,commas"]
    );
    assert_eq!(config.diagnostics.globals, vec!["GameA"]);
}

#[test]
fn validates_unknown_keys_inside_inline_tables() {
    let error = parse_config("format = { line_wdth = 80 }\n").unwrap_err();

    assert_eq!(error.line, 1);
    assert_eq!(error.column, 12);
    assert_eq!(error.end_column, 21);
    assert!(error.message.contains("unknown format config key"));
    assert!(error.message.contains("did you mean `line_width`"));
}

#[test]
fn init_creates_default_config_and_src_directory() {
    let temp = TempDir::new("init");
    let result = init_project(temp.path()).expect("init should succeed");

    assert!(result.config_created);
    assert!(result.source_created);
    assert!(result.main_created);
    assert!(result.gitignore_created);
    assert!(!result.gitignore_updated);
    assert_eq!(
        fs::read_to_string(temp.path().join(CONFIG_FILE)).expect("failed to read config"),
        DEFAULT_CONFIG_TEXT
    );
    assert!(temp.path().join("src").is_dir());
    assert_eq!(
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read main"),
        DEFAULT_MAIN_TEXT
    );
    assert_eq!(
        fs::read_to_string(temp.path().join(".gitignore")).expect("failed to read gitignore"),
        "# Tua generated files\n/build/\n/dist/\n/.tua/\n"
    );
}

#[test]
fn init_extends_an_existing_gitignore_without_duplication() {
    let temp = TempDir::new("init-gitignore");
    let gitignore_path = temp.path().join(".gitignore");
    fs::write(&gitignore_path, "node_modules/\nbuild/\n").expect("failed to write gitignore");

    let first = init_project(temp.path()).expect("init should succeed");
    let contents = fs::read_to_string(&gitignore_path).expect("failed to read gitignore");

    assert!(!first.gitignore_created);
    assert!(first.gitignore_updated);
    assert_eq!(
        contents,
        "node_modules/\nbuild/\n\n# Tua generated files\n/dist/\n/.tua/\n"
    );

    let second = init_project(temp.path()).expect("second init should succeed");
    assert!(!second.gitignore_created);
    assert!(!second.gitignore_updated);
    assert_eq!(
        fs::read_to_string(gitignore_path).expect("failed to read gitignore"),
        contents
    );
}

#[cfg(unix)]
#[test]
fn init_rejects_symlink_destinations_before_creating_files() {
    use std::os::unix::fs::symlink;
    for name in [".gitignore", "src", "tua.toml"] {
        let project = TempDir::new("init-link-project");
        let outside = TempDir::new("init-link-outside");
        let victim = outside.path().join("victim");
        fs::write(&victim, "keep me\n").unwrap();
        let target = if name == "src" {
            outside.path()
        } else {
            &victim
        };
        symlink(target, project.path().join(name)).unwrap();
        let error = init_project(project.path()).unwrap_err();
        assert!(error.to_string().contains("symbolic link"));
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me\n");
        assert!(!outside.path().join("main.tua").exists());
        assert!(!project.path().join("src/main.tua").exists());
        if name != "tua.toml" {
            assert!(!project.path().join("tua.toml").exists());
        }
    }
}

#[test]
fn init_preserves_existing_gitignore_line_endings() {
    let temp = TempDir::new("init-gitignore-crlf");
    let gitignore_path = temp.path().join(".gitignore");
    fs::write(&gitignore_path, "node_modules/\r\n").expect("failed to write gitignore");

    init_project(temp.path()).expect("init should succeed");

    assert_eq!(
        fs::read_to_string(gitignore_path).expect("failed to read gitignore"),
        "node_modules/\r\n\r\n# Tua generated files\r\n/build/\r\n/dist/\r\n/.tua/\r\n"
    );
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
    assert_eq!(
        fs::read_to_string(temp.path().join("src/main.tua")).expect("failed to read main"),
        DEFAULT_MAIN_TEXT
    );
}

#[test]
fn init_does_not_overwrite_existing_main_file() {
    let temp = TempDir::new("init-existing-main");
    let source_dir = temp.path().join("src");
    let main_path = source_dir.join("main.tua");
    let existing = "print(\"custom\")\n";

    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    fs::write(&main_path, existing).expect("failed to write existing main");

    let result = init_project(temp.path()).expect("init should succeed");

    assert!(!result.source_created);
    assert!(!result.main_created);
    assert_eq!(
        fs::read_to_string(main_path).expect("failed to read main"),
        existing
    );
}

#[test]
fn parses_shader_validation_settings() {
    let config = parse_config("[shaders]\nvalidate = \"off\"\ntargets = [\"desktop\", \"gles\"]\n")
        .expect("shader settings should parse");

    assert_eq!(config.shaders.validate, ShaderValidationMode::Off);
    assert_eq!(
        config.shaders.targets,
        [
            crate::shader_validation::ShaderTarget::Desktop,
            crate::shader_validation::ShaderTarget::Gles
        ]
    );
    assert_eq!(
        parse_config("").expect("defaults").shaders,
        ShadersConfig::default()
    );

    for invalid in [
        "[shaders]\nvalidate = \"always\"\n",
        "[shaders]\ntargets = [\"metal\"]\n",
        "[shaders]\nvalidator = \"/usr/bin/evil\"\n",
    ] {
        assert!(
            parse_config(invalid).is_err(),
            "{invalid} should be rejected"
        );
    }
}
