//! Conservative source flow: runtime facts and host action outcomes are unknown.
use super::{mock::DialogueMockContext, Condition, ConditionKind, DialogueSource};
use crate::{Diagnostic, DiagnosticCode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn constant(condition: &Condition) -> Option<bool> {
    fn has_path(condition: &Condition) -> bool {
        match &condition.kind {
            ConditionKind::Path(_) => true,
            ConditionKind::Not(inner) => has_path(inner),
            ConditionKind::Binary(_, left, right) => has_path(left) || has_path(right),
            _ => false,
        }
    }
    // Reuse the runtime-compatible evaluator only for context-free expressions.
    // Invalid/nonportable expressions stay unknown, never proof of impossibility.
    if has_path(condition) {
        return None;
    }
    DialogueMockContext::parse("{}").ok()?.test(condition).ok()
}

impl DialogueSource {
    pub fn reachability_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let sections: BTreeMap<_, _> = self
            .sections
            .iter()
            .map(|section| (section.label.value.as_str(), section))
            .collect();
        let mut pending: Vec<_> = self
            .external_entries
            .iter()
            .map(|entry| entry.value.as_str())
            .collect();
        if let Some(first) = self.sections.first() {
            pending.push(first.label.value.as_str());
        }
        let mut visited = BTreeSet::new();
        while let Some(label) = pending.pop() {
            if !visited.insert(label) {
                continue;
            }
            let Some(section) = sections.get(label) else {
                continue;
            };
            if let Some(wait) = &section.wait {
                pending.push(&wait.target.value);
            }
            if let Some(action) = &section.action {
                pending.extend([action.success.value.as_str(), action.failure.value.as_str()]);
            }
            if let Some(branch) = &section.branch {
                match constant(&branch.condition) {
                    Some(true) => pending.push(&branch.yes.value),
                    Some(false) => pending.push(&branch.no.value),
                    None => pending.extend([branch.yes.value.as_str(), branch.no.value.as_str()]),
                }
            }
            for line in &section.lines {
                for choice in &line.choices {
                    if choice.visible.as_ref().and_then(constant) != Some(false)
                        && choice.enabled.as_ref().and_then(constant) != Some(false)
                    {
                        pending.push(&choice.target.value);
                    }
                }
                // Conservatively retain fallback edges: context can change between
                // display and selection, and the host can refresh the player.
                if let Some(fallback) = &line.fallback {
                    pending.push(&fallback.value);
                }
                if line.end || !line.choices.is_empty() {
                    break;
                }
            }
        }
        self.sections.iter().filter(|section| !visited.contains(section.label.value.as_str()))
            .map(|section| {
                let at = section.label.span;
                Diagnostic::new(path, at.start.line, at.start.column, crate::DiagnosticSeverity::Warning,
                    format!("section `{}` is unreachable from the first section and declared @entry labels; declare `@entry {}` if host code starts it", section.label.value, section.label.value))
                    .with_code(DiagnosticCode::DOMAIN_DIALOGUE)
                    .with_range(at.start.line, at.start.column, at.end.line, at.end.column)
            }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue_asset::parse_dialogue_source;

    #[test]
    fn external_entry_errors_preserve_unicode_ranges_and_header_ownership() {
        let path = Path::new("scene.tdialogue");
        let errors = parse_dialogue_source(
            path,
            "@dialogue 1\n@entry é\n[start]\n// @id hello\n: Hello (end)\n",
        )
        .unwrap_err();
        assert_eq!(
            (errors[0].line, errors[0].column, errors[0].end_column),
            (2, 8, 9)
        );
        let errors = parse_dialogue_source(
            path,
            "@dialogue 1\n[start]\n@entry start\n// @id hello\n: Hello (end)\n",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("before the first label")));
    }

    #[test]
    fn constants_external_roots_and_cycles_are_conservative() {
        let source = "@dialogue 1\n@context \"facts\"\n@entry external\n[start]\n@branch true [target:yes] [else:no]\n[yes]\n@wait 1 [target:start]\n[no]\n// @id no\n: No (end)\n[external]\n@wait 1 [target:child]\n[child]\n// @id child\n: Child (end)\n";
        let path = Path::new("scene.tdialogue");
        let document = parse_dialogue_source(path, source).unwrap();
        let warnings = document.reachability_diagnostics(path);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("`no`"));
        assert!(
            parse_dialogue_source(path, &source.replace("@entry external", "@entry absent"))
                .is_err()
        );
        assert!(parse_dialogue_source(
            path,
            &source.replace("@entry external", "@entry external\n@entry external")
        )
        .is_err());
        let dynamic = source.replace("@branch true", "@branch context.allowed");
        assert!(parse_dialogue_source(path, &dynamic)
            .unwrap()
            .reachability_diagnostics(path)
            .is_empty());
    }
}
