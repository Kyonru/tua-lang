//! Lower validated dialogue sections to ordinary Lua data, never executable text.
use super::{lua_string_literal, LoweringKind, LuaEmission, LuaLineProvenance};
use crate::dialogue_asset::{parse_dialogue_source, DialogueLine, DialogueSection};
use crate::{Diagnostic, SourceSpan};
use std::collections::BTreeMap;
use std::path::Path;

/// Compile the supported `.tdialogue` subset to a module accepted by tuadialogue.
/// No filesystem access or project execution during compilation. Character imports
/// become ordinary runtime requires; project analysis checks their typed exports.
/// The returned source map uses the same one-based positions as the parser.
pub fn emit_dialogue_source(
    path: impl AsRef<Path>,
    source: &str,
) -> Result<LuaEmission, Vec<Diagnostic>> {
    let _profile = crate::profile::span("dialogue.emit");
    let document = parse_dialogue_source(path, source)?;
    let entry = &document.sections[0].label;
    let speakers: BTreeMap<_, _> = document
        .speakers
        .iter()
        .map(|speaker| (speaker.id.value.as_str(), speaker.name.as_str()))
        .collect();
    let mut output = Writer(LuaEmission {
        code: String::new(),
        provenance: Vec::new(),
    });
    for (index, import) in document.imports.iter().enumerate() {
        output.line(
            &format!(
                "local __tua_characters_{} = require({})",
                index + 1,
                lua_string_literal(&import.module.value)
            ),
            import.module.span,
        );
    }
    if let Some(context) = &document.context {
        output.line(
            &format!(
                "local __tua_context = require({})",
                lua_string_literal(&context.module.value)
            ),
            context.module.span,
        );
    }
    output.line("return {", entry.span);
    output.line(
        if document.sections.iter().any(|section| {
            section
                .lines
                .iter()
                .any(|line| line.events.iter().any(|event| event.id.is_some()))
        }) {
            "  schemaVersion = 4,"
        } else if document.sections.iter().any(|section| {
            section.branch.is_some()
                || section.lines.iter().any(|line| {
                    line.fallback.is_some()
                        || line.choices.iter().any(|choice| {
                            choice.visible.is_some()
                                || choice.enabled.is_some()
                                || choice.reason.is_some()
                        })
                })
        }) {
            "  schemaVersion = 3,"
        } else if document.context.is_some() {
            "  schemaVersion = 2,"
        } else {
            "  schemaVersion = 1,"
        },
        entry.span,
    );
    if let Some(context) = &document.context {
        output.line("  context = __tua_context,", context.module.span);
        output.line("  bindings = {", context.module.span);
        for binding in &document.bindings {
            output.line("    {", binding.name.span);
            output.field("      name", &binding.name.value, binding.name.span);
            output.field("      path", &binding.path.value, binding.path.span);
            if let Some(value) = &binding.fallback {
                let value = match value {
                    crate::dialogue_asset::DialogueLiteral::String(value) => {
                        lua_string_literal(value)
                    }
                    crate::dialogue_asset::DialogueLiteral::Number(value) => value.clone(),
                    crate::dialogue_asset::DialogueLiteral::Boolean(value) => value.to_string(),
                };
                output.line(&format!("      fallback = {value},"), binding.path.span);
            }
            output.line("    },", binding.path.span);
        }
        output.line("  },", context.module.span);
    }
    if let Some(id) = &document.document_id {
        output.field("  documentId", &id.value, id.span);
    }
    output.field("  entry", &entry.value, entry.span);
    output.line("  nodes = {", entry.span);
    for section in &document.sections {
        if let Some(branch) = &section.branch {
            output.line(
                &format!("    [{}] = {{", lua_string_literal(&section.label.value)),
                section.label.span,
            );
            output.field("      kind", "branch", section.label.span);
            output.condition("      condition", &branch.condition);
            output.field("      yes", &branch.yes.value, branch.yes.span);
            output.field("      no", &branch.no.value, branch.no.span);
            output.line("    },", section.label.span);
        }
        for (index, line) in section.lines.iter().enumerate() {
            let origin = line.text.span;
            let id = node_id(section, index);
            let next = if !line.choices.is_empty() {
                choice_id(line)
            } else if line.end {
                "@end".into()
            } else {
                node_id(section, index + 1)
            };
            output.line(&format!("    [{}] = {{", lua_string_literal(&id)), origin);
            output.field("      kind", "line", origin);
            output.field("      text", &line.text.value, origin);
            output.field("      textId", &line.text.id.value, line.text.id.span);
            if let Some(speaker) = &line.speaker {
                if let Some(name) = speakers.get(speaker.value.as_str()) {
                    output.field("      speaker", name, speaker.span);
                } else if let Some((alias, member)) = speaker.value.split_once('.') {
                    let index = document
                        .imports
                        .iter()
                        .position(|import| import.alias.value == alias)
                        .expect("validated character alias");
                    output.line(
                        &format!(
                            "      speaker = __tua_characters_{}[{}].name,",
                            index + 1,
                            lua_string_literal(member)
                        ),
                        speaker.span,
                    );
                    output.line(
                        &format!(
                            "      speakerKey = __tua_characters_{}[{}].nameKey,",
                            index + 1,
                            lua_string_literal(member)
                        ),
                        speaker.span,
                    );
                    output.line(
                        &format!(
                            "      portrait = __tua_characters_{}[{}].portrait,",
                            index + 1,
                            lua_string_literal(member)
                        ),
                        speaker.span,
                    );
                }
            }
            if let Some(timing) = &line.timing {
                output.line(
                    &format!("      hold = {}, fade = {},", timing.hold, timing.fade),
                    timing.span,
                );
            }
            if !line.events.is_empty() {
                output.line("      events = {", origin);
                for event in &line.events {
                    let identity = event
                        .id
                        .as_ref()
                        .map(|id| {
                            format!(
                                "id = {}, source = {{ line = {}, column = {} }}, ",
                                lua_string_literal(&id.value),
                                event.span.start.line,
                                event.span.start.column
                            )
                        })
                        .unwrap_or_default();
                    output.line(
                        &format!(
                            "        {{ {identity}anchor = {}, delay = {}, name = {}, payload = {} }},",
                            lua_string_literal(&event.anchor),
                            event.delay,
                            lua_string_literal(&event.name),
                            lua_string_literal(&event.payload)
                        ),
                        event.span,
                    );
                }
                output.line("      },", origin);
            }
            output.field("      next", &next, origin);
            output.line("    },", origin);
            if !line.choices.is_empty() {
                output.line(
                    &format!("    [{}] = {{", lua_string_literal(&choice_id(line))),
                    origin,
                );
                output.field("      kind", "choice", origin);
                if let Some(fallback) = &line.fallback {
                    output.field("      fallback", &fallback.value, fallback.span);
                }
                output.line("      options = {", origin);
                for choice in &line.choices {
                    let at = choice.text.span;
                    output.line("        {", at);
                    output.field("          id", &choice.text.id.value, choice.text.id.span);
                    output.field("          text", &choice.text.value, at);
                    output.field("          target", &choice.target.value, choice.target.span);
                    if let Some(condition) = &choice.visible {
                        output.condition("          visible", condition);
                    }
                    if let Some(condition) = &choice.enabled {
                        output.condition("          enabled", condition);
                    }
                    if let Some(reason) = &choice.reason {
                        output.field("          reason", &reason.value, reason.span);
                        output.field("          reasonId", &reason.id.value, reason.id.span);
                    }
                    output.line("        },", at);
                }
                output.line("      },", origin);
                output.line("    },", origin);
            }
        }
    }
    output.line("    [\"@end\"] = { kind = \"end\" },", entry.span);
    output.line("  },", entry.span);
    output.line("}", entry.span);
    Ok(output.0)
}

// Authored labels cannot contain '@'; generated IDs cannot collide with them.
fn node_id(section: &DialogueSection, index: usize) -> String {
    if index == 0 {
        section.label.value.clone()
    } else {
        format!("@line:{}", section.lines[index].text.id.value)
    }
}

fn choice_id(line: &DialogueLine) -> String {
    format!("@choice:{}", line.text.id.value)
}

struct Writer(LuaEmission);
impl Writer {
    fn line(&mut self, value: &str, origin: SourceSpan) {
        self.0.code.push_str(value);
        self.0.code.push('\n');
        self.0.provenance.push(LuaLineProvenance {
            generated_line: self.0.provenance.len() + 1,
            source_span: origin,
            lowering: LoweringKind::Dialogue,
        });
    }

    fn condition(&mut self, key: &str, condition: &crate::dialogue_asset::Condition) {
        self.line(&format!("{key} = {{"), condition.span);
        self.instructions(condition);
        self.line("      },", condition.span);
    }
    fn instructions(&mut self, condition: &crate::dialogue_asset::Condition) {
        use crate::dialogue_asset::{ConditionKind as K, DialogueLiteral as L};
        let instruction = match &condition.kind {
            K::Nil => "op = \"nil\"".to_owned(),
            K::Value(value) => format!(
                "op = \"value\", value = {}",
                match value {
                    L::String(v) => lua_string_literal(v),
                    L::Number(v) => v.clone(),
                    L::Boolean(v) => v.to_string(),
                }
            ),
            K::Path(value) => format!("op = \"path\", path = {}", lua_string_literal(value)),
            K::Not(_) => "op = \"not\"".to_owned(),
            K::Binary(op, _, _) => format!("op = {}", lua_string_literal(op)),
        };
        self.line(&format!("        {{ {instruction} }},"), condition.span);
        match &condition.kind {
            K::Not(a) => self.instructions(a),
            K::Binary(_, a, b) => {
                self.instructions(a);
                self.instructions(b);
            }
            _ => {}
        }
    }

    fn field(&mut self, key: &str, value: &str, origin: SourceSpan) {
        self.line(&format!("{key} = {},", lua_string_literal(value)), origin);
    }
}
