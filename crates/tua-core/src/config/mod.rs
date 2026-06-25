use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{Diagnostic, DiagnosticCode};

pub const CONFIG_FILE: &str = "tua.toml";

pub const DEFAULT_CONFIG_TEXT: &str = "\
source = \"src\"
out = \"build\"
target = \"love11-luajit\"
extension = \"tua\"
emit_maps = true

[emit]
annotation_mode = \"off\"

[diagnostics]
duplicate_keys = false
missing_requires = false
self_assignments = false
doc_annotations = false
undefined_variables = false
unused_locals = false
unused_parameters = false
use_before_definition = false
duplicate_parameters = false
wrong_argument_count = false
bad_requires = false
impossible_boolean_expressions = false
duplicate_if_conditions = false
duplicate_binary_operands = false
float_equality = false
const_assignments = false
parameter_type_mismatches = false
return_type_mismatches = false
formatting_lints = false
inline_ignores = false
inline_ignore_scopes = [\"tua-ignore-line\", \"tua-ignore-next-line\"]
inline_ignore_max_per_file = 0
globals = []
";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConfig {
    pub source: PathBuf,
    pub out: PathBuf,
    pub target: String,
    pub extension: String,
    pub emit_maps: bool,
    pub emit: EmitConfig,
    pub diagnostics: DiagnosticsConfig,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            source: PathBuf::from("src"),
            out: PathBuf::from("build"),
            target: "love11-luajit".to_owned(),
            extension: "tua".to_owned(),
            emit_maps: true,
            emit: EmitConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    fn parse(line: usize, value: &str) -> Result<Self, ConfigParseError> {
        match parse_string(line, value)?.as_str() {
            "off" => Ok(Self::Off),
            "luacats" => Ok(Self::Luacats),
            _ => Err(ConfigParseError::new(
                line,
                "expected annotation mode \"off\" or \"luacats\"",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Luacats => "luacats",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsConfig {
    pub duplicate_keys: bool,
    pub missing_requires: bool,
    pub self_assignments: bool,
    pub doc_annotations: bool,
    pub undefined_variables: bool,
    pub unused_locals: bool,
    pub unused_parameters: bool,
    pub use_before_definition: bool,
    pub duplicate_parameters: bool,
    pub wrong_argument_count: bool,
    pub bad_requires: bool,
    pub impossible_boolean_expressions: bool,
    pub duplicate_if_conditions: bool,
    pub duplicate_binary_operands: bool,
    pub float_equality: bool,
    pub const_assignments: bool,
    pub parameter_type_mismatches: bool,
    pub return_type_mismatches: bool,
    pub formatting_lints: bool,
    pub inline_ignores: bool,
    pub inline_ignore_scopes: Vec<String>,
    pub inline_ignore_max_per_file: usize,
    pub globals: Vec<String>,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            duplicate_keys: false,
            missing_requires: false,
            self_assignments: false,
            doc_annotations: false,
            undefined_variables: false,
            unused_locals: false,
            unused_parameters: false,
            use_before_definition: false,
            duplicate_parameters: false,
            wrong_argument_count: false,
            bad_requires: false,
            impossible_boolean_expressions: false,
            duplicate_if_conditions: false,
            duplicate_binary_operands: false,
            float_equality: false,
            const_assignments: false,
            parameter_type_mismatches: false,
            return_type_mismatches: false,
            formatting_lints: false,
            inline_ignores: false,
            inline_ignore_scopes: vec![
                "tua-ignore-line".to_owned(),
                "tua-ignore-next-line".to_owned(),
            ],
            inline_ignore_max_per_file: 0,
            globals: Vec::new(),
        }
    }
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
    pub config_created: bool,
    pub source_created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl ConfigParseError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            column: 1,
            message: message.into(),
        }
    }
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
    let config_created = write_default_config_without_overwrite(&config_path)?;
    let source_dir = dir.join(ProjectConfig::default().source);
    let source_created = !source_dir.exists();

    fs::create_dir_all(&source_dir).map_err(|source| ConfigError::Io {
        path: source_dir.clone(),
        source,
    })?;

    Ok(InitResult {
        config_path,
        source_dir,
        config_created,
        source_created,
    })
}

fn write_default_config_without_overwrite(path: &Path) -> Result<bool, ConfigError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(DEFAULT_CONFIG_TEXT.as_bytes())
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

pub fn parse_config(input: &str) -> Result<ProjectConfig, ConfigParseError> {
    let mut config = ProjectConfig::default();
    let mut section = ConfigSection::Root;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw_line)
            .map_err(|message| ConfigParseError::new(line_number, message))?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if let Some(next_section) = parse_section_header(line_number, line)? {
            section = next_section;
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(ConfigParseError::new(line_number, "expected `key = value`"));
        };

        let key = raw_key.trim();
        let value = raw_value.trim();

        match section {
            ConfigSection::Root => match key {
                "source" => config.source = PathBuf::from(parse_string(line_number, value)?),
                "out" => config.out = PathBuf::from(parse_string(line_number, value)?),
                "target" => config.target = parse_string(line_number, value)?,
                "extension" => config.extension = parse_string(line_number, value)?,
                "emit_maps" => config.emit_maps = parse_bool(line_number, value)?,
                "" => {
                    return Err(ConfigParseError::new(line_number, "missing config key"));
                }
                other => {
                    return Err(ConfigParseError::new(
                        line_number,
                        format!("unknown config key `{other}`"),
                    ));
                }
            },
            ConfigSection::Emit => match key {
                "annotation_mode" => {
                    config.emit.annotation_mode = AnnotationMode::parse(line_number, value)?
                }
                "" => {
                    return Err(ConfigParseError::new(line_number, "missing config key"));
                }
                other => {
                    return Err(ConfigParseError::new(
                        line_number,
                        format!("unknown emit config key `{other}`"),
                    ));
                }
            },
            ConfigSection::Diagnostics => match key {
                "duplicate_keys" => {
                    config.diagnostics.duplicate_keys = parse_bool(line_number, value)?
                }
                "missing_requires" => {
                    config.diagnostics.missing_requires = parse_bool(line_number, value)?
                }
                "self_assignments" => {
                    config.diagnostics.self_assignments = parse_bool(line_number, value)?
                }
                "doc_annotations" => {
                    config.diagnostics.doc_annotations = parse_bool(line_number, value)?
                }
                "undefined_variables" => {
                    config.diagnostics.undefined_variables = parse_bool(line_number, value)?
                }
                "unused_locals" => {
                    config.diagnostics.unused_locals = parse_bool(line_number, value)?
                }
                "unused_parameters" => {
                    config.diagnostics.unused_parameters = parse_bool(line_number, value)?
                }
                "use_before_definition" => {
                    config.diagnostics.use_before_definition = parse_bool(line_number, value)?
                }
                "duplicate_parameters" => {
                    config.diagnostics.duplicate_parameters = parse_bool(line_number, value)?
                }
                "wrong_argument_count" => {
                    config.diagnostics.wrong_argument_count = parse_bool(line_number, value)?
                }
                "bad_requires" => config.diagnostics.bad_requires = parse_bool(line_number, value)?,
                "impossible_boolean_expressions" => {
                    config.diagnostics.impossible_boolean_expressions =
                        parse_bool(line_number, value)?
                }
                "duplicate_if_conditions" => {
                    config.diagnostics.duplicate_if_conditions = parse_bool(line_number, value)?
                }
                "duplicate_binary_operands" => {
                    config.diagnostics.duplicate_binary_operands = parse_bool(line_number, value)?
                }
                "float_equality" => {
                    config.diagnostics.float_equality = parse_bool(line_number, value)?
                }
                "const_assignments" => {
                    config.diagnostics.const_assignments = parse_bool(line_number, value)?
                }
                "parameter_type_mismatches" => {
                    config.diagnostics.parameter_type_mismatches = parse_bool(line_number, value)?
                }
                "return_type_mismatches" => {
                    config.diagnostics.return_type_mismatches = parse_bool(line_number, value)?
                }
                "formatting_lints" => {
                    config.diagnostics.formatting_lints = parse_bool(line_number, value)?
                }
                "inline_ignores" => {
                    config.diagnostics.inline_ignores = parse_bool(line_number, value)?
                }
                "inline_ignore_scopes" => {
                    config.diagnostics.inline_ignore_scopes =
                        parse_string_array(line_number, value)?
                }
                "inline_ignore_max_per_file" => {
                    config.diagnostics.inline_ignore_max_per_file = parse_usize(line_number, value)?
                }
                "globals" => config.diagnostics.globals = parse_string_array(line_number, value)?,
                "" => {
                    return Err(ConfigParseError::new(line_number, "missing config key"));
                }
                other => {
                    return Err(ConfigParseError::new(
                        line_number,
                        format!("unknown diagnostics config key `{other}`"),
                    ));
                }
            },
        }
    }

    Ok(config)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigSection {
    Root,
    Emit,
    Diagnostics,
}

fn parse_section_header(
    line_number: usize,
    line: &str,
) -> Result<Option<ConfigSection>, ConfigParseError> {
    if !line.starts_with('[') {
        return Ok(None);
    }

    let Some(section) = line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
    else {
        return Err(ConfigParseError::new(
            line_number,
            "expected section header `[name]`",
        ));
    };

    match section.trim() {
        "emit" => Ok(Some(ConfigSection::Emit)),
        "diagnostics" => Ok(Some(ConfigSection::Diagnostics)),
        "" => Err(ConfigParseError::new(line_number, "missing section name")),
        other => Err(ConfigParseError::new(
            line_number,
            format!("unknown config section `{other}`"),
        )),
    }
}

fn parse_string(line: usize, value: &str) -> Result<String, ConfigParseError> {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(ConfigParseError::new(line, "expected string value"));
    };

    let mut result = String::new();
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(ConfigParseError::new(line, "unfinished string escape"));
        };

        match escaped {
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            other => {
                return Err(ConfigParseError::new(
                    line,
                    format!("unsupported string escape `\\{other}`"),
                ));
            }
        }
    }

    Ok(result)
}

fn parse_bool(line: usize, value: &str) -> Result<bool, ConfigParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigParseError::new(line, "expected boolean value")),
    }
}

fn parse_usize(line: usize, value: &str) -> Result<usize, ConfigParseError> {
    value
        .parse()
        .map_err(|_| ConfigParseError::new(line, "expected non-negative integer value"))
}

fn parse_string_array(line: usize, value: &str) -> Result<Vec<String>, ConfigParseError> {
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(ConfigParseError::new(line, "expected string array value"));
    };
    let inner = inner.trim();

    if inner.is_empty() {
        return Ok(Vec::new());
    }

    inner
        .split(',')
        .map(|item| parse_string(line, item.trim()))
        .collect()
}

fn strip_comment(line: &str) -> Result<String, &'static str> {
    let mut result = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            result.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                result.push(ch);
                escaped = true;
            }
            '"' => {
                result.push(ch);
                in_string = !in_string;
            }
            '#' if !in_string => break,
            other => result.push(other),
        }
    }

    if in_string {
        Err("unterminated string value")
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
