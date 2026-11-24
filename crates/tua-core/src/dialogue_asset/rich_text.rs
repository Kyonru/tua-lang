//! Bounded rich-template parsing shared by future dialogue and catalog analysis.
//!
//! This parser accepts a text field, not a dialogue line: callers must separate
//! speaker/target/end syntax first. It is not yet enabled by the authoring parser.
//! Ranges are zero-based, half-open UTF-8 byte offsets in the original template;
//! adapters must convert them before exposing source positions or runtime ranges.
use std::ops::Range;

pub const MAX_BYTES: usize = 65_536;
pub const MAX_TOKENS: usize = 4_096;
pub const MAX_DEPTH: usize = 16;
pub const MAX_PAUSES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Line,
    Choice,
    Reason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkName {
    Emphasis,
    Style,
    Speed,
    Pronunciation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Emphasis,
    Style(String),
    Speed(f64),
    Pronunciation(String),
}

impl Mark {
    fn name(&self) -> MarkName {
        match self {
            Self::Emphasis => MarkName::Emphasis,
            Self::Style(_) => MarkName::Style,
            Self::Speed(_) => MarkName::Speed,
            Self::Pronunciation(_) => MarkName::Pronunciation,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Text(String),
    Placeholder(String),
    Open(Mark),
    Close(MarkName),
    Pause(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pub tokens: Vec<Token>,
    /// Mark-free template; percent escapes remain encoded for one formatting pass.
    pub plain_template: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub range: Range<usize>,
    pub message: &'static str,
}

struct OpenMark {
    name: MarkName,
    range: Range<usize>,
    content: usize,
}

struct Parser<'a> {
    source: &'a str,
    field: FieldKind,
    cursor: usize,
    tokens: Vec<Token>,
    stack: Vec<OpenMark>,
    plain: String,
    content: usize,
    pauses: usize,
    pause_seconds: f64,
    pause_rounding: f64,
}

/// Parse syntax only. Resolved grapheme boundaries and expanded byte limits must
/// also be checked after substitution, before committing a presentation.
pub fn parse(source: &str, field: FieldKind) -> Result<Template, ParseError> {
    let _profile = crate::profile::span("dialogue.rich_text.parse");
    if source.len() > MAX_BYTES {
        return Err(ParseError {
            range: 0..source.len(),
            message: "rich template exceeds 65536 UTF-8 bytes",
        });
    }
    let mut parser = Parser {
        source,
        field,
        cursor: 0,
        tokens: Vec::new(),
        stack: Vec::new(),
        plain: String::new(),
        content: 0,
        pauses: 0,
        pause_seconds: 0.0,
        pause_rounding: 0.0,
    };
    while parser.cursor < source.len() {
        parser.next()?;
    }
    if let Some(open) = parser.stack.last() {
        return Err(ParseError {
            range: open.range.clone(),
            message: "unclosed rich-text mark",
        });
    }
    if parser.content == 0 {
        return Err(ParseError {
            range: 0..source.len(),
            message: "rich text needs text or a placeholder",
        });
    }
    Ok(Template {
        tokens: parser.tokens,
        plain_template: parser.plain,
    })
}

impl Parser<'_> {
    fn error(&self, start: usize, message: &'static str) -> ParseError {
        ParseError {
            range: start..self.cursor,
            message,
        }
    }

    fn push(&mut self, start: usize, kind: TokenKind) -> Result<(), ParseError> {
        if self.tokens.len() == MAX_TOKENS {
            return Err(self.error(start, "rich template exceeds 4096 tokens"));
        }
        match &kind {
            TokenKind::Text(value) => {
                self.content += 1;
                // Literal percent characters must survive exactly one later format pass.
                self.plain.push_str(&value.replace('%', "%%"));
            }
            TokenKind::Placeholder(name) => {
                self.content += 1;
                self.plain.push_str("%{");
                self.plain.push_str(name);
                self.plain.push('}');
            }
            _ => {}
        }
        self.tokens.push(Token {
            kind,
            range: start..self.cursor,
        });
        Ok(())
    }

    fn next(&mut self) -> Result<(), ParseError> {
        let start = self.cursor;
        let rest = &self.source[start..];
        if rest.starts_with('[') {
            return self.mark();
        }
        if rest.starts_with('\\') {
            self.cursor += 1;
            let Some(ch) = self.source[self.cursor..].chars().next() else {
                return Err(self.error(start, "unfinished text escape"));
            };
            self.cursor += ch.len_utf8();
            let value = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' | '"' | '{' | '}' | '$' | '(' | ')' | '[' | ']' => ch,
                _ => return Err(self.error(start, "unknown text escape")),
            };
            return self.push(start, TokenKind::Text(value.to_string()));
        }
        if rest.starts_with("%%") {
            self.cursor += 2;
            // %%{name} is a literal placeholder, never a substituted value.
            if self.source[self.cursor..].starts_with('{') {
                self.placeholder_name(start)?;
                let literal = self.source[start + 1..self.cursor].to_owned();
                return self.push(start, TokenKind::Text(literal));
            }
            return self.push(start, TokenKind::Text("%".into()));
        }
        if rest.starts_with("%{") {
            self.cursor += 1;
            let name = self.placeholder_name(start)?;
            return self.push(start, TokenKind::Placeholder(name));
        }
        let ch = rest.chars().next().expect("cursor is before end");
        if matches!(ch, ']' | '{' | '}') || (ch.is_control() && ch != '\t') {
            self.cursor += ch.len_utf8();
            return Err(self.error(
                start,
                "escape reserved brackets/braces or control characters",
            ));
        }
        // A literal run retains a direct byte mapping. Escapes are separate tokens.
        self.cursor += ch.len_utf8();
        while let Some(ch) = self.source[self.cursor..].chars().next() {
            if matches!(ch, '[' | ']' | '{' | '}' | '\\' | '%') || ch.is_control() {
                break;
            }
            self.cursor += ch.len_utf8();
        }
        self.push(
            start,
            TokenKind::Text(self.source[start..self.cursor].into()),
        )
    }

    fn placeholder_name(&mut self, start: usize) -> Result<String, ParseError> {
        let begin = self.cursor + 1;
        let Some(end) = self.source[begin..].find('}').map(|n| begin + n) else {
            self.cursor = self.source.len();
            return Err(self.error(start, "unterminated named placeholder"));
        };
        self.cursor = end + 1;
        let name = self.source[begin..end].trim();
        if !identifier(name) {
            return Err(self.error(start, "expected a simple named placeholder"));
        }
        // Do not allow a physical line break to hide inside trimmed whitespace.
        if self.source[begin..end].chars().any(char::is_control) {
            return Err(self.error(start, "control character in named placeholder"));
        }
        Ok(name.to_owned())
    }

    fn mark(&mut self) -> Result<(), ParseError> {
        let start = self.cursor;
        self.cursor += 1;
        let body_start = self.cursor;
        let mut quoted = false;
        let mut escaped = false;
        while let Some(ch) = self.source[self.cursor..].chars().next() {
            self.cursor += ch.len_utf8();
            if ch.is_control() {
                return Err(self.error(start, "control character in rich-text mark"));
            }
            if escaped {
                escaped = false;
            } else if quoted && ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = !quoted;
            } else if ch == ']' && !quoted {
                let body = &self.source[body_start..self.cursor - 1];
                return self.mark_body(start, body);
            }
        }
        Err(self.error(start, "unterminated rich-text mark"))
    }

    fn mark_body(&mut self, start: usize, body: &str) -> Result<(), ParseError> {
        if let Some(close) = body.strip_prefix('/') {
            let name = match close {
                "em" => MarkName::Emphasis,
                "style" => MarkName::Style,
                "speed" => MarkName::Speed,
                "pronounce" => MarkName::Pronunciation,
                _ => return Err(self.error(start, "unknown closing mark")),
            };
            let Some(open) = self.stack.last() else {
                return Err(self.error(start, "closing mark has no opening mark"));
            };
            if open.name != name {
                return Err(self.error(start, "rich-text marks must close in reverse order"));
            }
            if open.content == self.content {
                return Err(self.error(start, "paired mark must contain text or a placeholder"));
            }
            self.stack.pop();
            return self.push(start, TokenKind::Close(name));
        }
        if let Some(value) = body.strip_prefix("pause=") {
            let seconds = decimal(value).filter(|n| *n <= 60.0);
            let Some(seconds) = seconds else {
                return Err(self.error(start, "pause must be decimal seconds from 0 through 60"));
            };
            if self.field != FieldKind::Line {
                return Err(self.error(start, "pause is only supported in lines"));
            }
            self.pauses += 1;
            // Compensate summation so 100 pauses of 1.2s do not spuriously
            // exceed the 120s limit through accumulated rounding error.
            let corrected = seconds - self.pause_rounding;
            let total = self.pause_seconds + corrected;
            self.pause_rounding = (total - self.pause_seconds) - corrected;
            self.pause_seconds = total;
            if self.pauses > MAX_PAUSES || self.pause_seconds > 120.0 {
                return Err(self.error(start, "rich text exceeds pause count or duration limit"));
            }
            return self.push(start, TokenKind::Pause(seconds));
        }
        let mark = if body == "em" {
            Mark::Emphasis
        } else if let Some(key) = body.strip_prefix("style=") {
            if key.len() > 64 || !identifier(key) {
                return Err(
                    self.error(start, "style needs an ASCII identifier of at most 64 bytes")
                );
            }
            Mark::Style(key.into())
        } else if let Some(value) = body.strip_prefix("speed=") {
            let Some(rate) = decimal(value).filter(|n| *n > 0.0 && *n <= 1000.0) else {
                return Err(
                    self.error(start, "speed must be decimal graphemes/second in (0, 1000]")
                );
            };
            if self.field != FieldKind::Line {
                return Err(self.error(start, "speed is only supported in lines"));
            }
            Mark::Speed(rate)
        } else if let Some(value) = body.strip_prefix("pronounce=") {
            let Some(value) = pronunciation(value) else {
                return Err(self.error(
                    start,
                    "pronunciation needs a quoted hint of 1 through 256 bytes",
                ));
            };
            if self
                .stack
                .iter()
                .any(|open| open.name == MarkName::Pronunciation)
            {
                return Err(self.error(start, "pronunciation marks cannot nest"));
            }
            Mark::Pronunciation(value)
        } else {
            return Err(self.error(start, "unknown rich-text mark"));
        };
        if self.stack.len() == MAX_DEPTH {
            return Err(self.error(start, "rich text exceeds 16 nested marks"));
        }
        self.stack.push(OpenMark {
            name: mark.name(),
            range: start..self.cursor,
            content: self.content,
        });
        self.push(start, TokenKind::Open(mark))
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
        })
}

// Deliberately exclude signs, exponents, hex, NaN and implicit decimal digits.
fn decimal(value: &str) -> Option<f64> {
    let mut parts = value.split('.');
    let whole = parts.next()?;
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(fraction) = parts.next() {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    value.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn pronunciation(value: &str) -> Option<String> {
    let mut chars = value.strip_prefix('"')?.strip_suffix('"')?.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next()? {
                ch @ ('\\' | '"') => result.push(ch),
                _ => return None,
            },
            '"' => return None,
            ch if ch.is_control() => return None,
            ch => result.push(ch),
        }
    }
    if result.is_empty() || result.len() > 256 || result.contains("%{") {
        return None;
    }
    Some(result)
}

#[cfg(test)]
mod tests;

pub(crate) struct MarkInfo {
    pub name: &'static str,
    pub insertion: &'static str,
    pub description: &'static str,
    pub timing: bool,
}

pub(crate) const MARKS: &[MarkInfo] = &[
    MarkInfo { name: "em", insertion: "em]", description: "[em]text[/em]: semantic emphasis; the host chooses its appearance", timing: false },
    MarkInfo { name: "style", insertion: "style=warning]", description: "[style=key]text[/style]: host-defined style; ASCII identifier, at most 64 bytes", timing: false },
    MarkInfo { name: "speed", insertion: "speed=30]", description: "[speed=30]text[/speed]: scoped graphemes/second in (0, 1000]; line-only", timing: true },
    MarkInfo { name: "pause", insertion: "pause=0.25]", description: "[pause=0.25]: wait at this boundary, 0–60 seconds; line-only", timing: true },
    MarkInfo { name: "pronounce", insertion: "pronounce=\"hint\"]", description: "[pronounce=\"hint\"]text[/pronounce]: opaque speech hint, 1–256 UTF-8 bytes; cannot nest", timing: false },
];

/// Recover text-field boundaries even while the markup itself is incomplete.
/// Comments/blank lines may separate the header and explicit opt-in directive.
pub(crate) fn authoring_field(
    source: &str,
    line_number: usize,
) -> Option<(&str, usize, FieldKind)> {
    let mut significant = source
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with("//"));
    if significant.next()?.trim() != "@dialogue 1" || significant.next()?.trim() != "@text rich" {
        return None;
    }
    let line = source.lines().nth(line_number.checked_sub(1)?)?;
    let trimmed = line.trim_start();
    let (text, kind) = if let Some(text) = trimmed.strip_prefix("-> ") {
        (text, FieldKind::Choice)
    } else if let Some(text) = trimmed.strip_prefix("@reason ") {
        (text, FieldKind::Reason)
    } else if !trimmed.starts_with(['@', '[', '/']) {
        let (speaker, text) = trimmed.split_once(':')?;
        if !speaker.is_empty() && !super::identifier(speaker) {
            return None;
        }
        (text.strip_prefix(' ').unwrap_or(text), FieldKind::Line)
    } else {
        return None;
    };
    let offset = line.len() - text.len();
    let text = if kind == FieldKind::Choice {
        text.rsplit_once(" [target:").map_or(text, |(body, _)| body)
    } else {
        text
    };
    Some((text, line[..offset].chars().count() + 1, kind))
}

/// Find an unescaped mark at a UTF-8 byte cursor, including an unfinished opener.
/// Quoted pronunciation brackets and escaped literal brackets never become marks.
pub(crate) fn mark_at(text: &str, cursor: usize) -> Option<(Range<usize>, &str)> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch != '[' {
            continue;
        }
        let mut quoted = false;
        let mut end = text.len();
        let mut closed = false;
        while let Some((at, ch)) = chars.next() {
            if quoted && ch == '\\' {
                chars.next();
                continue;
            }
            if ch == '"' {
                quoted = !quoted;
            }
            if ch == ']' && !quoted {
                end = at + 1;
                closed = true;
                break;
            }
        }
        if cursor > start && (cursor < end || !closed && cursor == end) {
            let body_end = if closed { end - 1 } else { end };
            return Some((start..end, &text[start + 1..body_end]));
        }
    }
    None
}
