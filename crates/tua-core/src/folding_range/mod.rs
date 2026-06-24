use std::collections::BTreeMap;
use std::path::Path;

use crate::{parse_tua_source, Diagnostic, ParsedSource, SourcePosition, SourceSpan};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldingRange {
    pub range: SourceSpan,
    pub kind: FoldingRangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoldingRangeKind {
    Function,
    Table,
    Comment,
    TypeAlias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockKind {
    Function,
    Other,
}

pub fn folding_ranges_tua_source(
    path: impl AsRef<Path>,
    source: impl Into<String>,
) -> Result<Vec<FoldingRange>, Vec<Diagnostic>> {
    let source = source.into();

    match parse_tua_source(path, source.clone()) {
        Ok(parsed) => Ok(folding_ranges_parsed_source(&parsed)),
        Err(_) => Ok(folding_ranges_source(&source, &[])),
    }
}

pub fn folding_ranges_parsed_source(parsed: &ParsedSource) -> Vec<FoldingRange> {
    let type_alias_spans = parsed
        .type_aliases
        .iter()
        .filter_map(|alias| alias.span)
        .collect::<Vec<_>>();

    folding_ranges_source(&parsed.source, &type_alias_spans)
}

pub(crate) fn folding_ranges_source(
    source: &str,
    type_alias_spans: &[SourceSpan],
) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();

    ranges.extend(comment_folding_ranges(source));
    ranges.extend(block_folding_ranges(source));
    ranges.extend(table_folding_ranges(source));
    ranges.extend(type_alias_folding_ranges(source));
    ranges.extend(
        type_alias_spans
            .iter()
            .copied()
            .filter(multiline)
            .map(|range| FoldingRange {
                range,
                kind: FoldingRangeKind::TypeAlias,
            }),
    );

    dedup_folding_ranges(ranges)
}

fn type_alias_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let lines = source_lines(source);
    let mut ranges = Vec::new();
    let mut line = 0;

    while line < lines.len() {
        let trimmed = lines[line].trim_start();
        let code = code_prefix(trimmed).trim();

        if !code.starts_with("type ") || !code.contains('=') {
            line += 1;
            continue;
        }

        let start_line = line;
        let start_column = lines[line].len().saturating_sub(trimmed.len()) + 1;
        let mut depth = brace_delta(code);
        let saw_table = code.contains('{');

        while saw_table && depth > 0 && line + 1 < lines.len() {
            line += 1;
            let next_code = code_prefix(lines[line]).trim();
            depth += brace_delta(next_code);
        }

        if saw_table && start_line < line {
            ranges.push(FoldingRange {
                range: SourceSpan {
                    start: SourcePosition {
                        line: start_line + 1,
                        column: start_column,
                    },
                    end: SourcePosition {
                        line: line + 1,
                        column: lines[line].chars().count() + 1,
                    },
                },
                kind: FoldingRangeKind::TypeAlias,
            });
        }

        line += 1;
    }

    ranges
}

fn comment_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let lines = source_lines(source);
    let mut ranges = Vec::new();
    let mut line = 0;

    while line < lines.len() {
        let trimmed = lines[line].trim_start();

        if trimmed.starts_with("--[[") {
            let start = SourcePosition {
                line: line + 1,
                column: lines[line].len().saturating_sub(trimmed.len()) + 1,
            };
            let mut end_line = line;

            while end_line < lines.len() {
                if lines[end_line].contains("]]") {
                    break;
                }

                end_line += 1;
            }

            if end_line < lines.len() && line < end_line {
                ranges.push(FoldingRange {
                    range: SourceSpan {
                        start,
                        end: SourcePosition {
                            line: end_line + 1,
                            column: lines[end_line].chars().count() + 1,
                        },
                    },
                    kind: FoldingRangeKind::Comment,
                });
            }

            line = end_line.saturating_add(1);
            continue;
        }

        if trimmed.starts_with("--") {
            let start_line = line;
            let start_column = lines[line].len().saturating_sub(trimmed.len()) + 1;

            while line < lines.len() && lines[line].trim_start().starts_with("--") {
                line += 1;
            }

            if line.saturating_sub(start_line) > 1 {
                let end_line = line - 1;
                ranges.push(FoldingRange {
                    range: SourceSpan {
                        start: SourcePosition {
                            line: start_line + 1,
                            column: start_column,
                        },
                        end: SourcePosition {
                            line: end_line + 1,
                            column: lines[end_line].chars().count() + 1,
                        },
                    },
                    kind: FoldingRangeKind::Comment,
                });
            }

            continue;
        }

        line += 1;
    }

    ranges
}

fn brace_delta(code: &str) -> isize {
    code.chars().fold(0, |depth, character| match character {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn block_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut stack = Vec::<(BlockKind, SourcePosition)>::new();

    for (index, line) in source_lines(source).iter().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();
        let first_column = line.len().saturating_sub(trimmed.len()) + 1;
        let code = code_prefix(trimmed).trim();

        if let Some(kind) = block_start_kind(code) {
            stack.push((
                kind,
                SourcePosition {
                    line: line_number,
                    column: first_column,
                },
            ));
        }

        if is_block_end(code) {
            if let Some((kind, start)) = stack.pop() {
                let end = SourcePosition {
                    line: line_number,
                    column: line.chars().count() + 1,
                };

                if kind == BlockKind::Function && start.line < end.line {
                    ranges.push(FoldingRange {
                        range: SourceSpan { start, end },
                        kind: FoldingRangeKind::Function,
                    });
                }
            }
        }
    }

    ranges
}

fn table_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut stack = Vec::<SourcePosition>::new();

    for (index, line) in source_lines(source).iter().enumerate() {
        let line_number = index + 1;

        for (byte, character) in code_prefix(line).char_indices() {
            match character {
                '{' => stack.push(SourcePosition {
                    line: line_number,
                    column: column_for_byte(line, byte),
                }),
                '}' => {
                    if let Some(start) = stack.pop() {
                        let end = SourcePosition {
                            line: line_number,
                            column: column_for_byte(line, byte) + 1,
                        };

                        if start.line < end.line {
                            ranges.push(FoldingRange {
                                range: SourceSpan { start, end },
                                kind: FoldingRangeKind::Table,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    ranges
}

fn block_start_kind(code: &str) -> Option<BlockKind> {
    if code.starts_with("function ")
        || code.starts_with("local function ")
        || code.contains(" function(")
        || code.contains(" function ")
    {
        return Some(BlockKind::Function);
    }

    (code.ends_with(" then") || code.ends_with(" do") || code == "do" || code == "repeat")
        .then_some(BlockKind::Other)
}

fn is_block_end(code: &str) -> bool {
    code == "end" || code.starts_with("end ") || code.starts_with("until ")
}

fn code_prefix(line: &str) -> &str {
    line.split_once("--").map(|(code, _)| code).unwrap_or(line)
}

fn source_lines(source: &str) -> Vec<&str> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

fn column_for_byte(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count() + 1
}

fn multiline(span: &SourceSpan) -> bool {
    span.start.line < span.end.line
}

fn dedup_folding_ranges(ranges: Vec<FoldingRange>) -> Vec<FoldingRange> {
    let mut deduped = BTreeMap::new();

    for range in ranges {
        deduped
            .entry((
                range.range.start.line,
                range.range.start.column,
                range.range.end.line,
                range.range.end.column,
            ))
            .or_insert(range);
    }

    deduped.into_values().collect()
}

#[cfg(test)]
mod tests;
