use std::path::Path;

use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

use super::model::{
    LuaDocAlias, LuaDocAnnotation, LuaDocAnnotationKind, LuaDocClass, LuaDocField, LuaDocGeneric,
    LuaDocGenericParameter, LuaDocParam, LuaDocParseOptions, LuaDocParseResult, LuaDocReturn,
    LuaDocReturnType, LuaDocTypeAnnotation, LuaDocVersion, SourcePosition, SourceSpan,
    TableFieldType, TuaType,
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
        for (index, raw_line) in source.lines().enumerate() {
            self.parse_line(index + 1, raw_line.strip_suffix('\r').unwrap_or(raw_line));
        }
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

    let end = if line[cursor..].starts_with('{') {
        balanced_brace_end(line, cursor)?
    } else {
        let mut end = cursor;

        while end < line.len() {
            let Some((token, _, next)) = take_token_at(line, 1, end) else {
                break;
            };

            if token == "|" || token.starts_with('|') || line[end..].trim_start().starts_with('|') {
                end = next;
                continue;
            }

            if end == cursor {
                end = next;
            }

            break;
        }

        end
    };

    let text = line[cursor..end].trim();

    (!text.is_empty()).then(|| (text.to_owned(), end))
}

fn balanced_brace_end(line: &str, cursor: usize) -> Option<usize> {
    let mut depth = 0;

    for (offset, ch) in line[cursor..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
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

fn parse_type_text(text: &str) -> TuaType {
    let text = text.trim();
    let union = split_top_level(text, '|');

    if union.len() > 1 {
        return TuaType::Union(union.into_iter().map(parse_type_text).collect());
    }

    if let Some(inner) = text
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    {
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

fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (index, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
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
        "class" | "field" | "alias" | "type" | "param" | "return" | "generic" | "version"
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
