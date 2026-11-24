//! Dialogue references use the same indexed exports and type environments as Tua imports.
use super::*;
use crate::dialogue_asset::{flow, CharacterImport, DialogueName};
use crate::types::PrimitiveType;

impl AnalysisSnapshot {
    pub(super) fn is_dialogue(&self, path: &Path) -> bool {
        self.file(path)
            .is_some_and(|file| file.kind == AnalysisSourceKind::Dialogue)
    }

    pub(super) fn dialogue_module(&self, path: &Path, import: &CharacterImport) -> Option<PathBuf> {
        let parsed = self.file(path)?.parsed.as_ref().ok()?;
        self.indexed_module_path_for_specifier(parsed, &import.module.value)
            .map(Path::to_path_buf)
    }

    pub(super) fn dialogue_character_type(&self, module: &Path, name: &str) -> Option<Type> {
        let export = self.module_graph.export(module, name)?;
        let source = self.module_graph.export_type_source_path(module, name)?;
        Some(
            self.project_type_environment(source)
                .map(|environment| environment.resolve_nested_aliases(&export.field.ty))
                .unwrap_or_else(|| export.field.ty.clone()),
        )
    }

    pub(super) fn dialogue_character_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for import in parsed.dialogue_imports.iter().filter(|import| {
            !super::dialogue_context::is_context_import(&file.source, import)
                && !super::dialogue_events::is_event_import(&file.source, import)
                && !super::dialogue_events::is_action_import(&file.source, import)
        }) {
            let Some(module) = self.dialogue_module(path, import) else {
                diagnostics.push(character_error(
                    path,
                    import.module.span,
                    format!("cannot resolve character module `{}`", import.module.value),
                ));
                continue;
            };
            let Some(model) = file.dialogue_model.get().and_then(Option::as_ref) else {
                continue;
            };
            for line in model.sections.iter().flat_map(|section| &section.lines) {
                let Some(speaker) = &line.speaker else {
                    continue;
                };
                let Some((alias, name)) = speaker.value.split_once('.') else {
                    continue;
                };
                if alias != import.alias.value {
                    continue;
                }
                match self.dialogue_character_type(&module, name) {
                    None => diagnostics.push(character_error(
                        path,
                        speaker.span,
                        format!(
                            "character module `{}` has no export `{name}`",
                            import.module.value
                        ),
                    )),
                    Some(ty) if !is_character(&ty) => {
                        let message = format!(
                            "character `{}` must be a record with a required string `name` and optional string `portrait` and `nameKey`; found {ty}",
                            speaker.value
                        );
                        diagnostics.push(character_error(path, speaker.span, message));
                    }
                    Some(ty) => {
                        if matches!(
                            line.speech,
                            Some(crate::dialogue_asset::DialogueSpeech::Character)
                        ) && !valid_character_speech(&ty)
                        {
                            diagnostics.push(character_error(path, speaker.span,
                                "character needs a required typed speech profile; use Dialogue.speechProfile".into()));
                        }

                        if let Some(expression) = &line.expression {
                            let selected = expression_fields(&ty)
                                .and_then(|fields| fields.field(&expression.value));
                            if !selected
                                .is_some_and(|field| !field.optional && valid_expression(&field.ty))
                            {
                                diagnostics.push(character_error(path, expression.span, format!("character `{}` needs a declared expression `{}` with a required string portrait and optional string animation", speaker.value, expression.value)));
                            }
                        }
                    }
                }
            }
        }
        diagnostics
    }

    // Lexical recovery allows completion while an expression name is unfinished.
    fn expression_target(
        &self,
        path: &Path,
        position: SourcePosition,
        prefix: bool,
    ) -> Option<(PathBuf, String, String, SourceSpan)> {
        let file = self.file(path)?;
        let lines: Vec<_> = file.source.lines().collect();
        let line = *lines.get(position.line.checked_sub(1)?)?;
        let rest = line.trim_start().strip_prefix("@expression ")?.trim_start();
        let start = line.chars().count() - rest.chars().count() + 1;
        let name = rest.trim_end();
        let end = start + name.chars().count();
        if position.column < start || position.column > end {
            return None;
        }
        let token = if prefix {
            name.chars().take(position.column - start).collect()
        } else {
            name.to_owned()
        };
        for previous in lines[..position.line - 1].iter().rev() {
            let previous = previous.trim_start();
            if previous.is_empty() || previous.starts_with("//") {
                continue;
            }
            if previous.starts_with('[')
                || previous.starts_with("->")
                || previous.starts_with("@action ")
                || previous.starts_with("@branch ")
            {
                return None;
            }
            if previous.starts_with('@') {
                continue;
            }
            let (speaker, _) = previous.split_once(':')?;
            let (alias, character) = speaker.split_once('.')?;
            let import = file
                .parsed
                .as_ref()
                .ok()?
                .dialogue_imports
                .iter()
                .find(|import| import.alias.value == alias)?;
            return Some((
                self.dialogue_module(path, import)?,
                character.to_owned(),
                token,
                SourceSpan {
                    start: SourcePosition {
                        line: position.line,
                        column: start,
                    },
                    end: SourcePosition {
                        line: position.line,
                        column: end,
                    },
                },
            ));
        }
        None
    }

    fn dialogue_label(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<(DialogueName, DialogueName)> {
        let source = &self.file(path)?.source;
        let name = flow::name_at(source, position)?;
        let mut matches = flow::labels(source)
            .into_iter()
            .filter(|label| label.value == name.value);
        let label = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some((name, label))
    }

    pub(super) fn dialogue_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<Completion> {
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        if let Some(name) = flow::name_at(&file.source, position) {
            let prefix: String = name
                .value
                .chars()
                .take(position.column - name.span.start.column)
                .collect();
            let labels = flow::labels(&file.source);
            let mut counts = std::collections::BTreeMap::new();
            for label in &labels {
                *counts.entry(label.value.as_str()).or_insert(0) += 1;
            }
            return labels
                .iter()
                .filter(|label| {
                    label.value.starts_with(&prefix) && counts[label.value.as_str()] == 1
                })
                .map(|label| Completion {
                    label: label.value.clone(),
                    kind: CompletionKind::Value,
                    detail: Some("dialogue label".into()),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect();
        }
        if let Some(items) = self.dialogue_rich_completion(path, position) {
            return items;
        }
        if let Some((module, character, prefix, _)) = self.expression_target(path, position, true) {
            let Some(ty) = self.dialogue_character_type(&module, &character) else {
                return Vec::new();
            };
            let Some(expressions) = expression_fields(&ty) else {
                return Vec::new();
            };
            return expressions
                .fields()
                .filter(|(name, field)| {
                    name.starts_with(&prefix) && !field.optional && valid_expression(&field.ty)
                })
                .map(|(name, field)| Completion {
                    label: name.to_owned(),
                    kind: CompletionKind::Field,
                    detail: Some(format!("expression: {}", field.ty)),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: Some(module.clone()),
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect();
        }
        if let Some(completions) = self.dialogue_event_completion(path, position) {
            return completions;
        }
        if let Some(completions) = self.dialogue_context_completion(path, position) {
            return completions;
        }
        let Some(file) = self.file(path) else {
            return Vec::new();
        };
        let Some((token, _)) = speaker_token(&file.source, position, true) else {
            return Vec::new();
        };
        let Some((alias, prefix)) = token.split_once('.') else {
            return Vec::new();
        };
        let Ok(parsed) = &file.parsed else {
            return Vec::new();
        };
        let Some(import) = parsed
            .dialogue_imports
            .iter()
            .find(|import| import.alias.value == alias)
        else {
            return Vec::new();
        };
        let Some(module) = self.dialogue_module(path, import) else {
            return Vec::new();
        };
        let Some(index) = self.module_graph.module(&module) else {
            return Vec::new();
        };
        index
            .exports
            .iter()
            .filter(|export| export.public_name.starts_with(prefix))
            .filter_map(|export| {
                let ty = self.dialogue_character_type(&module, &export.public_name)?;
                is_character(&ty).then(|| Completion {
                    label: export.public_name.clone(),
                    kind: CompletionKind::Field,
                    detail: Some(format!("character: {ty}")),
                    documentation: export.field.documentation.clone(),
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: Some(module.clone()),
                    source_module: Some(import.module.value.clone()),
                    additional_text_edits: Vec::new(),
                })
            })
            .collect()
    }

    pub(super) fn dialogue_hover(&self, path: &Path, position: SourcePosition) -> Option<Hover> {
        if let Some((name, _)) = self.dialogue_label(path, position) {
            return Some(Hover {
                is_markdown: false,
                contents: format!("dialogue label: {}", name.value),
                span: Some(name.span),
                documentation: None,
                context: vec!["Flow target in this dialogue file".into()],
            });
        }
        if let Some(hover) = self.dialogue_rich_hover(path, position) {
            return Some(hover);
        }
        if let Some((module, character, expression, span)) =
            self.expression_target(path, position, false)
        {
            let ty = self.dialogue_character_type(&module, &character)?;
            let field = expression_fields(&ty)?.field(&expression)?;
            return Some(Hover {
                is_markdown: false,
                contents: format!("{character}.{expression}: {}", field.ty),
                span: Some(span),
                documentation: None,
                context: vec![
                    "Per-line portrait selection; animation state belongs to the host".into(),
                ],
            });
        }
        if let Some(hover) = self.dialogue_event_hover(path, position) {
            return Some(hover);
        }
        if let Some(hover) = self.dialogue_context_hover(path, position) {
            return Some(hover);
        }
        let file = self.file(path)?;
        if self.config_for_path(path).i18n.enabled {
            if let Some(text) = dialogue_text(file, position) {
                return Some(Hover {
                    is_markdown: false,
                    contents: format!("dialogue text: {}\n{}", text.id.value, text.value),
                    span: Some(text.id.span),
                    documentation: None,
                    context: vec![
                        "Localization Explorer shows exact translations and fallback coverage"
                            .into(),
                    ],
                });
            }
        }
        let (token, span) = speaker_token(&file.source, position, false)?;
        let (alias, name) = token.split_once('.')?;
        let import = file
            .parsed
            .as_ref()
            .ok()?
            .dialogue_imports
            .iter()
            .find(|import| import.alias.value == alias)?;
        let module = self.dialogue_module(path, import)?;
        let export = self.module_graph.export(&module, name)?;
        let ty = self.dialogue_character_type(&module, name)?;
        Some(Hover {
            is_markdown: false,
            contents: format!("{token}: {ty}"),
            span: Some(span),
            documentation: export.field.documentation.clone(),
            context: vec![format!("module: {}", import.module.value)],
        })
    }

    pub(super) fn dialogue_definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        if let Some((_, label)) = self.dialogue_label(path, position) {
            return Some(Definition {
                path: path.to_owned(),
                span: label.span,
                kind: DefinitionKind::Local,
            });
        }
        if let Some((module, character, expression, _)) =
            self.expression_target(path, position, false)
        {
            let ty = self.dialogue_character_type(&module, &character)?;
            expression_fields(&ty)?.field(&expression)?;
            let source = self
                .module_graph
                .export_type_source_path(&module, &character)?;
            let export = self
                .module_graph
                .export(source, &character)
                .or_else(|| self.module_graph.export(&module, &character))?;
            return Some(Definition {
                path: source.to_owned(),
                span: export.definition_span,
                kind: DefinitionKind::TableField,
            });
        }
        if let Some(definition) = self.dialogue_event_definition(path, position) {
            return Some(definition);
        }
        if let Some(definition) = self.dialogue_context_definition(path, position) {
            return Some(definition);
        }
        let file = self.file(path)?;
        if self.config_for_path(path).i18n.enabled {
            if let Some(text) = dialogue_text(file, position) {
                let config = &self.config_for_path(path).i18n;
                if let Some(entry) = self.catalog_index_for_path(path).resolved_entry(
                    &text.id.value,
                    &config.default_locale,
                    &config.fallback_locale,
                ) {
                    return Some(Definition {
                        path: entry.path.clone(),
                        span: entry.span,
                        kind: DefinitionKind::TableField,
                    });
                }
            }
        }
        let imports = &file.parsed.as_ref().ok()?.dialogue_imports;
        if let Some(import) = imports
            .iter()
            .find(|import| contains(import.module.span, position))
        {
            return Some(Definition {
                path: self.dialogue_module(path, import)?,
                span: SourceSpan {
                    start: SourcePosition { line: 1, column: 1 },
                    end: SourcePosition { line: 1, column: 1 },
                },
                kind: DefinitionKind::Module,
            });
        }
        let (token, _) = speaker_token(&file.source, position, false)?;
        let (alias, name) = token.split_once('.')?;
        let import = imports.iter().find(|import| import.alias.value == alias)?;
        let module = self.dialogue_module(path, import)?;
        let export = self.module_graph.export(&module, name)?;
        Some(Definition {
            path: export.module_path.clone(),
            span: export.definition_span,
            kind: DefinitionKind::TableField,
        })
    }
}

fn is_character(ty: &Type) -> bool {
    match ty {
        Type::Readonly(inner) => is_character(inner),
        Type::Table(shape) => {
            shape
                .field("name")
                .is_some_and(|field| !field.optional && is_string(&field.ty))
                && shape
                    .field("nameKey")
                    .is_none_or(|field| is_optional_string(&field.ty))
                && shape
                    .field("portrait")
                    .is_none_or(|field| is_optional_string(&field.ty))
        }
        _ => false,
    }
}

fn is_string(ty: &Type) -> bool {
    match ty {
        Type::Primitive(PrimitiveType::String) | Type::StringLiteral(_) => true,
        Type::Union(types) => !types.is_empty() && types.iter().all(is_string),
        _ => false,
    }
}

fn contains(span: SourceSpan, position: SourcePosition) -> bool {
    span.start.line == position.line
        && position.column >= span.start.column
        && position.column <= span.end.column
}

/// Only the speaker position is a character reference, never arbitrary dialogue prose.
fn speaker_token(
    source: &str,
    position: SourcePosition,
    prefix: bool,
) -> Option<(String, SourceSpan)> {
    let line = source.lines().nth(position.line.checked_sub(1)?)?;
    let start = line.chars().count() - line.trim_start().chars().count() + 1;
    let token = line.trim_start().split(':').next()?.trim_end();
    if token.is_empty()
        || !token
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.' || ch == '-')
    {
        return None;
    }
    let end = start + token.chars().count();
    if position.column < start || position.column > end {
        return None;
    }
    let token = if prefix {
        token.chars().take(position.column - start).collect()
    } else {
        token.to_owned()
    };
    Some((
        token,
        SourceSpan {
            start: SourcePosition {
                line: position.line,
                column: start,
            },
            end: SourcePosition {
                line: position.line,
                column: end,
            },
        },
    ))
}

fn character_error(path: &Path, span: SourceSpan, message: String) -> Diagnostic {
    Diagnostic::error(path, span.start.line, span.start.column, message)
        .with_code(crate::DiagnosticCode::DOMAIN_DIALOGUE)
        .with_range(
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        )
}

fn is_optional_string(ty: &Type) -> bool {
    match ty {
        Type::Primitive(PrimitiveType::Nil) => true,
        Type::Union(types) => types.iter().all(is_optional_string),
        _ => is_string(ty),
    }
}

fn dialogue_text(
    file: &AnalysisFile,
    position: SourcePosition,
) -> Option<&crate::dialogue_asset::DialogueText> {
    file.dialogue_model
        .get()?
        .as_ref()?
        .sections
        .iter()
        .flat_map(|section| &section.lines)
        .flat_map(|line| {
            std::iter::once(&line.text).chain(
                line.choices
                    .iter()
                    .flat_map(|choice| std::iter::once(&choice.text).chain(choice.reason.iter())),
            )
        })
        .find(|text| contains(text.id.span, position))
}

fn expression_fields(ty: &Type) -> Option<&crate::types::TableShape> {
    let Type::Table(character) = unreadonly(ty) else {
        return None;
    };
    let Type::Table(expressions) = unreadonly(&character.field("expressions")?.ty) else {
        return None;
    };
    Some(expressions)
}

fn unreadonly(ty: &Type) -> &Type {
    match ty {
        Type::Readonly(inner) => unreadonly(inner),
        _ => ty,
    }
}

fn valid_expression(ty: &Type) -> bool {
    let Type::Table(expression) = unreadonly(ty) else {
        return false;
    };
    expression
        .field("portrait")
        .is_some_and(|field| !field.optional && is_string(&field.ty))
        && expression
            .field("animation")
            .is_none_or(|field| is_optional_string(&field.ty))
        && expression
            .fields()
            .all(|(name, _)| matches!(name, "portrait" | "animation"))
}

fn valid_character_speech(ty: &Type) -> bool {
    let Type::Table(character) = unreadonly(ty) else {
        return false;
    };
    let Some(field) = character.field("speech").filter(|f| !f.optional) else {
        return false;
    };
    let Type::Table(profile) = unreadonly(&field.ty) else {
        return false;
    };
    let text = profile
        .field("sampleSet")
        .is_some_and(|f| !f.optional && is_string(&f.ty));
    let numbers = [
        "pitchMin",
        "pitchMax",
        "variation",
        "gain",
        "minCueInterval",
    ]
    .iter()
    .all(|name| {
        profile.field(name).is_some_and(|f| {
            !f.optional && matches!(unreadonly(&f.ty), Type::Primitive(PrimitiveType::Number))
        })
    });
    fn cadence(ty: &Type) -> bool {
        match unreadonly(ty) {
            Type::StringLiteral(value) => matches!(value.as_str(), "character" | "word"),
            Type::Union(types) => !types.is_empty() && types.iter().all(cadence),
            _ => false,
        }
    }
    text && profile.fields().all(|(name, _)| {
        matches!(
            name,
            "sampleSet"
                | "pitchMin"
                | "pitchMax"
                | "variation"
                | "gain"
                | "minCueInterval"
                | "cadence"
        )
    }) && numbers
        && profile
            .field("cadence")
            .is_some_and(|f| !f.optional && cadence(&f.ty))
}
