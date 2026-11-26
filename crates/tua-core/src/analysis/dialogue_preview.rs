//! Resolve authored character literals through the existing module export index.
//! Dynamic expressions stay unknown. This never evaluates a character module.
use super::*;
use crate::dialogue_asset::preview::{DialoguePreview, DialoguePreviewCharacter};

impl AnalysisSnapshot {
    pub(super) fn add_dialogue_preview_characters(
        &self,
        path: &Path,
        preview: &mut DialoguePreview,
    ) {
        let Some(file) = self.file(path) else {
            return;
        };
        let Ok(parsed) = &file.parsed else {
            return;
        };
        let mut cache = BTreeMap::new();
        for (section, node) in preview.document.sections.iter().take(200).enumerate() {
            for (line_index, line) in node.lines.iter().take(100).enumerate() {
                let Some(speaker) = &line.speaker else {
                    continue;
                };
                let Some((alias, name)) = speaker.value.split_once('.') else {
                    if let Some(declared) = preview
                        .document
                        .speakers
                        .iter()
                        .find(|item| item.id.value == speaker.value)
                    {
                        preview.characters.insert(
                            (section, line_index),
                            DialoguePreviewCharacter {
                                name: Some(declared.name.clone()),
                                portrait: None,
                                expression: None,
                            },
                        );
                    }
                    continue;
                };
                let expression = line.expression.as_ref().map(|value| value.value.clone());
                let key = (speaker.value.clone(), expression.clone());
                let character = cache.entry(key).or_insert_with(|| {
                    let import = parsed
                        .dialogue_imports
                        .iter()
                        .find(|import| import.alias.value == alias)?;
                    let module = self.dialogue_module(path, import)?;
                    let export = self.module_graph.export(&module, name)?;
                    let origin = self.file(&export.module_path)?;
                    let parsed = origin.parsed.as_ref().ok()?;
                    let mut collector = ExpectedTableSyntaxCollector::default();
                    collector.visit_ast(&parsed.ast);
                    let table = declaration_table(&collector, export.definition_span)?;
                    let portrait_table = expression.as_ref().and_then(|name| {
                        table_field(&table, "expressions")
                            .and_then(as_table)
                            .and_then(|expressions| table_field(expressions, name))
                            .and_then(as_table)
                    });
                    Some(DialoguePreviewCharacter {
                        name: table_field(&table, "name").and_then(string_literal_expression_value),
                        portrait: if expression.is_some() {
                            portrait_table.and_then(|table| table_field(table, "portrait"))
                        } else {
                            table_field(&table, "portrait")
                        }
                        .and_then(string_literal_expression_value),
                        expression,
                    })
                });
                if let Some(character) = character {
                    preview
                        .characters
                        .insert((section, line_index), character.clone());
                }
            }
        }
    }
}

fn as_table(value: &Expression) -> Option<&TableConstructor> {
    if let Expression::TableConstructor(table) = value {
        Some(table)
    } else {
        None
    }
}

fn table_field<'a>(table: &'a TableConstructor, name: &str) -> Option<&'a Expression> {
    // Ambiguous duplicate fields and computed keys are deliberately not inferred.
    let mut found = None;
    for field in table.fields().iter() {
        let (key, value) = match field {
            Field::NameKey { key, value, .. } => (token_text(key), value),
            Field::ExpressionKey { key, value, .. } => {
                (string_literal_expression_value(key)?, value)
            }
            _ => continue,
        };
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(value);
        }
    }
    found.map(|v| &**v)
}

fn declaration_table(
    collector: &ExpectedTableSyntaxCollector,
    span: SourceSpan,
) -> Option<TableConstructor> {
    for local in &collector.locals {
        for (name, value) in local.names().iter().zip(local.expressions().iter()) {
            if node_span(name) == Some(span) {
                return as_table(value).cloned();
            }
        }
    }
    for table in &collector.tables {
        for field in table.fields().iter() {
            if let Field::NameKey { key, value, .. } = field {
                if node_span(key) == Some(span) {
                    return as_table(value).cloned();
                }
            }
        }
    }
    None
}
