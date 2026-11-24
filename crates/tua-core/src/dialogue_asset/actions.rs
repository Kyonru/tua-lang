//! Actions are declared host requests with two explicit outcome targets.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueAction {
    pub name: DialogueName,
    pub id: DialogueName,
    pub success: DialogueName,
    pub failure: DialogueName,
    pub payload: serde_json::Value,
    pub span: SourceSpan,
}

pub(super) fn action_import(raw: &str, number: usize) -> Option<Result<CharacterImport, String>> {
    let rest = raw.trim_start().strip_prefix("@actions ")?;
    let prefix = raw.len() - raw.trim_start().len();
    context::context_import(&format!("{}@context {}", &raw[..prefix], rest), number).map(|result| {
        result.map(|mut import| {
            import.alias.value = "__actions".into();
            import
        })
    })
}

impl Parser<'_> {
    pub(super) fn action_line(&mut self, raw: &str, number: usize, at: SourceSpan) -> bool {
        if let Some(import) = action_import(raw, number) {
            self.abandon_id();
            if !self.document.sections.is_empty() || self.document.actions.is_some() {
                self.error(at, "declare one @actions module before labels");
            } else {
                match import {
                    Ok(import) => self.document.actions = Some(import),
                    Err(message) => self.error(at, message.replace("context", "actions")),
                }
            }
            return true;
        }
        let Some(rest) = raw.trim().strip_prefix("@action ") else {
            return false;
        };
        self.abandon_id();
        if self.document.actions.is_none() {
            self.error(at, "@action requires an @actions module");
            return true;
        }
        let (name, rest) = take_word(rest);
        let (id, rest) = take_word(rest);
        let (success, rest) = take_word(rest);
        let (failure, payload) = take_word(rest);
        let tag = |value: &str, prefix: &str| {
            value
                .strip_prefix(prefix)
                .and_then(|v| v.strip_suffix(']'))
                .map(str::to_owned)
        };
        let (Some(id), Some(success), Some(failure)) = (
            tag(id, "[id:"),
            tag(success, "[success:"),
            tag(failure, "[failure:"),
        ) else {
            self.error(
                at,
                "expected @action name [id:stable.id] [success:label] [failure:label] JSON-record",
            );
            return true;
        };
        let (Some(name), Some(id), Some(success), Some(failure)) = (
            self.name(name, at),
            self.name(&id, at),
            self.name(&success, at),
            self.name(&failure, at),
        ) else {
            return true;
        };
        let payload = match events::payload(payload) {
            Ok(value) => value,
            Err(message) => {
                self.error(at, message);
                return true;
            }
        };
        let Some(section) = self.document.sections.last_mut() else {
            self.error(at, "action requires a label");
            return true;
        };
        if !section.lines.is_empty() || section.branch.is_some() || section.action.is_some() {
            self.error(
                at,
                "an action label contains exactly one action and no utterances or branch",
            );
            return true;
        }
        section.action = Some(DialogueAction {
            name,
            id,
            success,
            failure,
            payload,
            span: at,
        });
        true
    }
}
