use std::path::Path;

use crate::source_text::position_at;
use crate::{selection_ranges_tua_source, SelectionRange, SourceSpan};

#[test]
fn selection_range_grows_from_identifier_to_field_chain_statement_and_block() {
    let source = r#"
local function update(player: Player)
  player.hp = player.hp - 1
end
"#;
    let position = position_at(source, "hp =");
    let ranges = selection_ranges_tua_source(Path::new("src/main.tua"), source, &[position])
        .expect("selection ranges should be recoverable");
    let range = ranges[0]
        .as_ref()
        .expect("position should have a selection range");
    let texts = range_texts(source, range);

    assert_eq!(texts[0], "hp");
    assert_eq!(texts[1], "player.hp");
    assert!(texts.contains(&"player.hp = player.hp - 1"));
    assert!(texts
        .iter()
        .any(|text| text.contains("local function update")));
}

#[test]
fn selection_range_returns_line_scope_for_non_identifier_positions() {
    let source = "local value = {\n  hp = 10,\n}\n";
    let position = position_at(source, "= 10");
    let ranges = selection_ranges_tua_source(Path::new("src/main.tua"), source, &[position])
        .expect("selection ranges should be recoverable");
    let range = ranges[0]
        .as_ref()
        .expect("position should have a selection range");
    let texts = range_texts(source, range);

    assert_eq!(texts[0], "hp = 10,");
    assert!(texts.iter().any(|text| text.contains("local value")));
}

fn range_texts<'a>(source: &'a str, range: &'a SelectionRange) -> Vec<&'a str> {
    let mut texts = Vec::new();
    let mut current = Some(range);

    while let Some(range) = current {
        texts.push(source_text(source, range.range));
        current = range.parent.as_deref();
    }

    texts
}

fn source_text(source: &str, span: SourceSpan) -> &str {
    if span.start.line == span.end.line {
        let line = source
            .split('\n')
            .nth(span.start.line - 1)
            .expect("line should exist");
        let start = span.start.column - 1;
        let end = span.end.column - 1;

        return &line[start..end];
    }

    let start_index = byte_index(source, span.start);
    let end_index = byte_index(source, span.end);

    &source[start_index..end_index]
}

fn byte_index(source: &str, position: crate::SourcePosition) -> usize {
    let mut line = 1;
    let mut column = 1;

    for (index, character) in source.char_indices() {
        if line == position.line && column == position.column {
            return index;
        }

        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    source.len()
}
