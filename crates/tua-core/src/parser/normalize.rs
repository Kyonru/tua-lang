pub(super) fn normalize_tua_source_for_luau_parser(source: &str) -> String {
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
    for (offset, expected) in keyword.chars().enumerate() {
        if chars.get(index + offset) != Some(&expected) {
            return false;
        }
    }

    !chars
        .get(index + keyword.chars().count())
        .is_some_and(|character| is_identifier_char(*character))
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
