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

impl AnalysisSnapshot {
    pub(super) fn dialogue_text_actions(
        &self,
        path: &Path,
        range: AssistRange,
        kinds: &[AssistKind],
    ) -> Vec<Assist> {
        if !kinds.is_empty() && !kinds.contains(&AssistKind::RefactorDialogueText) {
            return Vec::new();
        }
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        dialogue_selection_actions(path, &file.source, range).unwrap_or_default()
    }
}

fn dialogue_selection_actions(
    path: &Path,
    source: &str,
    range: AssistRange,
) -> Option<Vec<Assist>> {
    if range.start.line != range.end.line || range.start.column >= range.end.column {
        return None;
    }
    let (text, column, kind) = rich_text::authoring_field(source, range.start.line)?;
    // The terminal marker is dialogue control syntax, never presentation text.
    let text = if kind == FieldKind::Line {
        text.trim_end()
            .strip_suffix("(end)")
            .filter(|prefix| prefix.chars().rev().take_while(|ch| *ch == '\\').count() % 2 == 0)
            .map_or(text, |prefix| prefix.strip_suffix(' ').unwrap_or(prefix))
    } else {
        text
    };
    let byte_at = |column_offset: usize| {
        text.char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .nth(column_offset)
    };
    let start = byte_at(range.start.column.checked_sub(column)?)?;
    let end = byte_at(range.end.column.checked_sub(column)?)?;
    let template = rich_text::parse(text, kind).ok()?;
    // Raw text can be split; escaped literals, placeholders and marks are atomic.
    for token in &template.tokens {
        let literal = matches!(&token.kind, rich_text::TokenKind::Text(value) if value == &text[token.range.clone()]);
        if !literal
            && [start, end]
                .iter()
                .any(|at| token.range.start < *at && *at < token.range.end)
        {
            return None;
        }
    }
    let selected = &text[start..end];
    if selected.trim().is_empty() || rich_text::parse(selected, kind).is_err() {
        return None;
    }
    let styles = [
        ("Emphasis", "em"),
        ("Thought / italic", "style=thought"),
        ("Underline", "style=underline"),
        ("Strikethrough", "style=strike"),
        ("Highlight", "style=highlight"),
        ("Whisper", "style=whisper"),
    ];
    Some(
        styles
            .into_iter()
            .filter_map(|(title, mark)| {
                let close = mark.split('=').next()?;
                let replacement = format!("[{mark}]{selected}[/{close}]");
                let candidate = format!("{}{replacement}{}", &text[..start], &text[end..]);
                rich_text::parse(&candidate, kind).ok()?;
                let edit = crate::TextEdit {
                    path: path.to_path_buf(),
                    span: SourceSpan {
                        start: range.start,
                        end: range.end,
                    },
                    new_text: replacement,
                };
                Some(Assist {
                    title: title.into(),
                    kind: AssistKind::RefactorDialogueText,
                    edit: crate::AssistEdit {
                        changes: [(path.to_path_buf(), vec![edit])].into_iter().collect(),
                        create_files: Vec::new(),
                    },
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod formatting_tests {
    use super::*;

    fn actions(source: &str, selected: &str) -> Vec<Assist> {
        let start = source.find(selected).unwrap();
        let prefix = &source[..start];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
        let mut host = AnalysisHost::new();
        host.set_file("scene.tdialogue", source);
        host.snapshot().code_actions(
            "scene.tdialogue",
            AssistRange {
                start: SourcePosition { line, column },
                end: SourcePosition {
                    line,
                    column: column + selected.chars().count(),
                },
            },
            &[],
            &[AssistKind::RefactorDialogueText],
        )
    }

    #[test]
    fn dialogue_formatting_preserves_unicode_crlf_and_balanced_marks() {
        let source =
            "@dialogue 1\r\n@text rich\r\n[start]\r\n: 🐸 café %{name} [em]hello[/em] (end)\r\n";
        for selected in ["café", "%{name}", "[em]hello[/em]", "hello"] {
            let edits = actions(source, selected);
            assert!(!edits.is_empty(), "{selected}");
            let edit = &edits[0].edit.changes[Path::new("scene.tdialogue")][0];
            assert_eq!(edit.new_text, format!("[em]{selected}[/em]"));
        }
    }

    #[test]
    fn dialogue_formatting_rejects_syntax_partial_tokens_and_plain_scripts() {
        let source = "@dialogue 1\n@text rich\n[start]\n: Hello %{name} \\[ [em]world[/em] (end)\n-> Choose [target:done]\n@reason Unavailable\n";
        for selected in [
            "name",
            "[em]world",
            "em",
            "(end)",
            "target",
            "start",
            "[",
            "\\",
        ] {
            assert!(actions(source, selected).is_empty(), "{selected}");
        }
        assert_eq!(actions(source, "Choose").len(), 6);
        assert_eq!(actions(source, "Unavailable").len(), 6);
        assert!(actions(&source.replace("@text rich\n", ""), "Hello").is_empty());
    }
}
