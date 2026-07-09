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
local theme = music "assets/theme.ogg"
local mainFont = font("assets/ui.ttf", 18)
local wave = shader "assets/wave.glsl"
local data = imageData "assets/player.png"
"#;
    let path = temp.write("src/main.tua", source);

    temp.write("assets/player.png", png_header(32, 16));
    temp.write("assets/readme.txt", b"not an image");
    temp.write("assets/theme.ogg", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");

    let parsed = parse_tua_source(&path, source).expect("source should parse");
    let assets = collect_asset_metadata(&parsed, std::slice::from_ref(&temp.path));

    assert_eq!(assets.len(), 7);
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
    assert_eq!(assets[3].kind, AssetLoaderKind::Music);
    assert_eq!(assets[3].inferred_love_type, "Source");
    assert_eq!(assets[3].status, AssetValidationStatus::Valid);
    assert_eq!(assets[4].kind, AssetLoaderKind::Font);
    assert_eq!(assets[4].inferred_love_type, "Font");
    assert_eq!(assets[4].status, AssetValidationStatus::Valid);
    assert_eq!(assets[5].kind, AssetLoaderKind::Shader);
    assert_eq!(assets[5].inferred_love_type, "Shader");
    assert_eq!(assets[5].status, AssetValidationStatus::Valid);
    assert_eq!(assets[6].kind, AssetLoaderKind::ImageData);
    assert_eq!(assets[6].inferred_love_type, "ImageData");
    assert_eq!(assets[6].dimensions, Some((32, 16)));
}

#[test]
fn completes_asset_paths_by_kind_and_prefix() {
    let temp = TempProject::new("completion");

    temp.write("assets/player.png", b"placeholder");
    temp.write("assets/jump.wav", b"placeholder");
    temp.write("assets/theme.ogg", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");
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

    assert_eq!(image_paths, vec!["assets/player.png".to_owned()]);
    assert_eq!(sound_paths, vec!["assets/jump.wav".to_owned()]);
    assert_eq!(music_paths, vec!["assets/theme.ogg".to_owned()]);
    assert_eq!(font_paths, vec!["assets/ui.ttf".to_owned()]);
    assert_eq!(shader_paths, vec!["assets/wave.glsl".to_owned()]);
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
