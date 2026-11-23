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
    output.line("return {", entry.span);
    output.field("  entry", &entry.value, entry.span);
    output.line("  nodes = {", entry.span);
    for section in &document.sections {
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
                }
            }
            output.field("      next", &next, origin);
            output.line("    },", origin);
            if !line.choices.is_empty() {
                output.line(
                    &format!("    [{}] = {{", lua_string_literal(&choice_id(line))),
                    origin,
                );
                output.field("      kind", "choice", origin);
                output.line("      options = {", origin);
                for choice in &line.choices {
                    let at = choice.text.span;
                    output.line("        {", at);
                    output.field("          id", &choice.text.id.value, choice.text.id.span);
                    output.field("          text", &choice.text.value, at);
                    output.field("          target", &choice.target.value, choice.target.span);
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

    fn field(&mut self, key: &str, value: &str, origin: SourceSpan) {
        self.line(&format!("{key} = {},", lua_string_literal(value)), origin);
    }
}
