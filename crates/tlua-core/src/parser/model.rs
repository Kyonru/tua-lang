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
    pub ty: TluaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalAnnotation {
    pub name: String,
    pub ty: TluaType,
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
    pub return_type: Option<TluaType>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterAnnotation {
    pub name: String,
    pub ty: TluaType,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TluaType {
    Named(String),
    Union(Vec<TluaType>),
    Table(Vec<TableFieldType>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableFieldType {
    pub name: String,
    pub optional: bool,
    pub ty: TluaType,
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
