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
";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConfig {
    pub source: PathBuf,
    pub out: PathBuf,
    pub target: String,
    pub extension: String,
    pub emit_maps: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            source: PathBuf::from("src"),
            out: PathBuf::from("build"),
            target: "love11-luajit".to_owned(),
            extension: "tua".to_owned(),
            emit_maps: true,
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

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw_line)
            .map_err(|message| ConfigParseError::new(line_number, message))?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(ConfigParseError::new(line_number, "expected `key = value`"));
        };

        let key = raw_key.trim();
        let value = raw_value.trim();

        match key {
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
        }
    }

    Ok(config)
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
