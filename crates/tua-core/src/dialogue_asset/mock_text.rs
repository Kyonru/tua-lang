//! Plain mock text, using the parser's decoded, mark-free template. No host code.
use super::mock::{preview_number, DialogueMockContext, DialogueMockIssue};
use super::{DialogueLiteral, DialogueSource, DialogueText};
use crate::SourceSpan;
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 65_536;

/// Resolved scalar arguments. Bindings override explicit args, including false.
/// Numbers use Rust's shortest decimal formatting, not a particular Lua tostring.
#[derive(Clone, Debug)]
pub struct DialogueMockTextArguments {
    values: BTreeMap<String, String>,
}

impl DialogueMockTextArguments {
    /// Validate all bindings, as the runtime does when accepting context. Missing
    /// bindings therefore fail even if the next text does not use them. No schema
    /// callbacks or localization resolvers run. Retained argument text is bounded
    /// to 64 KiB, as is each formatted output.
    pub fn resolve(
        document: &DialogueSource,
        facts: &DialogueMockContext,
    ) -> Result<Self, DialogueMockIssue> {
        let mut values = BTreeMap::new();
        let mut bytes = 0;
        // Explicit args were validated by DialogueMockContext::parse.
        if let Some(args) = facts.lookup("args").ok().and_then(Value::as_object) {
            for (name, value) in args {
                let formatted = scalar(value).expect("validated scalar mock arg");
                bytes += formatted.len();
                if bytes > MAX_BYTES {
                    let at = document
                        .sections
                        .first()
                        .map(|section| section.label.span)
                        .unwrap_or_else(|| super::span(1, 1, 0));
                    return Err(issue(at, "mock argument text exceeds 64 KiB"));
                }
                values.insert(name.clone(), formatted);
            }
        }
        for binding in &document.bindings {
            let at = binding.path.span;
            let value = facts
                .lookup(&binding.path.value)
                .map_err(|_| issue(at, "binding needs a plain record"))?;
            let formatted = if value.is_null() {
                match &binding.fallback {
                    Some(DialogueLiteral::String(value)) => {
                        if value.len() > MAX_BYTES {
                            return Err(issue(at, "mock argument text exceeds 64 KiB"));
                        }
                        value.clone()
                    }
                    Some(DialogueLiteral::Boolean(value)) => value.to_string(),
                    Some(DialogueLiteral::Number(value)) => preview_number(value)
                        .ok_or_else(|| {
                            issue(at, "mock binding number exceeds the safe numeric range")
                        })?
                        .to_string(),
                    None => return Err(issue(at, "missing dialogue binding")),
                }
            } else {
                scalar(value)
                    .ok_or_else(|| issue(at, "dialogue arguments must be finite scalar values"))?
            };
            bytes -= values.get(&binding.name.value).map_or(0, String::len);
            bytes = bytes.saturating_add(formatted.len());
            if bytes > MAX_BYTES {
                return Err(issue(at, "mock argument text exceeds 64 KiB"));
            }
            values.insert(binding.name.value.clone(), formatted);
        }
        Ok(Self { values })
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Expand named placeholders and %% once. Substituted strings are literal:
    /// `%{other}` and rich marks inside argument values are never reinterpreted.
    /// Rich templates use DialogueText.value, already stripped by the core parser.
    pub fn format(&self, text: &DialogueText) -> Result<String, DialogueMockIssue> {
        let mut output = String::new();
        let mut rest = text.value.as_str();
        let mut append = |value: &str| {
            if value.len() > MAX_BYTES.saturating_sub(output.len()) {
                return Err(issue(text.span, "mock text output exceeds 64 KiB"));
            }
            output.push_str(value);
            Ok(())
        };
        while let Some(index) = rest.find('%') {
            append(&rest[..index])?;
            rest = &rest[index + 1..];
            if let Some(next) = rest.strip_prefix('%') {
                append("%")?;
                rest = next;
            } else if let Some(next) = rest.strip_prefix('{') {
                let end = next
                    .find('}')
                    .ok_or_else(|| issue(text.span, "unterminated dialogue placeholder"))?;
                let name = next[..end].trim();
                let value = self
                    .values
                    .get(name)
                    .ok_or_else(|| issue(text.span, "missing or invalid dialogue argument"))?;
                append(value)?;
                rest = &next[end + 1..];
            } else {
                append("%")?;
            }
        }
        append(rest)?;
        Ok(output)
    }
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => value.as_f64().map(|value| value.to_string()),
        _ => None,
    }
}

fn issue(span: SourceSpan, message: &'static str) -> DialogueMockIssue {
    DialogueMockIssue { message, span }
}
