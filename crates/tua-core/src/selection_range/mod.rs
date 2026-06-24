use std::path::Path;

use crate::source_text::{dotted_chain_at_position, line_at};
use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionRange {
    pub range: SourceSpan,
    pub parent: Option<Box<SelectionRange>>,
}

pub fn selection_ranges_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
    positions: &[SourcePosition],
) -> Result<Vec<Option<SelectionRange>>, Vec<Diagnostic>> {
    let source = source.into();

    match parse_tua_source(path, source.clone()) {
        Ok(parsed) => Ok(selection_ranges_parsed_source(&parsed, positions)),
        Err(_) => Ok(selection_ranges_source(&source, None, positions)),
    }
}

pub fn selection_ranges_parsed_source(
    parsed: &ParsedSource,
    positions: &[SourcePosition],
) -> Vec<Option<SelectionRange>> {
    selection_ranges_source(&parsed.source, parsed.span, positions)
}

pub(crate) fn selection_ranges_source(
    source: &str,
    file_span: Option<SourceSpan>,
    positions: &[SourcePosition],
) -> Vec<Option<SelectionRange>> {
    let blocks = block_spans(source);

    positions
        .iter()
        .map(|position| selection_range_at(source, file_span, &blocks, *position))
        .collect()
}

fn selection_range_at(
    source: &str,
    file_span: Option<SourceSpan>,
    blocks: &[SourceSpan],
    position: SourcePosition,
) -> Option<SelectionRange> {
    let mut ranges = Vec::new();

    if let Some(chain) = dotted_chain_at_position(source, position) {
        let identifier = chain.parts.get(chain.cursor_index)?.span;
        ranges.push(identifier);

        if chain.parts.len() > 1 {
            let chain_span = SourceSpan {
                start: chain.parts.first()?.span.start,
                end: chain.parts.last()?.span.end,
            };
            ranges.push(chain_span);
        }
    }

    if let Some(statement) = statement_span(source, position.line) {
        ranges.push(statement);
    }

    let mut containing_blocks = blocks
        .iter()
        .copied()
        .filter(|span| span_contains_position(*span, position))
        .collect::<Vec<_>>();
    containing_blocks.sort_by_key(|span| span_size(*span));
    ranges.extend(containing_blocks);

    if let Some(file_span) = file_span.filter(|span| span_contains_position(*span, position)) {
        ranges.push(file_span);
    }

    ranges = dedup_spans(ranges);

    build_chain(ranges)
}

fn statement_span(source: &str, line_number: usize) -> Option<SourceSpan> {
    let line = line_at(source, line_number)?;
    let start = line
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, _)| index + 1)
        .unwrap_or(1);
    let end = line.chars().count() + 1;

    (start < end).then_some(SourceSpan {
        start: SourcePosition {
            line: line_number,
            column: start,
        },
        end: SourcePosition {
            line: line_number,
            column: end,
        },
    })
}

fn block_spans(source: &str) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    let mut block_stack = Vec::new();
    let mut table_stack = Vec::new();

    for (index, raw_line) in source.split('\n').enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed = line.trim_start();
        let first_column = line.len().saturating_sub(trimmed.len()) + 1;

        if is_block_start(trimmed) {
            block_stack.push(SourcePosition {
                line: line_number,
                column: first_column,
            });
        }

        for (byte, character) in code_prefix(line).char_indices() {
            match character {
                '{' => table_stack.push(SourcePosition {
                    line: line_number,
                    column: column_for_byte(line, byte),
                }),
                '}' => {
                    if let Some(start) = table_stack.pop() {
                        let end = SourcePosition {
                            line: line_number,
                            column: column_for_byte(line, byte) + 1,
                        };

                        if start.line < end.line {
                            spans.push(SourceSpan { start, end });
                        }
                    }
                }
                _ => {}
            }
        }

        if is_block_end(trimmed) {
            if let Some(start) = block_stack.pop() {
                let end = SourcePosition {
                    line: line_number,
                    column: line.chars().count() + 1,
                };

                if start.line < end.line {
                    spans.push(SourceSpan { start, end });
                }
            }
        }
    }

    dedup_spans(spans)
}

fn is_block_start(trimmed: &str) -> bool {
    let code = code_prefix(trimmed).trim();

    code.starts_with("function ")
        || code.starts_with("local function ")
        || code.contains(" function(")
        || code.contains(" function ")
        || code.ends_with(" then")
        || code.ends_with(" do")
        || code == "do"
        || code == "repeat"
}

fn is_block_end(trimmed: &str) -> bool {
    let code = code_prefix(trimmed).trim();

    code == "end" || code.starts_with("end ") || code.starts_with("until ")
}

fn code_prefix(line: &str) -> &str {
    line.split_once("--").map(|(code, _)| code).unwrap_or(line)
}

fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
}

fn build_chain(ranges: Vec<SourceSpan>) -> Option<SelectionRange> {
    let mut parent = None;

    for range in ranges.into_iter().rev() {
        parent = Some(SelectionRange {
            range,
            parent: parent.map(Box::new),
        });
    }

    parent
}

fn dedup_spans(mut spans: Vec<SourceSpan>) -> Vec<SourceSpan> {
    spans.sort_by_key(|span| {
        (
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        )
    });
    spans.dedup();
    spans.sort_by_key(|span| span_size(*span));
    spans
}

fn span_contains_position(span: SourceSpan, position: SourcePosition) -> bool {
    position_key(span.start) <= position_key(position)
        && position_key(position) < position_key(span.end)
}

fn span_size(span: SourceSpan) -> (usize, usize, usize, usize) {
    (
        span.end.line.saturating_sub(span.start.line),
        span.end.column.saturating_sub(span.start.column),
        span.start.line,
        span.start.column,
    )
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

#[cfg(test)]
mod tests;
