use std::path::Path;

use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan, TextEdit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteStyle {
    Preserve,
}

impl QuoteStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatOptions {
    pub indent_width: usize,
    pub line_width: usize,
    pub quote_style: QuoteStyle,
    pub final_newline: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 2,
            line_width: 100,
            quote_style: QuoteStyle::Preserve,
            final_newline: true,
        }
    }
}

pub fn format_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
) -> Result<String, Vec<Diagnostic>> {
    let source = source.into();
    let parsed = parse_tua_source(path, source)?;

    Ok(format_parsed_source(&parsed, options))
}

pub fn format_tua_source_range(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    range: SourceSpan,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    let path = path.as_ref().to_owned();
    let source = source.into();
    let formatted = format_tua_source(&path, source.clone(), options)?;

    Ok(local_format_edits(path, &source, &formatted, range, true))
}

pub fn format_tua_source_on_type(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    options: &FormatOptions,
    position: SourcePosition,
    trigger: &str,
) -> Result<Vec<TextEdit>, Vec<Diagnostic>> {
    let source = source.into();
    if !is_on_type_format_trigger(&source, position, trigger) {
        return Ok(Vec::new());
    }

    let path = path.as_ref().to_owned();
    let formatted = format_tua_source(&path, source.clone(), options)?;
    let line = on_type_target_line(position, trigger);
    let range = SourceSpan {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition {
            line: line + 1,
            column: 1,
        },
    };

    Ok(local_format_edits(path, &source, &formatted, range, false))
}

pub fn format_parsed_source(parsed: &ParsedSource, options: &FormatOptions) -> String {
    Formatter::new(options).format(&parsed.source)
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

struct Formatter<'a> {
    options: &'a FormatOptions,
    indent_level: usize,
    paren_depth: usize,
    brace_depth: usize,
    paren_continuation_indents: Vec<usize>,
    pending_continuation: bool,
    paren_suppressed_block_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MaskState {
    Code,
    ShortString { quote: u8, escaped: bool },
    LongString { close: Vec<u8> },
}

impl<'a> Formatter<'a> {
    fn new(options: &'a FormatOptions) -> Self {
        Self {
            options,
            indent_level: 0,
            paren_depth: 0,
            brace_depth: 0,
            paren_continuation_indents: Vec::new(),
            pending_continuation: false,
            paren_suppressed_block_depth: 0,
        }
    }

    fn format(&mut self, source: &str) -> String {
        let mut output = String::new();

        for raw_line in source.split_inclusive('\n') {
            let line = raw_line.trim_end_matches(['\n', '\r']);
            self.format_line(line, &mut output);
        }

        if !source.ends_with('\n') {
            if let Some(last) = source.lines().last() {
                if source.is_empty() || !source.split_inclusive('\n').any(|line| line == last) {
                    self.format_line(last, &mut output);
                }
            }
        }

        if self.options.final_newline {
            while output.ends_with("\n\n") {
                output.pop();
            }

            if !output.ends_with('\n') {
                output.push('\n');
            }
        } else {
            while output.ends_with('\n') {
                output.pop();
            }
        }

        output
    }

    fn format_line(&mut self, line: &str, output: &mut String) {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            output.push('\n');
            return;
        }

        let code = code_before_comment(trimmed);
        let dedent = line_starts_with_dedent(code);
        let closes_suppressed_block = dedent && self.paren_suppressed_block_depth > 0;
        let suppress_paren_for_block = self.paren_suppressed_block_depth > 0;

        if dedent {
            self.indent_level = self.indent_level.saturating_sub(1);
        }

        let normalized = normalize_spacing(trimmed);
        let closes_paren = code.trim_start().starts_with(')');
        let closes_brace = code.trim_start().starts_with('}');
        let starts_with_continuation = line_starts_with_continuation(code);
        let suppress_paren_for_brace = self.brace_depth > 0 || closes_brace;
        let paren_indent = if suppress_paren_for_brace || suppress_paren_for_block {
            0
        } else {
            self.paren_depth.saturating_sub(usize::from(closes_paren))
        };
        let paren_extra = self.paren_continuation_indents.iter().sum::<usize>();
        let continuation_indent = usize::from(
            (self.pending_continuation || starts_with_continuation)
                && paren_indent == 0
                && !dedent
                && !closes_paren,
        );
        let indent_level = self.indent_level + paren_indent + paren_extra + continuation_indent;
        let indent = " ".repeat(indent_level * self.options.indent_width);
        let formatted = break_long_groups(&normalized, &indent, self.options);

        output.push_str(&formatted);
        output.push('\n');

        self.update_paren_state(code, continuation_indent);
        self.brace_depth = update_group_depth(self.brace_depth, code, b'{', b'}');
        self.pending_continuation = self.paren_depth == 0 && line_ends_with_continuation(code);

        if closes_suppressed_block {
            self.paren_suppressed_block_depth = self.paren_suppressed_block_depth.saturating_sub(1);
        }

        if line_opens_block(code) {
            if self.paren_depth > 0 {
                self.paren_suppressed_block_depth += 1;
            }
            self.indent_level += 1;
        }

        if line_is_middle_block(code) {
            self.indent_level += 1;
        }
    }

    fn update_paren_state(&mut self, code: &str, continuation_indent: usize) {
        let bytes = code.as_bytes();
        let mut state = MaskState::Code;
        let mut index = 0;

        while index < bytes.len() {
            match &mut state {
                MaskState::Code => {
                    if bytes[index] == b'\'' || bytes[index] == b'"' {
                        state = MaskState::ShortString {
                            quote: bytes[index],
                            escaped: false,
                        };
                        index += 1;
                        continue;
                    }
                    if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                        state = MaskState::LongString { close };
                        index += consumed;
                        continue;
                    }

                    match bytes[index] {
                        b'(' => {
                            self.paren_depth += 1;
                            self.paren_continuation_indents.push(continuation_indent);
                        }
                        b')' => {
                            self.paren_depth = self.paren_depth.saturating_sub(1);
                            self.paren_continuation_indents.pop();
                        }
                        _ => {}
                    }

                    index += 1;
                }
                MaskState::ShortString { quote, escaped } => {
                    if *escaped {
                        *escaped = false;
                    } else if bytes[index] == b'\\' {
                        *escaped = true;
                    } else if bytes[index] == *quote {
                        state = MaskState::Code;
                    }
                    index += 1;
                }
                MaskState::LongString { close } => {
                    if bytes[index..].starts_with(close) {
                        index += close.len();
                        state = MaskState::Code;
                    } else {
                        index += 1;
                    }
                }
            }
        }
    }
}

fn normalize_spacing(line: &str) -> String {
    let mut state = MaskState::Code;
    let mut output = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    output.push_str(line[index..].trim_end());
                    break;
                }

                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    output.push(bytes[index] as char);
                    index += 1;
                    continue;
                }

                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    output.push_str(&line[index..index + consumed]);
                    index += consumed;
                    continue;
                }

                let operator = assignment_operator_at(bytes, index);
                if let Some(operator) = operator {
                    trim_trailing_spaces(&mut output);
                    output.push(' ');
                    output.push_str(operator);
                    output.push(' ');
                    index += operator.len();
                    while bytes
                        .get(index)
                        .is_some_and(|byte| byte.is_ascii_whitespace())
                    {
                        index += 1;
                    }
                    continue;
                }

                match bytes[index] {
                    b',' => {
                        trim_trailing_spaces(&mut output);
                        output.push_str(", ");
                        index += 1;
                        while bytes
                            .get(index)
                            .is_some_and(|byte| byte.is_ascii_whitespace())
                        {
                            index += 1;
                        }
                    }
                    b'(' => {
                        if should_trim_before_call_paren(&output) {
                            trim_trailing_spaces(&mut output);
                        }
                        output.push(bytes[index] as char);
                        index += 1;
                        while bytes
                            .get(index)
                            .is_some_and(|byte| byte.is_ascii_whitespace())
                        {
                            index += 1;
                        }
                    }
                    b'[' => {
                        trim_trailing_spaces(&mut output);
                        output.push(bytes[index] as char);
                        index += 1;
                        while bytes
                            .get(index)
                            .is_some_and(|byte| byte.is_ascii_whitespace())
                        {
                            index += 1;
                        }
                    }
                    b')' | b']' => {
                        trim_trailing_spaces(&mut output);
                        output.push(bytes[index] as char);
                        index += 1;
                    }
                    byte if byte.is_ascii_whitespace() => {
                        if !output.ends_with(' ') {
                            output.push(' ');
                        }
                        index += 1;
                    }
                    byte => {
                        output.push(byte as char);
                        index += 1;
                    }
                }
            }
            MaskState::ShortString { quote, escaped } => {
                output.push(bytes[index] as char);

                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }

                index += 1;
            }
            MaskState::LongString { close } => {
                output.push(bytes[index] as char);

                if bytes[index..].starts_with(close) {
                    for byte in close.iter().skip(1) {
                        output.push(*byte as char);
                    }
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    output.trim_end().to_owned()
}

fn break_long_groups(line: &str, indent: &str, options: &FormatOptions) -> String {
    let inline = format!("{indent}{line}");

    if inline.len() <= options.line_width {
        return inline;
    }

    break_long_table(line, indent, options)
        .or_else(|| break_long_call(line, indent, options))
        .unwrap_or(inline)
}

fn break_long_table(line: &str, indent: &str, options: &FormatOptions) -> Option<String> {
    let (open, close) = outer_group(line, b'{', b'}')?;
    let before = line[..open].trim_end();
    let inner = &line[open + 1..close];
    let after = line[close + 1..].trim_start();
    let items = split_top_level(inner, b',');

    if items.len() < 2 {
        return None;
    }

    let item_indent = format!("{indent}{}", " ".repeat(options.indent_width));
    let mut output = String::new();

    output.push_str(indent);
    output.push_str(before);
    if !before.is_empty() {
        output.push(' ');
    }
    output.push_str("{\n");

    for item in items {
        output.push_str(&item_indent);
        output.push_str(item.trim());
        output.push_str(",\n");
    }

    output.push_str(indent);
    output.push('}');
    output.push_str(after);

    Some(output)
}

fn break_long_call(line: &str, indent: &str, options: &FormatOptions) -> Option<String> {
    let (open, close) = outer_group(line, b'(', b')')?;
    let callee = line[..open].trim_end();

    if callee.is_empty() {
        return None;
    }

    let inner = &line[open + 1..close];
    let after = line[close + 1..].trim_start();
    let args = split_top_level(inner, b',');

    if args.len() < 2 {
        return None;
    }

    let item_indent = format!("{indent}{}", " ".repeat(options.indent_width));
    let mut output = String::new();

    output.push_str(indent);
    output.push_str(callee);
    output.push_str("(\n");

    let last_index = args.len().saturating_sub(1);
    for (index, arg) in args.into_iter().enumerate() {
        output.push_str(&item_indent);
        output.push_str(arg.trim());
        if index != last_index {
            output.push(',');
        }
        output.push('\n');
    }

    output.push_str(indent);
    output.push(')');
    output.push_str(after);

    Some(output)
}

fn outer_group(line: &str, open: u8, close: u8) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let open_index = top_level_byte(line, open)?;
    let mut state = MaskState::Code;
    let mut depth = 0usize;
    let mut index = open_index;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    return None;
                }
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close_marker, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString {
                        close: close_marker,
                    };
                    index += consumed;
                    continue;
                }
                if bytes[index] == open {
                    depth += 1;
                } else if bytes[index] == close {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some((open_index, index));
                    }
                }
                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    None
}

fn split_top_level(value: &str, separator: u8) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut state = MaskState::Code;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    index += consumed;
                    continue;
                }
                match bytes[index] {
                    b'(' | b'{' | b'[' => depth += 1,
                    b')' | b'}' | b']' => depth = depth.saturating_sub(1),
                    byte if byte == separator && depth == 0 => {
                        items.push(value[start..index].trim());
                        start = index + 1;
                    }
                    _ => {}
                }
                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    let tail = value[start..].trim();

    if !tail.is_empty() {
        items.push(tail);
    }

    items
}

fn top_level_byte(line: &str, needle: u8) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut state = MaskState::Code;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    return None;
                }
                if bytes[index] == needle {
                    return Some(index);
                }
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    index += consumed;
                    continue;
                }
                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    None
}

fn line_starts_with_dedent(code: &str) -> bool {
    let code = code.trim_start();

    starts_with_word(code, "end")
        || starts_with_word(code, "else")
        || starts_with_word(code, "elseif")
        || starts_with_word(code, "until")
        || code.starts_with('}')
}

fn line_is_middle_block(code: &str) -> bool {
    let code = code.trim_start();

    starts_with_word(code, "else") || starts_with_word(code, "elseif")
}

fn line_opens_block(code: &str) -> bool {
    let code = code.trim();

    if code.is_empty() || line_is_middle_block(code) {
        return false;
    }

    let closes_same_line = code_contains_word(code, "end") || code_contains_word(code, "until");

    !closes_same_line
        && (code_contains_word(code, "function")
            || starts_with_word(code, "if") && code_contains_word(code, "then")
            || starts_with_word(code, "for") && code_contains_word(code, "do")
            || starts_with_word(code, "while") && code_contains_word(code, "do")
            || starts_with_word(code, "do")
            || starts_with_word(code, "repeat")
            || starts_with_word(code, "enum")
            || line_opens_brace_block(code))
}

fn line_opens_brace_block(code: &str) -> bool {
    let bytes = code.as_bytes();
    let mut state = MaskState::Code;
    let mut depth = 0usize;
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    break;
                }
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    index += consumed;
                    continue;
                }

                match bytes[index] {
                    b'{' => {
                        depth += 1;
                        opens += 1;
                    }
                    b'}' => {
                        depth = depth.saturating_sub(1);
                        closes += 1;
                    }
                    _ => {}
                }

                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    depth > 0 && opens > closes
}

fn update_group_depth(current: usize, code: &str, open: u8, close: u8) -> usize {
    let bytes = code.as_bytes();
    let mut state = MaskState::Code;
    let mut depth = current;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    index += consumed;
                    continue;
                }

                match bytes[index] {
                    byte if byte == open => depth += 1,
                    byte if byte == close => depth = depth.saturating_sub(1),
                    _ => {}
                }

                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    depth
}

fn line_ends_with_continuation(code: &str) -> bool {
    let code = code.trim_end();

    if code.is_empty()
        || code.ends_with("==")
        || code.ends_with("<=")
        || code.ends_with(">=")
        || code.ends_with("~=")
    {
        return false;
    }

    code.as_bytes()
        .last()
        .is_some_and(|byte| matches!(byte, b'=' | b'+' | b'-' | b'*' | b'/' | b'%' | b'^'))
}

fn line_starts_with_continuation(code: &str) -> bool {
    code.trim_start().starts_with("..")
}

fn starts_with_word(value: &str, word: &str) -> bool {
    value.strip_prefix(word).is_some_and(|rest| {
        rest.as_bytes()
            .first()
            .is_none_or(|byte| !is_identifier_continue(*byte))
    })
}

fn code_contains_word(value: &str, word: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|part| part == word)
}

fn code_before_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut state = MaskState::Code;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    return &line[..index];
                }
                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    state = MaskState::ShortString {
                        quote: bytes[index],
                        escaped: false,
                    };
                    index += 1;
                    continue;
                }
                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    index += consumed;
                    continue;
                }
                index += 1;
            }
            MaskState::ShortString { quote, escaped } => {
                if *escaped {
                    *escaped = false;
                } else if bytes[index] == b'\\' {
                    *escaped = true;
                } else if bytes[index] == *quote {
                    state = MaskState::Code;
                }
                index += 1;
            }
            MaskState::LongString { close } => {
                if bytes[index..].starts_with(close) {
                    index += close.len();
                    state = MaskState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    line
}

fn assignment_operator_at(bytes: &[u8], index: usize) -> Option<&'static str> {
    const OPERATORS: &[&str] = &["//=", "..=", "+=", "-=", "*=", "/=", "%=", "^=", "="];

    for operator in OPERATORS {
        let operator_bytes = operator.as_bytes();

        if bytes[index..].starts_with(operator_bytes)
            && !matches!(
                (
                    bytes.get(index.wrapping_sub(1)),
                    bytes.get(index + operator_bytes.len())
                ),
                (Some(b'=' | b'<' | b'>' | b'~'), _) | (_, Some(b'='))
            )
        {
            return Some(operator);
        }
    }

    None
}

fn trim_trailing_spaces(output: &mut String) {
    while output.ends_with(' ') {
        output.pop();
    }
}

fn should_trim_before_call_paren(output: &str) -> bool {
    output.trim_end().as_bytes().last().is_some_and(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b')' | b']' | b'}')
    })
}

fn long_bracket_open(bytes: &[u8]) -> Option<(Vec<u8>, usize)> {
    if bytes.first() != Some(&b'[') {
        return None;
    }

    let mut index = 1;

    while bytes.get(index) == Some(&b'=') {
        index += 1;
    }

    if bytes.get(index) != Some(&b'[') {
        return None;
    }

    let mut close = Vec::with_capacity(index + 1);
    close.push(b']');
    close.extend(std::iter::repeat_n(b'=', index.saturating_sub(1)));
    close.push(b']');

    Some((close, index + 1))
}

fn is_identifier_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests;
