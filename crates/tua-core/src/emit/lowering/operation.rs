use std::fmt;
use std::ops::Range;

use crate::SourceSpan;

use super::{split_lines, LoweredLine, LoweringKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoweringOperation {
    pub(crate) range: Range<usize>,
    pub(crate) replacement: String,
    pub(crate) source_span: SourceSpan,
    pub(crate) lowering: LoweringKind,
}

impl LoweringOperation {
    pub(crate) fn replace(
        range: Range<usize>,
        replacement: impl Into<String>,
        source_span: SourceSpan,
        lowering: LoweringKind,
    ) -> Self {
        Self {
            range,
            replacement: replacement.into(),
            source_span,
            lowering,
        }
    }

    pub(crate) fn insert(
        offset: usize,
        text: impl Into<String>,
        source_span: SourceSpan,
        lowering: LoweringKind,
    ) -> Self {
        Self::replace(offset..offset, text, source_span, lowering)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LoweringOperationError {
    InvalidRange {
        range: Range<usize>,
        code_len: usize,
    },
    InvalidUtf8Boundary {
        offset: usize,
    },
    Overlap {
        previous: Range<usize>,
        next: Range<usize>,
    },
}

impl fmt::Display for LoweringOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { range, code_len } => write!(
                formatter,
                "lowering operation range {}..{} exceeds emitted code length {code_len}",
                range.start, range.end
            ),
            Self::InvalidUtf8Boundary { offset } => {
                write!(
                    formatter,
                    "lowering operation offset {offset} splits UTF-8 text"
                )
            }
            Self::Overlap { previous, next } => write!(
                formatter,
                "lowering operations overlap at {}..{} and {}..{}",
                previous.start, previous.end, next.start, next.end
            ),
        }
    }
}

pub(super) fn apply_operations(
    old: &[LoweredLine],
    code: &str,
    mut operations: Vec<LoweringOperation>,
) -> Result<Vec<LoweredLine>, LoweringOperationError> {
    operations.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then_with(|| left.range.end.cmp(&right.range.end))
    });
    validate_operations(code, &operations)?;

    let mut lines = Vec::new();
    let mut cursor = 0usize;

    for operation in operations {
        append_old_range(old, cursor..operation.range.start, &mut lines);
        append_segment(
            &mut lines,
            &operation.replacement,
            operation.source_span,
            operation.lowering,
            true,
        );
        cursor = operation.range.end;
    }

    append_old_range(old, cursor..code.len(), &mut lines);
    Ok(lines)
}

fn validate_operations(
    code: &str,
    operations: &[LoweringOperation],
) -> Result<(), LoweringOperationError> {
    let mut previous = None::<Range<usize>>;

    for operation in operations {
        let range = &operation.range;
        if range.start > range.end || range.end > code.len() {
            return Err(LoweringOperationError::InvalidRange {
                range: range.clone(),
                code_len: code.len(),
            });
        }
        for offset in [range.start, range.end] {
            if !code.is_char_boundary(offset) {
                return Err(LoweringOperationError::InvalidUtf8Boundary { offset });
            }
        }
        if let Some(previous_range) = &previous {
            if range.start < previous_range.end {
                return Err(LoweringOperationError::Overlap {
                    previous: previous_range.clone(),
                    next: range.clone(),
                });
            }
        }
        if range.start != range.end {
            previous = Some(range.clone());
        }
    }

    Ok(())
}

fn append_old_range(old: &[LoweredLine], range: Range<usize>, output: &mut Vec<LoweredLine>) {
    if range.is_empty() {
        return;
    }

    let mut line_start = 0usize;
    for line in old {
        let line_end = line_start + line.text.len();
        let start = range.start.max(line_start);
        let end = range.end.min(line_end);

        if start < end {
            append_segment(
                output,
                &line.text[start - line_start..end - line_start],
                line.source_span,
                line.lowering,
                false,
            );
        }
        if line_end >= range.end {
            break;
        }
        line_start = line_end;
    }
}

fn append_segment(
    output: &mut Vec<LoweredLine>,
    text: &str,
    source_span: SourceSpan,
    lowering: LoweringKind,
    generated: bool,
) {
    for piece in split_lines(text) {
        if let Some(line) = output.last_mut().filter(|line| !line.text.ends_with('\n')) {
            line.text.push_str(piece);
            if generated {
                line.source_span = source_span;
                line.lowering = lowering;
            }
        } else {
            output.push(LoweredLine {
                text: piece.to_owned(),
                source_span,
                lowering,
            });
        }
    }
}
