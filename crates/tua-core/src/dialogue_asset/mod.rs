//! Shared, non-executing parser for the version-one dialogue authoring subset.
use crate::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use std::collections::BTreeSet;
use std::path::Path;

mod actions;
pub mod contracts;
pub mod expressions;
pub mod flow;
mod media;
pub mod mock;
pub mod paths;
pub mod placeholders;
pub mod preview;
pub mod voice;
mod waits;
pub use actions::DialogueAction;
pub use media::DialogueSpeech;
pub use waits::DialogueWait;
mod conditions;
mod context;
mod events;
pub mod rich_text;
pub use conditions::{Branch, Condition, ConditionKind};
pub use context::{DialogueBinding, DialogueLiteral};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueName {
    pub value: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueRichText {
    pub source: String,
    pub kind: rich_text::FieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueText {
    pub rich: Option<DialogueRichText>,
    pub id: DialogueName,
    /// Decoded literal text; placeholders/effects are not evaluated.
    pub value: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueSpeaker {
    pub id: DialogueName,
    pub name: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueChoice {
    pub visible: Option<Condition>,
    pub enabled: Option<Condition>,
    pub reason: Option<DialogueText>,
    pub text: DialogueText,
    pub target: DialogueName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueEvent {
    pub id: Option<DialogueName>,
    pub data: Option<serde_json::Value>,
    pub anchor: String,
    pub delay: String,
    pub name: String,
    pub payload: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTiming {
    pub hold: String,
    pub fade: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueLine {
    pub voice: Option<serde_json::Value>,
    pub speech: Option<DialogueSpeech>,
    pub expression: Option<DialogueName>,
    pub fallback: Option<DialogueName>,
    pub events: Vec<DialogueEvent>,
    pub timing: Option<DialogueTiming>,
    pub speaker: Option<DialogueName>,
    pub text: DialogueText,
    pub end: bool,
    pub choices: Vec<DialogueChoice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueSection {
    pub wait: Option<DialogueWait>,
    pub action: Option<DialogueAction>,
    pub branch: Option<Branch>,
    pub label: DialogueName,
    pub lines: Vec<DialogueLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterImport {
    pub alias: DialogueName,
    pub module: DialogueName,
}

/// The same directive parser serves strict validation and incomplete editor buffers.
pub(crate) fn character_import(
    raw: &str,
    number: usize,
) -> Option<Result<CharacterImport, String>> {
    let text = raw.trim();
    let rest = text.strip_prefix("@characters ")?;
    let parsed = (|| {
        let (alias, quoted) = rest
            .split_once(char::is_whitespace)
            .ok_or("expected `@characters Alias \"project.module\"`")?;
        if !identifier(alias) || alias.contains(['.', '-']) {
            return Err("character import alias must be a simple identifier");
        }
        let quoted = quoted.trim();
        let module = quoted
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or("character module must be quoted")?;
        if !module
            .split('.')
            .all(|part| identifier(part) && !part.contains('-'))
        {
            return Err("character module must be an extensionless project module name");
        }
        let prefix = raw.chars().count() - raw.trim_start().chars().count();
        let alias_column = prefix + "@characters ".len() + 1;
        let module_column = raw
            .find('"')
            .map(|offset| raw[..offset].chars().count() + 2)
            .unwrap();
        Ok(CharacterImport {
            alias: DialogueName {
                value: alias.into(),
                span: span(number, alias_column, alias.chars().count()),
            },
            module: DialogueName {
                value: module.into(),
                span: span(number, module_column, module.chars().count()),
            },
        })
    })();
    Some(parsed.map_err(str::to_owned))
}

pub(crate) fn character_imports(source: &str) -> Vec<CharacterImport> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            character_import(line, index + 1)
                .or_else(|| context::context_import(line, index + 1))
                .or_else(|| events::event_import(line, index + 1))
                .or_else(|| actions::action_import(line, index + 1))?
                .ok()
        })
        .collect()
}

pub(crate) fn character_module_specifiers(
    source: &str,
) -> Vec<crate::module_link::ModuleSpecifierLiteral> {
    character_imports(source)
        .into_iter()
        .map(|import| crate::module_link::ModuleSpecifierLiteral {
            module: import.module.value,
            span: import.module.span,
            kind: crate::module_link::ModuleSpecifierKind::Require,
            type_only: false,
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueSource {
    pub rich_text: bool,
    pub actions: Option<CharacterImport>,
    pub events: Option<CharacterImport>,
    pub context: Option<CharacterImport>,
    pub bindings: Vec<DialogueBinding>,
    pub document_id: Option<DialogueName>,
    pub imports: Vec<CharacterImport>,
    pub speakers: Vec<DialogueSpeaker>,
    /// The first section is the entry. Sections never implicitly fall through.
    pub sections: Vec<DialogueSection>,
}

/// Parse and validate a single asset without reading files or executing Lua.
/// Positions are one-based Unicode scalar columns, with exclusive range ends.
/// Cross-document translation ID uniqueness belongs to project integration.
pub fn parse_dialogue_source(
    path: impl AsRef<Path>,
    source: &str,
) -> Result<DialogueSource, Vec<Diagnostic>> {
    let _profile = crate::profile::span("dialogue.parse");
    let mut parser = parser_for(path.as_ref());
    if source.len() > crate::input_limits::MAX_SOURCE_BYTES {
        parser.error(span(1, 1, 1), "dialogue source exceeds byte limit");
        return Err(parser.diagnostics);
    }
    for (index, raw) in source.lines().enumerate() {
        parser.line(index + 1, raw);
    }
    parser.finish()
}

fn parser_for(path: &Path) -> Parser<'_> {
    Parser {
        path,
        document: DialogueSource {
            rich_text: false,
            actions: None,
            events: None,
            context: None,
            bindings: Vec::new(),
            document_id: None,
            imports: Vec::new(),
            speakers: Vec::new(),
            sections: Vec::new(),
        },
        diagnostics: Vec::new(),
        pending_id: None,
        ids: BTreeSet::new(),
        labels: BTreeSet::new(),
        speakers: BTreeSet::new(),
        header: false,
        rich_directive_allowed: false,
    }
}

/// Parse header declarations with the real grammar while the text body is incomplete.
/// Invalid/ambiguous headers are not suitable for authoring suggestions.
pub(crate) fn binding_declarations(path: &Path, source: &str) -> Option<Vec<DialogueBinding>> {
    if source.len() > crate::input_limits::MAX_SOURCE_BYTES {
        return None;
    }
    let mut parser = parser_for(path);
    for (index, raw) in source.lines().enumerate() {
        parser.line(index + 1, raw);
        if !parser.document.sections.is_empty() {
            break;
        }
    }
    (parser.header && parser.document.context.is_some() && parser.diagnostics.is_empty())
        .then_some(parser.document.bindings)
}

fn span(line: usize, column: usize, width: usize) -> SourceSpan {
    SourceSpan {
        start: SourcePosition { line, column },
        end: SourcePosition {
            line,
            column: column + width,
        },
    }
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn take_word(value: &str) -> (&str, &str) {
    let value = value.trim();
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (&value[..end], value[end..].trim())
}

fn seconds(value: &str) -> Option<String> {
    let seconds: f64 = value.parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.to_string())
}

struct Parser<'a> {
    path: &'a Path,
    document: DialogueSource,
    diagnostics: Vec<Diagnostic>,
    pending_id: Option<DialogueName>,
    ids: BTreeSet<String>,
    labels: BTreeSet<String>,
    speakers: BTreeSet<String>,
    header: bool,
    rich_directive_allowed: bool,
}

impl Parser<'_> {
    fn error(&mut self, at: SourceSpan, message: impl Into<String>) {
        self.diagnostics.push(
            Diagnostic::error(self.path, at.start.line, at.start.column, message)
                .with_code(DiagnosticCode::DOMAIN_DIALOGUE)
                .with_range(at.start.line, at.start.column, at.end.line, at.end.column),
        );
    }

    fn name(&mut self, value: &str, at: SourceSpan) -> Option<DialogueName> {
        if !identifier(value) {
            self.error(
                at,
                "expected an identifier starting with a letter or underscore",
            );
            return None;
        }
        Some(DialogueName {
            value: value.into(),
            span: at,
        })
    }

    fn abandon_id(&mut self) {
        if let Some(id) = self.pending_id.take() {
            self.error(id.span, "text ID must precede a line or choice");
        }
    }

    fn line(&mut self, number: usize, raw: &str) {
        let text = raw.trim_start_matches([' ', '\t']);
        let column = raw.chars().count() - text.chars().count() + 1;
        let at = span(number, column, text.chars().count());
        let syntax = text.trim_end();
        if syntax.is_empty() {
            return;
        }
        if let Some(comment) = syntax.strip_prefix("//") {
            let annotation = comment.trim_start();
            if let Some(value) = annotation.strip_prefix("@id") {
                self.abandon_id();
                if !value.starts_with(char::is_whitespace) {
                    self.error(at, "expected `// @id identifier`");
                    return;
                }
                let value = value.trim();
                let offset = syntax.rfind(value).unwrap_or(syntax.len());
                let id_at = span(
                    number,
                    column + syntax[..offset].chars().count(),
                    value.chars().count(),
                );
                if let Some(id) = self.name(value, id_at) {
                    if !self.ids.insert(id.value.clone()) {
                        self.error(id.span, format!("duplicate text ID `{}`", id.value));
                    }
                    self.pending_id = Some(id);
                }
            }
            return;
        }
        if !self.header {
            self.header = true;
            self.rich_directive_allowed = true;
            if syntax != "@dialogue 1" {
                self.error(at, "first directive must be `@dialogue 1`");
            }
            self.abandon_id();
            return;
        }
        if syntax.starts_with("@text") {
            if syntax != "@text rich" {
                self.error(at, "expected `@text rich`");
            } else if !self.rich_directive_allowed || self.pending_id.is_some() {
                self.error(
                    at,
                    "declare `@text rich` once immediately after `@dialogue 1`",
                );
            } else {
                self.document.rich_text = true;
            }
            self.rich_directive_allowed = false;
            self.abandon_id();
            return;
        }
        self.rich_directive_allowed = false;
        if self.media_line(raw, at) || self.wait_line(raw, at) || self.action_line(raw, number, at)
        {
            return;
        }
        if self.condition_line(syntax, at) {
            return;
        }
        if self.event_import_line(raw, number) {
            return;
        }
        if self.context_line(raw, number) {
            return;
        }
        if let Some(rest) = syntax.strip_prefix("@document ") {
            self.abandon_id();
            if !self.document.sections.is_empty() || self.document.document_id.is_some() {
                self.error(at, "declare one document ID before the first label");
            } else {
                self.document.document_id = self.name(rest.trim(), at);
            }
        } else if let Some(rest) = syntax.strip_prefix("@event ") {
            self.event(rest, at, false);
        } else if let Some(rest) = syntax.strip_prefix("@emit ") {
            self.event(rest, at, true);
        } else if let Some(rest) = syntax.strip_prefix("@expression ") {
            self.abandon_id();
            let offset = syntax.len() - rest.trim_start().len();
            let name = self.name(
                rest.trim(),
                span(number, column + offset, rest.trim().chars().count()),
            );
            if let Some(name) = name {
                let Some(line) = self
                    .document
                    .sections
                    .last_mut()
                    .and_then(|s| s.lines.last_mut())
                else {
                    self.error(at, "expression requires a preceding utterance");
                    return;
                };
                if line.expression.is_some()
                    || !line.choices.is_empty()
                    || !line.speaker.as_ref().is_some_and(|speaker| {
                        speaker.value.split_once('.').is_some_and(|(alias, _)| {
                            self.document
                                .imports
                                .iter()
                                .any(|import| import.alias.value == alias)
                        }) && !self
                            .document
                            .speakers
                            .iter()
                            .any(|local| local.id.value == speaker.value)
                    })
                {
                    self.error(at, "declare one expression after an utterance with an imported character, before choices");
                } else {
                    line.expression = Some(name);
                }
            }
        } else if let Some(rest) = syntax.strip_prefix("@timing ") {
            self.timing(rest, at);
        } else if let Some(import) = character_import(raw, number) {
            self.abandon_id();
            if !self.document.sections.is_empty() {
                self.error(at, "declare character imports before the first label");
                return;
            }
            match import {
                Ok(import) => {
                    if self
                        .document
                        .imports
                        .iter()
                        .any(|previous| previous.alias.value == import.alias.value)
                    {
                        self.error(import.alias.span, "duplicate character import alias");
                    }
                    self.document.imports.push(import);
                }
                Err(message) => self.error(at, message),
            }
        } else if let Some(rest) = syntax.strip_prefix("@character ") {
            self.abandon_id();
            self.speaker(rest, at);
        } else if syntax.starts_with('[') {
            self.abandon_id();
            if let Some(label) = syntax.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if matches!(label, "else" | "endif") {
                    self.error(at, "conditional directives are not supported yet");
                    return;
                }
                let label_at = span(number, column + 1, label.chars().count());
                if let Some(label) = self.name(label, label_at) {
                    if !self.labels.insert(label.value.clone()) {
                        self.error(label.span, format!("duplicate label `{}`", label.value));
                    }
                    self.document.sections.push(DialogueSection {
                        wait: None,
                        action: None,
                        branch: None,
                        label,
                        lines: Vec::new(),
                    });
                }
            } else {
                self.error(
                    at,
                    "expected `[label]`; unsupported directive or trailing text",
                );
            }
        } else if let Some(rest) = text.strip_prefix("-> ") {
            self.choice(rest, at);
        } else if text.starts_with(['@', '$']) {
            self.abandon_id();
            self.error(at, "unsupported dialogue directive in version-one subset");
        } else {
            self.utterance(text, at);
        }
    }

    fn metadata_line(&mut self, at: SourceSpan) -> Option<&mut DialogueLine> {
        self.abandon_id();
        if self
            .document
            .sections
            .last()
            .and_then(|section| section.lines.last())
            .is_none_or(|line| !line.choices.is_empty())
        {
            self.error(
                at,
                "line metadata must follow an utterance, before any choices",
            );
            return None;
        }
        self.document.sections.last_mut()?.lines.last_mut()
    }

    fn event(&mut self, rest: &str, at: SourceSpan, typed: bool) {
        let (anchor, rest) = take_word(rest);
        let (delay, rest) = take_word(rest);
        let (name, payload) = take_word(rest);
        let (id, payload) = if let Some(tag) = payload.strip_prefix("[id:") {
            let Some((id, payload)) = tag.split_once(']') else {
                self.error(at, "event ID needs a closing ]");
                return;
            };
            let Some(id) = self.name(id, at) else {
                return;
            };
            (Some(id), payload.trim_start())
        } else {
            (None, payload)
        };
        let Some(delay) = seconds(delay) else {
            self.error(at, "event delay must be finite and nonnegative");
            return;
        };
        if !matches!(anchor, "shown" | "revealed") || !identifier(name) {
            self.error(
                at,
                "expected `@event shown|revealed seconds event.name [\"payload\"]`",
            );
            return;
        }
        let data = if typed {
            if self.document.events.is_none() || id.is_none() {
                self.error(
                    at,
                    "@emit requires an @events module and an explicit [id:...] tag",
                );
                return;
            }
            match events::payload(payload) {
                Ok(value) => Some(value),
                Err(message) => {
                    self.error(at, message);
                    return;
                }
            }
        } else {
            None
        };
        let payload = if typed || payload.is_empty() {
            String::new()
        } else {
            let Some(quoted) = payload.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
                self.error(at, "event payload must be a quoted string");
                return;
            };
            let Some(decoded) = self.decode(quoted, at, true) else {
                return;
            };
            decoded
        };
        if let Some(line) = self.metadata_line(at) {
            if line.events.len() >= 64 {
                self.error(at, "a line supports at most 64 events");
                return;
            }
            line.events.push(DialogueEvent {
                id,
                data,
                anchor: anchor.into(),
                delay,
                name: name.into(),
                payload,
                span: at,
            });
        }
    }

    fn timing(&mut self, rest: &str, at: SourceSpan) {
        let words: Vec<_> = rest.split_whitespace().collect();
        if words.len() != 4 || words[0] != "hold" || words[2] != "fade" {
            self.error(at, "expected `@timing hold seconds fade seconds`");
            return;
        }
        let (Some(hold), Some(fade)) = (seconds(words[1]), seconds(words[3])) else {
            self.error(at, "hold and fade must be finite and nonnegative");
            return;
        };
        if let Some(line) = self.metadata_line(at) {
            if line.timing.is_some() {
                self.error(at, "duplicate timing for this line");
                return;
            }
            line.timing = Some(DialogueTiming {
                hold,
                fade,
                span: at,
            });
        }
    }

    fn speaker(&mut self, rest: &str, at: SourceSpan) {
        if !self.document.sections.is_empty() {
            self.error(at, "declare speakers before the first label");
            return;
        }
        let Some((id, quoted)) = rest.split_once(char::is_whitespace) else {
            self.error(at, "expected `@character Id \"Display name\"`");
            return;
        };
        let Some(id) = self.name(
            id,
            span(at.start.line, at.start.column + 11, id.chars().count()),
        ) else {
            return;
        };
        let quoted = quoted.trim();
        let Some(value) = quoted.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
            self.error(at, "speaker display name must be quoted");
            return;
        };
        let Some(name) = self.decode(value, at, true) else {
            return;
        };
        if name.is_empty() {
            self.error(at, "speaker display name must not be empty");
        }
        if !self.speakers.insert(id.value.clone()) {
            self.error(id.span, format!("duplicate speaker `{}`", id.value));
        }
        self.document
            .speakers
            .push(DialogueSpeaker { id, name, span: at });
    }

    fn text(
        &mut self,
        value: &str,
        at: SourceSpan,
        kind: rich_text::FieldKind,
    ) -> Option<DialogueText> {
        let Some(id) = self.pending_id.take() else {
            self.error(at, "line or choice needs an explicit `// @id identifier`");
            return None;
        };
        let (value, rich) = if self.document.rich_text {
            match rich_text::parse(value, kind) {
                Ok(template) => (
                    template.plain_template,
                    Some(DialogueRichText {
                        source: value.to_owned(),
                        kind,
                    }),
                ),
                Err(error) => {
                    let position = span(
                        at.start.line,
                        at.start.column + value[..error.range.start].chars().count(),
                        value[error.range].chars().count(),
                    );
                    self.error(position, error.message);
                    return None;
                }
            }
        } else {
            (self.decode(value, at, false)?, None)
        };
        if value.trim().is_empty() {
            self.error(at, "dialogue text must not be empty");
            return None;
        }
        Some(DialogueText {
            rich,
            id,
            value,
            span: at,
        })
    }

    fn decode(&mut self, value: &str, at: SourceSpan, quoted: bool) -> Option<String> {
        let mut output = String::new();
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '%' if chars.peek() == Some(&'{') => {
                    chars.next();
                    let mut name = String::new();
                    let mut closed = false;
                    for next in chars.by_ref() {
                        if next == '}' {
                            closed = true;
                            break;
                        }
                        name.push(next);
                    }
                    let key = name.trim();
                    if !closed || !identifier(key) || key.contains(['.', '-']) {
                        self.error(at, "expected a named placeholder `%{name}`");
                        return None;
                    }
                    output.push_str(&format!("%{{{key}}}"));
                }
                '\\' => match chars.next() {
                    Some('n') => output.push('\n'),
                    Some('t') => output.push('\t'),
                    Some('r') => output.push('\r'),
                    Some(c @ ('\\' | '"' | '{' | '}' | '$' | '(' | ')' | '[' | ']')) => {
                        output.push(c)
                    }
                    _ => {
                        self.error(at, "unknown or unfinished text escape");
                        return None;
                    }
                },
                '[' | ']' => {
                    self.error(
                        at,
                        "inline directives are not supported; escape literal brackets",
                    );
                    return None;
                }
                '{' | '}' => {
                    self.error(
                        at,
                        "interpolation and rich text are not supported yet; escape literal braces",
                    );
                    return None;
                }
                '"' if quoted => {
                    self.error(at, "escape quotes inside speaker display names");
                    return None;
                }
                c if c.is_control() && c != '\t' => {
                    self.error(at, "unexpected control character in dialogue text");
                    return None;
                }
                _ => output.push(c),
            }
        }
        Some(output)
    }

    fn utterance(&mut self, text: &str, at: SourceSpan) {
        let Some((speaker, value)) = text.split_once(':') else {
            self.abandon_id();
            self.error(at, "expected `Speaker: text` or `: narration`");
            return;
        };
        let speaker = if speaker.is_empty() {
            None
        } else {
            let Some(name) = self.name(
                speaker,
                span(at.start.line, at.start.column, speaker.chars().count()),
            ) else {
                self.abandon_id();
                return;
            };
            Some(name)
        };
        let value_column = at.start.column + text.chars().count() - value.chars().count()
            + usize::from(value.starts_with(' '));
        let value = value.strip_prefix(' ').unwrap_or(value);
        let (value, end) = if let Some(prefix) = value.trim_end().strip_suffix("(end)") {
            let escaped = prefix.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1;
            if escaped {
                (value, false)
            } else {
                (prefix.strip_suffix(' ').unwrap_or(prefix), true)
            }
        } else {
            (value, false)
        };
        let value_at = span(at.start.line, value_column, value.chars().count());
        let Some(text) = self.text(value, value_at, rich_text::FieldKind::Line) else {
            return;
        };
        let Some(section) = self.document.sections.last_mut() else {
            self.error(at, "line requires a preceding label");
            return;
        };
        if section
            .lines
            .last()
            .is_some_and(|line| line.end || !line.choices.is_empty())
        {
            self.error(
                at,
                "line after a terminal line or choices requires a new label",
            );
            return;
        }
        if section.branch.is_some() || section.action.is_some() || section.wait.is_some() {
            self.error(at, "branch/action/wait labels cannot contain utterances");
            return;
        }
        section.lines.push(DialogueLine {
            voice: None,
            speech: None,
            expression: None,
            fallback: None,
            events: Vec::new(),
            timing: None,
            speaker,
            text,
            end,
            choices: Vec::new(),
        });
    }

    fn choice(&mut self, text: &str, at: SourceSpan) {
        let Some((value, target)) = text.trim_end().rsplit_once(" [target:") else {
            self.abandon_id();
            self.error(at, "choice must end with ` [target:label]`");
            return;
        };
        let Some(target) = target.strip_suffix(']') else {
            self.abandon_id();
            self.error(at, "choice target must end with `]`");
            return;
        };
        let target_at = span(
            at.start.line,
            at.start.column + 3 + value.chars().count() + 9,
            target.chars().count(),
        );
        let Some(target) = self.name(target, target_at) else {
            self.abandon_id();
            return;
        };
        let Some(text) = self.text(
            value,
            span(at.start.line, at.start.column + 3, value.chars().count()),
            rich_text::FieldKind::Choice,
        ) else {
            return;
        };
        let Some(line) = self
            .document
            .sections
            .last_mut()
            .and_then(|s| s.lines.last_mut())
        else {
            self.error(at, "choice requires a preceding utterance");
            return;
        };
        if line.end {
            self.error(at, "terminal utterance cannot have choices");
            return;
        }
        line.choices.push(DialogueChoice {
            text,
            target,
            visible: None,
            enabled: None,
            reason: None,
        });
    }

    fn finish(mut self) -> Result<DialogueSource, Vec<Diagnostic>> {
        if self.document.context.is_none() && !self.document.bindings.is_empty() {
            self.error(
                self.document.bindings[0].path.span,
                "placeholder bindings require an @context module",
            );
        }
        self.abandon_id();
        if !self.header || self.document.sections.is_empty() {
            self.error(
                span(1, 1, 1),
                "dialogue needs a version header and at least one labeled section",
            );
        }
        let mut errors = Vec::new();
        let mut event_ids = BTreeSet::new();
        let labels: BTreeSet<_> = self
            .document
            .sections
            .iter()
            .map(|s| s.label.value.as_str())
            .collect();
        let speakers: BTreeSet<_> = self
            .document
            .speakers
            .iter()
            .map(|s| s.id.value.as_str())
            .collect();
        for speaker in &self.document.speakers {
            if self.document.imports.iter().any(|import| {
                speaker.id.value == import.alias.value
                    || speaker
                        .id
                        .value
                        .starts_with(&format!("{}.", import.alias.value))
            }) {
                errors.push((
                    speaker.id.span,
                    "local speaker conflicts with a character import alias".into(),
                ));
            }
        }
        for section in &self.document.sections {
            if let Some(wait) = &section.wait {
                if !labels.contains(wait.target.value.as_str()) {
                    errors.push((
                        wait.target.span,
                        format!("unknown wait target `{}`", wait.target.value),
                    ));
                }
            }
            if let Some(branch) = &section.branch {
                for target in [&branch.yes, &branch.no] {
                    if !labels.contains(target.value.as_str()) {
                        errors.push((target.span, format!("unknown target `{}`", target.value)));
                    }
                }
            }
            if let Some(action) = &section.action {
                if self.document.document_id.is_none() {
                    errors.push((action.id.span, "action IDs require an @document ID".into()));
                }
                if !event_ids.insert(&action.id.value) {
                    errors.push((
                        action.id.span,
                        format!("duplicate event/action ID `{}`", action.id.value),
                    ));
                }
                for target in [&action.success, &action.failure] {
                    if !labels.contains(target.value.as_str()) {
                        errors.push((
                            target.span,
                            format!("unknown action target `{}`", target.value),
                        ));
                    }
                }
            }
            if section.lines.is_empty()
                && section.branch.is_none()
                && section.action.is_none()
                && section.wait.is_none()
            {
                errors.push((
                    section.label.span,
                    "label needs at least one utterance".into(),
                ));
            } else if section
                .lines
                .last()
                .is_some_and(|line| !line.end && line.choices.is_empty())
            {
                errors.push((
                    section.label.span,
                    "section must finish with `(end)` or choices".into(),
                ));
            }
            for line in &section.lines {
                for event in &line.events {
                    if let Some(id) = &event.id {
                        if self.document.document_id.is_none() {
                            errors.push((id.span, "event IDs require an @document ID".into()));
                        }
                        if !event_ids.insert(&id.value) {
                            errors.push((id.span, format!("duplicate event ID `{}`", id.value)));
                        }
                    }
                }
                if let Some(target) = &line.fallback {
                    if !labels.contains(target.value.as_str()) {
                        errors.push((
                            target.span,
                            format!("unknown fallback target `{}`", target.value),
                        ));
                    }
                }
                if line
                    .choices
                    .iter()
                    .any(|c| c.visible.is_some() || c.enabled.is_some())
                    && line.fallback.is_none()
                {
                    errors.push((
                        line.text.span,
                        "conditional choices require an explicit @fallback label".into(),
                    ));
                }
                if let Some(speaker) = &line.speaker {
                    let imported = speaker
                        .value
                        .split_once('.')
                        .is_some_and(|(alias, member)| {
                            !member.contains('.')
                                && self
                                    .document
                                    .imports
                                    .iter()
                                    .any(|import| import.alias.value == alias)
                        });
                    if !speakers.contains(speaker.value.as_str()) && !imported {
                        errors.push((speaker.span, format!("unknown speaker `{}`", speaker.value)));
                    }
                }
                for choice in &line.choices {
                    if !labels.contains(choice.target.value.as_str()) {
                        errors.push((
                            choice.target.span,
                            format!("unknown target `{}`", choice.target.value),
                        ));
                    }
                }
            }
        }
        for (at, message) in errors {
            self.error(at, message);
        }
        if self.diagnostics.is_empty() {
            Ok(self.document)
        } else {
            Err(self.diagnostics)
        }
    }
}

// A validated asset has a typed export but no Lua symbols or executable body.
// Dedicated dialogue queries will use the authored model, never generated offsets.
pub(crate) fn parse_module_source(
    path: &Path,
    source: &str,
) -> Result<crate::ParsedSource, Vec<Diagnostic>> {
    // Syntax diagnostics and the authored model are cached separately by analysis.
    // Keep an opaque Lua body even during incomplete edits, so dependency edges
    // and typed asset exports remain available without inventing Lua symbols.
    let mut parsed = crate::parse_lua_source(path, "return {}")?;
    parsed.dialogue_imports = character_imports(source);
    Ok(parsed)
}
