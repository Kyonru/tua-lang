use std::path::Path;

use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

use super::model::{
    FunctionTypeParameter, LuaDocAlias, LuaDocAnnotation, LuaDocAnnotationKind, LuaDocAs,
    LuaDocCast, LuaDocClass, LuaDocDeprecated, LuaDocDiagnostic, LuaDocEnum, LuaDocField,
    LuaDocGeneric, LuaDocGenericParameter, LuaDocMeta, LuaDocModule, LuaDocNoDiscard,
    LuaDocOverload, LuaDocParam, LuaDocParseOptions, LuaDocParseResult, LuaDocReturn,
    LuaDocReturnType, LuaDocTypeAnnotation, LuaDocVararg, LuaDocVersion, SourcePosition,
    SourceSpan, TableFieldType, TuaType,
};

pub fn parse_lua_doc_annotations(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> LuaDocParseResult {
    parse_lua_doc_annotations_with_options(path, source, LuaDocParseOptions::default())
}

pub fn parse_lua_doc_annotations_with_options(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: LuaDocParseOptions,
) -> LuaDocParseResult {
    let source = source.into();
    let mut result = collect_lua_doc_annotations(path.as_ref(), &source);

    if !options.diagnostics_enabled {
        result.diagnostics.clear();
    }

    result
}

pub(crate) fn collect_lua_doc_annotations(path: &Path, source: &str) -> LuaDocParseResult {
    let mut parser = LuaDocParser {
        path,
        annotations: Vec::new(),
        diagnostics: Vec::new(),
    };

    parser.parse(source);

    LuaDocParseResult {
        annotations: parser.annotations,
        diagnostics: parser.diagnostics,
    }
}

struct LuaDocParser<'a> {
    path: &'a Path,
    annotations: Vec<LuaDocAnnotation>,
    diagnostics: Vec<Diagnostic>,
}

impl LuaDocParser<'_> {
    fn parse(&mut self, source: &str) {
        let lines = source
            .lines()
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect::<Vec<_>>();
        let mut index = 0;

        while index < lines.len() {
            if let Some(consumed) = self.parse_multiline_alias(&lines, index) {
                index += consumed;
                continue;
            }
            self.parse_line(index + 1, lines[index]);
            index += 1;
        }
    }

    /// LuaCATS spells a long alias as a bare `---@alias Name` followed by one
    /// `---| type` line per member. Returns the number of lines consumed.
    fn parse_multiline_alias(&mut self, lines: &[&str], index: usize) -> Option<usize> {
        let line = lines[index];
        let leading = line.len() - line.trim_start().len();
        let header = line[leading..].strip_prefix("---")?.trim_start();
        let after_tag = header.strip_prefix("@alias")?;
        if !after_tag.starts_with(char::is_whitespace) {
            return None;
        }
        let tag_start = line.len() - header.len();
        let tag_span = line_span(index + 1, tag_start, tag_start + "@alias".len());
        let (name, name_span, cursor) = take_token_at(
            line,
            index + 1,
            skip_spaces(line, tag_start + "@alias".len()),
        )?;
        if !line[cursor..].trim().is_empty() {
            return None;
        }

        let mut members = Vec::new();
        let mut type_span: Option<SourceSpan> = None;
        for (offset, member_line) in lines[index + 1..].iter().enumerate() {
            let member_leading = member_line.len() - member_line.trim_start().len();
            let Some(rest) = member_line[member_leading..].strip_prefix("---|") else {
                break;
            };
            // `---|>` marks LuaLS's default member; it is still a member.
            let marker = member_line.len() - rest.len();
            let cursor = marker + usize::from(rest.starts_with('>'));
            let line_number = index + 2 + offset;
            let Ok((ty, span, _)) = required_type(member_line, line_number, cursor, "alias member")
            else {
                break;
            };
            members.push(ty);
            type_span = Some(match type_span {
                Some(first) => SourceSpan {
                    start: first.start,
                    end: span.end,
                },
                None => span,
            });
        }

        let type_span = type_span?;
        let consumed = members.len() + 1;
        let ty = if members.len() == 1 {
            members.pop()?
        } else {
            TuaType::Union(members)
        };
        self.annotations.push(LuaDocAnnotation {
            kind: LuaDocAnnotationKind::Alias(LuaDocAlias {
                name,
                name_span,
                ty,
                type_span,
            }),
            raw: line[leading..].trim().to_owned(),
            documentation: None,
            span: line_span(index + 1, leading, line.len()),
            tag_span,
        });
        Some(consumed)
    }

    fn parse_line(&mut self, line_number: usize, line: &str) {
        let leading = line.len() - line.trim_start().len();
        let trimmed = &line[leading..];

        if !trimmed.starts_with("---") {
            return;
        }

        let after_marker_start = leading + 3;
        let after_marker = &line[after_marker_start..];
        let spaces = after_marker.len() - after_marker.trim_start().len();
        let tag_start = after_marker_start + spaces;

        if !line[tag_start..].starts_with('@') {
            return;
        }

        let Some((tag, tag_span, mut cursor)) = take_token_at(line, line_number, tag_start) else {
            return;
        };
        let tag_name = tag.trim_start_matches('@');
        let raw = line[leading..].trim().to_owned();
        let span = line_span(line_number, leading, line.len());

        if !is_supported_tag(tag_name) {
            return;
        }

        cursor = skip_spaces(line, cursor);

        match self.parse_annotation_kind(line, line_number, tag_name, cursor) {
            Ok((kind, documentation)) => self.annotations.push(LuaDocAnnotation {
                kind,
                raw,
                documentation,
                span,
                tag_span,
            }),
            Err(message) => self.diagnostics.push(annotation_diagnostic(
                self.path,
                line_number,
                tag_start + 1,
                tag_name,
                message,
            )),
        }
    }

    fn parse_annotation_kind(
        &self,
        line: &str,
        line_number: usize,
        tag: &str,
        cursor: usize,
    ) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
        match tag {
            "class" => parse_class(line, line_number, cursor),
            "field" => parse_field(line, line_number, cursor),
            "alias" => parse_alias(line, line_number, cursor),
            "type" => parse_type_annotation(line, line_number, cursor),
            "param" => parse_param(line, line_number, cursor),
            "return" => parse_return(line, line_number, cursor),
            "generic" => parse_generic(line, line_number, cursor),
            "version" => parse_version(line, line_number, cursor),
            "enum" => parse_enum(line, line_number, cursor),
            "meta" => parse_meta(line, line_number, cursor),
            "module" => parse_module(line, line_number, cursor),
            "diagnostic" => parse_diagnostic(line, line_number, cursor),
            "cast" => parse_cast(line, line_number, cursor),
            "as" => parse_as(line, line_number, cursor),
            "overload" => parse_overload(line, line_number, cursor),
            "vararg" => parse_vararg(line, line_number, cursor),
            "deprecated" => Ok((
                LuaDocAnnotationKind::Deprecated(LuaDocDeprecated),
                documentation_from(line, cursor),
            )),
            "nodiscard" => Ok((
                LuaDocAnnotationKind::NoDiscard(LuaDocNoDiscard),
                documentation_from(line, cursor),
            )),
            _ => unreachable!("unsupported tags are filtered before parsing"),
        }
    }
}

fn parse_class(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (name, name_span, cursor) = required_token(line, line_number, cursor, "class name")?;
    let mut cursor = skip_spaces(line, cursor);
    let mut extends = None;
    let mut extends_span = None;

    if line[cursor..].starts_with(':') {
        cursor = skip_spaces(line, cursor + 1);
        let (parent, parent_span, next) =
            required_token(line, line_number, cursor, "parent class")?;

        extends = Some(parent);
        extends_span = Some(parent_span);
        cursor = skip_spaces(line, next);
    }

    Ok((
        LuaDocAnnotationKind::Class(LuaDocClass {
            name,
            name_span,
            extends,
            extends_span,
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_field(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (first, first_span, cursor) = required_token(line, line_number, cursor, "field name")?;
    let (visibility, name, name_span, cursor) = if is_visibility(&first) {
        let (name, name_span, cursor) = required_token(line, line_number, cursor, "field name")?;

        (Some(first), name, name_span, cursor)
    } else {
        (None, first, first_span, cursor)
    };
    let (name, optional) = optional_name(name);
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "field type")?;

    Ok((
        LuaDocAnnotationKind::Field(LuaDocField {
            name,
            name_span,
            optional,
            visibility,
            ty,
            type_span,
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_alias(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (name, name_span, cursor) = required_token(line, line_number, cursor, "alias name")?;
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "alias type")?;

    Ok((
        LuaDocAnnotationKind::Alias(LuaDocAlias {
            name,
            name_span,
            ty,
            type_span,
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_type_annotation(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "type")?;

    Ok((
        LuaDocAnnotationKind::Type(LuaDocTypeAnnotation { ty, type_span }),
        documentation_from(line, cursor),
    ))
}

fn parse_param(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (name, name_span, cursor) = required_token(line, line_number, cursor, "parameter name")?;
    let (name, optional) = optional_name(name);
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "parameter type")?;

    Ok((
        LuaDocAnnotationKind::Param(LuaDocParam {
            name,
            name_span,
            optional,
            ty,
            type_span,
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_return(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "return type")?;

    Ok((
        LuaDocAnnotationKind::Return(LuaDocReturn {
            returns: vec![LuaDocReturnType { ty, type_span }],
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_generic(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Err("expected generic parameter list".to_owned());
    }

    let mut parameters = Vec::new();
    let mut cursor = cursor;

    while cursor < line.len() {
        cursor = skip_spaces_and_commas(line, cursor);

        if cursor >= line.len() {
            break;
        }

        let Some((name, name_span, next)) = take_generic_token_at(line, line_number, cursor) else {
            break;
        };
        let (name, bound, bound_span) = match name.split_once(':') {
            Some((raw_name, raw_bound))
                if !raw_name.trim().is_empty() && !raw_bound.trim().is_empty() =>
            {
                let bound_start = cursor + raw_name.len() + 1;
                (
                    raw_name.trim().to_owned(),
                    Some(parse_type_text(raw_bound)),
                    Some(line_span(line_number, bound_start, cursor + name.len())),
                )
            }
            Some(_) => return Err("expected generic bound after `:`".to_owned()),
            None => (name, None, None),
        };

        parameters.push(LuaDocGenericParameter {
            name,
            name_span,
            bound,
            bound_span,
        });

        cursor = next;

        if !line[cursor..].trim_start().starts_with(',') {
            break;
        }
    }

    if parameters.is_empty() {
        return Err("expected generic parameter list".to_owned());
    }

    Ok((
        LuaDocAnnotationKind::Generic(LuaDocGeneric { parameters }),
        documentation_from(line, cursor),
    ))
}

fn parse_version(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Err("expected version expression".to_owned());
    }

    let expression = line[cursor..].trim().to_owned();
    let expression_span = line_span(line_number, cursor, line.len());

    Ok((
        LuaDocAnnotationKind::Version(LuaDocVersion {
            expression,
            expression_span,
        }),
        None,
    ))
}

fn parse_enum(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Ok((
            LuaDocAnnotationKind::Enum(LuaDocEnum {
                name: None,
                name_span: None,
            }),
            None,
        ));
    }

    let (name, name_span, cursor) = required_token(line, line_number, cursor, "enum name")?;

    Ok((
        LuaDocAnnotationKind::Enum(LuaDocEnum {
            name: Some(name),
            name_span: Some(name_span),
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_meta(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Ok((
            LuaDocAnnotationKind::Meta(LuaDocMeta {
                expression: None,
                expression_span: None,
            }),
            None,
        ));
    }

    Ok((
        LuaDocAnnotationKind::Meta(LuaDocMeta {
            expression: Some(line[cursor..].trim().to_owned()),
            expression_span: Some(line_span(line_number, cursor, line.len())),
        }),
        None,
    ))
}

fn parse_module(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (module, module_span, cursor) = required_token(line, line_number, cursor, "module name")?;

    Ok((
        LuaDocAnnotationKind::Module(LuaDocModule {
            module: unquote_literal(&module).unwrap_or(module),
            module_span,
        }),
        documentation_from(line, cursor),
    ))
}

fn parse_diagnostic(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Err("expected diagnostic expression".to_owned());
    }

    Ok((
        LuaDocAnnotationKind::Diagnostic(LuaDocDiagnostic {
            expression: line[cursor..].trim().to_owned(),
            expression_span: line_span(line_number, cursor, line.len()),
        }),
        None,
    ))
}

fn parse_cast(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let cursor = skip_spaces(line, cursor);

    if cursor >= line.len() {
        return Err("expected cast expression".to_owned());
    }

    Ok((
        LuaDocAnnotationKind::Cast(LuaDocCast {
            expression: line[cursor..].trim().to_owned(),
            expression_span: line_span(line_number, cursor, line.len()),
        }),
        None,
    ))
}

fn parse_as(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "cast type")?;

    Ok((
        LuaDocAnnotationKind::As(LuaDocAs { ty, type_span }),
        documentation_from(line, cursor),
    ))
}

fn parse_overload(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (ty, type_span, cursor) =
        required_type(line, line_number, cursor, "overload function type")?;

    if !matches!(ty, TuaType::Function { .. }) {
        return Err("expected overload function type".to_owned());
    }

    Ok((
        LuaDocAnnotationKind::Overload(LuaDocOverload { ty, type_span }),
        documentation_from(line, cursor),
    ))
}

fn parse_vararg(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Result<(LuaDocAnnotationKind, Option<String>), String> {
    let (ty, type_span, cursor) = required_type(line, line_number, cursor, "vararg type")?;

    Ok((
        LuaDocAnnotationKind::Vararg(LuaDocVararg { ty, type_span }),
        documentation_from(line, cursor),
    ))
}

fn required_token(
    line: &str,
    line_number: usize,
    cursor: usize,
    label: &str,
) -> Result<(String, SourceSpan, usize), String> {
    let cursor = skip_spaces(line, cursor);

    take_token_at(line, line_number, cursor).ok_or_else(|| format!("expected {label}"))
}

fn required_type(
    line: &str,
    line_number: usize,
    cursor: usize,
    label: &str,
) -> Result<(TuaType, SourceSpan, usize), String> {
    let cursor = skip_spaces(line, cursor);
    let Some((text, end)) = take_type_text(line, cursor) else {
        return Err(format!("expected {label}"));
    };

    let span = line_span(line_number, cursor, end);

    Ok((parse_type_text(&text), span, end))
}

fn take_type_text(line: &str, cursor: usize) -> Option<(String, usize)> {
    if cursor >= line.len() {
        return None;
    }

    let mut end = cursor;
    let mut expect_type = true;

    while end < line.len() {
        let token_start = skip_spaces(line, end);

        if token_start >= line.len() {
            break;
        }

        if line[token_start..].starts_with('|') {
            if expect_type {
                break;
            }

            end = token_start + 1;
            expect_type = true;
            continue;
        }

        if !expect_type {
            break;
        }

        let next = type_atom_end(line, token_start)?;

        end = next;
        expect_type = false;

        let after = skip_spaces(line, end);

        if line[after..].starts_with('|') {
            continue;
        }
        break;
    }

    let text = line[cursor..end].trim();

    (!text.is_empty()).then(|| (text.to_owned(), end))
}

fn type_atom_end(line: &str, cursor: usize) -> Option<usize> {
    if starts_with_quote(line, cursor) {
        return quoted_literal_end(line, cursor);
    }

    if line[cursor..].starts_with('{') {
        return balanced_delimiter_end(line, cursor, '{', '}');
    }

    if line[cursor..].starts_with('[') {
        return balanced_delimiter_end(line, cursor, '[', ']');
    }

    if line[cursor..].starts_with("fun(") {
        return function_type_end(line, cursor);
    }

    let mut end = cursor;

    while end < line.len() {
        let Some(ch) = line[end..].chars().next() else {
            break;
        };

        if ch.is_whitespace() || ch == '|' || ch == ',' || ch == ')' || ch == '}' || ch == ']' {
            break;
        }

        if line[end..].starts_with("[]") {
            break;
        }

        if ch == '<' {
            end = balanced_delimiter_end(line, end, '<', '>')?;
            continue;
        }

        end += ch.len_utf8();
    }

    if line[end..].starts_with("[]") {
        end += 2;
    }

    (end > cursor).then_some(end)
}

fn function_type_end(line: &str, cursor: usize) -> Option<usize> {
    let args_start = cursor + "fun".len();
    let args_end = balanced_delimiter_end(line, args_start, '(', ')')?;
    let after_args = skip_spaces(line, args_end);

    if !line[after_args..].starts_with(':') {
        return Some(args_end);
    }

    let return_start = skip_spaces(line, after_args + 1);
    let (_, return_end) = take_type_text(line, return_start)?;

    Some(return_end)
}

fn balanced_delimiter_end(line: &str, cursor: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (offset, ch) in line[cursor..].char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }

            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            _ if ch == open => depth += 1,
            _ if ch == close => {
                depth -= 1;

                if depth == 0 {
                    return Some(cursor + offset + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}

pub(crate) fn parse_type_text(text: &str) -> TuaType {
    let text = text.trim();

    // Function return types may contain top-level unions and array suffixes, so
    // recognize the complete function form before applying outer precedence.
    if let Some(function) =
        parse_function_type_text(text).or_else(|| parse_native_function_type_text(text))
    {
        return function;
    }

    let union = split_top_level(text, '|');

    if union.len() > 1 {
        return TuaType::Union(union.into_iter().map(parse_type_text).collect());
    }

    if let Some(base) = text.strip_suffix('?') {
        let base = base.trim();

        if !base.is_empty() {
            return TuaType::Union(vec![
                parse_type_text(base),
                TuaType::Named("nil".to_owned()),
            ]);
        }
    }

    if let Some(base) = text.strip_suffix("[]") {
        return TuaType::Array(Box::new(parse_type_text(base)));
    }

    if is_quoted_string_literal(text) {
        return TuaType::StringLiteral(unquote_literal(text).unwrap_or_else(|| text.to_owned()));
    }

    if let Some(inner) = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return TuaType::Tuple(
            split_top_level(inner, ',')
                .into_iter()
                .map(parse_type_text)
                .collect(),
        );
    }

    if let Some(inner) = parenthesized_type_inner(text) {
        return parse_type_text(inner);
    }

    if text == "identifier" {
        return TuaType::Named("string".to_owned());
    }

    if let Some((key, value)) = parse_table_generic_type_text(text) {
        return TuaType::Dictionary {
            key: Box::new(parse_type_text(key)),
            value: Box::new(parse_type_text(value)),
        };
    }

    if let Some(inner) = text
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    {
        if let Some((key, value)) = parse_dictionary_field(inner) {
            return TuaType::Dictionary {
                key: Box::new(parse_type_text(key)),
                value: Box::new(parse_type_text(value)),
            };
        }

        return TuaType::Table(
            split_top_level(inner, ',')
                .into_iter()
                .filter_map(parse_table_field)
                .collect(),
        );
    }

    TuaType::Named(text.to_owned())
}

fn parse_table_field(text: &str) -> Option<TableFieldType> {
    let (raw_name, raw_ty) = text.split_once(':')?;
    let mut name = raw_name.trim().to_owned();
    let optional = name.ends_with('?');

    if optional {
        name.pop();
    }

    Some(TableFieldType {
        name,
        optional,
        ty: parse_type_text(raw_ty),
        name_span: None,
        span: None,
    })
}

fn parse_dictionary_field(text: &str) -> Option<(&str, &str)> {
    let text = text.trim();
    let inner = text.strip_prefix('[')?;
    let (key, rest) = inner.split_once(']')?;
    let value = rest.trim_start().strip_prefix(':')?;

    Some((key.trim(), value.trim()))
}

fn parse_function_type_text(text: &str) -> Option<TuaType> {
    let text = text.trim();
    let rest = text.strip_prefix("fun(")?;
    let args_end = matching_delimiter_index(rest, '(', ')')?;
    let args = &rest[..args_end];
    let after_args = rest[args_end + 1..].trim_start();
    let returns = if let Some(return_text) = after_args.strip_prefix(':') {
        split_top_level(return_text.trim(), ',')
            .into_iter()
            .map(parse_type_text)
            .collect()
    } else {
        Vec::new()
    };
    let parameters = split_top_level(args, ',')
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(parse_function_type_parameter)
        .collect();

    Some(TuaType::Function {
        parameters,
        returns,
    })
}

fn parse_native_function_type_text(text: &str) -> Option<TuaType> {
    let arrow = top_level_arrow_index(text)?;
    let parameters = parenthesized_type_inner(text[..arrow].trim())?;
    let returns = text[arrow + 2..].trim();

    if returns.is_empty() {
        return None;
    }

    let parameters = split_top_level(parameters, ',')
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(parse_function_type_parameter)
        .collect();
    let returns = parenthesized_type_inner(returns)
        .map(|inner| {
            let parts = split_top_level(inner, ',');

            if parts.len() > 1 {
                parts.into_iter().map(parse_type_text).collect()
            } else {
                vec![parse_type_text(returns)]
            }
        })
        .unwrap_or_else(|| vec![parse_type_text(returns)]);

    Some(TuaType::Function {
        parameters,
        returns,
    })
}

fn parenthesized_type_inner(text: &str) -> Option<&str> {
    let text = text.trim();
    let rest = text.strip_prefix('(')?;
    let close = matching_delimiter_index(rest, '(', ')')?;

    (close + 1 == rest.len()).then(|| &rest[..close])
}

fn top_level_arrow_index(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }

            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            '{' | '[' | '(' | '<' => depth += 1,
            '}' | ']' | ')' | '>' => depth = depth.saturating_sub(1),
            '-' if depth == 0 && text[index..].starts_with("->") => return Some(index),
            _ => {}
        }
    }

    None
}

fn parse_function_type_parameter(text: &str) -> FunctionTypeParameter {
    let text = text.trim();

    if let Some(ty) = text.strip_prefix("...") {
        let ty = ty.trim_start_matches(':').trim();

        return FunctionTypeParameter {
            name: Some("...".to_owned()),
            ty: if ty.is_empty() {
                TuaType::Named("any".to_owned())
            } else {
                parse_type_text(ty)
            },
            optional: false,
            variadic: true,
        };
    }

    if let Some((name, ty)) = text.split_once(':') {
        let (name, optional) = optional_name(name.trim().to_owned());

        return FunctionTypeParameter {
            name: Some(name),
            ty: parse_type_text(ty),
            optional,
            variadic: false,
        };
    }

    FunctionTypeParameter {
        name: None,
        ty: parse_type_text(text),
        optional: false,
        variadic: false,
    }
}

fn parse_table_generic_type_text(text: &str) -> Option<(&str, &str)> {
    let inner = text
        .trim()
        .strip_prefix("table<")?
        .strip_suffix('>')?
        .trim();
    let parts = split_top_level(inner, ',');
    let [key, value] = parts.as_slice() else {
        return None;
    };

    Some((key, value))
}

fn matching_delimiter_index(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in text.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }

            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            _ if ch == open => depth += 1,
            _ if ch == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in text.char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }

            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            '{' | '[' | '(' | '<' => depth += 1,
            '}' | ']' | ')' | '>' => depth -= 1,
            _ if ch == separator && depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(text[start..].trim());
    parts
}

fn starts_with_quote(line: &str, cursor: usize) -> bool {
    line[cursor..].starts_with('\'') || line[cursor..].starts_with('"')
}

fn quoted_literal_end(line: &str, cursor: usize) -> Option<usize> {
    let quote = line[cursor..].chars().next()?;
    let mut escaped = false;

    for (offset, ch) in line[cursor + quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == quote {
            return Some(cursor + quote.len_utf8() + offset + ch.len_utf8());
        }
    }

    None
}

fn is_quoted_string_literal(text: &str) -> bool {
    let text = text.trim();

    (text.len() >= 2 && text.starts_with('\'') && text.ends_with('\''))
        || (text.len() >= 2 && text.starts_with('"') && text.ends_with('"'))
}

fn take_token_at(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Option<(String, SourceSpan, usize)> {
    if cursor >= line.len() {
        return None;
    }

    let end = line[cursor..]
        .char_indices()
        .find_map(|(offset, ch)| ch.is_whitespace().then_some(cursor + offset))
        .unwrap_or(line.len());

    (end > cursor).then(|| {
        (
            line[cursor..end].to_owned(),
            line_span(line_number, cursor, end),
            end,
        )
    })
}

fn take_generic_token_at(
    line: &str,
    line_number: usize,
    cursor: usize,
) -> Option<(String, SourceSpan, usize)> {
    if cursor >= line.len() {
        return None;
    }

    let end = line[cursor..]
        .char_indices()
        .find_map(|(offset, ch)| (ch.is_whitespace() || ch == ',').then_some(cursor + offset))
        .unwrap_or(line.len());

    (end > cursor).then(|| {
        (
            line[cursor..end].to_owned(),
            line_span(line_number, cursor, end),
            end,
        )
    })
}

fn documentation_from(line: &str, cursor: usize) -> Option<String> {
    let text = line[cursor..].trim();

    (!text.is_empty()).then(|| text.to_owned())
}

fn optional_name(mut name: String) -> (String, bool) {
    let optional = name.ends_with('?');

    if optional {
        name.pop();
    }

    (name, optional)
}

fn is_visibility(value: &str) -> bool {
    matches!(value, "public" | "protected" | "private" | "package")
}

fn is_supported_tag(tag: &str) -> bool {
    matches!(
        tag,
        "class"
            | "field"
            | "alias"
            | "type"
            | "param"
            | "return"
            | "generic"
            | "version"
            | "enum"
            | "meta"
            | "module"
            | "diagnostic"
            | "cast"
            | "as"
            | "overload"
            | "vararg"
            | "deprecated"
            | "nodiscard"
    )
}

fn skip_spaces(line: &str, mut cursor: usize) -> usize {
    while cursor < line.len() {
        let Some(ch) = line[cursor..].chars().next() else {
            break;
        };

        if !ch.is_whitespace() {
            break;
        }

        cursor += ch.len_utf8();
    }

    cursor
}

fn skip_spaces_and_commas(line: &str, mut cursor: usize) -> usize {
    while cursor < line.len() {
        let Some(ch) = line[cursor..].chars().next() else {
            break;
        };

        if !ch.is_whitespace() && ch != ',' {
            break;
        }

        cursor += ch.len_utf8();
    }

    cursor
}

fn line_span(line_number: usize, start: usize, end: usize) -> SourceSpan {
    SourceSpan {
        start: SourcePosition {
            line: line_number,
            column: start + 1,
        },
        end: SourcePosition {
            line: line_number,
            column: end + 1,
        },
    }
}

fn unquote_literal(text: &str) -> Option<String> {
    let text = text.trim();
    let quote = text.chars().next()?;

    if !matches!(quote, '\'' | '"') || !text.ends_with(quote) || text.len() < 2 {
        return None;
    }

    Some(text[quote.len_utf8()..text.len() - quote.len_utf8()].to_owned())
}

fn annotation_diagnostic(
    path: &Path,
    line: usize,
    column: usize,
    tag: &str,
    message: String,
) -> Diagnostic {
    Diagnostic::new(
        path,
        line,
        column,
        DiagnosticSeverity::Warning,
        format!("malformed Lua doc annotation `@{tag}`: {message}"),
    )
    .with_code(DiagnosticCode::SYNTAX_DOC_ANNOTATION)
}
