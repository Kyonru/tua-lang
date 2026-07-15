#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct TextMatchRank {
    pub category: u8,
    pub index: usize,
    pub len: usize,
}

pub(crate) fn text_matches(value: &str, prefix: &str) -> bool {
    text_match_rank(value, prefix).is_some()
}

pub(crate) fn text_match_rank(value: &str, prefix: &str) -> Option<TextMatchRank> {
    if prefix.is_empty() {
        return Some(TextMatchRank {
            category: 0,
            index: 0,
            len: value.len(),
        });
    }

    if value.starts_with(prefix) {
        return Some(TextMatchRank {
            category: 0,
            index: 0,
            len: value.len(),
        });
    }

    let value_lower = value.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();

    if value_lower.starts_with(&prefix_lower) {
        return Some(TextMatchRank {
            category: 1,
            index: 0,
            len: value.len(),
        });
    }

    if let Some(index) = value.find(prefix) {
        return Some(TextMatchRank {
            category: 2,
            index,
            len: value.len(),
        });
    }

    value_lower.find(&prefix_lower).map(|index| TextMatchRank {
        category: 3,
        index,
        len: value.len(),
    })
}
