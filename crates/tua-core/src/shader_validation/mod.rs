//! Shader compilation checks backed by LÖVE's own `love.graphics.validateShader`.
//!
//! LÖVE wraps shader code in a version-specific header before compiling it, so
//! only LÖVE knows exactly what a shader file means. Tua runs the user's LÖVE
//! with a small bundled program ([`VALIDATOR_MAIN`]) and speaks a line-based
//! protocol with it. This module holds the pure parts: encoding requests,
//! reading answers, and turning them into diagnostics. Callers own the process.

mod glslang;
pub use glslang::{parse_glslang_output, shader_compile_units, ShaderCompileUnit};

use std::collections::BTreeMap;
use std::path::Path;

use crate::asset::AssetReference;
use crate::{Diagnostic, DiagnosticCode};

/// `main.lua` of the validator game. Write it with [`VALIDATOR_CONF`] into an
/// empty directory and run `love <directory>`.
pub const VALIDATOR_MAIN: &str = include_str!("validator/main.lua");

/// `conf.lua` of the validator game: a 1×1 borderless window off screen.
pub const VALIDATOR_CONF: &str = include_str!("validator/conf.lua");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ShaderTarget {
    /// Desktop OpenGL, as LÖVE runs on Windows, macOS, and Linux.
    Desktop,
    /// OpenGL ES, as LÖVE runs on Android, iOS, and the web.
    Gles,
}

impl ShaderTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Gles => "gles",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "desktop" => Some(Self::Desktop),
            "gles" => Some(Self::Gles),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Gles => "OpenGL ES (mobile and web)",
        }
    }
}

impl<'de> serde::Deserialize<'de> for ShaderTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown shader target `{value}`; expected `desktop` or `gles`"
            ))
        })
    }
}

/// One framed request: `SHADER <id> <target> <byte-count>\n<source>`.
pub fn encode_request(id: usize, target: ShaderTarget, source: &str) -> Vec<u8> {
    let mut request = format!("SHADER {id} {} {}\n", target.as_str(), source.len()).into_bytes();
    request.extend_from_slice(source.as_bytes());
    request
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShaderStage {
    Vertex,
    Pixel,
}

impl ShaderStage {
    fn label(self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Pixel => "pixel",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShaderError {
    pub stage: Option<ShaderStage>,
    /// The line in the shader file, as LÖVE numbers user code from 1.
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShaderValidationOutcome {
    Valid,
    Invalid(Vec<ShaderError>),
    /// LÖVE refused the code before compiling it, as for a shader without an
    /// `effect` or `position` function.
    Rejected(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShaderValidatorOutput {
    pub love_version: Option<String>,
    pub results: BTreeMap<usize, ShaderValidationOutcome>,
    /// A Lua error that stopped the validator.
    pub fatal: Option<String>,
}

/// Reads everything the validator wrote to stdout.
pub fn parse_validator_output(output: &str) -> ShaderValidatorOutput {
    let mut parsed = ShaderValidatorOutput::default();

    for line in output.lines() {
        if let Some(version) = line.strip_prefix("READY ") {
            parsed.love_version = Some(version.trim().to_owned());
        } else if let Some(message) = line.strip_prefix("FATAL ") {
            parsed.fatal = Some(unescape(message));
        } else if let Some(result) = line.strip_prefix("RESULT ") {
            let mut parts = result.splitn(3, ' ');
            let (Some(id), Some(status)) = (parts.next(), parts.next()) else {
                continue;
            };
            let Ok(id) = id.parse::<usize>() else {
                continue;
            };
            let message = unescape(parts.next().unwrap_or_default());
            let outcome = match status {
                "valid" => ShaderValidationOutcome::Valid,
                "invalid" => ShaderValidationOutcome::Invalid(shader_errors(&message)),
                "rejected" => ShaderValidationOutcome::Rejected(rejection_reason(&message)),
                _ => continue,
            };
            parsed.results.insert(id, outcome);
        }
    }

    parsed
}

fn unescape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => output.push('\n'),
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }

    output
}

/// Splits LÖVE's message, such as
/// `Error validating pixel shader:\n\nERROR: 0:4: '=' : cannot convert ...`,
/// into one error per compiler line.
fn shader_errors(message: &str) -> Vec<ShaderError> {
    let mut stage = None;
    let mut errors = Vec::new();

    for line in message.lines().map(str::trim) {
        if line.starts_with("Error validating vertex shader") {
            stage = Some(ShaderStage::Vertex);
            continue;
        }
        if line.starts_with("Error validating pixel shader") {
            stage = Some(ShaderStage::Pixel);
            continue;
        }
        let Some(rest) = line.strip_prefix("ERROR: ") else {
            continue;
        };
        // `ERROR: 0:<line>: <message>`; summaries such as `2 compilation
        // errors.` have no location.
        let mut location = rest.splitn(3, ':');
        let (Some(_), Some(line_number), Some(text)) =
            (location.next(), location.next(), location.next())
        else {
            continue;
        };
        let Ok(line_number) = line_number.trim().parse::<usize>() else {
            continue;
        };
        let Some(text) = compiler_error_text(text) else {
            continue;
        };
        errors.push(ShaderError {
            stage,
            line: Some(line_number.max(1)),
            message: text,
        });
    }

    if errors.is_empty() {
        errors.push(ShaderError {
            stage,
            line: None,
            message: message.split_whitespace().collect::<Vec<_>>().join(" "),
        });
    }
    errors
}

/// The cleaned message of one compiler error, or `None` for glslang's
/// `compilation terminated` follow-up, which both validators print.
fn compiler_error_text(text: &str) -> Option<String> {
    let text = clean_compiler_message(text);
    (!text.is_empty() && text != "compilation terminated").then_some(text)
}

/// Turns glslang's `'=' :  cannot convert from ' const float' to ...` into
/// `cannot convert from 'const float' to ...`. Identifier tokens name the
/// problem (`'tint' : undeclared identifier`), so they are kept.
fn clean_compiler_message(text: &str) -> String {
    let text = text.trim();
    let text = match text.split_once(" : ") {
        Some((token, rest))
            if token.len() >= 2 && token.starts_with('\'') && token.ends_with('\'') =>
        {
            let name = token[1..token.len() - 1].trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                format!("'{name}': {rest}")
            } else {
                rest.to_owned()
            }
        }
        _ => text.to_owned(),
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // glslang pads the start of quoted types: `' const float'`.
    let mut cleaned = String::with_capacity(text.len());
    let mut quoted = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        cleaned.push(character);
        if character == '\'' {
            quoted = !quoted;
            if quoted {
                while characters.peek() == Some(&' ') {
                    characters.next();
                }
            }
        }
    }

    cleaned.trim().to_owned()
}

/// `bad argument #2 to '?' (missing 'position' or 'effect' function?)`
/// becomes the text in parentheses.
fn rejection_reason(message: &str) -> String {
    let message = message.trim();
    match (message.find('('), message.rfind(')')) {
        (Some(open), Some(close)) if message.starts_with("bad argument") && open < close => {
            message[open + 1..close].to_owned()
        }
        _ => message.to_owned(),
    }
}

/// Diagnostics for one shader and target: each compiler error on the shader
/// file, and one summary at every Tua call that loads the shader.
pub fn shader_validation_diagnostics(
    shader_path: &Path,
    relative_path: &str,
    references: &[AssetReference],
    target: ShaderTarget,
    outcome: &ShaderValidationOutcome,
) -> Vec<Diagnostic> {
    let errors = match outcome {
        ShaderValidationOutcome::Valid => return Vec::new(),
        ShaderValidationOutcome::Invalid(errors) => errors.clone(),
        ShaderValidationOutcome::Rejected(reason) => vec![ShaderError {
            stage: None,
            line: None,
            message: format!("LÖVE rejects this shader: {reason}"),
        }],
    };

    let mut diagnostics = errors
        .iter()
        .map(|error| {
            let stage = error
                .stage
                .map(|stage| format!(" ({} stage)", stage.label()))
                .unwrap_or_default();
            Diagnostic::error(
                shader_path,
                error.line.unwrap_or(1),
                1,
                format!(
                    "shader does not compile for {}{stage}: {}",
                    target.label(),
                    error.message
                ),
            )
            .with_code(DiagnosticCode::ASSET_SHADER_COMPILE)
        })
        .collect::<Vec<_>>();

    if let Some(first) = errors.first() {
        let location = first
            .line
            .map(|line| format!("line {line}: "))
            .unwrap_or_default();
        diagnostics.extend(references.iter().map(|reference| {
            Diagnostic::error(
                &reference.path,
                reference.span.start.line,
                reference.span.start.column,
                format!(
                    "shader `{relative_path}` does not compile for {}: {location}{}",
                    target.label(),
                    first.message
                ),
            )
            .with_range(
                reference.span.start.line,
                reference.span.start.column,
                reference.span.end.line,
                reference.span.end.column,
            )
            .with_code(DiagnosticCode::ASSET_SHADER_COMPILE)
        }));
    }

    diagnostics
}

#[cfg(test)]
mod tests;

/// One immutable input to the external validator. The source may be an editor overlay.
#[derive(Clone, Debug)]
pub struct ShaderValidationRequest {
    pub source_origin: Option<crate::SourcePosition>,
    pub mode: crate::config::ShaderValidationMode,
    pub asset: crate::asset::ProjectAsset,
    pub target: ShaderTarget,
    pub source: String,
}

/// Process orchestration is supplied by the executable; core analysis never spawns LOVE.
pub trait ShaderValidator: Send {
    /// Trusted tool identity; changes invalidate cached outcomes after upgrades.
    fn cache_key(&self) -> String {
        String::new()
    }

    /// Release external resources when no referenced shaders need validation.
    fn stop(&mut self) {}

    fn validate(
        &mut self,
        requests: &[ShaderValidationRequest],
    ) -> Result<Vec<ShaderValidationOutcome>, String>;
}

/// Collect only statically referenced shader assets, with bounded input and deduplicated targets.
pub fn shader_validation_requests(
    inventory: &crate::asset::AssetInventory,
    config: &crate::config::ShadersConfig,
    mut read: impl FnMut(&Path) -> Result<String, String>,
) -> Result<Vec<ShaderValidationRequest>, String> {
    if config.validate == crate::config::ShaderValidationMode::Off {
        return Ok(Vec::new());
    }
    let targets: std::collections::BTreeSet<_> = config.targets.iter().copied().collect();
    let mut requests = Vec::new();
    let mut bytes = 0usize;
    for asset in &inventory.assets {
        if !asset
            .references
            .iter()
            .any(|reference| reference.loader_kind == crate::asset::AssetLoaderKind::Shader)
        {
            continue;
        }
        if targets.is_empty() {
            break;
        }
        let source = read(&asset.path)?;
        if crate::asset::ShaderSource::from_source(source.clone(), None)
            .is_some_and(|source| source.missing_entry_point())
        {
            // The checker already reports TL2017 at each loader without a process.
            continue;
        }
        bytes = bytes.saturating_add(source.len().saturating_mul(targets.len()));
        if bytes > 16 * 1024 * 1024 || requests.len() + targets.len() > 512 {
            return Err("shader validation input exceeds 16 MiB or 512 requests".to_owned());
        }
        for target in &targets {
            requests.push(ShaderValidationRequest {
                source_origin: None,
                mode: config.validate,
                asset: asset.clone(),
                target: *target,
                source: source.clone(),
            });
        }
    }
    Ok(requests)
}

pub fn validation_diagnostics(
    requests: &[ShaderValidationRequest],
    outcomes: &[ShaderValidationOutcome],
) -> Vec<Diagnostic> {
    requests
        .iter()
        .zip(outcomes)
        .flat_map(|(request, outcome)| {
            shader_validation_diagnostics(
                &request.asset.path,
                &request.asset.relative_path,
                &request.asset.references,
                request.target,
                outcome,
            )
            .into_iter()
            .map(|mut diagnostic| {
                if let Some(origin) = request.source_origin {
                    let start = crate::asset::map_position(
                        origin,
                        crate::SourcePosition {
                            line: diagnostic.line,
                            column: diagnostic.column,
                        },
                    );
                    let end = crate::asset::map_position(
                        origin,
                        crate::SourcePosition {
                            line: diagnostic.end_line,
                            column: diagnostic.end_column,
                        },
                    );
                    diagnostic.line = start.line;
                    diagnostic.column = start.column;
                    diagnostic.end_line = end.line;
                    diagnostic.end_column = end.column;
                }
                diagnostic
            })
        })
        .collect()
}

/// Bound combined asset and inline batches before starting external tools.
pub fn check_request_limits(requests: &[ShaderValidationRequest]) -> Result<(), String> {
    if requests.len() > 512
        || requests.iter().map(|r| r.source.len()).sum::<usize>() > 16 * 1024 * 1024
    {
        Err("shader validation input exceeds 16 MiB or 512 requests".into())
    } else {
        Ok(())
    }
}
