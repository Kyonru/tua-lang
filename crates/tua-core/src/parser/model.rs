use std::path::PathBuf;

use full_moon::tokenizer::Position;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    pub(crate) fn from_full_moon(position: Position) -> Self {
        Self {
            line: position.line().max(1),
            column: position.character().max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    pub(crate) fn from_range(start: Position, end: Position) -> Self {
        Self {
            start: SourcePosition::from_full_moon(start),
            end: SourcePosition::from_full_moon(end),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedSource {
    pub path: PathBuf,
    pub source: String,
    pub ast: full_moon::ast::Ast,
    pub span: Option<SourceSpan>,
    pub lua_doc_annotations: Vec<LuaDocAnnotation>,
    pub lua_doc_diagnostics: Vec<crate::Diagnostic>,
    pub type_aliases: Vec<TypeAlias>,
    pub local_annotations: Vec<LocalAnnotation>,
    pub function_annotations: Vec<FunctionAnnotation>,
    pub compound_assignments: Vec<CompoundAssignment>,
}

impl ParsedSource {
    pub fn statement_count(&self) -> usize {
        self.ast.nodes().stmts().count()
    }

    pub fn has_return(&self) -> bool {
        self.ast.nodes().last_stmt().is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TuaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAnnotation {
    pub name: String,
    pub ty: TuaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAnnotation {
    pub name: Option<String>,
    pub name_span: Option<SourceSpan>,
    pub parameters: Vec<ParameterAnnotation>,
    pub parameter_count: usize,
    pub has_varargs: bool,
    pub return_type: Option<TuaType>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterAnnotation {
    pub name: String,
    pub ty: TuaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TuaType {
    Named(String),
    Union(Vec<TuaType>),
    Table(Vec<TableFieldType>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableFieldType {
    pub name: String,
    pub optional: bool,
    pub ty: TuaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompoundAssignment {
    pub lhs: Vec<String>,
    pub operator: CompoundOperator,
    pub rhs: String,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompoundOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

impl CompoundOperator {
    pub fn as_sugar(self) -> &'static str {
        match self {
            Self::Add => "+=",
            Self::Subtract => "-=",
            Self::Multiply => "*=",
            Self::Divide => "/=",
            Self::Modulo => "%=",
        }
    }

    pub fn as_binary(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Modulo => "%",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LuaDocParseOptions {
    pub diagnostics_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocParseResult {
    pub annotations: Vec<LuaDocAnnotation>,
    pub diagnostics: Vec<crate::Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocAnnotation {
    pub kind: LuaDocAnnotationKind,
    pub raw: String,
    pub documentation: Option<String>,
    pub span: SourceSpan,
    pub tag_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LuaDocAnnotationKind {
    Class(LuaDocClass),
    Field(LuaDocField),
    Alias(LuaDocAlias),
    Type(LuaDocTypeAnnotation),
    Param(LuaDocParam),
    Return(LuaDocReturn),
    Generic(LuaDocGeneric),
    Version(LuaDocVersion),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocClass {
    pub name: String,
    pub name_span: SourceSpan,
    pub extends: Option<String>,
    pub extends_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocField {
    pub name: String,
    pub name_span: SourceSpan,
    pub optional: bool,
    pub visibility: Option<String>,
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocAlias {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocTypeAnnotation {
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocParam {
    pub name: String,
    pub name_span: SourceSpan,
    pub optional: bool,
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocReturn {
    pub returns: Vec<LuaDocReturnType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocReturnType {
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocGeneric {
    pub parameters: Vec<LuaDocGenericParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocGenericParameter {
    pub name: String,
    pub name_span: SourceSpan,
    pub bound: Option<TuaType>,
    pub bound_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocVersion {
    pub expression: String,
    pub expression_span: SourceSpan,
}
