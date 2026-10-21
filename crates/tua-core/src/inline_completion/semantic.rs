use super::eligibility::{word_before, CursorContext};

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
}
