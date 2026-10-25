use std::{borrow::Cow, path::Path};

use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan, TextEdit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuoteStyle {
    Preserve,
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
            quote_style: QuoteStyle::Preserve,
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
    open_parens: Vec<OpenParen>,
    line_number: usize,
    pending_continuation: bool,
    suppressed_paren_blocks: Vec<SuppressedParenBlock>,
    multiline_function_header_depths: Vec<usize>,
    split_control_header: Option<SplitControlHeader>,
    multiline_state: MultilineState,
    has_content: bool,
    pending_blank_line: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MaskState {
    Code,
    ShortString { quote: u8, escaped: bool },
    LongString { close: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MultilineState {
    Code,
    ShortString { quote: u8 },
    LongBracket { close: Vec<u8> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SplitControlHeader {
    Then,
    Do,
    UntilCondition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenParen {
    line: usize,
    continuation_indent: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SuppressedParenBlock {
    depth: usize,
    promoted_indent: usize,
}

impl<'a> Formatter<'a> {
    fn new(options: &'a FormatOptions) -> Self {
        Self {
            options,
            indent_level: 0,
            paren_depth: 0,
            brace_depth: 0,
            open_parens: Vec::new(),
            line_number: 0,
            pending_continuation: false,
            suppressed_paren_blocks: Vec::new(),
            multiline_function_header_depths: Vec::new(),
            split_control_header: None,
            multiline_state: MultilineState::Code,
            has_content: false,
            pending_blank_line: false,
        }
    }

    fn format(&mut self, source: &str) -> String {
        let mut output = String::new();

        for raw_line in source.split_inclusive('\n') {
            let line = raw_line.trim_end_matches(['\n', '\r']);
            self.format_source_line(line, &mut output);
        }

        if !source.ends_with('\n') {
            if let Some(last) = source.lines().last() {
                if source.is_empty() || !source.split_inclusive('\n').any(|line| line == last) {
                    self.format_source_line(last, &mut output);
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

        let line_ending = match self.options.line_ending {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
            LineEnding::Preserve if source.contains("\r\n") => "\r\n",
            LineEnding::Preserve => "\n",
        };
        if line_ending == "\r\n" {
            output.replace('\n', line_ending)
        } else {
            output
        }
    }

    fn format_source_line(&mut self, line: &str, output: &mut String) {
        if !matches!(self.multiline_state, MultilineState::Code) {
            output.push_str(line);
            output.push('\n');
            self.has_content = true;
            self.pending_blank_line = false;
            self.update_multiline_state(line);
            return;
        }

        let is_blank = line.trim().is_empty();

        if is_blank {
            if self.has_content {
                self.pending_blank_line = true;
            }
        } else {
            if self.pending_blank_line {
                output.push('\n');
                self.pending_blank_line = false;
            }
            self.format_line(line, output);
            self.has_content = true;
        }

        self.update_multiline_state(line);
    }

    fn update_multiline_state(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut index = 0;

        while index < bytes.len() {
            match &mut self.multiline_state {
                MultilineState::Code => {
                    if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                        if let Some((close, consumed)) = long_bracket_open(&bytes[index + 2..]) {
                            self.multiline_state = MultilineState::LongBracket { close };
                            index += 2 + consumed;
                            continue;
                        }
                        break;
                    }

                    if bytes[index] == b'\'' || bytes[index] == b'"' {
                        let quote = bytes[index];
                        index += 1;
                        let mut escaped = false;
                        let mut closed = false;

                        while index < bytes.len() {
                            if escaped {
                                escaped = false;
                            } else if bytes[index] == b'\\' {
                                escaped = true;
                            } else if bytes[index] == quote {
                                index += 1;
                                closed = true;
                                break;
                            }
                            index += 1;
                        }
                        if !closed && escaped {
                            self.multiline_state = MultilineState::ShortString { quote };
                        }
                        continue;
                    }

                    if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                        self.multiline_state = MultilineState::LongBracket { close };
                        index += consumed;
                    } else {
                        index += 1;
                    }
                }
                MultilineState::LongBracket { close } => {
                    if bytes[index..].starts_with(close) {
                        index += close.len();
                        self.multiline_state = MultilineState::Code;
                    } else {
                        index += 1;
                    }
                }
                MultilineState::ShortString { quote } => {
                    let mut escaped = false;
                    while index < bytes.len() {
                        if escaped {
                            escaped = false;
                        } else if bytes[index] == b'\\' {
                            escaped = true;
                        } else if bytes[index] == *quote {
                            index += 1;
                            self.multiline_state = MultilineState::Code;
                            break;
                        }
                        index += 1;
                    }
                }
            }
        }
    }

    fn format_line(&mut self, line: &str, output: &mut String) {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            output.push('\n');
            return;
        }

        let code = code_before_comment(trimmed);
        let code = code.as_ref();
        let split_header = self.split_control_header;
        let closes_split_header = split_header.is_some_and(|header| header.closes_with(code));
        let opens_split_header = split_control_header(code);
        let dedent = line_starts_with_dedent(code);
        let middle_block = line_is_middle_block(code);
        let closes_suppressed_block =
            dedent && !middle_block && !self.suppressed_paren_blocks.is_empty();
        let suppressed_paren_depth = self
            .suppressed_paren_blocks
            .last()
            .map_or(0, |block| block.depth);

        if dedent {
            self.indent_level = self.indent_level.saturating_sub(1);
        }

        let normalized = normalize_spacing(trimmed, self.options.quote_style);
        let closes_paren = code.trim_start().starts_with(')');
        let closes_brace = code.trim_start().starts_with('}');
        let starts_with_continuation = line_starts_with_continuation(code);
        // Parentheses around an open table are already folded into its block
        // indent through `suppressed_paren_blocks`; a call inside a table
        // still indents its own arguments.
        let paren_indent = if closes_brace {
            0
        } else {
            self.paren_levels(self.paren_depth)
                .saturating_sub(usize::from(closes_paren))
                .saturating_sub(self.paren_levels(suppressed_paren_depth))
        };
        let paren_extra = self
            .open_parens
            .iter()
            .enumerate()
            .filter(|(index, paren)| self.paren_starts_level(*index, paren))
            .map(|(_, paren)| paren.continuation_indent)
            .sum::<usize>();
        let continuation_indent = usize::from(
            (self.pending_continuation || starts_with_continuation)
                && paren_indent == 0
                && !dedent
                && !closes_paren
                && split_header.is_none(),
        );
        let closes_function_header = closes_paren
            && self
                .multiline_function_header_depths
                .last()
                .is_some_and(|depth| *depth == self.paren_depth);
        let split_header_indent = usize::from(
            split_header.is_some()
                && paren_indent == 0
                && (!closes_split_header
                    || (!split_header.is_some_and(|header| header.is_bare_closer(code))
                        && !closes_paren)),
        );
        let indent_level = self
            .indent_level
            .saturating_sub(usize::from(closes_function_header))
            + paren_indent
            + paren_extra
            + continuation_indent
            + split_header_indent;
        let indent = match self.options.indent_style {
            IndentStyle::Spaces => " ".repeat(indent_level * self.options.indent_width),
            IndentStyle::Tabs => "\t".repeat(indent_level),
        };
        let formatted = break_long_groups(&normalized, &indent, self.options);

        output.push_str(&formatted);
        output.push('\n');

        self.update_paren_state(code, continuation_indent);
        self.line_number += 1;
        if closes_function_header {
            self.multiline_function_header_depths.pop();
            // The parameter list no longer contributes a parenthesis level
            // inside the body. Keep only enclosing calls suppressed.
            if let Some(block) = self.suppressed_paren_blocks.last_mut() {
                block.depth = self.paren_depth;
            }
        }
        self.brace_depth = update_group_depth(self.brace_depth, code, b'{', b'}');
        // A comma outside every group continues an expression list, such as
        // the values of `return a,` or the names of `local a,`.
        self.pending_continuation = self.paren_depth == 0
            && (line_ends_with_continuation(code)
                || (self.brace_depth == 0 && code.trim_end().ends_with(',')));

        if closes_suppressed_block {
            let suppressed = self
                .suppressed_paren_blocks
                .pop()
                .expect("suppressed block should retain its promoted indent");
            self.indent_level = self.indent_level.saturating_sub(suppressed.promoted_indent);
        }

        if line_opens_block(code) || closes_split_header {
            if self.paren_depth > 0 {
                self.indent_level += paren_indent;
                self.suppressed_paren_blocks.push(SuppressedParenBlock {
                    depth: self.paren_depth,
                    promoted_indent: paren_indent,
                });
                if function_parameter_list_is_open(code) {
                    self.multiline_function_header_depths.push(self.paren_depth);
                }
            }
            self.indent_level += 1;
        }

        if middle_block && opens_split_header.is_none() {
            self.indent_level += 1;
        }

        self.split_control_header = match (split_header, closes_split_header) {
            (Some(SplitControlHeader::UntilCondition), false) if !code.trim().is_empty() => {
                line_ends_with_continuation(code).then_some(SplitControlHeader::UntilCondition)
            }
            (Some(_), true) => None,
            (Some(header), false) => Some(header),
            (None, _) => opens_split_header,
        };
    }

    /// Parentheses opened on the same line, as in `print(lookup(`, share one
    /// indentation level, and a line that starts with `)` closes that level.
    fn paren_levels(&self, depth: usize) -> usize {
        self.open_parens
            .iter()
            .take(depth)
            .enumerate()
            .filter(|(index, paren)| self.paren_starts_level(*index, paren))
            .count()
    }

    fn paren_starts_level(&self, index: usize, paren: &OpenParen) -> bool {
        index == 0 || self.open_parens[index - 1].line != paren.line
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
                            self.open_parens.push(OpenParen {
                                line: self.line_number,
                                continuation_indent,
                            });
                        }
                        b')' => {
                            self.paren_depth = self.paren_depth.saturating_sub(1);
                            self.open_parens.pop();
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

impl SplitControlHeader {
    fn closes_with(self, code: &str) -> bool {
        let code = code.trim();

        match self {
            Self::Then => code_contains_word(code, "then"),
            Self::Do => code_contains_word(code, "do"),
            Self::UntilCondition => false,
        }
    }

    fn is_bare_closer(self, code: &str) -> bool {
        match self {
            Self::Then => code.trim() == "then",
            Self::Do => code.trim() == "do",
            Self::UntilCondition => false,
        }
    }
}

fn split_control_header(code: &str) -> Option<SplitControlHeader> {
    let code = code.trim();
    if (starts_with_word(code, "if") || starts_with_word(code, "elseif"))
        && !code_contains_word(code, "then")
    {
        Some(SplitControlHeader::Then)
    } else if (starts_with_word(code, "while") || starts_with_word(code, "for"))
        && !code_contains_word(code, "do")
    {
        Some(SplitControlHeader::Do)
    } else if code == "until" {
        Some(SplitControlHeader::UntilCondition)
    } else {
        None
    }
}

fn normalize_spacing(line: &str, quote_style: QuoteStyle) -> String {
    let mut state = MaskState::Code;
    let mut output = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if !bytes[index].is_ascii() {
            let character = line[index..]
                .chars()
                .next()
                .expect("formatter index should remain on a UTF-8 boundary");
            output.push(character);
            index += character.len_utf8();
            continue;
        }

        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    if !output.is_empty() && !output.ends_with(' ') {
                        output.push(' ');
                    }
                    output.push_str(line[index..].trim_end());
                    break;
                }

                if bytes[index] == b'\'' || bytes[index] == b'"' {
                    if let Some(end) = short_string_end(bytes, index) {
                        output.push_str(&format_short_string(&line[index..=end], quote_style));
                        index = end + 1;
                        continue;
                    }

                    output.push_str(&line[index..]);
                    break;
                }

                if let Some((close, consumed)) = long_bracket_open(&bytes[index..]) {
                    state = MaskState::LongString { close };
                    output.push_str(&line[index..index + consumed]);
                    index += consumed;
                    continue;
                }

                let operator = assignment_operator_at(bytes, index);
                if let Some(operator) = operator {
                    push_spaced_operator(&mut output, operator);
                    index += operator.len();
                    while bytes
                        .get(index)
                        .is_some_and(|byte| byte.is_ascii_whitespace())
                    {
                        index += 1;
                    }
                    continue;
                }

                if let Some(operator) = binary_operator_at(bytes, index, &output) {
                    push_spaced_operator(&mut output, operator);
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
                        if should_space_before_group_paren(&output) {
                            trim_trailing_spaces(&mut output);
                            output.push(' ');
                        } else if should_trim_before_call_paren(&output) {
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
                        // Only an index hugs its target; a `[key]` table field
                        // keeps the space after `{` or `,`.
                        if previous_token_is_indexable(&output) {
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
                    // A one-line table pads its contents, as in `{ 1, 2 }`,
                    // while an empty table stays `{}`.
                    b'{' => {
                        output.push('{');
                        index += 1;
                        while bytes
                            .get(index)
                            .is_some_and(|byte| byte.is_ascii_whitespace())
                        {
                            index += 1;
                        }
                        if bytes.get(index).is_some_and(|byte| *byte != b'}') {
                            output.push(' ');
                        }
                    }
                    b'}' => {
                        trim_trailing_spaces(&mut output);
                        if !output.is_empty() && !output.ends_with('{') {
                            output.push(' ');
                        }
                        output.push('}');
                        index += 1;
                    }
                    b')' | b']' => {
                        trim_trailing_spaces(&mut output);
                        output.push(bytes[index] as char);
                        index += 1;
                    }
                    byte @ (b'-' | b'#') if !output.is_empty() => {
                        // A unary operator hugs its operand, except that
                        // `- -x` keeps its space so it never becomes `--x`.
                        // A line that starts with `-` may continue a binary
                        // expression from the previous line, so it is kept.
                        output.push(byte as char);
                        index += 1;
                        let mut operand = index;
                        while bytes
                            .get(operand)
                            .is_some_and(|byte| byte.is_ascii_whitespace())
                        {
                            operand += 1;
                        }
                        if bytes.get(operand) != Some(&b'-') {
                            index = operand;
                        }
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

fn short_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate().skip(start + 1) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == quote {
            return Some(index);
        }
    }

    None
}

fn format_short_string(literal: &str, style: QuoteStyle) -> String {
    let source_quote = literal
        .chars()
        .next()
        .expect("short string literal should have an opening quote");
    let target_quote = match style {
        QuoteStyle::Preserve => return literal.to_owned(),
        QuoteStyle::Double => '"',
        QuoteStyle::Single => '\'',
    };
    if source_quote == target_quote {
        return literal.to_owned();
    }

    let content = &literal[source_quote.len_utf8()..literal.len() - source_quote.len_utf8()];
    let mut output = String::with_capacity(literal.len() + 2);
    output.push(target_quote);
    let mut characters = content.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            let Some(escaped) = characters.next() else {
                output.push('\\');
                break;
            };
            if escaped == source_quote {
                output.push(source_quote);
            } else {
                output.push('\\');
                output.push(escaped);
            }
        } else if character == target_quote {
            output.push('\\');
            output.push(character);
        } else {
            output.push(character);
        }
    }
    output.push(target_quote);
    output
}

fn break_long_groups(line: &str, indent: &str, options: &FormatOptions) -> String {
    let inline = format!("{indent}{line}");

    if inline.chars().count() <= options.line_width {
        return inline;
    }

    let keeps_blocks = |wrapped: &String| wrap_keeps_one_line_blocks(line, wrapped);
    let table = || break_long_table(line, indent, options).filter(keeps_blocks);
    let call = || break_long_call(line, indent, options).filter(keeps_blocks);
    // Split the outermost group first. Expanding a table passed as an
    // argument would leave the arguments after it on one long line.
    let call_opens_first = matches!(
        (top_level_byte(line, b'('), top_level_byte(line, b'{')),
        (Some(paren), Some(brace)) if paren < brace
    );
    let wrapped = if call_opens_first {
        call().or_else(table)
    } else {
        table().or_else(call)
    };

    wrapped.unwrap_or(inline)
}

fn break_long_table(line: &str, indent: &str, options: &FormatOptions) -> Option<String> {
    let (open, close) = outer_group(line, b'{', b'}')?;
    let before = line[..open].trim_end();
    let inner = &line[open + 1..close];
    let after = &line[close + 1..];
    let items = split_top_level(inner, b',');

    if items.len() < 2 {
        return None;
    }

    let item_indent = format!("{indent}{}", indent_unit(options));
    let mut output = String::new();

    output.push_str(indent);
    output.push_str(before);
    if !before.is_empty()
        && !before
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b'(' | b'[' | b'{'))
    {
        output.push(' ');
    }
    output.push_str("{\n");

    // Items are wrapped in turn, so one pass reaches the layout that a second
    // pass would otherwise produce for a nested group that is still too long.
    for item in items {
        output.push_str(&break_long_groups(
            &format!("{},", item.trim()),
            &item_indent,
            options,
        ));
        output.push('\n');
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
    let after = &line[close + 1..];
    let args = split_top_level(inner, b',');

    if args.len() < 2 {
        return None;
    }

    let item_indent = format!("{indent}{}", indent_unit(options));
    let mut output = String::new();

    output.push_str(indent);
    output.push_str(callee);
    output.push_str("(\n");

    let last_index = args.len().saturating_sub(1);
    for (index, arg) in args.into_iter().enumerate() {
        let separator = if index == last_index { "" } else { "," };
        output.push_str(&break_long_groups(
            &format!("{}{separator}", arg.trim()),
            &item_indent,
            options,
        ));
        output.push('\n');
    }

    output.push_str(indent);
    output.push(')');
    output.push_str(after);

    Some(output)
}

fn indent_unit(options: &FormatOptions) -> String {
    match options.indent_style {
        IndentStyle::Spaces => " ".repeat(options.indent_width),
        IndentStyle::Tabs => "\t".to_owned(),
    }
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
    line_can_open_block(code)
        && (line_opens_keyword_block(code) || line_opens_brace_block(code.trim()))
}

fn line_can_open_block(code: &str) -> bool {
    let code = code.trim();

    !code.is_empty()
        && !line_is_middle_block(code)
        && !code_contains_word(code, "end")
        && !code_contains_word(code, "until")
}

fn line_opens_keyword_block(code: &str) -> bool {
    let code = code.trim();

    code_contains_word(code, "function")
        || starts_with_word(code, "if") && code_contains_word(code, "then")
        || starts_with_word(code, "for") && code_contains_word(code, "do")
        || starts_with_word(code, "while") && code_contains_word(code, "do")
        || starts_with_word(code, "do")
        || starts_with_word(code, "repeat")
        || starts_with_word(code, "enum")
}

/// A block that opens and closes on one line, such as
/// `function(a, b) return a end`, must not be split: the wrapped header would
/// open a block whose `end` sits on a line that does not close it.
fn wrap_keeps_one_line_blocks(line: &str, wrapped: &str) -> bool {
    let opens_keyword_block = |code: &str| {
        let code = code_before_comment(code.trim());
        line_can_open_block(&code) && line_opens_keyword_block(&code)
    };

    opens_keyword_block(line) || !wrapped.lines().any(opens_keyword_block)
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

    if code.is_empty() {
        return false;
    }

    // `...` is a complete vararg expression; `..` still needs its right operand.
    (code.ends_with("..") && !code.ends_with("..."))
        || code.as_bytes().last().is_some_and(|byte| {
            matches!(
                byte,
                b'=' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'%' | b'^'
            )
        })
        || code
            .split_whitespace()
            .next_back()
            .is_some_and(|word| matches!(word, "and" | "or" | "not"))
}

fn line_starts_with_continuation(code: &str) -> bool {
    let code = code.trim_start();

    // No statement starts with an operator, field or method access, or a type
    // union bar, so such a line continues the previous statement.
    code.starts_with(['.', ':', '+', '-', '*', '/', '%', '^', '=', '<', '>', '|'])
        || code.starts_with("~=")
        || starts_with_word(code, "and")
        || starts_with_word(code, "or")
}

fn starts_with_word(value: &str, word: &str) -> bool {
    value.strip_prefix(word).is_some_and(|rest| {
        rest.as_bytes()
            .first()
            .is_none_or(|byte| !is_identifier_continue(*byte))
    })
}

fn code_contains_word(value: &str, word: &str) -> bool {
    let bytes = value.as_bytes();
    let mut state = MaskState::Code;
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
                if is_identifier_continue(bytes[index]) {
                    let start = index;
                    index += 1;
                    while bytes
                        .get(index)
                        .is_some_and(|byte| is_identifier_continue(*byte))
                    {
                        index += 1;
                    }
                    if &value[start..index] == word {
                        return true;
                    }
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

    false
}

fn function_parameter_list_is_open(code: &str) -> bool {
    let bytes = code.as_bytes();
    let mut state = MaskState::Code;
    let mut function_start = None;
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
                if is_identifier_continue(bytes[index]) {
                    let start = index;
                    index += 1;
                    while bytes
                        .get(index)
                        .is_some_and(|byte| is_identifier_continue(*byte))
                    {
                        index += 1;
                    }
                    if &code[start..index] == "function" {
                        function_start = Some(start);
                    }
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

    function_start.is_some_and(|start| update_group_depth(0, &code[start..], b'(', b')') > 0)
}

/// Returns the code of a line without its comments. A block comment that
/// closes on the same line is replaced by a space, so code after it still
/// decides whether the line continues the statement.
fn code_before_comment(line: &str) -> Cow<'_, str> {
    let bytes = line.as_bytes();
    let mut state = MaskState::Code;
    let mut code = String::new();
    let mut copied = 0;
    let mut index = 0;

    while index < bytes.len() {
        match &mut state {
            MaskState::Code => {
                if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
                    let comment_end =
                        long_bracket_open(&bytes[index + 2..]).and_then(|(close, consumed)| {
                            let body = index + 2 + consumed;
                            find_subslice(&bytes[body..], &close)
                                .map(|offset| body + offset + close.len())
                        });
                    let Some(comment_end) = comment_end else {
                        if copied == 0 {
                            return Cow::Borrowed(&line[..index]);
                        }
                        code.push_str(&line[copied..index]);
                        return Cow::Owned(code);
                    };
                    code.push_str(&line[copied..index]);
                    code.push(' ');
                    copied = comment_end;
                    index = comment_end;
                    continue;
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

    if copied == 0 {
        Cow::Borrowed(line)
    } else {
        code.push_str(&line[copied..]);
        Cow::Owned(code)
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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

fn binary_operator_at(bytes: &[u8], index: usize, output: &str) -> Option<&'static str> {
    if bytes[index..].starts_with(b"->") {
        return Some("->");
    }

    for operator in [
        "==", "~=", "<=", ">=", "//", "..", "+", "-", "*", "/", "%", "^", "<", ">",
    ] {
        let operator_bytes = operator.as_bytes();
        if !bytes[index..].starts_with(operator_bytes) {
            continue;
        }
        if operator == ".."
            && (bytes.get(index.wrapping_sub(1)) == Some(&b'.')
                || bytes.get(index + 2) == Some(&b'.'))
        {
            continue;
        }
        if matches!(operator, "+" | "-") && sign_belongs_to_number(bytes, index) {
            continue;
        }
        if matches!(operator, "+" | "-") && !previous_token_allows_binary_operator(output) {
            continue;
        }
        return Some(operator);
    }

    None
}

fn sign_belongs_to_number(bytes: &[u8], index: usize) -> bool {
    let Some(exponent) = index.checked_sub(1).and_then(|index| bytes.get(index)) else {
        return false;
    };
    if !matches!(exponent, b'e' | b'E' | b'p' | b'P')
        || !bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
    {
        return false;
    }

    index
        .checked_sub(2)
        .and_then(|index| bytes.get(index))
        .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'.')
}

fn previous_token_allows_binary_operator(output: &str) -> bool {
    let output = output.trim_end();
    let Some(last) = output.as_bytes().last() else {
        return false;
    };
    if matches!(last, b')' | b']' | b'}' | b'\'' | b'"') || last.is_ascii_digit() {
        return true;
    }
    if !is_identifier_continue(*last) {
        return false;
    }

    !output
        .rsplit(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .is_some_and(|word| {
            matches!(
                word,
                "and"
                    | "do"
                    | "elseif"
                    | "for"
                    | "if"
                    | "in"
                    | "not"
                    | "or"
                    | "return"
                    | "then"
                    | "while"
            )
        })
}

fn push_spaced_operator(output: &mut String, operator: &str) {
    trim_trailing_spaces(output);
    if !output.is_empty() {
        output.push(' ');
    }
    output.push_str(operator);
    output.push(' ');
}

fn trim_trailing_spaces(output: &mut String) {
    while output.ends_with(' ') {
        output.pop();
    }
}

fn should_trim_before_call_paren(output: &str) -> bool {
    let output = output.trim_end();
    if should_space_before_group_paren(output) {
        return false;
    }
    output.as_bytes().last().is_some_and(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b')' | b']' | b'}')
    })
}

fn previous_token_is_indexable(output: &str) -> bool {
    output.trim_end().as_bytes().last().is_some_and(|byte| {
        is_identifier_continue(*byte) || matches!(byte, b')' | b']' | b'}' | b'\'' | b'"')
    })
}

fn should_space_before_group_paren(output: &str) -> bool {
    output
        .split_whitespace()
        .next_back()
        .is_some_and(|word| matches!(word, "if" | "elseif" | "while" | "for" | "return" | "not"))
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
mod corpus;
#[cfg(test)]
mod tests;
