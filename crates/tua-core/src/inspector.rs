use std::path::PathBuf;

use crate::{AssetValidationStatus, Diagnostic, SourceSpan};

#[derive(Clone, Debug, PartialEq)]
pub enum InspectorItem {
    SpriteSheet(SpriteSheetInspection),
}

impl InspectorItem {
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::SpriteSheet(sprite) => sprite.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteSheetInspection {
    pub name: String,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
    pub image: Option<SpriteAssetInspection>,
    pub frame: Option<SpriteFrameInspection>,
    pub animations: Vec<SpriteAnimationInspection>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpriteAssetInspection {
    pub path: String,
    pub resolved_path: Option<PathBuf>,
    pub status: AssetValidationStatus,
    pub dimensions: Option<(u32, u32)>,
    pub span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpriteFrameInspection {
    pub width: u32,
    pub height: u32,
    pub left: u32,
    pub top: u32,
    pub border: u32,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteAnimationInspection {
    pub name: String,
    pub from: u32,
    pub to: u32,
    pub row: u32,
    pub column: Option<u32>,
    pub frame: Option<SpriteFrameInspection>,
    pub fps: f64,
    pub durations: Option<String>,
    pub on_loop: Option<String>,
    pub looping: bool,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}
