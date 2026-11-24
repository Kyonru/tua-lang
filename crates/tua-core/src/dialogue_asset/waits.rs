//! Duration waits are explicit flow boundaries, not presentation pause marks.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueWait {
    pub duration: String,
    pub target: DialogueName,
    pub span: SourceSpan,
}

impl Parser<'_> {
    pub(super) fn wait_line(&mut self, raw: &str, at: SourceSpan) -> bool {
        let Some(rest) = raw.trim().strip_prefix("@wait ") else {
            return false;
        };
        self.abandon_id();
        let (duration, tag) = take_word(rest);
        let Some(duration) = seconds(duration).filter(|v| v.parse::<f64>().unwrap() <= 86400.0)
        else {
            self.error(
                at,
                "wait duration must be finite and between 0 and 86400 seconds",
            );
            return true;
        };
        let Some(target) = tag
            .strip_prefix("[target:")
            .and_then(|v| v.strip_suffix(']'))
        else {
            self.error(at, "expected @wait seconds [target:label]");
            return true;
        };
        let column = raw.trim_end().chars().count() - tag.chars().count() + 9;
        let Some(target) = self.name(target, span(at.start.line, column, target.chars().count()))
        else {
            return true;
        };
        let Some(section) = self.document.sections.last_mut() else {
            self.error(at, "wait requires a label");
            return true;
        };
        if !section.lines.is_empty()
            || section.branch.is_some()
            || section.action.is_some()
            || section.wait.is_some()
        {
            self.error(
                at,
                "a wait label contains exactly one wait and no other flow",
            );
            return true;
        }
        section.wait = Some(DialogueWait {
            duration,
            target,
            span: at,
        });
        true
    }
}
