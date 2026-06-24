use crate::types::{FunctionType, Type};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AssetLoaderKind {
    Image,
    Sound,
}

impl AssetLoaderKind {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Sound => "sound",
        }
    }

    pub(crate) const fn emitted_callee(self) -> &'static str {
        match self {
            Self::Image => "love.graphics.newImage",
            Self::Sound => "love.audio.newSource",
        }
    }

    pub(crate) fn emitted_call(self, path_literal: &str) -> String {
        match self {
            Self::Image => format!("{}({path_literal})", self.emitted_callee()),
            Self::Sound => format!("{}({path_literal}, \"static\")", self.emitted_callee()),
        }
    }

    pub(crate) fn return_type(self) -> Type {
        match self {
            Self::Image => Type::alias("Image"),
            Self::Sound => Type::alias("Source"),
        }
    }

    pub(crate) fn function_type(self) -> FunctionType {
        FunctionType::new([Type::string()], self.return_type())
    }

    pub(crate) const fn detail(self) -> &'static str {
        match self {
            Self::Image => "function image(path: string): Image",
            Self::Sound => "function sound(path: string): Source",
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
        }
    }

    pub(crate) const fn supported_extensions(self) -> &'static [&'static str] {
        match self {
            Self::Image => &["bmp", "exr", "hdr", "jpeg", "jpg", "pic", "png", "tga"],
            Self::Sound => &["flac", "mp3", "ogg", "wav"],
        }
    }

    pub(crate) const fn asset_label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Sound => "sound",
        }
    }
}

pub(crate) fn loader_kind(name: &str) -> Option<AssetLoaderKind> {
    match name {
        "image" => Some(AssetLoaderKind::Image),
        "sound" => Some(AssetLoaderKind::Sound),
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
        "Quad",
        "Shader",
        "Source",
        "SpriteBatch",
        "World",
    ]
}
