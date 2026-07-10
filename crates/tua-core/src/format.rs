use std::path::Path;

use crate::{parse_tua_source, Diagnostic, ParsedSource};

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

pub fn format_parsed_source(parsed: &ParsedSource, options: &FormatOptions) -> String {
    Formatter::new(options).format(&parsed.source)
}

struct Formatter<'a> {
    options: &'a FormatOptions,
    indent_level: usize,
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

        if dedent {
            self.indent_level = self.indent_level.saturating_sub(1);
        }

        let normalized = normalize_spacing(trimmed);
        let indent = " ".repeat(self.indent_level * self.options.indent_width);
        let formatted = break_long_groups(&normalized, &indent, self.options);

        output.push_str(&formatted);
        output.push('\n');

        if line_opens_block(code) {
            self.indent_level += 1;
        }

        if line_is_middle_block(code) {
            self.indent_level += 1;
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
                    b'(' | b'[' => {
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

    for arg in args {
        output.push_str(&item_indent);
        output.push_str(arg.trim());
        output.push_str(",\n");
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
