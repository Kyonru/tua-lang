use std::collections::BTreeMap;

use super::context::is_identifier;
use super::{Completion, CompletionKind};
use crate::source_text::{byte_for_column, is_identifier_continue, is_identifier_start};
use crate::SourcePosition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LuaSymbol {
    pub(super) name: String,
    kind: CompletionKind,
    detail: String,
    line: usize,
}

impl LuaSymbol {
    pub(super) fn into_completion(self) -> Completion {
        Completion {
            label: self.name,
            kind: self.kind,
            detail: Some(self.detail),
            documentation: None,
            insert_text: None,
            filter_text: None,
            is_snippet: false,
            module_path: None,
            source_module: None,
            additional_text_edits: Vec::new(),
        }
    }
}

pub(super) fn lua_symbols_before_position(
    source: &str,
    position: SourcePosition,
) -> Vec<LuaSymbol> {
    let mut symbols = BTreeMap::new();

    for (line_index, line) in source.split('\n').enumerate() {
        let line_number = line_index + 1;

        if line_number > position.line {
            break;
        }

        let visible = if line_number == position.line {
            &line[..byte_for_column(line, position.column).min(line.len())]
        } else {
            line
        };
        let code = visible.split("--").next().unwrap_or("").trim_start();

        collect_function_header_parameters(code, line_number, &mut symbols);

        if let Some(name) = parse_local_function_name(code) {
            symbols.insert(
                name.clone(),
                LuaSymbol {
                    name,
                    kind: CompletionKind::Function,
                    detail: "local function".to_owned(),
                    line: line_number,
                },
            );
            continue;
        }

        if let Some(name) = parse_function_name(code) {
            symbols.insert(
                name.clone(),
                LuaSymbol {
                    name,
                    kind: CompletionKind::Function,
                    detail: "function".to_owned(),
                    line: line_number,
                },
            );
            continue;
        }

        for name in parse_local_names(code) {
            symbols.insert(
                name.clone(),
                LuaSymbol {
                    name,
                    kind: CompletionKind::Variable,
                    detail: "local".to_owned(),
                    line: line_number,
                },
            );
        }
    }

    let mut symbols = symbols.into_values().collect::<Vec<_>>();

    symbols.sort_by(|left, right| {
        right
            .line
            .cmp(&left.line)
            .then_with(|| left.name.cmp(&right.name))
    });
    symbols
}

fn parse_local_function_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("local function ")?;
    let name = take_identifier(rest)?;

    Some(name.to_owned())
}

fn parse_function_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("function ")?;
    let name = take_dotted_name(rest)?;
    let simple_name = name.rsplit(['.', ':']).next()?;

    is_identifier(simple_name).then(|| simple_name.to_owned())
}

fn parse_local_names(line: &str) -> Vec<String> {
    let Some(mut rest) = line.strip_prefix("local ") else {
        return Vec::new();
    };

    if rest.starts_with("function ") {
        return Vec::new();
    }

    let mut names = Vec::new();
    loop {
        rest = rest.trim_start();

        let Some(name) = take_identifier(rest) else {
            break;
        };
        names.push(name.to_owned());

        rest = &rest[name.len()..];
        rest = rest.trim_start();

        if rest.starts_with(':') {
            let next_separator = rest
                .char_indices()
                .find_map(|(index, character)| {
                    (character == ',' || character == '=').then_some(index)
                })
                .unwrap_or(rest.len());
            rest = &rest[next_separator..];
        }

        rest = rest.trim_start();

        if rest.starts_with(',') {
            rest = &rest[1..];
            continue;
        }

        break;
    }

    names
}

fn collect_function_header_parameters(
    line: &str,
    line_number: usize,
    symbols: &mut BTreeMap<String, LuaSymbol>,
) {
    let Some(open) = line.find('(') else {
        return;
    };
    let Some(close) = line[open + 1..].find(')').map(|offset| open + 1 + offset) else {
        return;
    };

    if !line[..open].contains("function") {
        return;
    }

    for parameter in line[open + 1..close].split(',') {
        let parameter = parameter.trim();

        if parameter == "..." {
            continue;
        }

        let name = parameter
            .split(':')
            .next()
            .map(str::trim)
            .unwrap_or(parameter);

        if !is_identifier(name) {
            continue;
        }

        symbols.insert(
            name.to_owned(),
            LuaSymbol {
                name: name.to_owned(),
                kind: CompletionKind::Variable,
                detail: "parameter".to_owned(),
                line: line_number,
            },
        );
    }
}

fn take_identifier(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();

    if !bytes.first().is_some_and(|byte| is_identifier_start(*byte)) {
        return None;
    }

    let end = bytes
        .iter()
        .position(|byte| !is_identifier_continue(*byte))
        .unwrap_or(value.len());

    Some(&value[..end])
}

fn take_dotted_name(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .find_map(|(index, character)| {
            (!(character == '_'
                || character == '.'
                || character == ':'
                || character.is_ascii_alphanumeric()))
            .then_some(index)
        })
        .unwrap_or(value.len());

    (end > 0).then(|| &value[..end])
}
