//! Rich authoring queries share source recognition with dialogue parsing. They
//! recover an unfinished mark without requiring a valid remainder of the file.
use super::*;
use crate::dialogue_asset::rich_text::{self, FieldKind};

impl AnalysisSnapshot {
    pub(super) fn dialogue_rich_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let file = self.file(path)?;
        let (text, column, kind) = rich_text::authoring_field(&file.source, position.line)?;
        let byte = text
            .char_indices()
            .nth(position.column.checked_sub(column)?)
            .map_or(text.len(), |(byte, _)| byte);
        let (range, _) = rich_text::mark_at(text, byte)?;
        let prefix = text.get(range.start + 1..byte)?;
        if prefix.starts_with('/') || prefix.contains(['=', '"', ']']) {
            return None;
        }
        Some(
            rich_text::MARKS
                .iter()
                .filter(|mark| {
                    mark.name.starts_with(prefix) && (kind == FieldKind::Line || !mark.timing)
                })
                .map(|mark| Completion {
                    label: mark.name.into(),
                    kind: CompletionKind::Keyword,
                    detail: Some(mark.description.into()),
                    documentation: None,
                    deprecation: None,
                    insert_text: Some(if text[range.clone()].ends_with(']') {
                        mark.insertion.trim_end_matches(']').into()
                    } else {
                        mark.insertion.into()
                    }),
                    filter_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect(),
        )
    }

    pub(super) fn dialogue_rich_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let file = self.file(path)?;
        let (text, column, _) = rich_text::authoring_field(&file.source, position.line)?;
        let byte = text
            .char_indices()
            .nth(position.column.checked_sub(column)?)
            .map_or(text.len(), |(byte, _)| byte);
        let (range, body) = rich_text::mark_at(text, byte)?;
        let name = body.trim_start_matches('/').split('=').next()?;
        let mark = rich_text::MARKS.iter().find(|mark| mark.name == name)?;
        Some(Hover {
            is_markdown: false,
            contents: mark.description.into(),
            span: Some(SourceSpan {
                start: SourcePosition {
                    line: position.line,
                    column: column + text[..range.start].chars().count(),
                },
                end: SourcePosition {
                    line: position.line,
                    column: column + text[..range.end].chars().count(),
                },
            }),
            documentation: None,
            context: vec![
                "Presentation metadata only; renderer/audio resources belong to the host".into(),
            ],
        })
    }
}
