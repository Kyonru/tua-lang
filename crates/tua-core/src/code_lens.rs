use std::path::PathBuf;

use crate::{SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeLens {
    pub range: SourceSpan,
    pub target: CodeLensTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeLensTarget {
    References(CodeLensReferenceTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeLensReferenceTarget {
    pub path: PathBuf,
    pub position: SourcePosition,
    pub name: String,
    pub kind: CodeLensReferenceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeLensReferenceKind {
    ExportedFunction,
    TypeAlias,
}
