use std::path::PathBuf;

use crate::{SourceSpan, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeExplanation {
    pub expression: String,
    pub ty: Type,
    pub span: SourceSpan,
    pub steps: Vec<TypeExplanationStep>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeExplanationStep {
    pub kind: TypeExplanationStepKind,
    pub title: String,
    pub detail: String,
    pub ty: Option<Type>,
    pub path: Option<PathBuf>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeExplanationStepKind {
    Annotation,
    Inference,
    Widening,
    Narrowing,
    Module,
    Field,
    Dynamic,
    Unresolved,
}
