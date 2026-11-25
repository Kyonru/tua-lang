//! Static authoring facts, not an execution trace or reachability claim.
use super::{DialogueName, DialogueSource};
use std::sync::Arc;

/// Sections and their source locations/text remain in the shared parsed document.
/// All indices in routes refer to that exact document and are zero-based.
#[derive(Clone, Debug)]
pub struct DialoguePreview {
    pub document: Arc<DialogueSource>,
    pub routes: Vec<DialogueRoute>,
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
        Self { document, routes }
    }
}
