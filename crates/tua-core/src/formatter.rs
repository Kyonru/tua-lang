mod dialogue;
mod stylua;

use std::path::Path;

use crate::{
    parse_lua_source, parse_tua_source, Diagnostic, ParsedLanguage, ParsedSource, SourcePosition,
    SourceSpan, TextEdit,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteStyle {
    Preserve,
    AutoDouble,
    Double,
    Single,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndentStyle {
    Spaces,
    Tabs,
}

impl IndentStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spaces => "spaces",
            Self::Tabs => "tabs",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    Lf,
    Crlf,
    Preserve,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Crlf => "crlf",
            Self::Preserve => "preserve",
        }
    }
}

impl QuoteStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::AutoDouble => "auto-double",
            Self::Double => "double",
            Self::Single => "single",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatOptions {
    pub indent_style: IndentStyle,
    pub indent_width: usize,
    pub line_width: usize,
    pub quote_style: QuoteStyle,
    pub line_ending: LineEnding,
    pub final_newline: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_style: IndentStyle::Spaces,
            indent_width: 2,
            line_width: 100,
            quote_style: QuoteStyle::AutoDouble,
            line_ending: LineEnding::Lf,
            final_newline: true,
        }
    }
}

pub fn format_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
) -> Result<String, Vec<Diagnostic>> {
    format_source(path, source, options, ParsedLanguage::Tua)
}

/// Format authored source using the language selected by the project model.
pub fn format_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    language: ParsedLanguage,
) -> Result<String, Vec<Diagnostic>> {
    let source = source.into();
    if path
        .as_ref()
        .extension()
        .is_some_and(|extension| extension == "tdialogue")
    {
        return dialogue::format(path.as_ref(), &source, options);
    }
    let path = path.as_ref();
    let parsed = parse_source(path, source, language)?;

    stylua::format(&parsed, options, None)
}

pub fn format_tua_source_range(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    range: SourceSpan,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    format_source_range(path, source, options, range, ParsedLanguage::Tua)
}

pub fn format_source_range(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    range: SourceSpan,
    language: ParsedLanguage,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    let path = path.as_ref().to_owned();
    if path
        .extension()
        .is_some_and(|extension| extension == "tdialogue")
    {
        return Ok(Vec::new());
    }
    let source = source.into();
    let parsed = parse_source(&path, &source, language)?;
    let formatted = stylua::format(
        &parsed,
        options,
        Some(stylua_lib::Range::from_values(
            Some(source_byte_offset(&source, range.start)),
            Some(source_byte_offset(&source, range.end).saturating_sub(1)),
        )),
    )?;

    Ok(validated_local_format_edits(
        path, &source, &formatted, range, true, language,
    ))
}

pub fn format_tua_source_on_type(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    position: SourcePosition,
    trigger: &str,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    format_source_on_type(
        path,
        source,
        options,
        position,
        trigger,
        ParsedLanguage::Tua,
    )
}

pub fn format_source_on_type(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    position: SourcePosition,
    trigger: &str,
    language: ParsedLanguage,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    if path
        .as_ref()
        .extension()
        .is_some_and(|extension| extension == "tdialogue")
    {
        return Ok(Vec::new());
    }
    let source = source.into();
    if !is_on_type_format_trigger(&source, position, trigger) {
        return Ok(Vec::new());
    }

    let path = path.as_ref().to_owned();
    let formatted = format_source(&path, source.clone(), options, language)?;
    let line = on_type_target_line(position, trigger);
    let range = SourceSpan {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition {
            line: line + 1,
            column: 1,
        },
    };

    Ok(validated_local_format_edits(
        path, &source, &formatted, range, false, language,
    ))
}

/// Format an already parsed source; retain its source if output validation fails.
/// Use `format_tua_source` when formatting diagnostics are needed.
pub fn format_parsed_source(parsed: &ParsedSource, options: &FormatOptions) -> String {
    stylua::format(parsed, options, None).unwrap_or_else(|_| parsed.source.clone())
}

fn parse_source(
    path: &Path,
    source: impl Into<String>,
    language: ParsedLanguage,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    match language {
        ParsedLanguage::Lua => parse_lua_source(path, source),
        ParsedLanguage::Tua => parse_tua_source(path, source),
    }
}

fn validated_local_format_edits(
    path: std::path::PathBuf,
    source: &str,
    formatted: &str,
    range: SourceSpan,
    allow_line_count_change: bool,
    language: ParsedLanguage,
) -> Vec<TextEdit> {
    let edits = local_format_edits(
        path.clone(),
        source,
        formatted,
        range,
        allow_line_count_change,
    );
    if edits.is_empty() {
        return edits;
    }
    let mut edited = source.to_owned();
    for edit in edits.iter().rev() {
        let start = source_byte_offset(source, edit.span.start);
        let end = source_byte_offset(source, edit.span.end);
        edited.replace_range(start..end, &edit.new_text);
    }
    if parse_source(&path, &edited, language).is_ok()
        && stylua::equivalent_sources(source, &edited, language)
    {
        edits
    } else {
        Vec::new()
    }
}

fn local_format_edits(
    path: std::path::PathBuf,
    source: &str,
    formatted: &str,
    range: SourceSpan,
    allow_line_count_change: bool,
) -> Vec<TextEdit> {
    if source == formatted {
        return Vec::new();
    }

    let Some((start_line, end_line)) = selected_line_interval(range) else {
        return Vec::new();
    };
    let source_lines = line_segments(source);
    let formatted_lines = line_segments(formatted);

    if source_lines.len() == formatted_lines.len() {
        return changed_line_edits(
            path,
            &source_lines,
            &formatted_lines,
            start_line,
            end_line,
            range,
        );
    }

    if !allow_line_count_change || range.start.column != 1 || range.end.column != 1 {
        return Vec::new();
    }

    changed_line_block_edit(
        path,
        source,
        &source_lines,
        &formatted_lines,
        start_line,
        end_line,
    )
    .into_iter()
    .collect()
}

fn selected_line_interval(range: SourceSpan) -> Option<(usize, usize)> {
    if position_key(range.end) < position_key(range.start) {
        return None;
    }

    let start = range.start.line.checked_sub(1)?;
    let end = if range.end.column == 1 && range.end.line > range.start.line {
        range.end.line - 1
    } else {
        range.end.line
    };

    (start < end).then_some((start, end))
}

fn changed_line_edits(
    path: std::path::PathBuf,
    source_lines: &[&str],
    formatted_lines: &[&str],
    start_line: usize,
    end_line: usize,
    range: SourceSpan,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    for index in start_line..end_line.min(source_lines.len()).min(formatted_lines.len()) {
        let source_line = line_body(source_lines[index]);
        let formatted_line = line_body(formatted_lines[index]);

        let Some((span, new_text)) = changed_line_edit(index, source_line, formatted_line) else {
            continue;
        };
        if range_contains_span(range, span) {
            edits.push(TextEdit {
                path: path.clone(),
                span,
                new_text,
            });
        }
    }

    edits
}

fn changed_line_edit(
    line_index: usize,
    source: &str,
    formatted: &str,
) -> Option<(SourceSpan, String)> {
    if source == formatted {
        return None;
    }

    let prefix = common_prefix_chars(source, formatted);
    let suffix = common_suffix_chars(&source[prefix..], &formatted[prefix..]);
    let source_end = source.len().saturating_sub(suffix);
    let formatted_end = formatted.len().saturating_sub(suffix);

    Some((
        SourceSpan {
            start: SourcePosition {
                line: line_index + 1,
                column: source[..prefix].chars().count() + 1,
            },
            end: SourcePosition {
                line: line_index + 1,
                column: source[..source_end].chars().count() + 1,
            },
        },
        formatted[prefix..formatted_end].to_owned(),
    ))
}

fn changed_line_block_edit(
    path: std::path::PathBuf,
    source: &str,
    source_lines: &[&str],
    formatted_lines: &[&str],
    start_line: usize,
    end_line: usize,
) -> Option<TextEdit> {
    let prefix = common_prefix_lines(source_lines, formatted_lines);
    let suffix = common_suffix_lines(&source_lines[prefix..], &formatted_lines[prefix..]);
    let source_end = source_lines.len().saturating_sub(suffix);
    let formatted_end = formatted_lines.len().saturating_sub(suffix);

    if prefix < start_line || source_end > end_line {
        return None;
    }

    Some(TextEdit {
        path,
        span: full_line_span(source, prefix, source_end),
        new_text: formatted_lines[prefix..formatted_end].concat(),
    })
}

fn full_line_span(source: &str, start: usize, end: usize) -> SourceSpan {
    let document_end = document_end_position(source);
    let span_end = if end < line_segments(source).len() {
        SourcePosition {
            line: end + 1,
            column: 1,
        }
    } else {
        document_end
    };

    SourceSpan {
        start: SourcePosition {
            line: start + 1,
            column: 1,
        },
        end: span_end,
    }
}

fn line_segments(source: &str) -> Vec<&str> {
    if source.is_empty() {
        Vec::new()
    } else {
        source.split_inclusive('\n').collect()
    }
}

fn line_body(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

fn common_prefix_lines(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_lines(left: &[&str], right: &[&str]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_prefix_chars(left: &str, right: &str) -> usize {
    let mut prefix = 0;

    for ((left_index, left_char), (_, right_char)) in left.char_indices().zip(right.char_indices())
    {
        if left_char != right_char {
            break;
        }
        prefix = left_index + left_char.len_utf8();
    }

    prefix
}

fn common_suffix_chars(left: &str, right: &str) -> usize {
    let mut suffix = 0;

    for ((_, left_char), (_, right_char)) in
        left.char_indices().rev().zip(right.char_indices().rev())
    {
        if left_char != right_char {
            break;
        }
        suffix += left_char.len_utf8();
    }

    suffix
}

fn document_end_position(source: &str) -> SourcePosition {
    let mut line = 1usize;
    let mut column = 1usize;

    for ch in source.chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    SourcePosition { line, column }
}

fn range_contains_span(range: SourceSpan, span: SourceSpan) -> bool {
    position_key(range.start) <= position_key(span.start)
        && position_key(span.end) <= position_key(range.end)
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn is_on_type_format_trigger(source: &str, position: SourcePosition, trigger: &str) -> bool {
    match trigger {
        "\n" | "}" | ")" | "]" | "," => true,
        "d" => line_prefix_before_position(source, position)
            .is_some_and(|prefix| prefix.trim_end().ends_with("end")),
        _ => false,
    }
}

fn on_type_target_line(position: SourcePosition, trigger: &str) -> usize {
    if trigger == "\n" {
        position.line.saturating_sub(1).max(1)
    } else {
        position.line
    }
}

fn line_prefix_before_position(source: &str, position: SourcePosition) -> Option<&str> {
    let line = source.lines().nth(position.line.checked_sub(1)?)?;
    let byte = byte_for_column(line, position.column);

    Some(&line[..byte.min(line.len())])
}

fn byte_for_column(line: &str, column: usize) -> usize {
    let target = column.saturating_sub(1);

    line.char_indices()
        .nth(target)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

#[cfg(test)]
mod corpus;
#[cfg(test)]
mod tests;

fn source_byte_offset(source: &str, position: SourcePosition) -> usize {
    let mut offset = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index + 1 == position.line {
            return offset + byte_for_column(line, position.column);
        }
        offset += line.len();
    }
    source.len()
}
