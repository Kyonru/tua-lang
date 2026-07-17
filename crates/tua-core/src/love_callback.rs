use std::collections::BTreeSet;

use full_moon::ast;
use full_moon::tokenizer::TokenReference;

use crate::lua::{self, LuaItem};
use crate::ParsedSource;

const CORE_CALLBACKS: &[&str] = &["load", "update", "draw"];

pub(crate) fn item(label: &str) -> Option<LuaItem> {
    lua::love_callback_item(label)
}

pub(crate) fn items() -> Vec<LuaItem> {
    lua::love_callback_items()
}

pub(crate) fn declaration_name(
    function: &ast::FunctionDeclaration,
) -> Option<(String, &TokenReference)> {
    if function.name().method_name().is_some() {
        return None;
    }

    let names = function.name().names().iter().collect::<Vec<_>>();
    let [root, callback] = names.as_slice() else {
        return None;
    };

    (token_text(root) == "love").then(|| (token_text(callback), *callback))
}

pub(crate) fn closest_name(name: &str) -> Option<&'static str> {
    let name_len = name.chars().count();
    let (candidate, distance) = items()
        .into_iter()
        .map(|item| (item.label, edit_distance(name, item.label)))
        .min_by_key(|(candidate, distance)| (*distance, *candidate))?;
    let longest = name_len.max(candidate.chars().count());

    (distance <= 2 && distance.saturating_mul(3) <= longest).then_some(candidate)
}

pub(crate) fn declaration_suffix(label: &str, body: &str) -> Option<String> {
    let parameters = parameter_names(item(label)?)?.join(", ");
    let body = if body.is_empty() {
        String::new()
    } else {
        format!("  {body}\n")
    };

    Some(format!("{label}({parameters})\n{body}end"))
}

pub(crate) fn missing_core_declarations(parsed: &ParsedSource) -> Vec<String> {
    let declared = parsed
        .function_annotations
        .iter()
        .filter_map(|function| function.name.as_deref())
        .filter_map(|name| name.strip_prefix("love."))
        .filter(|label| item(label).is_some())
        .collect::<BTreeSet<_>>();

    CORE_CALLBACKS
        .iter()
        .filter(|label| !declared.contains(**label))
        .filter_map(|label| {
            declaration_suffix(label, "").map(|declaration| format!("function love.{declaration}"))
        })
        .collect()
}

fn parameter_names(item: LuaItem) -> Option<Vec<&'static str>> {
    let signature = lua::signature_variants(item)?.first()?;

    Some(
        signature
            .parameters
            .iter()
            .filter_map(|parameter| {
                let name = parameter
                    .label
                    .split_once(':')
                    .map(|(name, _)| name)
                    .unwrap_or(parameter.label)
                    .trim()
                    .trim_end_matches('?');

                (!name.is_empty() && name != "...").then_some(name)
            })
            .collect(),
    )
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();

    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        current.push(left_index + 1);

        for (right_index, right_char) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }

        previous = current;
    }

    previous[right.len()]
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    #[test]
    fn finds_close_callback_names_without_flagging_unrelated_extensions() {
        assert_eq!(closest_name("udpate"), Some("update"));
        assert_eq!(closest_name("keypresed"), Some("keypressed"));
        assert_eq!(closest_name("custom"), None);
    }

    #[test]
    fn renders_only_missing_core_callback_declarations() {
        let parsed = parse_tua_source("src/main.tua", "function love.update(dt: number)\nend\n")
            .expect("source should parse");

        assert_eq!(
            missing_core_declarations(&parsed),
            vec![
                "function love.load(arg, unfilteredArg)\nend",
                "function love.draw()\nend",
            ]
        );
    }
}
