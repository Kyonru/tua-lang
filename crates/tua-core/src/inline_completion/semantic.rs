use super::eligibility::{word_before, CursorContext};
#[cfg(test)]
use std::collections::BTreeSet;

pub(crate) fn statement_snippet(
    source: &str,
    offset: usize,
    context: CursorContext,
    include_ecs: bool,
    min_prefix_chars: usize,
) -> Option<String> {
    if context != CursorContext::StatementPrefix {
        return None;
    }
    let typed = word_before(source, offset);
    if typed.chars().count() < min_prefix_chars {
        return None;
    }
    let matches = crate::completion::syntax_snippet_completions(typed, include_ecs)
        .into_iter()
        .filter_map(|completion| {
            let text = completion.insert_text?;
            text.strip_prefix(typed)
                .filter(|rest| !rest.is_empty())
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let [text] = matches.as_slice() else {
        return None;
    };
    Some(text.clone())
}

#[cfg(test)]
pub(crate) fn love_callback(
    source: &str,
    offset: usize,
    declared: &BTreeSet<String>,
) -> Option<String> {
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = source[line_start..offset].trim_start();
    let typed = prefix.strip_prefix("function love.")?;
    if !typed.chars().all(super::eligibility::is_identifier_char) {
        return None;
    }
    let matches = crate::love_callback::items()
        .into_iter()
        .map(|item| item.label)
        .filter(|label| label.starts_with(typed) && !declared.contains(*label))
        .collect::<Vec<_>>();
    let [label] = matches.as_slice() else {
        return None;
    };
    let declaration = crate::love_callback::declaration_suffix(label, "")?;
    Some(declaration[typed.len()..].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_three_typed_characters_and_a_unique_match() {
        assert_eq!(
            statement_snippet("whi", 3, CursorContext::StatementPrefix, false, 3).as_deref(),
            Some("le ${1:condition} do\n  ${0}\nend")
        );
        assert_eq!(
            statement_snippet("lo", 2, CursorContext::StatementPrefix, false, 3),
            None
        );
        assert!(statement_snippet("loc", 3, CursorContext::StatementPrefix, false, 3).is_none());
    }

    #[test]
    fn love_callback_requires_a_unique_undeclared_match() {
        assert_eq!(
            love_callback("function love.upd", 17, &BTreeSet::new()).as_deref(),
            Some("ate(dt)\nend")
        );
        assert!(love_callback(
            "function love.upd",
            17,
            &BTreeSet::from(["update".to_owned()])
        )
        .is_none());
    }
}
