//! Typed notification contracts share imported module facts with context/character analysis.
use super::*;
use crate::dialogue_asset::contracts::token_at as event_token;
use crate::dialogue_asset::CharacterImport;
use crate::DiagnosticCode;

pub(super) fn is_event_import(source: &str, import: &CharacterImport) -> bool {
    source
        .lines()
        .nth(import.module.span.start.line.saturating_sub(1))
        .is_some_and(|line| line.trim_start().starts_with("@events "))
}

pub(super) fn is_action_import(source: &str, import: &CharacterImport) -> bool {
    source
        .lines()
        .nth(import.module.span.start.line.saturating_sub(1))
        .is_some_and(|line| line.trim_start().starts_with("@actions "))
}

fn unwrap(ty: &Type) -> &Type {
    match ty {
        Type::Readonly(inner) => unwrap(inner),
        _ => ty,
    }
}

impl AnalysisSnapshot {
    fn contract_at(&self, path: &Path, position: SourcePosition) -> Option<(PathBuf, Type)> {
        let line = self
            .file(path)?
            .source
            .lines()
            .nth(position.line.checked_sub(1)?)?;
        self.event_contract(path, line.trim_start().starts_with("@action "))
    }

    fn event_contract(&self, path: &Path, actions: bool) -> Option<(PathBuf, Type)> {
        let file = self.file(path)?;
        let import = file
            .parsed
            .as_ref()
            .ok()?
            .dialogue_imports
            .iter()
            .find(|import| {
                if actions {
                    is_action_import(&file.source, import)
                } else {
                    is_event_import(&file.source, import)
                }
            })?;
        let module = self.dialogue_module(path, import)?;
        let value = self.indexed_module_value_type(&module)?;
        let value = self
            .type_database
            .resolve_type(&self.module_graph, &module, &value);
        let Type::Table(table) = unwrap(&value) else {
            return None;
        };
        if table.field("kind")?.ty
            != Type::string_literal(if actions { "actions" } else { "events" })
        {
            return None;
        }
        Some((module, unwrap(&table.field("payloads")?.ty).clone()))
    }

    pub(super) fn dialogue_event_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(model) = self
            .file(path)
            .and_then(|f| f.dialogue_model.get())
            .and_then(Option::as_ref)
        else {
            return Vec::new();
        };
        let Some(import) = &model.events else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut report = |span: SourceSpan, message: String| {
            result.push(
                Diagnostic::error(path, span.start.line, span.start.column, message)
                    .with_code(DiagnosticCode::DOMAIN_DIALOGUE)
                    .with_range(
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column,
                    ),
            );
        };
        let Some((_, Type::Table(payloads))) = self.event_contract(path, false) else {
            report(
                import.module.span,
                "event module must export a Dialogue.events definition".into(),
            );
            return result;
        };
        for event in model
            .sections
            .iter()
            .flat_map(|s| &s.lines)
            .flat_map(|line| &line.events)
        {
            let Some(data) = &event.data else {
                continue;
            };
            let Some(field) = payloads.field(&event.name) else {
                report(
                    event.span,
                    format!("unknown declared dialogue event `{}`", event.name),
                );
                continue;
            };
            if let Err(message) = check_payload(data, &field.ty, "payload") {
                report(event.span, format!("event `{}`: {message}", event.name));
            }
        }
        result
    }

    pub(super) fn dialogue_action_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let Some(model) = self
            .file(path)
            .and_then(|f| f.dialogue_model.get())
            .and_then(Option::as_ref)
        else {
            return Vec::new();
        };
        let Some(import) = &model.actions else {
            return Vec::new();
        };
        let mut result = Vec::new();
        let mut report = |span: SourceSpan, message: String| {
            result.push(
                Diagnostic::error(path, span.start.line, span.start.column, message)
                    .with_code(DiagnosticCode::DOMAIN_DIALOGUE)
                    .with_range(
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column,
                    ),
            );
        };
        let Some((_, Type::Table(payloads))) = self.event_contract(path, true) else {
            report(
                import.module.span,
                "action module must export a Dialogue.actions definition".into(),
            );
            return result;
        };
        for action in model
            .sections
            .iter()
            .filter_map(|section| section.action.as_ref())
        {
            let Some(field) = payloads.field(&action.name.value) else {
                report(
                    action.span,
                    format!("unknown declared dialogue action `{}`", action.name.value),
                );
                continue;
            };
            if let Err(message) = check_payload(&action.payload, &field.ty, "payload") {
                report(
                    action.span,
                    format!("action `{}`: {message}", action.name.value),
                );
            }
        }
        result
    }

    pub(super) fn dialogue_event_completion(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Vec<Completion>> {
        let file = self.file(path)?;
        let (name, field, token, span) = event_token(&file.source, position)?;
        let prefix: String = token
            .chars()
            .take(position.column - span.start.column)
            .collect();
        let (_, Type::Table(events)) = self.contract_at(path, position)? else {
            return Some(Vec::new());
        };
        let table = if field {
            let Type::Table(table) = unwrap(&events.field(&name)?.ty) else {
                return Some(Vec::new());
            };
            table
        } else {
            &events
        };
        Some(
            table
                .fields()
                .filter(|(name, _)| name.starts_with(&prefix))
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

    pub(super) fn dialogue_event_hover(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Hover> {
        let (name, field, token, span) = event_token(&self.file(path)?.source, position)?;
        let (_, Type::Table(events)) = self.contract_at(path, position)? else {
            return None;
        };
        let payload = &events.field(&name)?.ty;
        let ty = if field {
            let Type::Table(table) = unwrap(payload) else {
                return None;
            };
            &table.field(&token)?.ty
        } else {
            payload
        };
        Some(Hover {
            is_markdown: false,
            contents: format!("{token}: {ty}"),
            span: Some(span),
            documentation: None,
            context: vec![
                "Declared dialogue contract; payload captured at player construction".into(),
            ],
        })
    }

    pub(super) fn dialogue_event_definition(
        &self,
        path: &Path,
        position: SourcePosition,
    ) -> Option<Definition> {
        self.dialogue_event_hover(path, position)?;
        let (module, _) = self.contract_at(path, position)?;
        let export = self.module_graph.export(&module, "payloads")?;
        let source = self
            .module_graph
            .export_type_source_path(&module, "payloads")?;
        let origin = self
            .module_graph
            .export(source, "payloads")
            .unwrap_or(export);
        Some(Definition {
            path: source.to_path_buf(),
            span: origin.definition_span,
            kind: DefinitionKind::TableField,
        })
    }
}

fn check_payload(value: &serde_json::Value, ty: &Type, path: &str) -> Result<(), String> {
    match (value, unwrap(ty)) {
        (_, Type::Union(types)) => {
            if types
                .iter()
                .any(|ty| check_payload(value, ty, path).is_ok())
            {
                return Ok(());
            }
        }
        (serde_json::Value::Object(values), Type::Table(table)) => {
            for (name, field) in table.fields() {
                if !field.optional
                    && !crate::types::is_assignable(&Type::nil(), &field.ty)
                    && !values.contains_key(name)
                {
                    return Err(format!("{path} needs field `{name}`"));
                }
            }
            for (name, value) in values {
                let field = table
                    .field(name)
                    .ok_or_else(|| format!("unknown {path} field `{name}`"))?;
                check_payload(value, &field.ty, &format!("{path}.{name}"))?;
            }
            return Ok(());
        }
        (serde_json::Value::Array(values), Type::Array(item)) => {
            for value in values {
                check_payload(value, item, path)?;
            }
            return Ok(());
        }
        _ => {
            let value_type = match value {
                serde_json::Value::String(value) => Type::string_literal(value),
                serde_json::Value::Bool(_) => Type::boolean(),
                serde_json::Value::Number(_) => Type::number(),
                _ => return Err(format!("{path} does not match {ty}")),
            };
            if crate::types::is_assignable(&value_type, ty) {
                return Ok(());
            }
        }
    }
    Err(format!("{path} does not match {ty}"))
}
