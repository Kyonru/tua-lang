//! Static authoring facts, not an execution trace or reachability claim.
use super::{DialogueName, DialogueSource};
use std::sync::Arc;

/// Sections and their source locations/text remain in the shared parsed document.
/// All indices in routes refer to that exact document and are zero-based.
#[derive(Clone, Debug)]
pub struct DialoguePreview {
    pub document: Arc<DialogueSource>,
    pub routes: Vec<DialogueRoute>,
    pub characters: std::collections::BTreeMap<(usize, usize), DialoguePreviewCharacter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueRoute {
    pub section: usize,
    /// Includes the target token's navigation span. No inferred fallthrough edges.
    pub target: DialogueName,
    pub kind: DialogueRouteKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueRouteKind {
    Choice { line: usize, choice: usize },
    Fallback { line: usize },
    Wait,
    BranchTrue,
    BranchFalse,
    ActionSuccess,
    ActionFailure,
}

impl DialoguePreview {
    pub(crate) fn new(document: Arc<DialogueSource>) -> Self {
        let mut routes = Vec::new();
        for (section, node) in document.sections.iter().enumerate() {
            let mut push = |target: &DialogueName, kind| {
                routes.push(DialogueRoute {
                    section,
                    target: target.clone(),
                    kind,
                });
            };
            if let Some(wait) = &node.wait {
                push(&wait.target, DialogueRouteKind::Wait);
            }
            if let Some(branch) = &node.branch {
                push(&branch.yes, DialogueRouteKind::BranchTrue);
                push(&branch.no, DialogueRouteKind::BranchFalse);
            }
            if let Some(action) = &node.action {
                push(&action.success, DialogueRouteKind::ActionSuccess);
                push(&action.failure, DialogueRouteKind::ActionFailure);
            }
            for (line, utterance) in node.lines.iter().enumerate() {
                for (choice, option) in utterance.choices.iter().enumerate() {
                    push(&option.target, DialogueRouteKind::Choice { line, choice });
                }
                if let Some(fallback) = &utterance.fallback {
                    push(fallback, DialogueRouteKind::Fallback { line });
                }
            }
        }
        Self {
            document,
            routes,
            characters: Default::default(),
        }
    }
}

/// Independent condition probes; these do not select routes or execute a player.
#[derive(Clone, Debug)]
pub struct DialogueMockPreview {
    pub conditions: Vec<DialogueMockCondition>,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct DialogueMockCondition {
    pub section: usize,
    pub line: Option<usize>,
    pub choice: Option<usize>,
    pub kind: DialogueMockConditionKind,
    pub span: crate::SourceSpan,
    pub result: Result<bool, super::mock::DialogueMockIssue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueMockConditionKind {
    Branch,
    Visibility,
    Availability,
}

impl DialoguePreview {
    /// Probe at most 4,096 authored conditions against explicit facts. Conditions
    /// on hidden/unreachable choices are still tested; this is not a flow trace.
    pub fn evaluate_conditions(
        &self,
        facts: &super::mock::DialogueMockContext,
    ) -> DialogueMockPreview {
        let mut preview = DialogueMockPreview {
            conditions: Vec::new(),
            truncated: false,
        };
        let mut push = |section, line, choice, kind, condition: &super::Condition| {
            if preview.conditions.len() == 4096 {
                preview.truncated = true;
                return false;
            }
            preview.conditions.push(DialogueMockCondition {
                section,
                line,
                choice,
                kind,
                span: condition.span,
                result: facts.test(condition),
            });
            true
        };
        'sections: for (section, node) in self.document.sections.iter().enumerate() {
            if let Some(branch) = &node.branch {
                if !push(
                    section,
                    None,
                    None,
                    DialogueMockConditionKind::Branch,
                    &branch.condition,
                ) {
                    break;
                }
            }
            for (line, utterance) in node.lines.iter().enumerate() {
                for (choice, option) in utterance.choices.iter().enumerate() {
                    for (kind, condition) in [
                        (DialogueMockConditionKind::Visibility, &option.visible),
                        (DialogueMockConditionKind::Availability, &option.enabled),
                    ] {
                        if let Some(condition) = condition {
                            if !push(section, Some(line), Some(choice), kind, condition) {
                                break 'sections;
                            }
                        }
                    }
                }
            }
        }
        preview
    }
}

#[derive(Clone, Debug)]
pub struct DialogueMockTextPreview {
    pub texts: Vec<DialogueMockText>,
    pub binding_error: Option<super::mock::DialogueMockIssue>,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct DialogueMockText {
    pub rich: Option<Vec<DialoguePreviewRun>>,
    pub section: usize,
    pub line: usize,
    pub choice: Option<usize>,
    pub kind: DialogueMockTextKind,
    pub span: crate::SourceSpan,
    pub result: Result<String, super::mock::DialogueMockIssue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueMockTextKind {
    Line,
    Choice,
    Reason,
}

impl DialoguePreview {
    /// Format at most 4,096 fields and 1 MiB of combined output. Each field also
    /// retains the formatter's 64 KiB limit. No locales or rich styles resolve.
    pub fn format_texts(
        &self,
        facts: &super::mock::DialogueMockContext,
    ) -> DialogueMockTextPreview {
        let mut preview = DialogueMockTextPreview {
            texts: Vec::new(),
            binding_error: None,
            truncated: false,
        };
        let args = match super::mock_text::DialogueMockTextArguments::resolve(&self.document, facts)
        {
            Ok(args) => args,
            Err(error) => {
                preview.binding_error = Some(error);
                return preview;
            }
        };
        let mut bytes = 0;
        let mut rich_bytes = 0usize;
        let mut push = |section, line, choice, kind, text: &super::DialogueText| {
            if preview.texts.len() == 4096 {
                preview.truncated = true;
                return false;
            }
            let result = args.format(text);
            let length = result.as_ref().map_or(0, String::len);
            if length > (1024 * 1024usize).saturating_sub(bytes) {
                preview.truncated = true;
                return false;
            }
            bytes += length;
            let rich = if result.is_ok() && rich_bytes < 1024 * 1024 {
                rich_runs(text, Some(&args)).filter(|runs| {
                    let size = rich_run_bytes(runs);
                    if size > (1024 * 1024usize).saturating_sub(rich_bytes) {
                        return false;
                    }
                    rich_bytes += size;
                    true
                })
            } else {
                None
            };
            preview.texts.push(DialogueMockText {
                section,
                line,
                choice,
                kind,
                span: text.span,
                rich,
                result,
            });
            true
        };
        'sections: for (section, node) in self.document.sections.iter().enumerate() {
            for (line, utterance) in node.lines.iter().enumerate() {
                if !push(
                    section,
                    line,
                    None,
                    DialogueMockTextKind::Line,
                    &utterance.text,
                ) {
                    break 'sections;
                }
                for (choice, option) in utterance.choices.iter().enumerate() {
                    if !push(
                        section,
                        line,
                        Some(choice),
                        DialogueMockTextKind::Choice,
                        &option.text,
                    ) {
                        break 'sections;
                    }
                    if let Some(reason) = &option.reason {
                        if !push(
                            section,
                            line,
                            Some(choice),
                            DialogueMockTextKind::Reason,
                            reason,
                        ) {
                            break 'sections;
                        }
                    }
                }
            }
        }
        preview
    }
}

/// Presentation facts from the shared rich-text parser. These are not HTML or
/// runtime grapheme ranges; host renderers choose how semantic styles look.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DialoguePreviewRun {
    pub text: String,
    pub marks: Vec<String>,
    pub annotation: Option<String>,
}

pub fn rich_runs(
    text: &super::DialogueText,
    args: Option<&super::mock_text::DialogueMockTextArguments>,
) -> Option<Vec<DialoguePreviewRun>> {
    use super::rich_text::{Mark, TokenKind};
    let rich = text.rich.as_ref()?;
    let template = super::rich_text::parse(&rich.source, rich.kind).ok()?;
    let mut marks = Vec::new();
    let mut runs = Vec::new();
    let mut bytes = 0usize;
    for token in template.tokens {
        let (value, annotation) = match token.kind {
            TokenKind::Open(mark) => {
                marks.push(match mark {
                    Mark::Emphasis => "em".into(),
                    Mark::Style(name) => format!("style:{name}"),
                    Mark::Speed(speed) => format!("speed:{speed}"),
                    Mark::Pronunciation(hint) => format!("pronounce:{hint}"),
                });
                continue;
            }
            TokenKind::Close(_) => {
                marks.pop();
                continue;
            }
            TokenKind::Pause(seconds) => (String::new(), Some(format!("Pause {seconds}s"))),
            TokenKind::Text(value) => (value, None),
            TokenKind::Placeholder(name) => {
                let value = format!("%{{{name}}}");
                let value = if let Some(args) = args {
                    args.value(&name)?.to_owned()
                } else {
                    value
                };
                (value, None)
            }
        };
        bytes = bytes
            .saturating_add(value.len())
            .saturating_add(marks.iter().map(String::len).sum::<usize>())
            .saturating_add(annotation.as_ref().map_or(0, String::len));
        if bytes > 65_536 {
            return None;
        }
        runs.push(DialoguePreviewRun {
            text: value,
            marks: marks.clone(),
            annotation,
        });
    }
    Some(runs)
}

/// Authored literal character fields, not values obtained by executing a module.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DialoguePreviewCharacter {
    pub name: Option<String>,
    pub portrait: Option<String>,
    pub expression: Option<String>,
}

pub fn rich_run_bytes(runs: &[DialoguePreviewRun]) -> usize {
    runs.iter()
        .map(|run| {
            run.text.len()
                + run.marks.iter().map(String::len).sum::<usize>()
                + run.annotation.as_ref().map_or(0, String::len)
        })
        .sum()
}
