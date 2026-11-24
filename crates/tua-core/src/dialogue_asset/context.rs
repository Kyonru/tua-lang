//! Context declarations contain module/path references and scalar defaults, never code.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueLiteral {
    String(String),
    Number(String),
    Boolean(bool),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueBinding {
    pub name: DialogueName,
    pub path: DialogueName,
    pub fallback: Option<DialogueLiteral>,
}

pub(super) fn context_import(raw: &str, number: usize) -> Option<Result<CharacterImport, String>> {
    let rest = raw.trim().strip_prefix("@context ")?.trim();
    Some((|| {
        let value = rest
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or("expected `@context \"project.module\"`")?;
        if !value
            .split('.')
            .all(|part| identifier(part) && !part.contains('-'))
        {
            return Err("context module must be an extensionless project module name".into());
        }
        let column = raw
            .find('"')
            .map(|offset| raw[..offset].chars().count() + 2)
            .unwrap();
        Ok(CharacterImport {
            alias: DialogueName {
                value: "__context".into(),
                span: span(number, 1, 8),
            },
            module: DialogueName {
                value: value.into(),
                span: span(number, column, value.chars().count()),
            },
        })
    })())
}

impl Parser<'_> {
    pub(super) fn context_line(&mut self, raw: &str, number: usize) -> bool {
        let text = raw.trim();
        if !text.starts_with("@context ") && !text.starts_with("@bind ") {
            return false;
        }
        self.abandon_id();
        let at = span(
            number,
            raw.chars().count() - raw.trim_start().chars().count() + 1,
            text.chars().count(),
        );
        if !self.document.sections.is_empty() {
            self.error(at, "declare context and bindings before labels");
            return true;
        }
        if let Some(import) = context_import(raw, number) {
            match import {
                Ok(import) if self.document.context.is_none() => {
                    self.document.context = Some(import)
                }
                Ok(_) => self.error(at, "only one context module is allowed"),
                Err(message) => self.error(at, message),
            }
            return true;
        }
        let (name, rest) = take_word(text.strip_prefix("@bind ").unwrap());
        let (path, rest) = take_word(rest);
        if !identifier(name) || name.contains(['.', '-']) {
            self.error(at, "binding name must be a simple identifier");
            return true;
        }
        let parts = path.split('.').collect::<Vec<_>>();
        if parts.len() < 2
            || parts.len() > 32
            || !matches!(parts[0], "context" | "locals")
            || !parts[1..]
                .iter()
                .all(|part| identifier(part) && !part.contains('-'))
        {
            self.error(
                at,
                "binding path must be `context.field` or `locals.field` (at most 32 segments)",
            );
            return true;
        }
        if self.document.bindings.len() >= 256
            || self.document.bindings.iter().any(|b| b.name.value == name)
        {
            self.error(at, "duplicate binding name or more than 256 bindings");
            return true;
        }
        let fallback = if rest.is_empty() {
            None
        } else if let Some(value) = rest.strip_prefix("default ") {
            let value = value.trim();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                let Some(value) = self.decode(&value[1..value.len() - 1], at, true) else {
                    return true;
                };
                Some(DialogueLiteral::String(value))
            } else if value == "true" || value == "false" {
                Some(DialogueLiteral::Boolean(value == "true"))
            } else if !value.starts_with('+')
                && value.parse::<f64>().is_ok_and(|value| value.is_finite())
            {
                Some(DialogueLiteral::Number(value.into()))
            } else {
                self.error(
                    at,
                    "binding default must be a string, finite number or boolean literal",
                );
                return true;
            }
        } else {
            self.error(
                at,
                "expected `default scalar-literal` after the binding path",
            );
            return true;
        };
        let path_start = raw.find(path).unwrap();
        let name_start = raw.find("@bind ").unwrap() + 6;
        let name_start =
            name_start + raw[name_start..].len() - raw[name_start..].trim_start().len();
        self.document.bindings.push(DialogueBinding {
            name: DialogueName {
                value: name.into(),
                span: span(
                    number,
                    raw[..name_start].chars().count() + 1,
                    name.chars().count(),
                ),
            },
            path: DialogueName {
                value: path.into(),
                span: span(
                    number,
                    raw[..path_start].chars().count() + 1,
                    path.chars().count(),
                ),
            },
            fallback,
        });
        true
    }
}
