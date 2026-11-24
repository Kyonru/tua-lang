//! Context paths resolve against imported types; analysis never runs project code.
use super::*;
use crate::dialogue_asset::paths::token_at as binding_token;
use crate::dialogue_asset::{placeholders, DialogueBinding, DialogueLiteral};
use crate::types::PrimitiveType;

fn unwrapped(ty: &Type) -> &Type {
    match ty {
        Type::Readonly(inner) => unwrapped(inner),
        _ => ty,
    }
}

fn field_path(mut ty: Type, parts: &[&str]) -> Result<(Type, bool), String> {
    let mut optional = false;
    for part in parts {
        if let Type::Union(types) = unwrapped(&ty) {
            let non_nil: Vec<_> = types
                .iter()
                .filter(|ty| **ty != Type::nil())
                .cloned()
                .collect();
            optional |= non_nil.len() != types.len();
            ty = Type::union(non_nil);
        }
        let Type::Table(table) = unwrapped(&ty) else {
            return Err(format!("cannot read `{part}` from {ty}"));
        };
        let field = table
            .field(part)
            .ok_or_else(|| format!("unknown dialogue context field `{part}`"))?;
        optional |= field.optional;
        ty = field.ty.clone();
    }
    if let Type::Union(types) = unwrapped(&ty) {
        let non_nil: Vec<_> = types
            .iter()
            .filter(|ty| **ty != Type::nil())
            .cloned()
            .collect();
        optional |= non_nil.len() != types.len();
        ty = Type::union(non_nil);
    }
    Ok((ty, optional))
}

fn scalar(ty: &Type) -> bool {
    match unwrapped(ty) {
        Type::Primitive(PrimitiveType::String | PrimitiveType::Number | PrimitiveType::Boolean)
        | Type::StringLiteral(_) => true,
        Type::Union(types) => !types.is_empty() && types.iter().all(scalar),
        _ => false,
    }
}

pub(super) fn is_context_import(
    source: &str,
    import: &crate::dialogue_asset::CharacterImport,
) -> bool {
    source
        .lines()
        .nth(import.module.span.start.line.saturating_sub(1))
        .is_some_and(|line| line.trim_start().starts_with("@context "))
}

impl AnalysisSnapshot {
    fn context_root(&self, path: &Path, root: &str) -> Option<Type> {
        let file = self.file(path)?;
        let import = file
            .parsed
            .as_ref()
            .ok()?
            .dialogue_imports
            .iter()
            .find(|i| is_context_import(&file.source, i))?;
        let module = self.dialogue_module(path, import)?;
        let value = self.indexed_module_value_type(&module)?;
        let value = self
            .type_database
            .resolve_type(&self.module_graph, &module, &value);
        let Type::Table(table) = unwrapped(&value) else {
            return None;
        };
        let kind = &table.field("kind")?.ty;
        if kind != &Type::string_literal("context") {
            return None;
        }
        Some(
            table
                .field(match root {
                    "context" => "facts",
                    "locals" => "locals",
                    _ => return None,
                })?
                .ty
                .clone(),
        )
    }

    pub(super) fn context_path(&self, path: &Path, value: &str) -> Result<(Type, bool), String> {
        let parts: Vec<_> = value.split('.').collect();
        let root = self
            .context_root(path, parts[0])
            .ok_or("context module must export a Dialogue.context definition")?;
        field_path(root, &parts[1..])
    }

    pub(super) fn dialogue_context_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(model) = self
            .file(path)
            .and_then(|f| f.dialogue_model.get())
            .and_then(Option::as_ref)
        else {
            return Vec::new();
        };
        let Some(import) = &model.context else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut report = |span: SourceSpan, message: String| {
            result.push(
                Diagnostic::error(path, span.start.line, span.start.column, message)
                    .with_code(crate::DiagnosticCode::DOMAIN_DIALOGUE)
                    .with_range(
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column,
                    ),
            )
        };
        if self.context_root(path, "context").is_none()
            || self.context_root(path, "locals").is_none()
        {
            report(
                import.module.span,
                "context module must export a Dialogue.context definition".into(),
            );
            return result;
        }
        for binding in &model.bindings {
            let issue = match self.context_path(path, &binding.path.value) {
                Err(message) => Some(message),
                Ok((ty, optional)) => {
                    if !scalar(&ty) {
                        Some(format!(
                            "dialogue placeholder needs a scalar field, found {ty}"
                        ))
                    } else if optional && binding.fallback.is_none() {
                        Some("optional dialogue field requires a default".into())
                    } else if let Some(value) = &binding.fallback {
                        let fallback = match value {
                            DialogueLiteral::String(_) => Type::string(),
                            DialogueLiteral::Number(_) => Type::number(),
                            DialogueLiteral::Boolean(_) => Type::boolean(),
                        };
                        // Literal enum fields accept a matching literal rather than all strings.
                        let literal = match value {
                            DialogueLiteral::String(value) => Type::string_literal(value),
                            _ => fallback.clone(),
                        };
                        if crate::types::is_assignable(&literal, &ty) {
                            None
                        } else {
                            Some(format!(
                                "binding default {fallback} is incompatible with {ty}"
                            ))
                        }
                    } else {
                        None
                    }
                }
            };
            if let Some(message) = issue {
                report(binding.path.span, message);
            }
        }
        for text in
            model
                .sections
                .iter()
                .flat_map(|s| &s.lines)
                .flat_map(|line| {
                    std::iter::once(&line.text).chain(line.choices.iter().flat_map(|choice| {
                        std::iter::once(&choice.text).chain(choice.reason.iter())
                    }))
                })
        {
            match crate::i18n::dialogue_placeholders(&text.value) {
                Err(message) => report(text.span, message),
                Ok(names) => {
                    for name in names {
                        if !model.bindings.iter().any(|b| b.name.value == name) {
                            report(
                                text.span,
                                format!("dialogue placeholder `{name}` has no binding"),
                            );
                        }
                    }
                }
            }
        }
        result
    }

    fn placeholder_binding(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<(DialogueBinding, SourceSpan)> {
        let file = self.file(path)?;
        let name = placeholders::name_at(&file.source, position)?;
        let bindings = crate::dialogue_asset::binding_declarations(path, &file.source)?;
        let binding = bindings
            .into_iter()
            .find(|binding| binding.name.value == name.value)?;
        Some((binding, name.span))
    }

    pub(super) fn dialogue_placeholder_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let file = self.file(path)?;
        let name = placeholders::name_at(&file.source, position)?;
        let prefix: String = name
            .value
            .chars()
            .take(position.column - name.span.start.column)
            .collect();
        let bindings =
            crate::dialogue_asset::binding_declarations(path, &file.source).unwrap_or_default();
        Some(
            bindings
                .into_iter()
                .filter(|binding| binding.name.value.starts_with(&prefix))
                .filter_map(|binding| {
                    let (ty, optional) = self.context_path(path, &binding.path.value).ok()?;
                    if !scalar(&ty) {
                        return None;
                    }
                    Some(Completion {
                        label: binding.name.value,
                        kind: CompletionKind::Value,
                        detail: Some(format!(
                            "{}: {ty}{}",
                            binding.path.value,
                            if optional { "?" } else { "" }
                        )),
                        documentation: None,
                        deprecation: None,
                        insert_text: None,
                        filter_text: None,
                        is_snippet: false,
                        module_path: None,
                        source_module: None,
                        additional_text_edits: Vec::new(),
                    })
                })
                .collect(),
        )
    }

    pub(super) fn dialogue_placeholder_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let (binding, span) = self.placeholder_binding(path, position)?;
        let (ty, optional) = self.context_path(path, &binding.path.value).ok()?;
        Some(Hover {
            is_markdown: false,
            contents: format!(
                "placeholder {}: {ty}{}\n{}",
                binding.name.value,
                if optional { "?" } else { "" },
                binding.path.value
            ),
            span: Some(span),
            documentation: None,
            context: vec!["Value supplied by @bind; refreshed explicitly by the host".into()],
        })
    }

    pub(super) fn dialogue_placeholder_definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        let (binding, _) = self.placeholder_binding(path, position)?;
        Some(Definition {
            path: path.to_owned(),
            span: binding.name.span,
            kind: DefinitionKind::Local,
        })
    }

    pub(super) fn dialogue_context_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let (token, _) = binding_token(&self.file(path)?.source, position, true)?;
        let (parent, prefix) = token.rsplit_once('.')?;
        let (ty, _) = self.context_path(path, parent).ok()?;
        let Type::Table(table) = unwrapped(&ty) else {
            return Some(Vec::new());
        };
        Some(
            table
                .fields()
                .filter(|(name, _)| name.starts_with(prefix))
                .map(|(name, field)| Completion {
                    label: name.into(),
                    kind: CompletionKind::Field,
                    detail: Some(format!(
                        "{}{}",
                        field.ty,
                        if field.optional { "?" } else { "" }
                    )),
                    documentation: None,
                    deprecation: None,
                    insert_text: None,
                    filter_text: None,
                    is_snippet: false,
                    module_path: None,
                    source_module: None,
                    additional_text_edits: Vec::new(),
                })
                .collect(),
        )
    }

    pub(super) fn dialogue_context_definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        let file = self.file(path)?;
        let (token, _) = binding_token(&file.source, position, false)?;
        self.context_path(path, &token).ok()?;
        let import = file
            .parsed
            .as_ref()
            .ok()?
            .dialogue_imports
            .iter()
            .find(|i| is_context_import(&file.source, i))?;
        let module = self.dialogue_module(path, import)?;
        let field = if token.starts_with("locals.") {
            "locals"
        } else {
            "facts"
        };
        let export = self.module_graph.export(&module, field)?;
        let source = self.module_graph.export_type_source_path(&module, field)?;
        let origin = self.module_graph.export(source, field).unwrap_or(export);
        Some(Definition {
            path: source.to_path_buf(),
            span: origin.definition_span,
            kind: DefinitionKind::TableField,
        })
    }

    pub(super) fn dialogue_context_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let (token, span) = binding_token(&self.file(path)?.source, position, false)?;
        let (ty, optional) = self.context_path(path, &token).ok()?;
        Some(Hover {
            is_markdown: false,
            contents: format!("{token}: {ty}{}", if optional { "?" } else { "" }),
            span: Some(span),
            documentation: None,
            context: vec!["Read-only dialogue binding; refreshed explicitly by the host".into()],
        })
    }
}
