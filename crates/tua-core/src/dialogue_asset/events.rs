//! Declarative JSON payloads; no expressions or executable Lua in event arguments.
use super::*;

pub(super) fn event_import(raw: &str, number: usize) -> Option<Result<CharacterImport, String>> {
    let rest = raw.trim_start().strip_prefix("@events ")?;
    let prefix = raw.len() - raw.trim_start().len();
    let rewritten = format!("{}@context {}", &raw[..prefix], rest);
    context::context_import(&rewritten, number).map(|result| {
        result.map(|mut import| {
            import.alias.value = "__events".into();
            import.module.span.start.column -= 1;
            import.module.span.end.column -= 1;
            import
        })
    })
}

impl Parser<'_> {
    pub(super) fn event_import_line(&mut self, raw: &str, number: usize) -> bool {
        let Some(import) = event_import(raw, number) else {
            return false;
        };
        self.abandon_id();
        let at = span(number, 1, raw.chars().count());
        if !self.document.sections.is_empty() || self.document.events.is_some() {
            self.error(at, "declare one @events module before labels");
        } else {
            match import {
                Ok(import) => self.document.events = Some(import),
                Err(message) => self.error(at, message.replace("context", "events")),
            }
        }
        true
    }
}

pub(super) fn payload(source: &str) -> Result<serde_json::Value, String> {
    if source.len() > 8192 {
        return Err("event payload exceeds 8192 bytes".into());
    }
    // serde rejects malformed/nonfinite numbers and caps parser nesting itself.
    let value: serde_json::Value = serde_json::from_str(source)
        .map_err(|error| format!("event payload must be a JSON record: {error}"))?;
    reject_duplicate_keys(source)?;
    if !value.is_object() {
        return Err("event payload must be a JSON record".into());
    }
    fn bounded(value: &serde_json::Value, depth: usize, remaining: &mut usize) -> bool {
        if depth > 16 || *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        match value {
            serde_json::Value::Array(values) => {
                values.iter().all(|v| bounded(v, depth + 1, remaining))
            }
            serde_json::Value::Object(values) => {
                values.values().all(|v| bounded(v, depth + 1, remaining))
            }
            serde_json::Value::Null => false,
            _ => true,
        }
    }
    if !bounded(&value, 0, &mut 256) {
        return Err("event payload allows 16 nesting levels and 256 values; omit optional fields instead of null".into());
    }
    Ok(value)
}

// JSON syntax has already been validated. Preserve duplicate-key mistakes instead
// of letting serde_json's map representation silently keep only the last value.
fn reject_duplicate_keys(source: &str) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut objects = Vec::<BTreeSet<String>>::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => objects.push(BTreeSet::new()),
            b'}' => {
                objects.pop();
            }
            b'"' => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'"' {
                    cursor += if bytes[cursor] == b'\\' { 2 } else { 1 };
                }
                if source[cursor + 1..].trim_start().starts_with(':') {
                    let key: String =
                        serde_json::from_str(&source[start..=cursor]).expect("validated JSON key");
                    if objects
                        .last_mut()
                        .is_some_and(|keys| !keys.insert(key.clone()))
                    {
                        return Err(format!("duplicate event payload key `{key}`"));
                    }
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    Ok(())
}
