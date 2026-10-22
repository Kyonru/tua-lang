use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::formatter::{FormatOptions, QuoteStyle};
use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

mod parser;

pub use parser::{
    closest_config_child_name, config_section_names, config_setting_metadata,
    config_settings_in_section, config_settings_metadata, parse_config, ConfigSettingMetadata,
};

pub const CONFIG_FILE: &str = "tua.toml";

pub const DEFAULT_CONFIG_TEXT: &str = "\
source = \"src\"
out = \"build\"
main = \"main.tua\"
target = \"love11-luajit\"
extension = \"tua\"
emit_maps = true
type_libraries = []
asset_pipelines = []

[emit]
annotation_mode = \"off\"

[format]
indent_width = 2
line_width = 100
quote_style = \"preserve\"
final_newline = true

[jit]
enabled = false
variable_type_drift = true
table_shape_drift = true
hot_loop_hints = true

[ecs]
enabled = true
backend = \"tua\"

[diagnostics]
rules = true
duplicate_keys = false
missing_requires = true
self_assignments = false
doc_annotations = false
undefined_variables = false
unused_locals = false
unused_parameters = false
use_before_definition = false
duplicate_parameters = false
bad_requires = false
impossible_boolean_expressions = false
duplicate_if_conditions = false
duplicate_binary_operands = false
float_equality = false
const_assignments = false
assignment_arity = false
local_declaration_arity = false
unused_assignments = false
binary_operator_type_mismatches = false
uncalled_local_functions = false
max_cyclomatic_complexity = 0
divide_by_zero = false
almost_swapped = false
mixed_tables = false
unknown_record_fields = false
inline_ignores = true
inline_ignore_scopes = [\"tua-ignore-line\", \"tua-ignore-next-line\", \"tua-ignore-file\"]
inline_ignore_max_per_file = 0
globals = []
severity = {}
";

pub const DEFAULT_MAIN_TEXT: &str = "\
local message: string = \"hello from tua\"
print(message)
";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub source: PathBuf,
    pub out: PathBuf,
    pub main: PathBuf,
    pub target: String,
    pub extension: String,
    pub emit_maps: bool,
    pub type_libraries: Vec<TypeLibraryConfig>,
    pub asset_pipelines: Vec<AssetPipelineConfig>,
    pub emit: EmitConfig,
    pub format: FormatConfig,
    pub jit: JitConfig,
    pub ecs: EcsConfig,
    pub diagnostics: DiagnosticsConfig,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            source: PathBuf::from("src"),
            out: PathBuf::from("build"),
            main: PathBuf::from("main.tua"),
            target: "love11-luajit".to_owned(),
            extension: "tua".to_owned(),
            emit_maps: true,
            type_libraries: Vec::new(),
            asset_pipelines: Vec::new(),
            emit: EmitConfig::default(),
            format: FormatConfig::default(),
            jit: JitConfig::default(),
            ecs: EcsConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TypeLibraryConfig {
    pub path: PathBuf,
    pub declaration_prefix: String,
    pub runtime_prefix: String,
}

/// One opt-in external asset importer. Paths are relative to the project root.
/// The importer writes a versioned manifest and generated files into the staging
/// paths supplied through `TUA_ASSET_PIPELINE_*` environment variables.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssetPipelineConfig {
    pub name: String,
    pub inputs: Vec<String>,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub builtin: Option<String>,
    pub output: PathBuf,
    #[serde(default)]
    pub version: String,
    #[serde(default = "default_asset_pipeline_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_asset_pipeline_max_manifest_bytes")]
    pub max_manifest_bytes: u64,
    #[serde(default = "default_asset_pipeline_max_stderr_bytes")]
    pub max_stderr_bytes: u64,
    #[serde(default)]
    pub options: BTreeMap<String, serde_json::Value>,
}

const fn default_asset_pipeline_timeout_ms() -> u64 {
    60_000
}

const fn default_asset_pipeline_max_manifest_bytes() -> u64 {
    1_048_576
}

const fn default_asset_pipeline_max_stderr_bytes() -> u64 {
    65_536
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EmitConfig {
    pub annotation_mode: AnnotationMode,
}

impl Default for EmitConfig {
    fn default() -> Self {
        Self {
            annotation_mode: AnnotationMode::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationMode {
    Off,
    Luacats,
}

impl AnnotationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Luacats => "luacats",
        }
    }
}

impl<'de> Deserialize<'de> for AnnotationMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "off" => Ok(Self::Off),
            "luacats" => Ok(Self::Luacats),
            _ => Err(D::Error::custom(
                "expected annotation mode \"off\" or \"luacats\"",
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FormatConfig {
    pub indent_width: usize,
    pub line_width: usize,
    #[serde(deserialize_with = "parser::deserialize_quote_style")]
    pub quote_style: QuoteStyle,
    pub final_newline: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_width: 2,
            line_width: 100,
            quote_style: QuoteStyle::Preserve,
            final_newline: true,
        }
    }
}

impl FormatConfig {
    pub fn options(&self) -> FormatOptions {
        FormatOptions {
            indent_width: self.indent_width,
            line_width: self.line_width,
            quote_style: self.quote_style,
            final_newline: self.final_newline,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct JitConfig {
    pub enabled: bool,
    pub variable_type_drift: bool,
    pub table_shape_drift: bool,
    pub hot_loop_hints: bool,
}

impl Default for JitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            variable_type_drift: true,
            table_shape_drift: true,
            hot_loop_hints: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EcsConfig {
    pub enabled: bool,
    pub backend: String,
}

impl Default for EcsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "tua".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticsConfig {
    pub rules: bool,
    pub duplicate_keys: bool,
    pub missing_requires: bool,
    pub self_assignments: bool,
    pub doc_annotations: bool,
    pub undefined_variables: bool,
    pub unused_locals: bool,
    pub unused_parameters: bool,
    pub use_before_definition: bool,
    pub duplicate_parameters: bool,
    pub bad_requires: bool,
    pub impossible_boolean_expressions: bool,
    pub duplicate_if_conditions: bool,
    pub duplicate_binary_operands: bool,
    pub float_equality: bool,
    pub const_assignments: bool,
    pub assignment_arity: bool,
    pub local_declaration_arity: bool,
    pub unused_assignments: bool,
    pub binary_operator_type_mismatches: bool,
    pub uncalled_local_functions: bool,
    pub max_cyclomatic_complexity: usize,
    pub divide_by_zero: bool,
    pub almost_swapped: bool,
    pub mixed_tables: bool,
    pub unknown_record_fields: bool,
    pub inline_ignores: bool,
    pub inline_ignore_scopes: Vec<String>,
    pub inline_ignore_max_per_file: usize,
    pub globals: Vec<String>,
    #[serde(deserialize_with = "deserialize_diagnostic_severity")]
    pub severity: BTreeMap<DiagnosticCode, DiagnosticLevel>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Allow,
    Hint,
    Info,
    Warning,
    Error,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            rules: true,
            duplicate_keys: false,
            missing_requires: true,
            self_assignments: false,
            doc_annotations: false,
            undefined_variables: false,
            unused_locals: false,
            unused_parameters: false,
            use_before_definition: false,
            duplicate_parameters: false,
            bad_requires: false,
            impossible_boolean_expressions: false,
            duplicate_if_conditions: false,
            duplicate_binary_operands: false,
            float_equality: false,
            const_assignments: false,
            assignment_arity: false,
            local_declaration_arity: false,
            unused_assignments: false,
            binary_operator_type_mismatches: false,
            uncalled_local_functions: false,
            max_cyclomatic_complexity: 0,
            divide_by_zero: false,
            almost_swapped: false,
            mixed_tables: false,
            unknown_record_fields: false,
            inline_ignores: true,
            inline_ignore_scopes: vec![
                "tua-ignore-line".to_owned(),
                "tua-ignore-next-line".to_owned(),
                "tua-ignore-file".to_owned(),
            ],
            inline_ignore_max_per_file: 0,
            globals: Vec::new(),
            severity: BTreeMap::new(),
        }
    }
}

impl DiagnosticsConfig {
    pub fn enable_all_lints(&mut self) {
        self.rules = true;
        self.duplicate_keys = true;
        self.missing_requires = true;
        self.self_assignments = true;
        self.doc_annotations = true;
        self.undefined_variables = true;
        self.unused_locals = true;
        self.unused_parameters = true;
        self.use_before_definition = true;
        self.duplicate_parameters = true;
        self.bad_requires = true;
        self.impossible_boolean_expressions = true;
        self.duplicate_if_conditions = true;
        self.duplicate_binary_operands = true;
        self.float_equality = true;
        self.const_assignments = true;
        self.assignment_arity = true;
        self.local_declaration_arity = true;
        self.unused_assignments = true;
        self.binary_operator_type_mismatches = true;
    }

    pub fn apply_severity_policy(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        diagnostics
            .into_iter()
            .filter_map(|mut diagnostic| {
                let level = diagnostic.code.and_then(|code| self.severity.get(&code));

                match level {
                    Some(DiagnosticLevel::Allow) => None,
                    Some(DiagnosticLevel::Hint) => {
                        diagnostic.severity = DiagnosticSeverity::Hint;
                        Some(diagnostic)
                    }
                    Some(DiagnosticLevel::Info) => {
                        diagnostic.severity = DiagnosticSeverity::Info;
                        Some(diagnostic)
                    }
                    Some(DiagnosticLevel::Warning) => {
                        diagnostic.severity = DiagnosticSeverity::Warning;
                        Some(diagnostic)
                    }
                    Some(DiagnosticLevel::Error) => {
                        diagnostic.severity = DiagnosticSeverity::Error;
                        Some(diagnostic)
                    }
                    None => Some(diagnostic),
                }
            })
            .collect()
    }
}

fn deserialize_diagnostic_severity<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<DiagnosticCode, DiagnosticLevel>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = BTreeMap::<String, DiagnosticLevel>::deserialize(deserializer)?;
    let mut severity = BTreeMap::new();

    for (raw, level) in entries {
        let Some(number) = raw
            .strip_prefix("TL")
            .and_then(|value| value.parse::<u16>().ok())
        else {
            return Err(D::Error::custom(format!(
                "diagnostic severity key `{raw}` must be an exact code such as `TL4035`"
            )));
        };
        let Some(code) = DiagnosticCode::new(number).filter(|code| code.is_known()) else {
            return Err(D::Error::custom(format!(
                "unknown diagnostic severity code `{raw}`"
            )));
        };

        severity.insert(code, level);
    }

    Ok(severity)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: ProjectConfig,
    pub found: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitResult {
    pub config_path: PathBuf,
    pub source_dir: PathBuf,
    pub main_path: PathBuf,
    pub config_created: bool,
    pub source_created: bool,
    pub main_created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigParseError {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub message: String,
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl Error for ConfigParseError {}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: ConfigParseError,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "{}: {}", path.display(), source)
            }
            Self::Parse { path, source } => {
                write!(f, "{}: {}", path.display(), source)
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

impl ConfigError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Self::Io { path, source } => Diagnostic::error(path, 1, 1, source.to_string())
                .with_code(DiagnosticCode::CONFIG_IO),
            Self::Parse { path, source } => {
                Diagnostic::error(path, source.line, source.column, source.message.clone())
                    .with_range(
                        source.line,
                        source.column,
                        source.end_line,
                        source.end_column,
                    )
                    .with_code(DiagnosticCode::CONFIG_PARSE)
            }
        }
    }
}

pub fn load_config_from_dir(dir: impl AsRef<Path>) -> Result<LoadedConfig, ConfigError> {
    let path = dir.as_ref().join(CONFIG_FILE);

    match fs::read_to_string(&path) {
        Ok(contents) => {
            let config = parse_config(&contents).map_err(|source| ConfigError::Parse {
                path: path.clone(),
                source,
            })?;

            Ok(LoadedConfig {
                path,
                config,
                found: true,
            })
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(LoadedConfig {
            path,
            config: ProjectConfig::default(),
            found: false,
        }),
        Err(source) => Err(ConfigError::Io { path, source }),
    }
}

pub fn init_project(dir: impl AsRef<Path>) -> Result<InitResult, ConfigError> {
    let dir = dir.as_ref();

    fs::create_dir_all(dir).map_err(|source| ConfigError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let config_path = dir.join(CONFIG_FILE);
    let config_created = write_file_without_overwrite(&config_path, DEFAULT_CONFIG_TEXT)?;
    let defaults = ProjectConfig::default();
    let source_dir = dir.join(&defaults.source);
    let source_created = !source_dir.exists();

    fs::create_dir_all(&source_dir).map_err(|source| ConfigError::Io {
        path: source_dir.clone(),
        source,
    })?;
    let main_path = source_dir.join(&defaults.main);
    let main_created = write_file_without_overwrite(&main_path, DEFAULT_MAIN_TEXT)?;

    Ok(InitResult {
        config_path,
        source_dir,
        main_path,
        config_created,
        source_created,
        main_created,
    })
}

fn write_file_without_overwrite(path: &Path, contents: &str) -> Result<bool, ConfigError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(contents.as_bytes())
                .map_err(|source| ConfigError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;

            Ok(true)
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(ConfigError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests;
