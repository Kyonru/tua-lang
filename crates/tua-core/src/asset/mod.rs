use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Index, Prefix, Suffix};
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
    ImageFont,
    SoundData,
    Video,
    VideoStream,
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
            Self::ImageFont => "imageFont",
            Self::SoundData => "soundData",
            Self::Video => "video",
            Self::VideoStream => "videoStream",
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
            Self::ImageFont => "love.graphics.newImageFont",
            Self::SoundData => "love.sound.newSoundData",
            Self::Video => "love.graphics.newVideo",
            Self::VideoStream => "love.video.newVideoStream",
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
            Self::ImageFont => Type::alias("Font"),
            Self::SoundData => Type::alias("SoundData"),
            Self::Video => Type::alias("Video"),
            Self::VideoStream => Type::alias("VideoStream"),
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
            Self::ImageFont => "Font",
            Self::SoundData => "SoundData",
            Self::Video => "Video",
            Self::VideoStream => "VideoStream",
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
            Self::ImageFont => vec![Type::string(), Type::string()],
            Self::SoundData | Self::Video | Self::VideoStream => vec![Type::string()],
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
            Self::ImageFont => &["path", "glyphs"],
            Self::SoundData | Self::Video | Self::VideoStream => &["path"],
            Self::Canvas => &["width", "height"],
            Self::Quad => &["x", "y", "width", "height", "sw", "sh"],
            Self::SpriteBatch => &["image", "size"],
            Self::Particles => &["image", "count"],
        }
    }

    pub(crate) const fn required_argument_count(self) -> usize {
        match self {
            Self::Image | Self::Sound | Self::Music | Self::Shader | Self::ImageData => 1,
            Self::SoundData | Self::Video | Self::VideoStream => 1,
            Self::ImageFont => 2,
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
            Self::ImageFont => "function love.graphics.newImageFont(path: string, glyphs: string): Font",
            Self::SoundData => "function love.sound.newSoundData(path: string): SoundData",
            Self::Video => "function love.graphics.newVideo(path: string): Video",
            Self::VideoStream => "function love.video.newVideoStream(path: string): VideoStream",
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
            Self::ImageFont => {
                "Loads an image font asset. Emits love.graphics.newImageFont(path, glyphs) in generated Lua."
            }
            Self::SoundData => {
                "Loads sound data. Emits love.sound.newSoundData(path) in generated Lua."
            }
            Self::Video => "Loads a video asset. Emits love.graphics.newVideo(path) in generated Lua.",
            Self::VideoStream => {
                "Loads a video stream. Emits love.video.newVideoStream(path) in generated Lua."
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
            Self::Image | Self::ImageData | Self::ImageFont => {
                &["bmp", "exr", "hdr", "jpeg", "jpg", "pic", "png", "tga"]
            }
            Self::Sound | Self::Music | Self::SoundData => &["flac", "mp3", "ogg", "wav"],
            Self::Font => &["otf", "ttf"],
            Self::Shader => &["frag", "fs", "glsl", "vert", "vs"],
            Self::Video | Self::VideoStream => &["ogg", "ogm", "ogv"],
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
            Self::ImageFont => "image font",
            Self::SoundData => "sound data",
            Self::Video => "video",
            Self::VideoStream => "video stream",
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
            | Self::ImageData
            | Self::ImageFont
            | Self::SoundData
            | Self::Video
            | Self::VideoStream => true,
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
            AssetValidationStatus::WrongKind => "wrong asset type",
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
    WrongKind,
    Unchecked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssetPathCandidate {
    pub path: String,
    pub kind: AssetLoaderKind,
    pub extension: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AssetIndex {
    roots: Vec<PathBuf>,
    by_relative: BTreeMap<String, AssetIndexEntry>,
    by_absolute: BTreeMap<PathBuf, String>,
}

#[derive(Clone, Debug, PartialEq)]
struct AssetIndexEntry {
    relative_path: String,
    absolute_path: PathBuf,
    extension: Option<String>,
    dimensions: Option<(u32, u32)>,
}

impl AssetIndex {
    pub(crate) fn from_roots(roots: &[PathBuf]) -> Self {
        let roots = normalize_asset_roots(roots);
        let mut index = Self {
            roots: roots.clone(),
            by_relative: BTreeMap::new(),
            by_absolute: BTreeMap::new(),
        };

        for root in roots {
            collect_asset_index_entries(&root, &root, &mut index);
        }

        index
    }

    pub(crate) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub(crate) fn complete_paths(
        &self,
        kind: AssetLoaderKind,
        prefix: &str,
    ) -> Vec<AssetPathCandidate> {
        self.by_relative
            .values()
            .filter_map(|entry| {
                let extension = entry.extension.as_deref()?;

                if !kind.supported_extensions().contains(&extension) {
                    return None;
                }

                if !entry.relative_path.starts_with(prefix) {
                    return None;
                }

                Some(AssetPathCandidate {
                    path: entry.relative_path.clone(),
                    kind,
                    extension: Some(extension.to_owned()),
                })
            })
            .collect()
    }

    fn entry_for_path(&self, relative_path: &str) -> Option<&AssetIndexEntry> {
        let path = Path::new(relative_path);

        if path.is_absolute() {
            let normalized = normalize_path_components(path);

            return self
                .by_absolute
                .get(&normalized)
                .and_then(|relative| self.by_relative.get(relative));
        }

        self.by_relative.get(relative_path)
    }
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
        "SoundData",
        "SpriteBatch",
        "Video",
        "VideoStream",
        "World",
    ]
}

pub(crate) fn asset_loader_kind_for_callee_parts(parts: &[String]) -> Option<AssetLoaderKind> {
    asset_loader_spec_for_callee_parts(parts).map(|spec| spec.kind)
}

pub(crate) fn asset_source_type_label(path: &str) -> Option<&'static str> {
    asset_extension(path)
        .as_deref()
        .and_then(source_type_label_for_extension)
}

pub(crate) fn collect_asset_metadata(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
) -> Vec<AssetMetadata> {
    collect_asset_metadata_with_index(parsed, asset_roots, None)
}

pub(crate) fn collect_asset_metadata_with_index(
    parsed: &ParsedSource,
    asset_roots: &[PathBuf],
    asset_index: Option<&AssetIndex>,
) -> Vec<AssetMetadata> {
    let mut visitor = AssetVisitor {
        assets: Vec::new(),
        roots: if asset_index.is_some() {
            normalize_asset_roots_without_fs(asset_roots)
        } else {
            normalize_asset_roots(asset_roots)
        },
        index: asset_index,
    };

    visitor.visit_ast(&parsed.ast);
    for declaration in &parsed.sprite_declarations {
        if let Some(metadata) =
            crate::sprite::asset_metadata_for_declaration(declaration, &visitor.roots, asset_index)
        {
            visitor.assets.push(metadata);
        }
    }
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
    validate_asset_with_index(kind, relative_path, span, asset_roots, None)
}

pub(crate) fn validate_asset_with_index(
    kind: AssetLoaderKind,
    relative_path: &str,
    span: SourceSpan,
    asset_roots: &[PathBuf],
    asset_index: Option<&AssetIndex>,
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

    if let Some(asset_index) = asset_index {
        return validate_asset_from_index(kind, relative_path, span, asset_roots, asset_index);
    }

    let wrong_kind = extension
        .as_deref()
        .is_some_and(|extension| source_type_label_for_extension(extension).is_some())
        && extension
            .as_deref()
            .is_some_and(|extension| !kind.supported_extensions().contains(&extension));
    let unsupported = extension
        .as_deref()
        .is_none_or(|extension| !kind.supported_extensions().contains(&extension));
    let resolved_path = if unsupported {
        None
    } else {
        resolve_asset_path(relative_path, asset_roots)
    };
    let path_is_absolute = Path::new(relative_path).is_absolute();
    let status = if wrong_kind {
        AssetValidationStatus::WrongKind
    } else if unsupported {
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

pub(crate) fn resolved_asset_path(
    metadata: &AssetMetadata,
    asset_roots: &[PathBuf],
    asset_index: Option<&AssetIndex>,
) -> Option<PathBuf> {
    if metadata.status != AssetValidationStatus::Valid {
        return None;
    }

    asset_index
        .and_then(|index| index.entry_for_path(&metadata.relative_path))
        .map(|entry| entry.absolute_path.clone())
        .or_else(|| resolve_asset_path(&metadata.relative_path, asset_roots))
}

struct AssetVisitor<'a> {
    assets: Vec<AssetMetadata>,
    roots: Vec<PathBuf>,
    index: Option<&'a AssetIndex>,
}

impl Visitor for AssetVisitor<'_> {
    fn visit_function_call(&mut self, function_call: &FunctionCall) {
        if let Some(call) = asset_loader_call(function_call) {
            let metadata = if let Some(index) = self.index {
                validate_asset_with_index(
                    call.kind,
                    &call.path,
                    call.path_span,
                    &self.roots,
                    Some(index),
                )
            } else {
                validate_asset(call.kind, &call.path, call.path_span, &self.roots)
            };

            self.assets.push(metadata);
        }
    }
}

pub(crate) struct AssetLoaderCall {
    pub(crate) kind: AssetLoaderKind,
    pub(crate) path: String,
    pub(crate) path_span: SourceSpan,
}

pub(crate) fn asset_loader_call(function_call: &FunctionCall) -> Option<AssetLoaderCall> {
    let (callee_parts, call) = field_chain_before_first_call(function_call)?;
    let spec = asset_loader_spec_for_callee_parts(&callee_parts)?;
    let (path, path_span) = call_string_argument(call, 0)?;
    let kind = kind_for_call_arguments(spec.kind, call);

    if matches!(spec.path_mode, AssetPathMode::PathLikeOnly) && !looks_like_asset_path(&path) {
        return None;
    }

    Some(AssetLoaderCall {
        kind,
        path,
        path_span,
    })
}

fn field_chain_before_first_call(function_call: &FunctionCall) -> Option<(Vec<String>, &Call)> {
    let Prefix::Name(name) = function_call.prefix() else {
        return None;
    };

    let mut parts = vec![token_text(name)];

    for suffix in function_call.suffixes() {
        match suffix {
            Suffix::Index(Index::Dot { name, .. }) => parts.push(token_text(name)),
            Suffix::Call(call @ Call::AnonymousCall(_)) => return Some((parts, call)),
            _ => return None,
        }
    }

    None
}

#[derive(Clone, Copy)]
struct AssetLoaderSpec {
    kind: AssetLoaderKind,
    path_mode: AssetPathMode,
}

#[derive(Clone, Copy)]
enum AssetPathMode {
    Always,
    PathLikeOnly,
}

fn asset_loader_spec_for_callee_parts(parts: &[String]) -> Option<AssetLoaderSpec> {
    if parts.len() == 1 {
        let kind = loader_kind(&parts[0])?;

        return kind.has_asset_path().then_some(AssetLoaderSpec {
            kind,
            path_mode: AssetPathMode::Always,
        });
    }

    let kind = if parts_match(parts, &["love", "graphics", "newImage"]) {
        AssetLoaderKind::Image
    } else if parts_match(parts, &["love", "graphics", "newImageFont"]) {
        AssetLoaderKind::ImageFont
    } else if parts_match(parts, &["love", "graphics", "newFont"]) {
        AssetLoaderKind::Font
    } else if parts_match(parts, &["love", "graphics", "newShader"]) {
        return Some(AssetLoaderSpec {
            kind: AssetLoaderKind::Shader,
            path_mode: AssetPathMode::PathLikeOnly,
        });
    } else if parts_match(parts, &["love", "image", "newImageData"]) {
        AssetLoaderKind::ImageData
    } else if parts_match(parts, &["love", "audio", "newSource"]) {
        AssetLoaderKind::Sound
    } else if parts_match(parts, &["love", "sound", "newSoundData"]) {
        AssetLoaderKind::SoundData
    } else if parts_match(parts, &["love", "graphics", "newVideo"]) {
        AssetLoaderKind::Video
    } else if parts_match(parts, &["love", "video", "newVideoStream"]) {
        AssetLoaderKind::VideoStream
    } else {
        return None;
    };

    Some(AssetLoaderSpec {
        kind,
        path_mode: AssetPathMode::Always,
    })
}

fn kind_for_call_arguments(kind: AssetLoaderKind, call: &Call) -> AssetLoaderKind {
    if kind == AssetLoaderKind::Sound
        && call_string_argument(call, 1).is_some_and(|(mode, _)| mode == "stream")
    {
        AssetLoaderKind::Music
    } else {
        kind
    }
}

fn call_string_argument(call: &Call, index: usize) -> Option<(String, SourceSpan)> {
    match call {
        Call::AnonymousCall(FunctionArgs::String(path)) if index == 0 => {
            Some((string_literal_value(path)?, node_span(path)?))
        }
        Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }) => {
            let Expression::String(path) = arguments.iter().nth(index)? else {
                return None;
            };

            Some((string_literal_value(path)?, node_span(path)?))
        }
        _ => None,
    }
}

fn looks_like_asset_path(path: &str) -> bool {
    path.contains('/')
        || path.contains('\\')
        || (asset_extension(path).is_some() && !path.chars().any(char::is_whitespace))
}

fn parts_match(parts: &[String], expected: &[&str]) -> bool {
    parts.len() == expected.len()
        && parts
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
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

fn collect_asset_index_entries(root: &Path, dir: &Path, index: &mut AssetIndex) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_asset_index_entries(root, &path, index);
            continue;
        }

        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        let extension = extension.to_ascii_lowercase();

        if !supported_asset_extension(&extension) {
            continue;
        }

        let Some(relative_path) = path.strip_prefix(root).ok().map(normalize_relative_path) else {
            continue;
        };

        if index.by_relative.contains_key(&relative_path) {
            continue;
        }

        let absolute_path = normalize_path_components(&path);
        let dimensions = if extension == "png" {
            read_png_dimensions(&path)
        } else {
            None
        };

        index
            .by_absolute
            .insert(absolute_path.clone(), relative_path.clone());
        index.by_relative.insert(
            relative_path.clone(),
            AssetIndexEntry {
                relative_path,
                absolute_path,
                extension: Some(extension),
                dimensions,
            },
        );
    }
}

fn validate_asset_from_index(
    kind: AssetLoaderKind,
    relative_path: &str,
    span: SourceSpan,
    asset_roots: &[PathBuf],
    asset_index: &AssetIndex,
) -> AssetMetadata {
    let extension = asset_extension(relative_path);
    let wrong_kind = extension
        .as_deref()
        .is_some_and(|extension| source_type_label_for_extension(extension).is_some())
        && extension
            .as_deref()
            .is_some_and(|extension| !kind.supported_extensions().contains(&extension));
    let unsupported = extension
        .as_deref()
        .is_none_or(|extension| !kind.supported_extensions().contains(&extension));
    let indexed = (!unsupported)
        .then(|| asset_index.entry_for_path(relative_path))
        .flatten();
    let path_is_absolute = Path::new(relative_path).is_absolute();
    let roots_are_empty = asset_roots.is_empty() && asset_index.roots().is_empty();
    let status = if wrong_kind {
        AssetValidationStatus::WrongKind
    } else if unsupported {
        AssetValidationStatus::UnsupportedExtension
    } else if indexed.is_some() {
        AssetValidationStatus::Valid
    } else if roots_are_empty && !path_is_absolute {
        AssetValidationStatus::Unchecked
    } else {
        AssetValidationStatus::Missing
    };
    let dimensions = indexed
        .filter(|_| matches!(kind, AssetLoaderKind::Image | AssetLoaderKind::ImageData))
        .and_then(|entry| entry.dimensions);

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

fn normalize_asset_roots_without_fs(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter(|root| !root.as_os_str().is_empty())
        .map(|root| root.components().collect::<PathBuf>())
        .collect()
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_path_components(path: &Path) -> PathBuf {
    path.components().collect()
}

fn asset_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

fn supported_asset_extension(extension: &str) -> bool {
    source_type_label_for_extension(extension).is_some()
}

fn source_type_label_for_extension(extension: &str) -> Option<&'static str> {
    if AssetLoaderKind::Image
        .supported_extensions()
        .contains(&extension)
    {
        Some("image")
    } else if AssetLoaderKind::Sound
        .supported_extensions()
        .contains(&extension)
        && AssetLoaderKind::Video
            .supported_extensions()
            .contains(&extension)
    {
        Some("audio or video")
    } else if AssetLoaderKind::Sound
        .supported_extensions()
        .contains(&extension)
    {
        Some("audio")
    } else if AssetLoaderKind::Font
        .supported_extensions()
        .contains(&extension)
    {
        Some("font")
    } else if AssetLoaderKind::Shader
        .supported_extensions()
        .contains(&extension)
    {
        Some("shader")
    } else if AssetLoaderKind::Video
        .supported_extensions()
        .contains(&extension)
    {
        Some("video")
    } else {
        None
    }
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
