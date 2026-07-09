use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::Visitor;

use crate::types::{FunctionType, Type};
use crate::{ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AssetLoaderKind {
    Image,
    Sound,
    Music,
    Font,
    Shader,
    ImageData,
    Canvas,
    Quad,
    SpriteBatch,
    Particles,
}

impl AssetLoaderKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Sound => "sound",
            Self::Music => "music",
            Self::Font => "font",
            Self::Shader => "shader",
            Self::ImageData => "imageData",
            Self::Canvas => "canvas",
            Self::Quad => "quad",
            Self::SpriteBatch => "spriteBatch",
            Self::Particles => "particles",
        }
    }

    pub(crate) const fn emitted_callee(self) -> &'static str {
        match self {
            Self::Image => "love.graphics.newImage",
            Self::Sound => "love.audio.newSource",
            Self::Music => "love.audio.newSource",
            Self::Font => "love.graphics.newFont",
            Self::Shader => "love.graphics.newShader",
            Self::ImageData => "love.image.newImageData",
            Self::Canvas => "love.graphics.newCanvas",
            Self::Quad => "love.graphics.newQuad",
            Self::SpriteBatch => "love.graphics.newSpriteBatch",
            Self::Particles => "love.graphics.newParticleSystem",
        }
    }

    pub(crate) fn emitted_call(self, arguments: &[String]) -> Option<String> {
        if arguments.len() < self.required_argument_count() {
            return None;
        }

        match self {
            Self::Sound => Some(format!(
                "{}({}, \"static\")",
                self.emitted_callee(),
                arguments.first()?
            )),
            Self::Music => Some(format!(
                "{}({}, \"stream\")",
                self.emitted_callee(),
                arguments.first()?
            )),
            _ => Some(format!(
                "{}({})",
                self.emitted_callee(),
                arguments.join(", ")
            )),
        }
    }

    pub(crate) fn return_type(self) -> Type {
        match self {
            Self::Image => Type::alias("Image"),
            Self::Sound | Self::Music => Type::alias("Source"),
            Self::Font => Type::alias("Font"),
            Self::Shader => Type::alias("Shader"),
            Self::ImageData => Type::alias("ImageData"),
            Self::Canvas => Type::alias("Canvas"),
            Self::Quad => Type::alias("Quad"),
            Self::SpriteBatch => Type::alias("SpriteBatch"),
            Self::Particles => Type::alias("ParticleSystem"),
        }
    }

    pub const fn return_type_name(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Sound | Self::Music => "Source",
            Self::Font => "Font",
            Self::Shader => "Shader",
            Self::ImageData => "ImageData",
            Self::Canvas => "Canvas",
            Self::Quad => "Quad",
            Self::SpriteBatch => "SpriteBatch",
            Self::Particles => "ParticleSystem",
        }
    }

    pub(crate) fn function_type(self) -> FunctionType {
        FunctionType::new(self.parameter_types(), self.return_type())
    }

    pub(crate) fn parameter_types(self) -> Vec<Type> {
        match self {
            Self::Image | Self::Sound | Self::Music | Self::Shader | Self::ImageData => {
                vec![Type::string()]
            }
            Self::Font => vec![Type::string(), Type::number()],
            Self::Canvas => vec![Type::number(), Type::number()],
            Self::Quad => vec![
                Type::number(),
                Type::number(),
                Type::number(),
                Type::number(),
                Type::number(),
                Type::number(),
            ],
            Self::SpriteBatch | Self::Particles => vec![Type::alias("Image"), Type::number()],
        }
    }

    pub(crate) const fn parameter_names(self) -> &'static [&'static str] {
        match self {
            Self::Image | Self::Sound | Self::Music | Self::Shader | Self::ImageData => &["path"],
            Self::Font => &["path", "size"],
            Self::Canvas => &["width", "height"],
            Self::Quad => &["x", "y", "width", "height", "sw", "sh"],
            Self::SpriteBatch => &["image", "size"],
            Self::Particles => &["image", "count"],
        }
    }

    pub(crate) const fn required_argument_count(self) -> usize {
        match self {
            Self::Image | Self::Sound | Self::Music | Self::Shader | Self::ImageData => 1,
            Self::Font | Self::Canvas | Self::SpriteBatch | Self::Particles => 2,
            Self::Quad => 6,
        }
    }

    pub(crate) const fn supports_string_sugar(self) -> bool {
        matches!(
            self,
            Self::Image | Self::Sound | Self::Music | Self::Shader | Self::ImageData
        )
    }

    pub(crate) const fn detail(self) -> &'static str {
        match self {
            Self::Image => "function image(path: string): Image",
            Self::Sound => "function sound(path: string): Source",
            Self::Music => "function music(path: string): Source",
            Self::Font => "function font(path: string, size: number): Font",
            Self::Shader => "function shader(path: string): Shader",
            Self::ImageData => "function imageData(path: string): ImageData",
            Self::Canvas => "function canvas(width: number, height: number): Canvas",
            Self::Quad => {
                "function quad(x: number, y: number, width: number, height: number, sw: number, sh: number): Quad"
            }
            Self::SpriteBatch => "function spriteBatch(image: Image, size: number): SpriteBatch",
            Self::Particles => "function particles(image: Image, count: number): ParticleSystem",
        }
    }

    pub(crate) const fn documentation(self) -> &'static str {
        match self {
            Self::Image => {
                "Loads an image asset. Emits love.graphics.newImage(path) in generated Lua."
            }
            Self::Sound => {
                "Loads a static sound asset. Emits love.audio.newSource(path, \"static\") in generated Lua."
            }
            Self::Music => {
                "Loads a streaming music asset. Emits love.audio.newSource(path, \"stream\") in generated Lua."
            }
            Self::Font => "Loads a font asset. Emits love.graphics.newFont(path, size) in generated Lua.",
            Self::Shader => {
                "Loads a shader asset. Emits love.graphics.newShader(path) in generated Lua."
            }
            Self::ImageData => {
                "Loads image data. Emits love.image.newImageData(path) in generated Lua."
            }
            Self::Canvas => {
                "Creates a canvas. Emits love.graphics.newCanvas(width, height) in generated Lua."
            }
            Self::Quad => "Creates a quad. Emits love.graphics.newQuad(...) in generated Lua.",
            Self::SpriteBatch => {
                "Creates a sprite batch. Emits love.graphics.newSpriteBatch(image, size) in generated Lua."
            }
            Self::Particles => {
                "Creates a particle system. Emits love.graphics.newParticleSystem(image, count) in generated Lua."
            }
        }
    }

    pub const fn supported_extensions(self) -> &'static [&'static str] {
        match self {
            Self::Image | Self::ImageData => {
                &["bmp", "exr", "hdr", "jpeg", "jpg", "pic", "png", "tga"]
            }
            Self::Sound | Self::Music => &["flac", "mp3", "ogg", "wav"],
            Self::Font => &["otf", "ttf"],
            Self::Shader => &["frag", "fs", "glsl", "vert", "vs"],
            Self::Canvas | Self::Quad | Self::SpriteBatch | Self::Particles => &[],
        }
    }

    pub const fn asset_label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Sound => "sound",
            Self::Music => "music",
            Self::Font => "font",
            Self::Shader => "shader",
            Self::ImageData => "image data",
            Self::Canvas => "canvas",
            Self::Quad => "quad",
            Self::SpriteBatch => "sprite batch",
            Self::Particles => "particle system",
        }
    }

    pub const fn has_asset_path(self) -> bool {
        match self {
            Self::Image
            | Self::Sound
            | Self::Music
            | Self::Font
            | Self::Shader
            | Self::ImageData => true,
            Self::Canvas | Self::Quad | Self::SpriteBatch | Self::Particles => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetMetadata {
    pub kind: AssetLoaderKind,
    pub relative_path: String,
    pub extension: Option<String>,
    pub span: SourceSpan,
    pub inferred_love_type: &'static str,
    pub status: AssetValidationStatus,
    pub dimensions: Option<(u32, u32)>,
    pub duration_seconds: Option<f32>,
}

impl AssetMetadata {
    pub fn validation_label(&self) -> &'static str {
        match self.status {
            AssetValidationStatus::Valid => "valid",
            AssetValidationStatus::Unchecked => "unchecked",
            AssetValidationStatus::Missing => "missing",
            AssetValidationStatus::UnsupportedExtension => "unsupported extension",
        }
    }

    pub fn hover_markdown(&self) -> String {
        let mut lines = vec![
            format!("asset `{}`", self.relative_path),
            format!("kind: {}", self.kind.asset_label()),
            format!("LOVE type: {}", self.inferred_love_type),
            format!("status: {}", self.validation_label()),
        ];

        if let Some(extension) = &self.extension {
            lines.push(format!("extension: .{extension}"));
        }

        if let Some((width, height)) = self.dimensions {
            lines.push(format!("dimensions: {width}x{height}"));
        }

        if let Some(duration) = self.duration_seconds {
            lines.push(format!("duration: {duration:.2}s"));
        }

        lines.join("\n")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetValidationStatus {
    Valid,
    Missing,
    UnsupportedExtension,
    Unchecked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssetPathCandidate {
    pub path: String,
    pub kind: AssetLoaderKind,
    pub extension: Option<String>,
}

pub(crate) fn loader_kind(name: &str) -> Option<AssetLoaderKind> {
    match name {
        "image" => Some(AssetLoaderKind::Image),
        "sound" => Some(AssetLoaderKind::Sound),
        "music" => Some(AssetLoaderKind::Music),
        "font" => Some(AssetLoaderKind::Font),
        "shader" => Some(AssetLoaderKind::Shader),
        "imageData" => Some(AssetLoaderKind::ImageData),
        "canvas" => Some(AssetLoaderKind::Canvas),
        "quad" => Some(AssetLoaderKind::Quad),
        "spriteBatch" => Some(AssetLoaderKind::SpriteBatch),
        "particles" => Some(AssetLoaderKind::Particles),
        _ => None,
    }
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    &[
        "Body",
        "Canvas",
        "Font",
        "Image",
        "ImageData",
        "ParticleSystem",
        "Quad",
        "Shader",
        "Source",
        "SpriteBatch",
        "World",
    ]
}

pub(crate) fn collect_asset_metadata(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
) -> Vec<AssetMetadata> {
    let mut visitor = AssetVisitor {
        assets: Vec::new(),
        roots: normalize_asset_roots(asset_roots),
    };

    visitor.visit_ast(&parsed.ast);
    visitor.assets
}

pub(crate) fn asset_at_position(
    assets: &[AssetMetadata],
    position: SourcePosition,
) -> Option<&AssetMetadata> {
    assets
        .iter()
        .find(|asset| contains_position(asset.span, position))
}

pub(crate) fn complete_asset_paths(
    roots: &[PathBuf],
    kind: AssetLoaderKind,
    prefix: &str,
) -> Vec<AssetPathCandidate> {
    let mut candidates = BTreeMap::new();

    for root in normalize_asset_roots(roots) {
        collect_asset_path_candidates(&root, &root, kind, prefix, &mut candidates);
    }

    candidates.into_values().collect()
}

pub(crate) fn validate_asset(
    kind: AssetLoaderKind,
    relative_path: &str,
    span: SourceSpan,
    asset_roots: &[PathBuf],
) -> AssetMetadata {
    let extension = asset_extension(relative_path);
    if !kind.has_asset_path() {
        return AssetMetadata {
            kind,
            relative_path: relative_path.to_owned(),
            extension,
            span,
            inferred_love_type: kind.return_type_name(),
            status: AssetValidationStatus::Unchecked,
            dimensions: None,
            duration_seconds: None,
        };
    }
    let unsupported = extension
        .as_deref()
        .is_none_or(|extension| !kind.supported_extensions().contains(&extension));
    let resolved_path = if unsupported {
        None
    } else {
        resolve_asset_path(relative_path, asset_roots)
    };
    let path_is_absolute = Path::new(relative_path).is_absolute();
    let status = if unsupported {
        AssetValidationStatus::UnsupportedExtension
    } else if resolved_path.is_some() {
        AssetValidationStatus::Valid
    } else if asset_roots.is_empty() && !path_is_absolute {
        AssetValidationStatus::Unchecked
    } else {
        AssetValidationStatus::Missing
    };
    let dimensions = resolved_path
        .as_deref()
        .filter(|_| matches!(kind, AssetLoaderKind::Image | AssetLoaderKind::ImageData))
        .and_then(read_png_dimensions);

    AssetMetadata {
        kind,
        relative_path: relative_path.to_owned(),
        extension,
        span,
        inferred_love_type: kind.return_type_name(),
        status,
        dimensions,
        duration_seconds: None,
    }
}

struct AssetVisitor {
    assets: Vec<AssetMetadata>,
    roots: Vec<PathBuf>,
}

impl Visitor for AssetVisitor {
    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        if let Some(call) = asset_loader_call(function_call) {
            self.assets.push(validate_asset(
                call.kind,
                &call.path,
                call.path_span,
                &self.roots,
            ));
        }
    }
}

struct AssetLoaderCall {
    kind: AssetLoaderKind,
    path: String,
    path_span: SourceSpan,
}

fn asset_loader_call(function_call: &FunctionCall) -> Option<AssetLoaderCall> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };
    let kind = loader_kind(&token_text(name))?;
    if !kind.has_asset_path() {
        return None;
    }
    let call = function_call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(call) => Some(call),
        _ => None,
    })?;
    let (path, path_span) = match call {
        Call::AnonymousCall(FunctionArgs::String(path)) => {
            (string_literal_value(path)?, node_span(path)?)
        }
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(path) = arguments.iter().next()? else {
                return None;
            };

            (string_literal_value(path)?, node_span(path)?)
        }
        _ => return None,
    };

    Some(AssetLoaderCall {
        kind,
        path,
        path_span,
    })
}

fn collect_asset_path_candidates(
    root: &Path,
    dir: &Path,
    kind: AssetLoaderKind,
    prefix: &str,
    candidates: &mut BTreeMap<String, AssetPathCandidate>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_asset_path_candidates(root, &path, kind, prefix, candidates);
            continue;
        }

        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        let extension = extension.to_ascii_lowercase();

        if !kind.supported_extensions().contains(&extension.as_str()) {
            continue;
        }

        let Some(relative) = path.strip_prefix(root).ok().map(normalize_relative_path) else {
            continue;
        };

        if relative.starts_with(prefix) {
            candidates
                .entry(relative.clone())
                .or_insert(AssetPathCandidate {
                    path: relative,
                    kind,
                    extension: Some(extension),
                });
        }
    }
}

fn resolve_asset_path(relative_path: &str, asset_roots: &[PathBuf]) -> Option<PathBuf> {
    let path = Path::new(relative_path);

    if path.is_absolute() {
        return path.exists().then(|| path.to_path_buf());
    }

    asset_roots
        .iter()
        .map(|root| root.join(path))
        .find(|candidate| candidate.exists())
}

fn read_png_dimensions(path: &Path) -> Option<(u32, u32)> {
    let bytes = fs::read(path).ok()?;

    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);

    Some((width, height))
}

fn normalize_asset_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| root.components().collect::<PathBuf>())
        .collect()
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn asset_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

fn string_literal_value(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        full_moon::tokenizer::TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn token_text(token: &TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    let start = (span.start.line, span.start.column);
    let end = (span.end.line, span.end.column);
    let position = (position.line, position.column);

    start <= position && position < end
}

#[cfg(test)]
mod tests;
