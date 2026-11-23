//! Dialogue references use the same indexed exports and type environments as Tua imports.
use super::*;
use crate::dialogue_asset::CharacterImport;
use crate::types::PrimitiveType;

impl AnalysisSnapshot {
    pub(super) fn is_dialogue(&self, path: &Path) -> bool {
        self.file(path)
            .is_some_and(|file| file.kind == AnalysisSourceKind::Dialogue)
    }

    fn dialogue_module(&self, path: &Path, import: &CharacterImport) -> Option<PathBuf> {
        let parsed = self.file(path)?.parsed.as_ref().ok()?;
        self.indexed_module_path_for_specifier(parsed, &import.module.value)
            .map(Path::to_path_buf)
    }

    fn dialogue_character_type(&self, module: &Path, name: &str) -> Option<Type> {
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
        for import in &parsed.dialogue_imports {
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
            for speaker in model
                .sections
                .iter()
                .flat_map(|section| &section.lines)
                .filter_map(|line| line.speaker.as_ref())
            {
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
                            "character `{}` must be a record with a required string `name`; found {ty}",
                            speaker.value
                        );
                        diagnostics.push(character_error(path, speaker.span, message));
                    }
                    _ => {}
                }
            }
        }
        diagnostics
    }

    pub(super) fn dialogue_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Vec<Completion> {
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
        let file = self.file(path)?;
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
        let file = self.file(path)?;
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
        Type::Table(shape) => shape
            .field("name")
            .is_some_and(|field| !field.optional && is_string(&field.ty)),
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
