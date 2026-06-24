use std::path::Path;

use crate::{folding_ranges_tua_source, FoldingRangeKind};

#[test]
fn folding_ranges_include_nested_functions_and_table_constructors() {
    let source = r#"
local config = {
  player = {
    hp = 10,
  },
}

local function outer()
  local function inner()
    return config.player.hp
  end
end
"#;
    let ranges = folding_ranges_tua_source(Path::new("src/main.tua"), source)
        .expect("folding ranges should be recoverable");

    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Table
            && range.range.start.line == 2
            && range.range.end.line == 6
    }));
    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Table
            && range.range.start.line == 3
            && range.range.end.line == 5
    }));
    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Function
            && range.range.start.line == 8
            && range.range.end.line == 12
    }));
    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Function
            && range.range.start.line == 9
            && range.range.end.line == 11
    }));
}

#[test]
fn folding_ranges_include_comments_and_multiline_type_aliases() {
    let source = r#"
-- First line
-- Second line
type Player = {
  hp: number,
  name: string,
}
"#;
    let ranges = folding_ranges_tua_source(Path::new("src/main.tua"), source)
        .expect("folding ranges should be recoverable");

    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Comment
            && range.range.start.line == 2
            && range.range.end.line == 3
    }));
    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::TypeAlias
            && range.range.start.line == 4
            && range.range.end.line == 7
    }));
}

#[test]
fn folding_ranges_are_stable_for_incomplete_sources() {
    let source = r#"
-- docs
-- still docs
local config = {
  hp = 10,
"#;
    let ranges = folding_ranges_tua_source(Path::new("src/main.tua"), source)
        .expect("incomplete source should still produce best-effort folds");

    assert!(ranges.iter().any(|range| {
        range.kind == FoldingRangeKind::Comment
            && range.range.start.line == 2
            && range.range.end.line == 3
    }));
}
