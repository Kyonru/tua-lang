//! Readable statement and table layout from parsed boundaries, never punctuation
//! in strings/comments. Short single-action guards and callbacks remain compact.
use full_moon::{ast, node::Node, tokenizer::Position, visitors::Visitor};

use super::{byte_for_column, normalize_spacing, starts_with_word, FormatOptions};
use crate::{ParsedSource, SourcePosition};

pub(super) fn expand_layout(parsed: &ParsedSource, options: &FormatOptions) -> Option<String> {
    struct Layout<'a> {
        parsed: &'a ParsedSource,
        options: &'a FormatOptions,
        lines: Vec<usize>,
        edits: Vec<(usize, usize, String)>,
    }
    impl Layout<'_> {
        fn offset(&self, position: Position) -> Option<usize> {
            let origin = self.parsed.raw.parser_position_origin(SourcePosition {
                line: position.line(),
                column: position.character(),
            })?;
            if origin.class_generated {
                return None;
            }
            let position = origin.source_position;
            let start = *self.lines.get(position.line.checked_sub(1)?)?;
            let line = self.parsed.source.get(start..)?.split('\n').next()?;
            Some(start + byte_for_column(line, position.column))
        }
        fn span(&self, node: &impl Node) -> Option<(usize, usize)> {
            let start = self.offset(node.start_position()?)?;
            let end = self.offset(node.end_position()?)?;
            (start < end).then_some((start, end))
        }
        fn separator(&self, left: usize, right: usize) -> bool {
            self.parsed.source.get(left..right).is_some_and(|gap| {
                !gap.contains('\n') && gap.chars().all(|c| c.is_whitespace() || c == ';')
            })
        }
    }
    impl Visitor for Layout<'_> {
        fn visit_table_constructor_end(&mut self, table: &ast::TableConstructor) {
            let (open, close) = table.braces().tokens();
            let (Some(open), Some(close)) = (
                self.offset(open.token().start_position()),
                self.offset(close.token().start_position()),
            ) else {
                return;
            };
            let source = &self.parsed.source;
            if source.as_bytes().get(open) != Some(&b'{')
                || source.as_bytes().get(close) != Some(&b'}')
            {
                return;
            }
            // A table already laid out across lines, or containing an expanded
            // callback, has one field per line. Keep small flat tables compact.
            if !source[open..close].contains('\n')
                && !self
                    .edits
                    .iter()
                    .any(|(start, _, _)| *start > open && *start < close)
            {
                return;
            }
            let mut fields = Vec::new();
            for field in table.fields() {
                let Some(span) = self.span(field) else {
                    return;
                };
                fields.push(span);
            }
            let Some(first) = fields.first() else {
                return;
            };
            if source[open + 1..first.0].trim().is_empty() {
                self.edits.push((open + 1, first.0, "\n".into()));
            }
            for pair in fields.windows(2) {
                let gap = &source[pair[0].1..pair[1].0];
                if gap
                    .chars()
                    .all(|c| c.is_whitespace() || c == ',' || c == ';')
                {
                    self.edits.push((
                        pair[0].1,
                        pair[1].0,
                        if gap.contains(';') { ";\n" } else { ",\n" }.into(),
                    ));
                }
            }
            let last = fields.last().unwrap().1;
            if source[last..close]
                .chars()
                .all(|c| c.is_whitespace() || c == ',' || c == ';')
            {
                self.edits.push((last, close, ",\n".into()));
            }
        }
        fn visit_block(&mut self, block: &ast::Block) {
            let mut spans = Vec::new();
            for statement in block.stmts() {
                let Some(span) = self.span(statement) else {
                    return;
                };
                spans.push(span);
            }
            if let Some(statement) = block.last_stmt() {
                let Some(span) = self.span(statement) else {
                    return;
                };
                spans.push(span);
            }
            let Some(&(first, _)) = spans.first() else {
                return;
            };
            let line_start = self.parsed.source[..first].rfind('\n').map_or(0, |i| i + 1);
            let line_end = self.parsed.source[first..]
                .find('\n')
                .map_or(self.parsed.source.len(), |i| first + i);
            let source_line = &self.parsed.source[line_start..line_end];
            let indentation = source_line
                .chars()
                .take_while(|character| character.is_whitespace())
                .map(|character| {
                    if character == '\t' {
                        self.options.indent_width
                    } else {
                        1
                    }
                })
                .sum::<usize>();
            let long_inline_body = !self.parsed.source[line_start..first].trim().is_empty()
                && indentation
                    + normalize_spacing(source_line.trim(), self.options.quote_style)
                        .chars()
                        .count()
                    > self.options.line_width;
            if !long_inline_body
                && !spans
                    .windows(2)
                    .any(|pair| self.separator(pair[0].1, pair[1].0))
            {
                return;
            }
            for pair in spans.windows(2) {
                let (left, right) = (pair[0].1, pair[1].0);
                if self.separator(left, right) {
                    // A semicolon before a parenthesized call can disambiguate it
                    // from an argument list belonging to the previous expression.
                    let keep = self.parsed.source[right..].starts_with('(')
                        && self.parsed.source[left..right].contains(';');
                    self.edits
                        .push((left, right, if keep { ";\n" } else { "\n" }.into()));
                }
            }
            let first = spans[0].0;
            let line_start = self.parsed.source[..first].rfind('\n').map_or(0, |i| i + 1);
            if !self.parsed.source[line_start..first].trim().is_empty() {
                self.edits.push((first, first, "\n".into()));
            }
            let last = spans.last().unwrap().1;
            let suffix = &self.parsed.source[last..];
            let gap = suffix.len() - suffix.trim_start_matches([' ', '\t', ';']).len();
            let rest = &suffix[gap..];
            if ["end", "else", "elseif", "until"]
                .iter()
                .any(|word| starts_with_word(rest, word))
            {
                self.edits.push((last, last + gap, "\n".into()));
            }
        }
    }
    let mut layout = Layout {
        parsed,
        options,
        lines: std::iter::once(0)
            .chain(parsed.source.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
        edits: Vec::new(),
    };
    layout.visit_ast(&parsed.ast);
    layout
        .edits
        .retain(|(start, end, replacement)| parsed.source[*start..*end] != *replacement);
    if layout.edits.is_empty() {
        return None;
    }
    layout.edits.sort_by_key(|(start, end, _)| (*start, *end));
    layout.edits.dedup();
    let mut output = parsed.source.clone();
    let mut boundary = output.len();
    for (start, end, replacement) in layout.edits.into_iter().rev() {
        // Synthetic/lowered nodes must never overlap real source edits.
        if end > boundary {
            return None;
        }
        output.replace_range(start..end, &replacement);
        boundary = start;
    }
    Some(output)
}
