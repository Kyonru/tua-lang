use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::parse_tua_source;

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "tua-asset-metadata-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("failed to create temp project");

        Self { path }
    }

    fn write(&self, path: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }

        fs::write(&path, contents).expect("failed to write fixture file");
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn collects_love_asset_metadata_with_validation_status() {
    let temp = TempProject::new("metadata");
    let source = r#"
local player = image "assets/player.png"
local missing = sound "assets/missing.wav"
local wrong = image "assets/readme.txt"
local wrongKind = image "assets/jump.wav"
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
local raw = love.graphics.newImage("assets/player.png")
local streamed = love.audio.newSource("assets/theme.ogg", "stream")
local fontImage = love.graphics.newImageFont("assets/font.png", "abc")
local soundData = love.sound.newSoundData("assets/jump.wav")
local video = love.graphics.newVideo("assets/intro.ogv")
"#;
    let path = temp.write("src/main.tua", source);

    temp.write("assets/player.png", png_header(32, 16));
    temp.write("assets/font.png", png_header(8, 8));
    temp.write("assets/jump.wav", b"placeholder");
    temp.write("assets/readme.txt", b"not an image");
    temp.write("assets/theme.ogg", b"placeholder");
    temp.write("assets/intro.ogv", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");

    let parsed = parse_tua_source(&path, source).expect("source should parse");
    let assets = collect_asset_metadata(&parsed, std::slice::from_ref(&temp.path));

    assert_eq!(assets.len(), 13);
    assert_eq!(assets[0].kind, AssetLoaderKind::Image);
    assert_eq!(assets[0].relative_path, "assets/player.png");
    assert_eq!(assets[0].extension.as_deref(), Some("png"));
    assert_eq!(assets[0].inferred_love_type, "Image");
    assert_eq!(assets[0].status, AssetValidationStatus::Valid);
    assert_eq!(assets[0].dimensions, Some((32, 16)));
    assert_eq!(assets[1].status, AssetValidationStatus::Missing);
    assert_eq!(
        assets[2].status,
        AssetValidationStatus::UnsupportedExtension
    );
    assert_eq!(assets[3].status, AssetValidationStatus::WrongKind);
    assert_eq!(assets[4].kind, AssetLoaderKind::Music);
    assert_eq!(assets[4].inferred_love_type, "Source");
    assert_eq!(assets[4].status, AssetValidationStatus::Valid);
    assert_eq!(assets[5].kind, AssetLoaderKind::Font);
    assert_eq!(assets[5].inferred_love_type, "Font");
    assert_eq!(assets[5].status, AssetValidationStatus::Valid);
    assert_eq!(assets[6].kind, AssetLoaderKind::Shader);
    assert_eq!(assets[6].inferred_love_type, "Shader");
    assert_eq!(assets[6].status, AssetValidationStatus::Valid);
    assert_eq!(assets[7].kind, AssetLoaderKind::ImageData);
    assert_eq!(assets[7].inferred_love_type, "ImageData");
    assert_eq!(assets[7].dimensions, Some((32, 16)));
    assert_eq!(assets[8].kind, AssetLoaderKind::Image);
    assert_eq!(assets[8].dimensions, Some((32, 16)));
    assert_eq!(assets[9].kind, AssetLoaderKind::Music);
    assert_eq!(assets[10].kind, AssetLoaderKind::ImageFont);
    assert_eq!(assets[10].inferred_love_type, "Font");
    assert_eq!(assets[11].kind, AssetLoaderKind::SoundData);
    assert_eq!(assets[11].inferred_love_type, "SoundData");
    assert_eq!(assets[12].kind, AssetLoaderKind::Video);
    assert_eq!(assets[12].inferred_love_type, "Video");
}

#[test]
fn completes_asset_paths_by_kind_and_prefix() {
    let temp = TempProject::new("completion");

    temp.write("assets/player.png", b"placeholder");
    temp.write("assets/jump.wav", b"placeholder");
    temp.write("assets/theme.ogg", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");
    temp.write("assets/intro.ogv", b"placeholder");
    temp.write("assets/readme.txt", b"placeholder");

    let image_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Image,
        "assets/p",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();
    let sound_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Sound,
        "assets/j",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();
    let music_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Music,
        "assets/t",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();
    let font_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Font,
        "assets/u",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();
    let shader_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Shader,
        "assets/w",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();
    let video_paths = complete_asset_paths(
        std::slice::from_ref(&temp.path),
        AssetLoaderKind::Video,
        "assets/i",
    )
    .into_iter()
    .map(|candidate| candidate.path)
    .collect::<Vec<_>>();

    assert_eq!(image_paths, vec!["assets/player.png".to_owned()]);
    assert_eq!(sound_paths, vec!["assets/jump.wav".to_owned()]);
    assert_eq!(music_paths, vec!["assets/theme.ogg".to_owned()]);
    assert_eq!(font_paths, vec!["assets/ui.ttf".to_owned()]);
    assert_eq!(shader_paths, vec!["assets/wave.glsl".to_owned()]);
    assert_eq!(video_paths, vec!["assets/intro.ogv".to_owned()]);
}

#[test]
fn finds_asset_metadata_at_source_position() {
    let temp = TempProject::new("position");
    let source = r#"local player = image "assets/player.png"
"#;
    let path = temp.write("src/main.tua", source);

    temp.write("assets/player.png", b"placeholder");

    let parsed = parse_tua_source(&path, source).expect("source should parse");
    let assets = collect_asset_metadata(&parsed, std::slice::from_ref(&temp.path));
    let asset = asset_at_position(
        &assets,
        SourcePosition {
            line: 1,
            column: 23,
        },
    )
    .expect("asset should be found at path literal");

    assert_eq!(asset.relative_path, "assets/player.png");
}

#[test]
fn indexed_validation_reports_canonical_path_casing() {
    let temp = TempProject::new("path-case");
    let source = r#"local player = image "Assets/Player.PNG"
"#;
    let path = temp.write("src/main.tua", source);

    temp.write("assets/player.png", b"placeholder");

    let parsed = parse_tua_source(&path, source).expect("source should parse");
    let index = AssetIndex::from_roots(std::slice::from_ref(&temp.path));
    let assets =
        collect_asset_metadata_with_index(&parsed, std::slice::from_ref(&temp.path), Some(&index));
    let asset = assets.first().expect("asset metadata should be collected");

    assert_eq!(asset.status, AssetValidationStatus::PathCaseMismatch);
    assert_eq!(asset.canonical_path.as_deref(), Some("assets/player.png"));
    assert_eq!(
        resolved_asset_path(asset, std::slice::from_ref(&temp.path), Some(&index)),
        Some(temp.path.join("assets/player.png"))
    );
}

#[test]
fn collects_sprite_image_asset_metadata() {
    let temp = TempProject::new("sprite-metadata");
    let source = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
  },
})
"#;
    let path = temp.write("src/sprites.tua", source);

    temp.write("assets/player.png", png_header(32, 16));

    let parsed = parse_tua_source(&path, source).expect("sprite source should parse");
    let assets = collect_asset_metadata(&parsed, std::slice::from_ref(&temp.path));
    let asset = assets
        .iter()
        .find(|asset| asset.relative_path == "assets/player.png")
        .expect("sprite image should be collected as asset metadata");

    assert_eq!(asset.kind, AssetLoaderKind::Image);
    assert_eq!(asset.status, AssetValidationStatus::Valid);
    assert_eq!(asset.dimensions, Some((32, 16)));
    assert_eq!(
        asset_at_position(
            &assets,
            SourcePosition {
                line: 2,
                column: 11
            },
        )
        .map(|asset| asset.relative_path.as_str()),
        Some("assets/player.png")
    );
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());

    bytes
}

#[test]
fn lists_shader_uniform_declarations() {
    let uniforms = shader_uniforms(
        r#"#pragma language glsl3
// uniform float commented;
/* uniform vec4 blockComment; */
extern number time;
uniform highp vec4 first, second;
uniform Image layers[2];
const float scale = 2.0;
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) { return color * scale; }
"#,
    )
    .expect("plain declarations should be listed");

    assert_eq!(
        uniforms.names.into_iter().collect::<Vec<_>>(),
        ["first", "layers", "second", "time"]
    );
    assert_eq!(shader_uniforms("uniform Lights { vec4 color; };"), None);
    assert_eq!(
        shader_uniforms("#define PARAM(name) uniform float name;\nPARAM(time)"),
        None
    );
}

#[test]
fn finds_shader_uniforms_that_no_code_reads() {
    let uniforms = shader_uniforms(
        r#"extern number time;
extern number unused; // unused stays unread even when a comment names it
uniform vec4 tint;
#ifdef VERTEX
uniform float sway;
vec4 position(mat4 transform, vec4 vertex) { return transform * vertex; }
#endif
#ifdef PIXEL
uniform float sway;
vec4 effect(vec4 color, Image tex, vec2 uv, vec2 screen) {
  return color * tint * sin(time);
}
#endif
"#,
    )
    .expect("plain declarations should be listed");

    assert_eq!(
        uniforms.unread.into_iter().collect::<Vec<_>>(),
        ["sway", "unused"]
    );
}

/// The shader fixtures and what the uniform parser finds in each. LOVE's own
/// results for the same files are recorded in
/// internal/shader-editor-support.md and reproduced by
/// `love scripts/love-shader-facts`.
#[test]
fn parses_the_shader_fixture_corpus() {
    macro_rules! fixture {
        ($name:literal) => {
            (
                $name,
                include_str!(concat!("fixtures/shaders/", $name, ".glsl")),
            )
        };
    }
    // (file name, source), declared uniforms, unread uniforms.
    type Expected = (
        (&'static str, &'static str),
        &'static [&'static str],
        &'static [&'static str],
    );
    let expected: [Expected; 11] = [
        (
            fixture!("arrays"),
            &["colorMatrix", "lights", "weights"],
            &[],
        ),
        (fixture!("compile_error"), &[], &[]),
        (fixture!("gles_only_error"), &[], &[]),
        (fixture!("glsl3"), &["frame"], &[]),
        (fixture!("love_builtins"), &[], &[]),
        (fixture!("missing_entry_point"), &["time"], &[]),
        (fixture!("multi_canvas"), &[], &[]),
        (fixture!("pixel_basic"), &["time", "tint"], &[]),
        (
            fixture!("unread_uniform"),
            &["strength", "time"],
            &["strength"],
        ),
        (fixture!("vertex_pixel"), &["sway"], &[]),
        (
            fixture!("water"),
            &["horizonY", "ripple1", "ripple2", "ripple3", "time"],
            &[],
        ),
    ];

    for ((name, source), names, unread) in expected {
        let uniforms =
            shader_uniforms(source).unwrap_or_else(|| panic!("{name} should list its uniforms"));
        assert_eq!(
            uniforms
                .names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            names,
            "{name} names"
        );
        assert_eq!(
            uniforms
                .unread
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            unread,
            "{name} unread"
        );
    }
}

#[test]
fn shader_uniform_model_keeps_types_stage_locations_and_comment_offsets() {
    let source = "#pragma language glsl3\r\n/* λ comment */ extern highp vec4 tint[4];\r\n#ifdef VERTEX\r\nuniform vec4 both;\r\nvec4 position(mat4 m, vec4 p) { return m * p + both; }\r\n#endif\r\n#ifdef PIXEL\r\nuniform vec4 both;\r\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return both + tint[0]; }\r\n#endif\r\n";
    let model = shader_uniforms(source).unwrap();
    assert!(model.has_effect && model.has_position);
    let tint = &model.uniforms["tint"];
    assert_eq!(tint.glsl_type, "vec4");
    assert_eq!(tint.array_len, Some(4));
    assert!(tint.read);
    let span = tint.declarations[0];
    let line = source.lines().nth(span.start.line - 1).unwrap();
    assert_eq!(
        line.chars()
            .skip(span.start.column - 1)
            .take(span.end.column - span.start.column)
            .collect::<String>(),
        "tint"
    );
    assert_eq!(
        model.uniforms["both"]
            .declarations
            .iter()
            .map(|span| span.start.line)
            .collect::<Vec<_>>(),
        [4, 8]
    );
    let source = "extern/*comment\nmore*/number time;\nvoid effect() {}";
    let model = shader_uniforms(source).unwrap();
    assert_eq!(model.uniforms["time"].declarations[0].start.line, 2);
    assert!(model.has_effect);
}

#[test]
fn shader_source_cache_reuses_unchanged_models_and_invalidates_changed_source() {
    let first = ShaderSource::from_source("extern number time;".into(), None).unwrap();
    let same = ShaderSource::from_source("extern number time;".into(), Some(&first)).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        first.uniforms.as_ref().unwrap(),
        same.uniforms.as_ref().unwrap()
    ));
    let changed = ShaderSource::from_source("extern vec4 tint;".into(), Some(&first)).unwrap();
    assert_ne!(first.fingerprint, changed.fingerprint);
    assert!(changed.uniforms.unwrap().names.contains("tint"));
}
