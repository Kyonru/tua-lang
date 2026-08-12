use std::collections::BTreeMap;

use full_moon::ast::luau::TypeInfo;
use full_moon::ast::span::ContainedSpan;
use full_moon::node::Node;
use full_moon::tokenizer::TokenReference;
use full_moon::visitors::VisitorMut;

use crate::emit::lowering;
use crate::parser::{NativeArrayTypeSyntax, ParserLineProvenance, SourcePosition, SourceSpan};
use crate::runtime_type::IS_CHECK_HELPER;

pub(super) fn normalize_tua_source_for_luau_parser(
    source: &str,
) -> (
    String,
    Vec<ParserLineProvenance>,
    Vec<NativeArrayTypeSyntax>,
) {
    let (class_source, provenance) = lowering::lower_classes_for_parser(source);
    let parser_source = normalize_parser_syntax(&class_source);
    let (parser_source, native_array_types) = normalize_native_array_types(&parser_source);
    debug_assert_eq!(
        class_source.split_inclusive('\n').count(),
        parser_source.split_inclusive('\n').count(),
        "parser syntax normalization must preserve class-lowered line count"
    );

    (parser_source, provenance, native_array_types)
}

/// Return whether source contains syntax that requires Tua's Luau-backed
/// parser. This deliberately prefers false positives: ordinary code can always
/// use the Tua parser, while plain generated data should reach Lua 5.1 without
/// first attempting and failing another grammar.
pub(super) fn requires_tua_grammar(source: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let protected = protected_syntax_characters(&chars);

    for index in 0..chars.len() {
        if protected[index] {
            continue;
        }

        let character = chars[index];
        if matches!(character, ':' | '?' | '`' | '@' | '&' | '|') {
            return true;
        }

        if matches!(character, '+' | '-' | '*' | '/' | '%' | '^')
            && chars.get(index + 1) == Some(&'=')
        {
            return true;
        }

        if (character == '/' && chars.get(index + 1) == Some(&'/'))
            || (character == '[' && chars.get(index + 1) == Some(&']'))
            || (character == '<' && chars.get(index + 1) == Some(&'<'))
            || (character == '>' && chars.get(index + 1) == Some(&'>'))
            || (character == '.'
                && chars.get(index + 1) == Some(&'.')
                && chars.get(index + 2) == Some(&'='))
            || (character == '-' && chars.get(index + 1) == Some(&'>'))
            || (character == '0' && matches!(chars.get(index + 1), Some('b' | 'B')))
        {
            return true;
        }

        // Functions can carry typed parameters, return annotations, and Tua
        // default arguments. Keeping all function-bearing modules on the Tua
        // grammar makes this classifier simple and correctness-first; large
        // generated data modules normally contain no executable declarations.
        if [
            "class", "const", "continue", "enum", "export", "final", "function", "if", "is",
            "open", "type",
        ]
        .iter()
        .any(|keyword| starts_keyword(&chars, index, keyword))
        {
            return true;
        }
    }

    false
}

pub(super) fn is_generated_lua_module(source: &str) -> bool {
    source
        .lines()
        .take(8)
        .map(str::trim)
        .any(|line| line == "--!tua generated")
}

pub(super) fn contains_unprotected_backtick(source: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let protected = protected_syntax_characters(&chars);

    chars
        .iter()
        .enumerate()
        .any(|(index, character)| *character == '`' && !protected[index])
}

pub(super) fn apply_native_array_types(
    ast: full_moon::ast::Ast,
    syntax: &[NativeArrayTypeSyntax],
) -> full_moon::ast::Ast {
    // The parser sees spaces where Tua wrote `[]`, so the item tokens retain
    // their original positions. Restore the semantic array nodes only after
    // dialect validation has distinguished native syntax from Luau `{ T }`.
    let mut lowerer = NativeArrayTypeLowerer {
        depths_by_item_end: syntax
            .iter()
            .map(|syntax| (position_key(syntax.item_end), syntax.depth))
            .collect(),
    };

    lowerer.visit_ast(ast)
}

struct NativeArrayTypeLowerer {
    depths_by_item_end: BTreeMap<(usize, usize), usize>,
}

impl VisitorMut for NativeArrayTypeLowerer {
    fn visit_type_info_end(&mut self, type_info: TypeInfo) -> TypeInfo {
        let Some(item_end) = native_array_item_end(&type_info) else {
            return type_info;
        };
        let Some(depth) = self.depths_by_item_end.remove(&position_key(item_end)) else {
            return type_info;
        };
        let type_text = type_info.to_string();
        let normalized_trailing =
            &type_text[type_text.trim_end_matches(char::is_whitespace).len()..];
        let erased_suffix = " ".repeat(depth * 2);
        let source_trailing = normalized_trailing
            .strip_prefix(&erased_suffix)
            .unwrap_or(normalized_trailing)
            .to_owned();

        (0..depth).fold(type_info, |item, _| TypeInfo::Array {
            braces: ContainedSpan::new(
                TokenReference::symbol("{").expect("array wrapper uses a valid opening brace"),
                TokenReference::symbol(&format!("}}{source_trailing}"))
                    .expect("array wrapper uses a valid closing brace"),
            ),
            access: None,
            type_info: Box::new(item),
        })
    }
}

pub(super) fn native_array_item_end(type_info: &TypeInfo) -> Option<SourcePosition> {
    let end = match type_info {
        TypeInfo::Table { braces, .. } => braces.tokens().1.end_position()?,
        TypeInfo::Tuple { parentheses, .. } => parentheses.tokens().1.end_position()?,
        _ => type_info.range()?.1,
    };

    Some(SourcePosition::from_full_moon(end))
}

fn normalize_native_array_types(source: &str) -> (String, Vec<NativeArrayTypeSyntax>) {
    // Full Moon has no postfix array grammar. Blanking the suffix keeps every
    // preceding token source-accurate; recorded suffixes are matched to parsed
    // type endings before the AST is wrapped by `apply_native_array_types`.
    let mut chars = source.chars().collect::<Vec<_>>();
    let protected = protected_syntax_characters(&chars);
    let mut syntax = Vec::new();
    let mut index = 0usize;

    while index + 1 < chars.len() {
        if protected[index]
            || chars[index] != '['
            || chars[index + 1] != ']'
            || protected[index + 1]
        {
            index += 1;
            continue;
        }

        let suffix_start = index;
        let mut depth = 0usize;
        while index + 1 < chars.len()
            && chars[index] == '['
            && chars[index + 1] == ']'
            && !protected[index]
            && !protected[index + 1]
        {
            chars[index] = ' ';
            chars[index + 1] = ' ';
            depth += 1;
            index += 2;
        }

        syntax.push(NativeArrayTypeSyntax {
            item_end: source_position_for_char_offset(&chars, suffix_start),
            suffix_span: SourceSpan {
                start: source_position_for_char_offset(&chars, suffix_start),
                end: source_position_for_char_offset(&chars, index),
            },
            depth,
        });
    }

    (chars.into_iter().collect(), syntax)
}

fn protected_syntax_characters(chars: &[char]) -> Vec<bool> {
    let mut protected = vec![false; chars.len()];
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '-' && chars.get(index + 1) == Some(&'-') {
            let end = long_bracket_end(chars, index + 2).unwrap_or_else(|| {
                chars[index..]
                    .iter()
                    .position(|character| *character == '\n')
                    .map_or(chars.len(), |offset| index + offset)
            });
            protected[index..end].fill(true);
            index = end;
            continue;
        }

        if let Some(end) = long_bracket_end(chars, index) {
            protected[index..end].fill(true);
            index = end;
            continue;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            let quote = chars[index];
            let start = index;
            index += 1;

            while index < chars.len() {
                if chars[index] == '\\' {
                    index = (index + 2).min(chars.len());
                    continue;
                }

                let character = chars[index];
                index += 1;
                if character == quote {
                    break;
                }
            }

            protected[start..index].fill(true);
            continue;
        }

        index += 1;
    }

    protected
}

fn long_bracket_end(chars: &[char], index: usize) -> Option<usize> {
    if chars.get(index) != Some(&'[') {
        return None;
    }

    let mut open_end = index + 1;
    while chars.get(open_end) == Some(&'=') {
        open_end += 1;
    }
    if chars.get(open_end) != Some(&'[') {
        return None;
    }

    let equals = open_end - index - 1;
    let mut cursor = open_end + 1;

    while cursor < chars.len() {
        if chars[cursor] != ']' {
            cursor += 1;
            continue;
        }

        let mut close_end = cursor + 1;
        for _ in 0..equals {
            if chars.get(close_end) != Some(&'=') {
                break;
            }
            close_end += 1;
        }

        if close_end == cursor + equals + 1 && chars.get(close_end) == Some(&']') {
            return Some(close_end + 1);
        }

        cursor += 1;
    }

    Some(chars.len())
}

fn source_position_for_char_offset(chars: &[char], offset: usize) -> SourcePosition {
    let mut position = SourcePosition { line: 1, column: 1 };

    for character in chars.iter().take(offset) {
        if *character == '\n' {
            position.line += 1;
            position.column = 1;
        } else {
            position.column += 1;
        }
    }

    position
}

fn position_key(position: SourcePosition) -> (usize, usize) {
    (position.line, position.column)
}

fn normalize_parser_syntax(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;

    while index < chars.len() {
        if starts_line_comment(&chars, index) {
            index = push_until_line_end(&chars, index, &mut normalized);
            continue;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(&chars, index, &mut normalized);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(&chars, index, &mut normalized);
            continue;
        }

        if starts_enum_declaration(&chars, index) {
            index = push_erased_enum_block(&chars, index, &mut normalized);
            continue;
        }

        if starts_keyword(&chars, index, "if") {
            index =
                push_condition_header_with_is_checks(&chars, index, "if", "then", &mut normalized);
            continue;
        }

        if starts_keyword(&chars, index, "elseif") {
            index = push_condition_header_with_is_checks(
                &chars,
                index,
                "elseif",
                "then",
                &mut normalized,
            );
            continue;
        }

        if starts_keyword(&chars, index, "while") {
            index =
                push_condition_header_with_is_checks(&chars, index, "while", "do", &mut normalized);
            continue;
        }

        if starts_function_keyword(&chars, index) {
            index =
                push_function_header_with_erased_parameter_defaults(&chars, index, &mut normalized);
            continue;
        }

        if is_optional_field_marker(&chars, index) {
            normalized.push(':');
            index += 2;
            index = push_type_with_trailing_optional(&chars, index, &mut normalized);
            continue;
        }

        normalized.push(chars[index]);
        index += 1;
    }

    normalized
}

fn is_optional_field_marker(chars: &[char], index: usize) -> bool {
    chars[index] == '?'
        && chars.get(index + 1) == Some(&':')
        && index > 0
        && is_identifier_char(chars[index - 1])
}

fn is_identifier_char(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn starts_function_keyword(chars: &[char], index: usize) -> bool {
    if !starts_keyword(chars, index, "function") {
        return false;
    }

    !index
        .checked_sub(1)
        .and_then(|previous| chars.get(previous))
        .is_some_and(|character| {
            is_identifier_char(*character) || *character == '.' || *character == ':'
        })
}

fn starts_enum_declaration(chars: &[char], index: usize) -> bool {
    if !starts_keyword(chars, index, "enum") {
        return false;
    }

    let line_start = chars[..index]
        .iter()
        .rposition(|character| *character == '\n')
        .map(|position| position + 1)
        .unwrap_or(0);

    chars[line_start..index]
        .iter()
        .all(|character| character.is_whitespace())
}

fn starts_keyword(chars: &[char], index: usize, keyword: &str) -> bool {
    if index
        .checked_sub(1)
        .and_then(|previous| chars.get(previous))
        .is_some_and(|character| is_identifier_char(*character))
    {
        return false;
    }

    for (offset, expected) in keyword.chars().enumerate() {
        if chars.get(index + offset) != Some(&expected) {
            return false;
        }
    }

    !chars
        .get(index + keyword.chars().count())
        .is_some_and(|character| is_identifier_char(*character))
}

fn push_condition_header_with_is_checks(
    chars: &[char],
    mut index: usize,
    keyword: &str,
    terminator: &str,
    output: &mut String,
) -> usize {
    for character in keyword.chars() {
        output.push(character);
        index += 1;
    }

    let mut depth = 0usize;

    while index < chars.len() {
        if starts_line_comment(chars, index) {
            return push_until_line_end(chars, index, output);
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(chars, index, output);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(chars, index, output);
            continue;
        }

        if depth == 0 && starts_keyword(chars, index, terminator) {
            for character in terminator.chars() {
                output.push(character);
                index += 1;
            }

            return index;
        }

        if let Some((replacement, end)) = runtime_is_check_replacement(chars, index, terminator) {
            output.push_str(&replacement);
            index = end;
            continue;
        }

        let character = chars[index];

        match character {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }

        output.push(character);
        index += 1;
    }

    index
}

fn runtime_is_check_replacement(
    chars: &[char],
    index: usize,
    terminator: &str,
) -> Option<(String, usize)> {
    if !can_start_runtime_is_lhs(chars, index) {
        return None;
    }

    let lhs_end = consume_identifier_chars(chars, index)?;
    let is_start = skip_inline_whitespace(chars, lhs_end);

    if is_start == lhs_end || !starts_keyword(chars, is_start, "is") {
        return None;
    }

    let target_start = skip_inline_whitespace(chars, is_start + "is".len());

    if target_start == is_start + "is".len() {
        return None;
    }

    let target_end = consume_runtime_is_target(chars, target_start, terminator)?;
    let target_text = chars[target_start..target_end]
        .iter()
        .collect::<String>()
        .trim()
        .to_owned();

    if target_text.is_empty() {
        return None;
    }

    let lhs = chars[index..lhs_end].iter().collect::<String>();
    let replacement = format!(
        "{IS_CHECK_HELPER}({lhs}, \"{}\")",
        lua_short_string_inner(&target_text)
    );

    Some((replacement, target_end))
}

fn can_start_runtime_is_lhs(chars: &[char], index: usize) -> bool {
    if !chars
        .get(index)
        .is_some_and(|character| is_identifier_start(*character))
    {
        return false;
    }

    !index
        .checked_sub(1)
        .and_then(|previous| chars.get(previous))
        .is_some_and(|character| {
            is_identifier_char(*character) || *character == '.' || *character == ':'
        })
}

fn consume_identifier_chars(chars: &[char], mut index: usize) -> Option<usize> {
    if !chars
        .get(index)
        .is_some_and(|character| is_identifier_start(*character))
    {
        return None;
    }

    index += 1;

    while chars
        .get(index)
        .is_some_and(|character| is_identifier_char(*character))
    {
        index += 1;
    }

    Some(index)
}

fn skip_inline_whitespace(chars: &[char], mut index: usize) -> usize {
    while chars
        .get(index)
        .is_some_and(|character| *character == ' ' || *character == '\t')
    {
        index += 1;
    }

    index
}

fn consume_runtime_is_target(chars: &[char], mut index: usize, terminator: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut end = index;

    while index < chars.len() {
        if depth == 0
            && (starts_keyword(chars, index, terminator)
                || starts_keyword(chars, index, "and")
                || starts_keyword(chars, index, "or"))
        {
            break;
        }

        let character = chars[index];

        if depth == 0 && (character == ')' || character == ',') {
            break;
        }

        if character == '\n' {
            break;
        }

        match character {
            '(' | '{' | '[' | '<' => depth += 1,
            ')' | '}' | ']' | '>' => depth = depth.saturating_sub(1),
            _ => {}
        }

        if !character.is_whitespace() {
            end = index + 1;
        }

        index += 1;
    }

    (end > 0).then_some(end)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn lua_short_string_inner(input: &str) -> String {
    let mut output = String::new();

    for character in input.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            _ => output.push(character),
        }
    }

    output
}

fn push_function_header_with_erased_parameter_defaults(
    chars: &[char],
    mut index: usize,
    output: &mut String,
) -> usize {
    for character in "function".chars() {
        output.push(character);
        index += 1;
    }

    let mut lookahead = index;
    while chars
        .get(lookahead)
        .is_some_and(|character| character.is_whitespace())
    {
        lookahead += 1;
    }

    if chars.get(lookahead) == Some(&'(') {
        return index;
    }

    while index < chars.len() {
        if starts_line_comment(chars, index) {
            return push_until_line_end(chars, index, output);
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(chars, index, output);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(chars, index, output);
            continue;
        }

        let character = chars[index];
        output.push(character);
        index += 1;

        if character == '(' {
            return push_parameter_list_with_erased_defaults(chars, index, output);
        }

        if character == '\n' {
            return index;
        }
    }

    index
}

fn push_parameter_list_with_erased_defaults(
    chars: &[char],
    mut index: usize,
    output: &mut String,
) -> usize {
    let mut depth = 0usize;
    let mut erasing_default = false;
    let mut string_quote = None;
    let mut escaped = false;
    let mut long_string = false;

    while index < chars.len() {
        let character = chars[index];

        if let Some(quote) = string_quote {
            output.push(if erasing_default { ' ' } else { character });
            index += 1;

            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                string_quote = None;
            }

            continue;
        }

        if long_string {
            output.push(if erasing_default { ' ' } else { character });

            if character == ']' && chars.get(index + 1) == Some(&']') {
                index += 1;
                output.push(if erasing_default { ' ' } else { chars[index] });
                long_string = false;
            }

            index += 1;
            continue;
        }

        if character == '"' || character == '\'' {
            string_quote = Some(character);
            output.push(if erasing_default { ' ' } else { character });
            index += 1;
            continue;
        }

        if character == '[' && chars.get(index + 1) == Some(&'[') {
            long_string = true;
            output.push(if erasing_default { ' ' } else { character });
            index += 1;
            output.push(if erasing_default { ' ' } else { chars[index] });
            index += 1;
            continue;
        }

        if !erasing_default && is_optional_field_marker(chars, index) {
            output.push(':');
            index += 2;
            index = push_parameter_type_with_trailing_optional(chars, index, output);
            continue;
        }

        match character {
            '(' | '{' | '[' => {
                depth += 1;
                output.push(if erasing_default { ' ' } else { character });
                index += 1;
            }
            ')' if depth == 0 => {
                output.push(')');
                return index + 1;
            }
            ')' | '}' | ']' => {
                depth = depth.saturating_sub(1);
                output.push(if erasing_default { ' ' } else { character });
                index += 1;
            }
            ',' if depth == 0 => {
                erasing_default = false;
                output.push(',');
                index += 1;
            }
            '=' if depth == 0 => {
                erasing_default = true;
                output.push(' ');
                index += 1;
            }
            _ => {
                output.push(if erasing_default { ' ' } else { character });
                index += 1;
            }
        }
    }

    index
}

fn push_parameter_type_with_trailing_optional(
    chars: &[char],
    mut index: usize,
    output: &mut String,
) -> usize {
    let mut depth = 0usize;

    while index < chars.len() {
        if starts_line_comment(chars, index) {
            insert_optional_before_trailing_whitespace(output);
            return index;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(chars, index, output);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(chars, index, output);
            continue;
        }

        let character = chars[index];

        match character {
            '{' | '(' | '[' => {
                depth += 1;
                output.push(character);
                index += 1;
            }
            ',' | ')' | '=' if depth == 0 => {
                insert_optional_before_trailing_whitespace(output);
                return index;
            }
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                output.push(character);
                index += 1;
            }
            _ => {
                output.push(character);
                index += 1;
            }
        }
    }

    insert_optional_before_trailing_whitespace(output);
    index
}

fn push_erased_enum_block(chars: &[char], mut index: usize, output: &mut String) -> usize {
    while index < chars.len() {
        let line_start = index;

        while index < chars.len() && chars[index] != '\n' {
            index += 1;
        }

        let line = chars[line_start..index].iter().collect::<String>();
        let closes_enum = line.trim() == "end";

        for _ in line_start..index {
            output.push(' ');
        }

        if index < chars.len() && chars[index] == '\n' {
            output.push('\n');
            index += 1;
        }

        if closes_enum {
            break;
        }
    }

    index
}

fn starts_line_comment(chars: &[char], index: usize) -> bool {
    chars[index] == '-' && chars.get(index + 1) == Some(&'-')
}

fn push_until_line_end(chars: &[char], mut index: usize, output: &mut String) -> usize {
    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == '\n' {
            break;
        }
    }

    index
}

fn push_short_string(chars: &[char], mut index: usize, output: &mut String) -> usize {
    let quote = chars[index];
    output.push(quote);
    index += 1;

    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == '\\' && index < chars.len() {
            output.push(chars[index]);
            index += 1;
            continue;
        }

        if character == quote {
            break;
        }
    }

    index
}

fn push_long_bracket(chars: &[char], mut index: usize, output: &mut String) -> usize {
    output.push(chars[index]);
    output.push(chars[index + 1]);
    index += 2;

    while index < chars.len() {
        let character = chars[index];
        output.push(character);
        index += 1;

        if character == ']' && chars.get(index) == Some(&']') {
            output.push(chars[index]);
            index += 1;
            break;
        }
    }

    index
}

fn push_type_with_trailing_optional(
    chars: &[char],
    mut index: usize,
    output: &mut String,
) -> usize {
    let mut depth = 0usize;

    while index < chars.len() {
        if starts_line_comment(chars, index) {
            index = push_until_line_end(chars, index, output);
            continue;
        }

        if chars[index] == '"' || chars[index] == '\'' {
            index = push_short_string(chars, index, output);
            continue;
        }

        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            index = push_long_bracket(chars, index, output);
            continue;
        }

        let character = chars[index];

        match character {
            '{' | '(' | '[' => {
                depth += 1;
                output.push(character);
                index += 1;
            }
            '}' if depth == 0 => {
                insert_optional_before_trailing_whitespace(output);
                output.push(character);
                return index + 1;
            }
            ',' if depth == 0 => {
                insert_optional_before_trailing_whitespace(output);
                output.push(character);
                return index + 1;
            }
            '}' | ')' | ']' => {
                depth = depth.saturating_sub(1);
                output.push(character);
                index += 1;
            }
            _ => {
                output.push(character);
                index += 1;
            }
        }
    }

    insert_optional_before_trailing_whitespace(output);
    index
}

fn insert_optional_before_trailing_whitespace(output: &mut String) {
    let insert_at = output.trim_end_matches(char::is_whitespace).len();
    output.insert(insert_at, '?');
}
