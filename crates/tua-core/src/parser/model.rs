use std::path::PathBuf;

use full_moon::tokenizer::Position;

use crate::classes::ClassDeclaration;

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
    pub raw: RawParsedSource,
    pub typed: TypedSourceModel,
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
    pub(crate) class_declarations: Vec<ClassDeclaration>,
}

impl ParsedSource {
    pub(crate) fn from_raw_and_typed(raw: RawParsedSource, typed: TypedSourceModel) -> Self {
        Self {
            path: raw.path.clone(),
            source: raw.source.clone(),
            ast: raw.ast.clone(),
            span: raw.span,
            lua_doc_annotations: typed.lua_doc_annotations.clone(),
            lua_doc_diagnostics: typed.lua_doc_diagnostics.clone(),
            type_aliases: typed.type_aliases.clone(),
            local_annotations: typed.local_annotations.clone(),
            function_annotations: typed.function_annotations.clone(),
            compound_assignments: typed.compound_assignments.clone(),
            class_declarations: typed.class_declarations.clone(),
            raw,
            typed,
        }
    }

    pub fn statement_count(&self) -> usize {
        self.ast.nodes().stmts().count()
    }

    pub fn has_return(&self) -> bool {
        self.ast.nodes().last_stmt().is_some()
    }
}

#[derive(Clone, Debug)]
pub struct RawParsedSource {
    pub path: PathBuf,
    pub source: String,
    pub parser_source: String,
    pub ast: full_moon::ast::Ast,
    pub span: Option<SourceSpan>,
    pub language: ParsedLanguage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedSource {
    pub original: String,
    pub parser_source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypedSourceModel {
    pub lua_doc_annotations: Vec<LuaDocAnnotation>,
    pub lua_doc_diagnostics: Vec<crate::Diagnostic>,
    pub type_aliases: Vec<TypeAlias>,
    pub local_annotations: Vec<LocalAnnotation>,
    pub function_annotations: Vec<FunctionAnnotation>,
    pub compound_assignments: Vec<CompoundAssignment>,
    pub(crate) class_declarations: Vec<ClassDeclaration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParsedLanguage {
    Lua,
    Tua,
}

impl RawParsedSource {
    pub fn statement_count(&self) -> usize {
        self.ast.nodes().stmts().count()
    }

    pub fn has_return(&self) -> bool {
        self.ast.nodes().last_stmt().is_some()
    }

    pub fn uses_normalized_parser_source(&self) -> bool {
        self.source != self.parser_source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TuaType,
    pub kind: TypeAliasKind,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAliasKind {
    Alias,
    Enum,
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
    pub required_parameter_count: usize,
    pub has_varargs: bool,
    pub default_parameters: Vec<ParameterDefault>,
    pub optional_parameters: Vec<ParameterOptional>,
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
pub struct ParameterDefault {
    pub name: String,
    pub expression: String,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterOptional {
    pub name: String,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TuaType {
    Named(String),
    StringLiteral(String),
    Union(Vec<TuaType>),
    Table(Vec<TableFieldType>),
    Array(Box<TuaType>),
    Tuple(Vec<TuaType>),
    Dictionary {
        key: Box<TuaType>,
        value: Box<TuaType>,
    },
    Function {
        parameters: Vec<FunctionTypeParameter>,
        returns: Vec<TuaType>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionTypeParameter {
    pub name: Option<String>,
    pub ty: TuaType,
    pub optional: bool,
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
    Enum(LuaDocEnum),
    Meta(LuaDocMeta),
    Module(LuaDocModule),
    Diagnostic(LuaDocDiagnostic),
    Cast(LuaDocCast),
    As(LuaDocAs),
    Overload(LuaDocOverload),
    Vararg(LuaDocVararg),
    Deprecated(LuaDocDeprecated),
    NoDiscard(LuaDocNoDiscard),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocEnum {
    pub name: Option<String>,
    pub name_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocMeta {
    pub expression: Option<String>,
    pub expression_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocModule {
    pub module: String,
    pub module_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocDiagnostic {
    pub expression: String,
    pub expression_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocCast {
    pub expression: String,
    pub expression_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocAs {
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocOverload {
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocVararg {
    pub ty: TuaType,
    pub type_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocDeprecated;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaDocNoDiscard;
