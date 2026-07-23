mod dialect;
mod doc;
mod model;
mod normalize;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use full_moon::ast::luau::{TypeArgument, TypeDeclaration, TypeField, TypeFieldKey, TypeInfo};
use full_moon::ast::{self, LuaVersion};
use full_moon::node::Node;

use crate::ast_utils::plain_field_chain_from_var;
use crate::classes;
use crate::source_text::{byte_for_column, column_for_byte};
use crate::{Diagnostic, DiagnosticCode};
pub(crate) use doc::parse_type_text as parse_tua_type_text;
pub use doc::{parse_lua_doc_annotations, parse_lua_doc_annotations_with_options};
pub(crate) use model::ParserLineProvenance;
pub use model::{
    CompoundAssignment, CompoundOperator, FunctionAnnotation, FunctionTypeParameter,
    LocalAnnotation, LuaDocAlias, LuaDocAnnotation, LuaDocAnnotationKind, LuaDocAs, LuaDocCast,
    LuaDocClass, LuaDocDeprecated, LuaDocDiagnostic, LuaDocEnum, LuaDocField, LuaDocGeneric,
    LuaDocGenericParameter, LuaDocMeta, LuaDocModule, LuaDocNoDiscard, LuaDocOverload, LuaDocParam,
    LuaDocParseOptions, LuaDocParseResult, LuaDocReturn, LuaDocReturnType, LuaDocTypeAnnotation,
    LuaDocVararg, LuaDocVersion, NormalizedSource, ParameterAnnotation, ParameterDefault,
    ParameterOptional, ParsedLanguage, ParsedSource, RawParsedSource, SourcePosition, SourceSpan,
    TableFieldType, TuaType, TypeAlias, TypeAliasKind, TypedSourceModel,
};
use normalize::normalize_tua_source_for_luau_parser;

pub fn parse_lua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let raw = parse_raw_lua_source(path, source)?;
    let typed = extract_lua_source_model(&raw);

    Ok(ParsedSource::from_raw_and_typed(raw, typed))
}

pub fn parse_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let raw = parse_raw_tua_source(path, source)?;
    let typed = extract_tua_source_model(&raw)?;

    Ok(ParsedSource::from_raw_and_typed(raw, typed))
}

pub fn normalize_tua_source(source: impl Into<String>) -> NormalizedSource {
    let original = source.into();
    let (parser_source, parser_provenance) = normalize_tua_source_for_luau_parser(&original);

    NormalizedSource {
        original,
        parser_source,
        parser_provenance,
    }
}

pub fn parse_raw_lua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<RawParsedSource, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let source = source.into();
    let parser_source = source.clone();
    let parser_provenance = source_line_provenance(&source);

    parse_raw_source(
        path,
        source,
        parser_source,
        parser_provenance,
        LuaVersion::lua51(),
        ParsedLanguage::Lua,
    )
}

pub fn parse_raw_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<RawParsedSource, Vec<Diagnostic>> {
    let path = path.as_ref().to_path_buf();
    let normalized = normalize_tua_source(source);

    let raw = parse_raw_source(
        path,
        normalized.original,
        normalized.parser_source,
        normalized.parser_provenance,
        LuaVersion::luau(),
        ParsedLanguage::Tua,
    )?;
    let diagnostics = dialect::validate_tua_dialect(&raw);

    if diagnostics.is_empty() {
        Ok(raw)
    } else {
        Err(diagnostics)
    }
}

pub fn extract_lua_source_model(raw: &RawParsedSource) -> TypedSourceModel {
    let doc_result = doc::collect_lua_doc_annotations(&raw.path, &raw.source);

    TypedSourceModel {
        lua_doc_annotations: doc_result.annotations,
        lua_doc_diagnostics: doc_result.diagnostics,
        ..TypedSourceModel::default()
    }
}

pub fn extract_tua_source_model(
    raw: &RawParsedSource,
) -> Result<TypedSourceModel, Vec<Diagnostic>> {
    let doc_result = doc::collect_lua_doc_annotations(&raw.path, &raw.source);
    let (class_declarations, class_diagnostics) =
        classes::parse_class_declarations(&raw.path, &raw.source);
    let (sprite_declarations, sprite_diagnostics) =
        crate::sprite::parse_sprite_declarations(&raw.ast);
    let syntax = collect_tua_syntax(&raw.path, &raw.source, &raw.ast);

    let mut diagnostics = class_diagnostics;
    diagnostics.extend(sprite_diagnostics);
    diagnostics.extend(syntax.diagnostics);

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(TypedSourceModel {
        lua_doc_annotations: doc_result.annotations,
        lua_doc_diagnostics: doc_result.diagnostics,
        type_aliases: syntax.type_aliases,
        local_annotations: syntax.local_annotations,
        function_annotations: syntax.function_annotations,
        compound_assignments: syntax.compound_assignments,
        parameter_default_references: syntax.parameter_default_references,
        sprite_declarations,
        class_declarations,
    })
}

fn parse_raw_source(
    path: PathBuf,
    source: String,
    parser_source: String,
    parser_provenance: Vec<ParserLineProvenance>,
    lua_version: LuaVersion,
    language: ParsedLanguage,
) -> Result<RawParsedSource, Vec<Diagnostic>> {
    match full_moon::parse_fallible(&parser_source, lua_version).into_result() {
        Ok(ast) => {
            let span = ast
                .range()
                .map(|(start, end)| SourceSpan::from_range(start, end));

            Ok(RawParsedSource {
                path,
                source,
                parser_source,
                parser_provenance,
                ast,
                span,
                language,
            })
        }
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| parser_error_to_diagnostic(&path, error))
            .collect()),
    }
}

fn source_line_provenance(source: &str) -> Vec<ParserLineProvenance> {
    let lines = if source.is_empty() {
        Vec::new()
    } else {
        source.split_inclusive('\n').collect::<Vec<_>>()
    };

    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line_number = index + 1;
            let text = line.trim_end_matches(['\r', '\n']);
            ParserLineProvenance {
                source_span: SourceSpan {
                    start: SourcePosition {
                        line: line_number,
                        column: 1,
                    },
                    end: SourcePosition {
                        line: line_number,
                        column: text.chars().count() + 1,
                    },
                },
                class_generated: false,
            }
        })
        .collect()
}

#[derive(Default)]
struct TuaSyntax {
    type_aliases: Vec<TypeAlias>,
    local_annotations: Vec<LocalAnnotation>,
    function_annotations: Vec<FunctionAnnotation>,
    compound_assignments: Vec<CompoundAssignment>,
    parameter_default_references: Vec<model::ParameterDefaultReference>,
    diagnostics: Vec<Diagnostic>,
}

fn collect_tua_syntax(path: &Path, source: &str, ast: &ast::Ast) -> TuaSyntax {
    let mut syntax = TuaSyntax::default();

    collect_block_tua_syntax(path, source, ast.nodes(), &mut syntax);
    collect_enum_declarations(path, source, &mut syntax);

    syntax
}

fn collect_block_tua_syntax(path: &Path, source: &str, block: &ast::Block, syntax: &mut TuaSyntax) {
    for stmt in block.stmts() {
        collect_stmt_tua_syntax(path, source, stmt, syntax);
    }
}

fn collect_stmt_tua_syntax(path: &Path, source: &str, stmt: &ast::Stmt, syntax: &mut TuaSyntax) {
    match stmt {
        ast::Stmt::TypeDeclaration(declaration) => {
            syntax
                .type_aliases
                .push(type_alias_from_declaration(declaration, source));
        }
        ast::Stmt::LocalAssignment(local) => {
            for (name, specifier) in local.names().iter().zip(local.type_specifiers()) {
                let Some(specifier) = specifier else {
                    continue;
                };

                syntax.local_annotations.push(LocalAnnotation {
                    name: token_text(name),
                    ty: tua_type_from_type_info(specifier.type_info()),
                    name_span: node_span(name),
                    span: node_span(specifier),
                });
            }
        }
        ast::Stmt::LocalFunction(function) => {
            let annotation = function_annotation(
                path,
                source,
                Some(token_text(function.name())),
                node_span(function.name()),
                function.body(),
                &mut syntax.diagnostics,
            );
            collect_parameter_default_references(source, &annotation, syntax);
            syntax.function_annotations.push(annotation);
            collect_block_tua_syntax(path, source, function.body().block(), syntax);
        }
        ast::Stmt::FunctionDeclaration(function) => {
            let annotation = function_annotation(
                path,
                source,
                Some(function.name().to_string()),
                node_span(function.name()),
                function.body(),
                &mut syntax.diagnostics,
            );
            collect_parameter_default_references(source, &annotation, syntax);
            syntax.function_annotations.push(annotation);
            collect_block_tua_syntax(path, source, function.body().block(), syntax);
        }
        ast::Stmt::CompoundAssignment(assignment) => {
            match compound_assignment_from_ast(path, assignment) {
                Ok(assignment) => syntax.compound_assignments.push(assignment),
                Err(diagnostic) => syntax.diagnostics.push(diagnostic),
            }
        }
        ast::Stmt::Do(block) => collect_block_tua_syntax(path, source, block.block(), syntax),
        ast::Stmt::If(if_stmt) => {
            collect_block_tua_syntax(path, source, if_stmt.block(), syntax);
            if let Some(else_ifs) = if_stmt.else_if() {
                for else_if in else_ifs {
                    collect_block_tua_syntax(path, source, else_if.block(), syntax);
                }
            }
            if let Some(else_block) = if_stmt.else_block() {
                collect_block_tua_syntax(path, source, else_block, syntax);
            }
        }
        ast::Stmt::GenericFor(for_stmt) => {
            collect_block_tua_syntax(path, source, for_stmt.block(), syntax);
        }
        ast::Stmt::NumericFor(for_stmt) => {
            collect_block_tua_syntax(path, source, for_stmt.block(), syntax);
        }
        ast::Stmt::Repeat(repeat) => collect_block_tua_syntax(path, source, repeat.block(), syntax),
        ast::Stmt::While(while_stmt) => {
            collect_block_tua_syntax(path, source, while_stmt.block(), syntax);
        }
        _ => {}
    }
}

fn type_alias_from_declaration(declaration: &TypeDeclaration, source: &str) -> TypeAlias {
    let ty = tua_type_from_type_info(declaration.type_definition());
    let mut span = node_span(declaration);
    if matches!(ty, TuaType::Table(_)) {
        span = span.map(|span| complete_table_alias_span(source, span));
    }

    TypeAlias {
        name: token_text(declaration.type_name()),
        ty,
        kind: TypeAliasKind::Alias,
        name_span: node_span(declaration.type_name()),
        span,
    }
}

fn complete_table_alias_span(source: &str, span: SourceSpan) -> SourceSpan {
    let mut depth = 0usize;
    let mut saw_open = false;

    for (index, line) in source.lines().enumerate().skip(span.start.line - 1) {
        let start_column = if index + 1 == span.start.line {
            span.start.column.saturating_sub(1)
        } else {
            0
        };
        let mut chars = line.chars().enumerate().skip(start_column).peekable();
        let mut quote = None;
        let mut escaped = false;

        while let Some((column, character)) = chars.next() {
            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
                continue;
            }

            if matches!(character, '\'' | '"') {
                quote = Some(character);
                continue;
            }
            if character == '-'
                && chars
                    .peek()
                    .is_some_and(|(_, next_character)| *next_character == '-')
            {
                break;
            }

            match character {
                '{' => {
                    saw_open = true;
                    depth += 1;
                }
                '}' if saw_open => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return SourceSpan {
                            start: span.start,
                            end: SourcePosition {
                                line: index + 1,
                                column: column + 2,
                            },
                        };
                    }
                }
                _ => {}
            }
        }
    }

    span
}

fn collect_enum_declarations(path: &Path, source: &str, syntax: &mut TuaSyntax) {
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        let Some((name, name_column, enum_column)) = enum_declaration(line) else {
            index += 1;
            continue;
        };

        let start_line = index + 1;
        let mut members = Vec::new();
        let mut seen_members = BTreeSet::new();
        let mut end_line = start_line;
        let mut found_end = false;
        index += 1;

        while index < lines.len() {
            let member_line = lines[index];
            end_line = index + 1;

            if member_line.trim() == "end" {
                found_end = true;
                index += 1;
                break;
            }

            match enum_member(member_line) {
                Ok(Some((member, column))) => {
                    if !seen_members.insert(member.clone()) {
                        syntax.diagnostics.push(enum_diagnostic(
                            path,
                            end_line,
                            column,
                            format!("duplicate enum member `{member}`"),
                        ));
                    }

                    members.push(TuaType::StringLiteral(member));
                }
                Ok(None) => {}
                Err((column, message)) => {
                    syntax
                        .diagnostics
                        .push(enum_diagnostic(path, end_line, column, message));
                }
            }

            index += 1;
        }

        if !found_end {
            syntax.diagnostics.push(enum_diagnostic(
                path,
                start_line,
                enum_column,
                format!("unterminated enum `{name}`: expected `end`"),
            ));
        }

        let name_len = name.chars().count();

        syntax.type_aliases.push(TypeAlias {
            name,
            ty: TuaType::Union(members),
            kind: TypeAliasKind::Enum,
            name_span: Some(SourceSpan {
                start: SourcePosition {
                    line: start_line,
                    column: name_column,
                },
                end: SourcePosition {
                    line: start_line,
                    column: name_column + name_len,
                },
            }),
            span: Some(SourceSpan {
                start: SourcePosition {
                    line: start_line,
                    column: enum_column,
                },
                end: SourcePosition {
                    line: end_line,
                    column: lines
                        .get(end_line.saturating_sub(1))
                        .map(|line| line.chars().count() + 1)
                        .unwrap_or(1),
                },
            }),
        });
    }
}

fn enum_declaration(line: &str) -> Option<(String, usize, usize)> {
    let enum_column = line
        .chars()
        .take_while(|character| character.is_whitespace())
        .count()
        + 1;
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("enum")?;

    if !rest
        .chars()
        .next()
        .is_some_and(|character| character.is_whitespace())
    {
        return None;
    }

    let name_start = line.len() - rest.len() + rest.len() - rest.trim_start().len();
    let rest = rest.trim_start();
    let name = rest
        .chars()
        .take_while(|character| *character == '_' || character.is_ascii_alphanumeric())
        .collect::<String>();

    if name.is_empty() {
        return None;
    }

    Some((name, line[..name_start].chars().count() + 1, enum_column))
}

fn enum_member(line: &str) -> Result<Option<(String, usize)>, (usize, String)> {
    let trimmed = line.trim_start();

    if trimmed.is_empty() || trimmed.starts_with("--") {
        return Ok(None);
    }

    let column = line.chars().count() - trimmed.chars().count() + 1;
    let Some((member, rest)) = parse_short_string_literal(trimmed) else {
        return Err((
            column,
            "invalid enum member: expected a string literal".to_owned(),
        ));
    };

    let rest = rest.trim_start();
    let rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();

    if rest.is_empty() || rest.starts_with("--") {
        Ok(Some((member, column)))
    } else {
        Err((
            column + trimmed.len() - rest.len(),
            "invalid enum member: expected line end, comma, or comment after string literal"
                .to_owned(),
        ))
    }
}

fn parse_short_string_literal(text: &str) -> Option<(String, &str)> {
    let mut chars = text.char_indices();
    let (_, quote) = chars.next()?;

    if quote != '"' && quote != '\'' {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;

    for (index, character) in chars {
        if escaped {
            value.push(character);
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == quote {
            return Some((value, &text[index + character.len_utf8()..]));
        }

        value.push(character);
    }

    None
}

fn enum_diagnostic(
    path: &Path,
    line: usize,
    column: usize,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(path, line, column, message).with_code(DiagnosticCode::SYNTAX_PARSE)
}

fn function_annotation(
    path: &Path,
    source: &str,
    name: Option<String>,
    name_span: Option<SourceSpan>,
    body: &ast::FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
) -> FunctionAnnotation {
    let parameter_names = body
        .parameters()
        .iter()
        .filter_map(|parameter| {
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(token_text(name))
        })
        .collect::<Vec<_>>();
    let parameter_count = parameter_names.len();
    let has_varargs = body
        .parameters()
        .iter()
        .any(|parameter| matches!(parameter, ast::Parameter::Ellipsis(_)));
    let default_parameters = parameter_defaults_from_body(source, body);
    let optional_parameters = parameter_optionals_from_body(source, body);
    let required_parameter_count = required_parameter_count_for_optional_parameters(
        path,
        &parameter_names,
        &default_parameters,
        &optional_parameters,
        diagnostics,
    );
    let parameters = body
        .parameters()
        .iter()
        .zip(body.type_specifiers())
        .filter_map(|(parameter, specifier)| {
            let specifier = specifier?;
            let ast::Parameter::Name(name) = parameter else {
                return None;
            };

            Some(ParameterAnnotation {
                name: token_text(name),
                ty: tua_type_from_type_info(specifier.type_info()),
                name_span: node_span(name),
                span: node_span(specifier),
            })
        })
        .collect();

    FunctionAnnotation {
        name,
        name_span,
        parameters,
        parameter_count,
        required_parameter_count,
        has_varargs,
        default_parameters,
        optional_parameters,
        return_type: body
            .return_type()
            .map(|specifier| tua_type_from_type_info(specifier.type_info())),
        span: node_span(body),
    }
}

fn parameter_optionals_from_body(source: &str, body: &ast::FunctionBody) -> Vec<ParameterOptional> {
    let Some((inner, inner_start)) = parameter_list_inner(source, body.parameters_parentheses())
    else {
        return Vec::new();
    };

    split_parameter_parts(inner, inner_start)
        .into_iter()
        .filter_map(|part| parameter_optional_from_part(source, part))
        .collect()
}

fn parameter_defaults_from_body(source: &str, body: &ast::FunctionBody) -> Vec<ParameterDefault> {
    let Some((inner, inner_start)) = parameter_list_inner(source, body.parameters_parentheses())
    else {
        return Vec::new();
    };

    split_parameter_parts(inner, inner_start)
        .into_iter()
        .filter_map(|part| parameter_default_from_part(source, part))
        .collect()
}

fn collect_parameter_default_references(
    source: &str,
    function: &FunctionAnnotation,
    syntax: &mut TuaSyntax,
) {
    let Some(function_span) = function.span else {
        return;
    };

    for default in &function.default_parameters {
        let Some(default_span) = default.span else {
            continue;
        };
        let Some(expression_end) = byte_for_source_position(source, default_span.end) else {
            continue;
        };
        let Some(expression_start) = expression_end.checked_sub(default.expression.len()) else {
            continue;
        };

        syntax.parameter_default_references.extend(
            parameter_default_references(source, &default.expression, expression_start)
                .into_iter()
                .map(|(name, position)| model::ParameterDefaultReference {
                    function_span,
                    name,
                    position,
                }),
        );
    }
}

fn parameter_default_references(
    source: &str,
    expression: &str,
    expression_start: usize,
) -> Vec<(String, SourcePosition)> {
    const RETURN_PREFIX: &str = "return ";

    let wrapper = format!("{RETURN_PREFIX}{expression}");
    let normalized = normalize_tua_source(wrapper);
    let Ok(raw) = parse_raw_source(
        PathBuf::from("<parameter-default>"),
        normalized.original,
        normalized.parser_source,
        normalized.parser_provenance,
        LuaVersion::luau(),
        ParsedLanguage::Tua,
    ) else {
        return Vec::new();
    };
    let parsed = ParsedSource::from_raw_and_typed(raw, TypedSourceModel::default());
    let scope_map = crate::scope::scope_map_parsed_source(&parsed);
    let mut seen = BTreeSet::new();
    let mut references = Vec::new();

    for reference in scope_map.references() {
        if matches!(
            reference.resolution,
            crate::scope::ScopeResolution::Symbol(_)
        ) {
            continue;
        }

        let Some(wrapper_byte) = byte_for_source_position(&parsed.source, reference.span.start)
        else {
            continue;
        };
        let Some(relative_byte) = wrapper_byte.checked_sub(RETURN_PREFIX.len()) else {
            continue;
        };
        let position = source_position_for_byte(source, expression_start + relative_byte);
        let key = (reference.name.clone(), position.line, position.column);

        if seen.insert(key) {
            references.push((reference.name.clone(), position));
        }
    }

    references
}

fn required_parameter_count_for_optional_parameters(
    path: &Path,
    parameter_names: &[String],
    defaults: &[ParameterDefault],
    optionals: &[ParameterOptional],
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    let mut required = 0usize;
    let mut seen_optional = false;
    let mut reported_non_trailing = false;
    let mut first_optional_span = None;
    let mut first_optional_is_default = false;

    for name in parameter_names {
        let default = defaults.iter().find(|default| default.name == *name);
        let optional = optionals.iter().find(|optional| optional.name == *name);
        let has_optional = default.is_some() || optional.is_some();

        if has_optional {
            if !seen_optional {
                first_optional_span = optional
                    .and_then(|optional| optional.span)
                    .or_else(|| default.and_then(|default| default.span));
                first_optional_is_default = optional.is_none() && default.is_some();
            }
            seen_optional = true;
            continue;
        }

        if seen_optional && !reported_non_trailing {
            reported_non_trailing = true;
            let message = if first_optional_is_default {
                "default parameters must be trailing"
            } else {
                "optional parameters must be trailing"
            };
            diagnostics.push(
                Diagnostic::error(
                    path,
                    first_optional_span.map(|span| span.start.line).unwrap_or(1),
                    first_optional_span
                        .map(|span| span.start.column)
                        .unwrap_or(1),
                    message,
                )
                .with_code(DiagnosticCode::SYNTAX_PARSE),
            );
        } else {
            required += 1;
        }
    }

    required
}

fn parameter_list_inner<'a>(
    source: &'a str,
    parentheses: &ast::span::ContainedSpan,
) -> Option<(&'a str, usize)> {
    let span = node_span(parentheses)?;
    let start = byte_for_source_position(source, span.start)?;
    let end = byte_for_source_position(source, span.end).unwrap_or(source.len());
    let range = source.get(start..end)?;
    let open = range.find('(')?;
    let close = range.rfind(')')?;

    if close <= open {
        return None;
    }

    let inner_start = start + open + 1;
    let inner_end = start + close;

    Some((source.get(inner_start..inner_end)?, inner_start))
}

#[derive(Clone, Copy)]
struct ParameterPart<'a> {
    text: &'a str,
    start: usize,
}

fn split_parameter_parts(input: &str, base: usize) -> Vec<ParameterPart<'_>> {
    let bytes = input.as_bytes();
    let mut parts = Vec::new();
    let mut state = ParameterScanState::Code;
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match &mut state {
            ParameterScanState::Code => {
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = ParameterScanState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }

                if bytes[index..].starts_with(b"[[") {
                    state = ParameterScanState::LongString;
                    index += 2;
                    continue;
                }

                match bytes[index] {
                    b'(' | b'{' | b'[' => depth += 1,
                    b')' | b'}' | b']' => depth = depth.saturating_sub(1),
                    b',' if depth == 0 => {
                        parts.push(ParameterPart {
                            text: &input[start..index],
                            start: base + start,
                        });
                        start = index + 1;
                    }
                    _ => {}
                }

                index += 1;
            }
            ParameterScanState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = ParameterScanState::Code;
                }

                index += 1;
            }
            ParameterScanState::LongString => {
                if bytes[index..].starts_with(b"]]") {
                    index += 2;
                    state = ParameterScanState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    parts.push(ParameterPart {
        text: &input[start..],
        start: base + start,
    });
    parts
}

fn parameter_optional_from_part(
    source: &str,
    part: ParameterPart<'_>,
) -> Option<ParameterOptional> {
    let trimmed_start = leading_ascii_whitespace(part.text);
    let trimmed = &part.text[trimmed_start..];
    let name_end_relative = identifier_end(trimmed)?;
    let after_name_untrimmed = &trimmed[name_end_relative..];
    let after_name = after_name_untrimmed.trim_start();

    if !after_name.starts_with("?:") {
        return None;
    }

    let name = trimmed[..name_end_relative].to_owned();
    let name_start = part.start + trimmed_start;
    let name_end = name_start + name.len();
    let whitespace_after_name = after_name_untrimmed.len() - after_name.len();
    let marker_end_relative = name_end_relative + whitespace_after_name + 2;
    let span_end = part.start + trimmed_start + marker_end_relative;

    Some(ParameterOptional {
        name,
        name_span: Some(SourceSpan {
            start: source_position_for_byte(source, name_start),
            end: source_position_for_byte(source, name_end),
        }),
        span: Some(SourceSpan {
            start: source_position_for_byte(source, name_start),
            end: source_position_for_byte(source, span_end),
        }),
    })
}

fn parameter_default_from_part(source: &str, part: ParameterPart<'_>) -> Option<ParameterDefault> {
    let trimmed_start = leading_ascii_whitespace(part.text);
    let trimmed = &part.text[trimmed_start..];
    let name_end = identifier_end(trimmed)?;
    let name = trimmed[..name_end].to_owned();
    let equal = top_level_equal(trimmed)?;
    let expression = trimmed[equal + 1..].trim();

    if expression.is_empty() {
        return None;
    }

    let name_start = part.start + trimmed_start;
    let name_end = name_start + name.len();
    let span_start = name_start;
    let span_end = part.start + trimmed_start + trimmed.trim_end().len();

    Some(ParameterDefault {
        name,
        expression: expression.to_owned(),
        name_span: Some(SourceSpan {
            start: source_position_for_byte(source, name_start),
            end: source_position_for_byte(source, name_end),
        }),
        span: Some(SourceSpan {
            start: source_position_for_byte(source, span_start),
            end: source_position_for_byte(source, span_end),
        }),
    })
}

fn leading_ascii_whitespace(input: &str) -> usize {
    input
        .bytes()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(input.len())
}

fn identifier_end(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();

    if !bytes
        .first()
        .is_some_and(|byte| *byte == b'_' || byte.is_ascii_alphabetic())
    {
        return None;
    }

    Some(
        bytes
            .iter()
            .position(|byte| !(*byte == b'_' || byte.is_ascii_alphanumeric()))
            .unwrap_or(bytes.len()),
    )
}

fn top_level_equal(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut state = ParameterScanState::Code;
    let mut depth = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match &mut state {
            ParameterScanState::Code => {
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = ParameterScanState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }

                if bytes[index..].starts_with(b"[[") {
                    state = ParameterScanState::LongString;
                    index += 2;
                    continue;
                }

                match bytes[index] {
                    b'(' | b'{' | b'[' => depth += 1,
                    b')' | b'}' | b']' => depth = depth.saturating_sub(1),
                    b'=' if depth == 0 => return Some(index),
                    _ => {}
                }

                index += 1;
            }
            ParameterScanState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = ParameterScanState::Code;
                }

                index += 1;
            }
            ParameterScanState::LongString => {
                if bytes[index..].starts_with(b"]]") {
                    index += 2;
                    state = ParameterScanState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    None
}

#[derive(Clone, Copy)]
enum ParameterScanState {
    Code,
    ShortString { quote: u8, escaped: bool },
    LongString,
}

fn byte_for_source_position(source: &str, position: SourcePosition) -> Option<usize> {
    let mut offset = 0usize;

    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        if line_index + 1 == position.line {
            let line = line.trim_end_matches(['\n', '\r']);
            return Some(offset + byte_for_column(line, position.column));
        }

        offset += line.len();
    }

    (position.line == source.lines().count() + 1).then_some(source.len())
}

fn source_position_for_byte(source: &str, byte: usize) -> SourcePosition {
    let byte = byte.min(source.len());
    let before = &source[..byte];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let column = column_for_byte(&source[line_start..byte], byte - line_start);

    SourcePosition { line, column }
}

fn compound_assignment_from_ast(
    path: &Path,
    assignment: &ast::CompoundAssignment,
) -> Result<CompoundAssignment, Diagnostic> {
    let operator = compound_operator_from_ast(assignment.compound_operator()).ok_or_else(|| {
        diagnostic_for_node(
            path,
            assignment.compound_operator(),
            "unsupported compound assignment operator: expected one of +=, -=, *=, /=, %=",
        )
    })?;

    let lhs = plain_field_chain_from_var(assignment.lhs()).ok_or_else(|| {
        diagnostic_for_node(
            path,
            assignment.lhs(),
            "invalid compound assignment left-hand side: expected a name or plain field chain",
        )
    })?;

    Ok(CompoundAssignment {
        lhs,
        operator,
        rhs: assignment.rhs().to_string().trim().to_owned(),
        span: node_span(assignment),
    })
}

fn compound_operator_from_ast(operator: &ast::CompoundOp) -> Option<CompoundOperator> {
    match operator {
        ast::CompoundOp::PlusEqual(_) => Some(CompoundOperator::Add),
        ast::CompoundOp::MinusEqual(_) => Some(CompoundOperator::Subtract),
        ast::CompoundOp::StarEqual(_) => Some(CompoundOperator::Multiply),
        ast::CompoundOp::SlashEqual(_) => Some(CompoundOperator::Divide),
        ast::CompoundOp::PercentEqual(_) => Some(CompoundOperator::Modulo),
        _ => None,
    }
}

pub(crate) fn tua_type_from_type_info(type_info: &TypeInfo) -> TuaType {
    match type_info {
        TypeInfo::Basic(token) => TuaType::Named(token_text(token)),
        TypeInfo::String(token) => {
            TuaType::StringLiteral(unquote_short_string_token(&token_text(token)))
        }
        TypeInfo::Optional { base, .. } => TuaType::Union(vec![
            tua_type_from_type_info(base),
            TuaType::Named("nil".to_owned()),
        ]),
        TypeInfo::Union(union) => {
            TuaType::Union(union.types().iter().map(tua_type_from_type_info).collect())
        }
        TypeInfo::Tuple { types, .. } => {
            TuaType::Tuple(types.iter().map(tua_type_from_type_info).collect())
        }
        TypeInfo::Callback {
            generics: None,
            arguments,
            return_type,
            ..
        } => TuaType::Function {
            parameters: arguments
                .iter()
                .map(function_type_parameter_from_type_argument)
                .collect(),
            returns: function_return_types_from_type_info(return_type),
        },
        TypeInfo::Table { fields, .. } => {
            let fields = fields.iter().collect::<Vec<_>>();

            if let [field] = fields.as_slice() {
                if let TypeFieldKey::IndexSignature { inner, .. } = field.key() {
                    return TuaType::Dictionary {
                        key: Box::new(tua_type_from_type_info(inner)),
                        value: Box::new(tua_type_from_type_info(field.value())),
                    };
                }
            }

            TuaType::Table(
                fields
                    .into_iter()
                    .filter_map(table_field_type_from_type_field)
                    .collect(),
            )
        }
        other => TuaType::Named(other.to_string()),
    }
}

fn function_type_parameter_from_type_argument(argument: &TypeArgument) -> FunctionTypeParameter {
    let (variadic, type_info) = match argument.type_info() {
        TypeInfo::Variadic { type_info, .. } => (true, type_info.as_ref()),
        type_info => (false, type_info),
    };
    let (optional, ty) = match type_info {
        TypeInfo::Optional { base, .. } => (true, tua_type_from_type_info(base)),
        _ => (false, tua_type_from_type_info(type_info)),
    };

    FunctionTypeParameter {
        name: argument.name().map(|(name, _)| token_text(name)),
        ty,
        optional,
        variadic,
    }
}

fn function_return_types_from_type_info(type_info: &TypeInfo) -> Vec<TuaType> {
    match type_info {
        TypeInfo::Tuple { types, .. } => types.iter().map(tua_type_from_type_info).collect(),
        _ => vec![tua_type_from_type_info(type_info)],
    }
}

fn unquote_short_string_token(value: &str) -> String {
    parse_short_string_literal(value)
        .map(|(literal, _)| literal)
        .unwrap_or_else(|| value.to_owned())
}

fn table_field_type_from_type_field(field: &TypeField) -> Option<TableFieldType> {
    let key = match field.key() {
        TypeFieldKey::Name(key) => key,
        _ => return None,
    };

    let mut name = token_text(key);
    let key_optional = name.ends_with('?');

    if key_optional {
        name.pop();
    }

    let (value_optional, ty) = table_field_value_type(field.value());

    Some(TableFieldType {
        name,
        optional: key_optional || value_optional,
        ty,
        name_span: node_span(key),
        span: node_span(field),
    })
}

fn table_field_value_type(type_info: &TypeInfo) -> (bool, TuaType) {
    match type_info {
        TypeInfo::Optional { base, .. } => (true, tua_type_from_type_info(base)),
        _ => (false, tua_type_from_type_info(type_info)),
    }
}

fn node_span(node: &impl Node) -> Option<SourceSpan> {
    node.range()
        .map(|(start, end)| SourceSpan::from_range(start, end))
}

fn token_text(token: &full_moon::tokenizer::TokenReference) -> String {
    token.to_string().trim().to_owned()
}

fn parser_error_to_diagnostic(path: &Path, error: full_moon::Error) -> Diagnostic {
    let (start, _) = error.range();
    let position = SourcePosition::from_full_moon(start);

    Diagnostic::error(
        path,
        position.line,
        position.column,
        error.error_message().into_owned(),
    )
    .with_code(DiagnosticCode::SYNTAX_PARSE)
}

fn diagnostic_for_node(path: &Path, node: &impl Node, message: impl Into<String>) -> Diagnostic {
    let position = node
        .range()
        .map(|(start, _)| SourcePosition::from_full_moon(start))
        .unwrap_or(SourcePosition { line: 1, column: 1 });

    Diagnostic::error(path, position.line, position.column, message)
        .with_code(DiagnosticCode::SYNTAX_COMPOUND_ASSIGNMENT)
}

#[cfg(test)]
mod tests;
