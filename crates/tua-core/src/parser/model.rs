use std::path::PathBuf;

use full_moon::tokenizer::Position;

use crate::classes::ClassDeclaration;
use crate::sprite::SpriteDeclaration;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeArrayTypeSyntax {
    pub item_end: SourcePosition,
    pub suffix_span: SourceSpan,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParserLineProvenance {
    pub source_fragment: Option<ParserSourceFragment>,
    pub source_span: SourceSpan,
    pub parser_start_column: usize,
    pub class_generated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParserSourceFragment {
    pub source_span: SourceSpan,
    pub parser_start_column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParserPositionOrigin {
    pub source_position: SourcePosition,
    pub class_generated: bool,
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
    pub type_imports: Vec<TypeImport>,
    pub local_annotations: Vec<LocalAnnotation>,
    pub function_annotations: Vec<FunctionAnnotation>,
    pub compound_assignments: Vec<CompoundAssignment>,
    pub(crate) parameter_default_references: Vec<ParameterDefaultReference>,
    pub(crate) sprite_declarations: Vec<SpriteDeclaration>,
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
            type_imports: typed.type_imports.clone(),
            local_annotations: typed.local_annotations.clone(),
            function_annotations: typed.function_annotations.clone(),
            compound_assignments: typed.compound_assignments.clone(),
            parameter_default_references: typed.parameter_default_references.clone(),
            sprite_declarations: typed.sprite_declarations.clone(),
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
    /// Class-lowered text before type/default erasure; absent when source is unchanged.
    pub(crate) syntax_source: Option<String>,
    pub(crate) parser_provenance: Vec<ParserLineProvenance>,
    pub(crate) native_array_types: Vec<NativeArrayTypeSyntax>,
    pub ast: full_moon::ast::Ast,
    pub span: Option<SourceSpan>,
    pub language: ParsedLanguage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedSource {
    pub original: String,
    pub parser_source: String,
    /// Class-lowered text before type/default erasure; absent when source is unchanged.
    pub(crate) syntax_source: Option<String>,
    pub(crate) parser_provenance: Vec<ParserLineProvenance>,
    pub(crate) native_array_types: Vec<NativeArrayTypeSyntax>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypedSourceModel {
    pub lua_doc_annotations: Vec<LuaDocAnnotation>,
    pub lua_doc_diagnostics: Vec<crate::Diagnostic>,
    pub type_aliases: Vec<TypeAlias>,
    pub type_imports: Vec<TypeImport>,
    pub local_annotations: Vec<LocalAnnotation>,
    pub function_annotations: Vec<FunctionAnnotation>,
    pub compound_assignments: Vec<CompoundAssignment>,
    pub(crate) parameter_default_references: Vec<ParameterDefaultReference>,
    pub(crate) sprite_declarations: Vec<SpriteDeclaration>,
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

    pub(crate) fn parser_position_for_source(
        &self,
        source_position: SourcePosition,
    ) -> Option<SourcePosition> {
        self.parser_provenance
            .iter()
            .enumerate()
            .find_map(|(line, origin)| {
                if let Some(fragment) = origin.source_fragment {
                    let span = fragment.source_span;
                    if span.start.line == source_position.line
                        && span.start.column <= source_position.column
                        && source_position.column <= span.end.column
                    {
                        return Some(SourcePosition {
                            line: line + 1,
                            column: source_position.column - span.start.column
                                + fragment.parser_start_column,
                        });
                    }
                }
                (!origin.class_generated
                    && origin.source_span.start.line == source_position.line
                    && origin.source_span.start.column <= source_position.column
                    && source_position.column <= origin.source_span.end.column)
                    .then(|| SourcePosition {
                        line: line + 1,
                        column: source_position.column - origin.source_span.start.column
                            + origin.parser_start_column,
                    })
            })
    }

    pub(crate) fn parser_position_origin(
        &self,
        parser_position: SourcePosition,
    ) -> Option<ParserPositionOrigin> {
        let provenance = self
            .parser_provenance
            .get(parser_position.line.saturating_sub(1))?;
        if let Some(fragment) = provenance.source_fragment {
            let width = fragment.source_span.end.column - fragment.source_span.start.column;
            if parser_position.column >= fragment.parser_start_column
                && parser_position.column <= fragment.parser_start_column + width
            {
                return Some(ParserPositionOrigin {
                    source_position: SourcePosition {
                        line: fragment.source_span.start.line,
                        column: fragment.source_span.start.column + parser_position.column
                            - fragment.parser_start_column,
                    },
                    class_generated: false,
                });
            }
        }
        let source_position = if provenance.class_generated {
            provenance.source_span.start
        } else {
            SourcePosition {
                line: provenance.source_span.start.line,
                column: provenance.source_span.start.column.saturating_add(
                    parser_position
                        .column
                        .saturating_sub(provenance.parser_start_column),
                ),
            }
        };

        Some(ParserPositionOrigin {
            source_position,
            class_generated: provenance.class_generated,
        })
    }

    pub(crate) fn remap_parser_diagnostic(&self, diagnostic: &mut crate::Diagnostic) {
        let parser_start = SourcePosition {
            line: diagnostic.line,
            column: diagnostic.column,
        };
        let parser_end = SourcePosition {
            line: diagnostic.end_line,
            column: diagnostic.end_column,
        };
        let Some(start) = self.parser_position_origin(parser_start) else {
            return;
        };
        let end = self.parser_position_origin(parser_end).unwrap_or(start);
        let source_start = start.source_position;
        let source_end = if start.class_generated || end.class_generated {
            SourcePosition {
                line: source_start.line,
                column: source_start.column.saturating_add(1),
            }
        } else {
            end.source_position
        };

        diagnostic.line = source_start.line;
        diagnostic.column = source_start.column;
        diagnostic.end_line = source_end.line.max(source_start.line);
        diagnostic.end_column = if diagnostic.end_line == source_start.line {
            source_end.column.max(source_start.column.saturating_add(1))
        } else {
            source_end.column.max(1)
        };
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: TuaType,
    pub kind: TypeAliasKind,
    pub name_span: Option<SourceSpan>,
    pub span: Option<SourceSpan>,
    pub exported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeImport {
    pub namespace: String,
    pub bindings: Vec<TypeImportBinding>,
    pub reexport: bool,
    pub module: String,
    pub namespace_span: SourceSpan,
    pub module_span: SourceSpan,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeImportBinding {
    pub imported: String,
    pub local: String,
    pub imported_span: SourceSpan,
    pub local_span: SourceSpan,
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
    pub signature_parameters: Vec<FunctionSignatureParameter>,
    pub parameter_count: usize,
    pub required_parameter_count: usize,
    pub has_varargs: bool,
    pub vararg_type: Option<TuaType>,
    pub default_parameters: Vec<ParameterDefault>,
    pub optional_parameters: Vec<ParameterOptional>,
    pub return_type: Option<TuaType>,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignatureParameter {
    pub name: String,
    pub ty: Option<TuaType>,
    pub name_span: Option<SourceSpan>,
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
pub(crate) struct ParameterDefaultReference {
    pub function_span: SourceSpan,
    pub name: String,
    pub position: SourcePosition,
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
    pub variadic: bool,
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
