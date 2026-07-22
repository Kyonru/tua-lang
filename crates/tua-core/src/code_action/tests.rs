use super::*;
use crate::{
    check_parsed_source_with_diagnostics_config, parse_tua_source, AnalysisHost, DiagnosticsConfig,
    Fixture, ProjectConfig,
};

#[test]
fn creates_missing_require_target_action() {
    let parsed = parse_tua_source("src/main.tua", "local Player = require(\"player\")\n")
        .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            missing_requires: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 24),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Create module `player`");
    assert_eq!(
        actions[0].edit.create_files[0].path,
        std::path::Path::new("src/player.tua")
    );
}

#[test]
fn snapshot_assists_create_same_workspace_edit_from_diagnostic() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = require("<|require|>player")
"#,
    );
    let mut config = ProjectConfig::default();

    config.diagnostics.missing_requires = true;

    let mut host = AnalysisHost::with_config(config);

    host.set_source_roots([std::path::PathBuf::from("src")]);
    host.set_file("src/main.tua", fixture.source("src/main.tua"));

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    let position = fixture.position("require");
    let actions = snapshot.assists("src/main.tua", range_line(position.line), &diagnostics);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Create module `player`");
    assert_eq!(
        actions[0].edit.create_files[0].path,
        std::path::Path::new("src/player.tua")
    );
    assert_eq!(
        actions[0].edit.changes[&std::path::PathBuf::from("src/player.tua")][0].new_text,
        "return {}\n"
    );
}

#[test]
fn missing_require_target_action_rejects_unsafe_paths() {
    let parsed = parse_tua_source("src/main.tua", "local Player = require(\"../player\")\n")
        .expect("source should parse");
    let diagnostic = Diagnostic::new(
        "src/main.tua",
        1,
        24,
        crate::DiagnosticSeverity::Warning,
        "bad require file `../player`",
    )
    .with_code(DiagnosticCode::LINT_MISSING_REQUIRE);
    let actions = code_actions_parsed_source(
        &parsed,
        &[diagnostic],
        range(1, 24),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions.is_empty());
}

#[test]
fn adds_local_annotation_from_inferred_type() {
    let parsed = parse_tua_source("src/main.tua", "local hp = 10\n").expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Add type annotation `: number`");
    assert_eq!(
        actions[0].edit.changes[&std::path::PathBuf::from("src/main.tua")][0].new_text,
        ": number"
    );
}

#[test]
fn generates_type_from_stable_table_literal() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"local player = {
  name = "Ada",
  hp = 100,
  alive = true,
  position = { x = 0, y = 0 },
}
"#,
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range(1, 7),
        std::path::Path::new("src"),
        "tua",
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Generate type `Player` from table literal")
        .expect("table type action should be present");
    let edits = &action.edit.changes[&parsed.path];

    assert_eq!(action.kind, AssistKind::QuickFix);
    assert_eq!(edits.len(), 2);
    assert_eq!(
        edits[0].new_text,
        r#"type Player = {
  name: string,
  hp: number,
  alive: boolean,
  position: {
    x: number,
    y: number,
  },
}

"#
    );
    assert_eq!(edits[1].new_text, ": Player");
}

#[test]
fn extracts_selected_expression_to_local_or_function() {
    let parsed = parse_tua_source(
        "src/main.tua",
        "local score = 10\nlocal bonus = 2\nlocal total = score + bonus\n",
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range_span(3, 15, 3, 28),
        std::path::Path::new("src"),
        "tua",
    );
    let local = actions
        .iter()
        .find(|action| action.title == "Extract local `extracted`")
        .expect("extract local action should be present");
    let function = actions
        .iter()
        .find(|action| action.title == "Extract function `extractedFunction`")
        .expect("extract function action should be present");
    let local_edits = &local.edit.changes[&parsed.path];
    let function_edits = &function.edit.changes[&parsed.path];

    assert_eq!(local.kind, AssistKind::QuickFix);
    assert_eq!(local_edits.len(), 2);
    assert_eq!(local_edits[0].new_text, "local extracted = score + bonus\n");
    assert_eq!(local_edits[1].new_text, "extracted");

    assert_eq!(function.kind, AssistKind::QuickFix);
    assert_eq!(function_edits.len(), 2);
    assert_eq!(
        function_edits[0].new_text,
        "local function extractedFunction(score, bonus)\n  return score + bonus\nend\n\n"
    );
    assert_eq!(
        function_edits[1].new_text,
        "extractedFunction(score, bonus)"
    );
}

#[test]
fn inlines_stable_local_with_multiple_resolved_references() {
    let parsed = parse_tua_source(
        "src/main.tua",
        "local total = score + bonus\nprint(total)\nprint(total)\n",
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range(1, 8),
        std::path::Path::new("src"),
        "tua",
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Inline local `total`")
        .expect("inline local action should be present");
    let edits = &action.edit.changes[&parsed.path];

    assert_eq!(action.kind, AssistKind::QuickFix);
    assert_eq!(edits.len(), 3);
    assert_eq!(edits[0].new_text, "");
    assert_eq!(edits[1].new_text, "(score + bonus)");
    assert_eq!(edits[2].new_text, "(score + bonus)");
}

#[test]
fn skips_multi_use_inline_local_for_unstable_initializers() {
    let parsed = parse_tua_source(
        "src/main.tua",
        "local total = compute()\nprint(total, total)\n",
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range(1, 8),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .all(|action| action.title != "Inline local `total`"));
}

#[test]
fn skips_type_generation_for_unstable_table_literals() {
    let parsed = parse_tua_source("src/main.tua", "local player = { hp = score }\n")
        .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .all(|action| !action.title.starts_with("Generate type `Player`")));
}

#[test]
fn creates_object_pool_for_repeated_zero_argument_constructor_call() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"local first = Bullet.new()
local second = Bullet.new()
"#,
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        range(1, 17),
        std::path::Path::new("src"),
        "tua",
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Create objectPool for repeated `Bullet.new()`")
        .expect("object pool action should be offered");
    let edits = &action.edit.changes[&parsed.path];

    assert_eq!(action.kind, AssistKind::QuickFix);
    assert_eq!(edits.len(), 2);
    assert!(edits[0]
        .new_text
        .contains("local bulletPool = objectPool({"));
    assert!(edits[0].new_text.contains("return Bullet.new()"));
    assert_eq!(edits[1].new_text, "bulletPool:acquire()");
}

#[test]
fn renames_a_misspelled_love_callback() {
    let source = "function love.udpate(dt: number)\nend\n";
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let diagnostics = diagnostics(&parsed, DiagnosticsConfig::default());
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 15),
        std::path::Path::new("src"),
        "tua",
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Rename to `love.update`")
        .expect("callback rename action should be present");

    assert_eq!(action.kind, AssistKind::QuickFix);
    assert_eq!(action.edit.changes[&parsed.path][0].new_text, "update");
}

#[test]
fn does_not_offer_a_rename_for_love_callback_signature_diagnostics() {
    let source = "function love.update(dt: string)\nend\n";
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let diagnostics = diagnostics(&parsed, DiagnosticsConfig::default());
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 15),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .all(|action| !action.title.starts_with("Rename to `love.")));
}

#[test]
fn creates_missing_core_love_callbacks_as_a_source_action() {
    let source = "function love.update(dt: number)\nend\n";
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let actions = code_actions_parsed_source_with_kinds(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceCreateLoveCallbacks],
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Create missing LOVE callbacks");
    assert_eq!(actions[0].kind, AssistKind::SourceCreateLoveCallbacks);
    assert_eq!(
        actions[0].edit.changes[&parsed.path][0].new_text,
        r#"
function love.load(arg, unfilteredArg)
end

function love.draw()
end
"#
    );
}

#[test]
fn removes_unused_local_and_self_assignment() {
    let parsed = parse_tua_source("src/main.tua", "local unused = 1\nhp = hp\n")
        .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            unused_locals: true,
            self_assignments: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(1, 7),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .any(|action| action.title == "Remove unused local"));

    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(2, 1),
        std::path::Path::new("src"),
        "tua",
    );

    assert!(actions
        .iter()
        .any(|action| action.title == "Remove self-assignment"));
}

#[test]
fn renames_duplicate_table_key() {
    let parsed = parse_tua_source(
        "src/main.tua",
        "local config = { hp = 10, hp = 20 }\nreturn config\n",
    )
    .expect("source should parse");
    let diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            duplicate_keys: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range_line(1),
        std::path::Path::new("src"),
        "tua",
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Rename duplicate key to `hp2`");
    assert_eq!(
        actions[0].edit.changes[&std::path::PathBuf::from("src/main.tua")][0].new_text,
        "hp2"
    );
}

#[test]
fn rewrites_love_dot_method_call_to_colon_call() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local player = image "assets/player.png"
player.setFilter("nearest")
"#,
    )
    .expect("source should parse");
    let diagnostics = diagnostics(&parsed, DiagnosticsConfig::default());
    let actions = code_actions_parsed_source(
        &parsed,
        &diagnostics,
        range(3, 8),
        std::path::Path::new("src"),
        "tua",
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Use colon method call")
        .expect("colon quick fix should be present");
    let edit = &action.edit.changes[&std::path::PathBuf::from("src/main.tua")][0];

    assert_eq!(edit.new_text, ":");
    assert_eq!(
        edit.span,
        SourceSpan {
            start: SourcePosition { line: 3, column: 7 },
            end: SourcePosition { line: 3, column: 8 },
        }
    );
}

#[test]
fn rewrites_love_constructors_to_asset_helpers() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"
local player = love.graphics.newImage("assets/player.png")
local jump = love.audio.newSource("assets/jump.wav", "static")
local theme = love.audio.newSource("assets/theme.ogg", "stream")
"#,
    )
    .expect("source should parse");
    let actions = code_actions_parsed_source(
        &parsed,
        &[],
        CodeActionRange {
            start: SourcePosition { line: 2, column: 1 },
            end: SourcePosition {
                line: 4,
                column: 80,
            },
        },
        std::path::Path::new("src"),
        "tua",
    );

    assert_action_rewrite(
        &actions,
        "Rewrite as `image` helper",
        "image \"assets/player.png\"",
    );
    assert_action_rewrite(
        &actions,
        "Rewrite as `sound` helper",
        "sound \"assets/jump.wav\"",
    );
    assert_action_rewrite(
        &actions,
        "Rewrite as `music` helper",
        "music \"assets/theme.ogg\"",
    );
}

#[test]
fn source_actions_sort_remove_and_organize_imports() {
    let parsed = parse_tua_source(
        "src/main.tua",
        r#"local Z = require("z")
local A = require("a")
local Unused = import("unused")
print(A, Z)
"#,
    )
    .expect("source should parse");
    let sort = code_actions_parsed_source_with_kinds(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceSortImports],
    );
    let remove = code_actions_parsed_source_with_kinds(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceRemoveUnusedImports],
    );
    let organize = code_actions_parsed_source_with_kinds(
        &parsed,
        &[],
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceOrganizeImports],
    );

    assert_eq!(sort[0].kind, AssistKind::SourceSortImports);
    assert_eq!(
        action_document_text(&sort[0], "src/main.tua"),
        r#"local A = require("a")
local Unused = import("unused")
local Z = require("z")
print(A, Z)
"#
    );
    assert_eq!(remove[0].kind, AssistKind::SourceRemoveUnusedImports);
    assert_eq!(
        action_document_text(&remove[0], "src/main.tua"),
        r#"local Z = require("z")
local A = require("a")
print(A, Z)
"#
    );
    assert_eq!(organize[0].kind, AssistKind::SourceOrganizeImports);
    assert_eq!(
        action_document_text(&organize[0], "src/main.tua"),
        r#"local A = require("a")
local Z = require("z")
print(A, Z)
"#
    );
}

#[test]
fn source_fix_all_applies_only_safe_non_overlapping_diagnostic_fixes() {
    let parsed = parse_tua_source(
        "src/main.tua",
        "local unused = 1\nhp = hp\nlocal used = 2\nprint(used)\n",
    )
    .expect("source should parse");
    let fix_diagnostics = diagnostics(
        &parsed,
        DiagnosticsConfig {
            unused_locals: true,
            self_assignments: true,
            ..DiagnosticsConfig::default()
        },
    );
    let actions = code_actions_parsed_source_with_kinds(
        &parsed,
        &fix_diagnostics,
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceFixAll],
    );
    let edits = &actions[0].edit.changes[&std::path::PathBuf::from("src/main.tua")];

    assert_eq!(actions[0].kind, AssistKind::SourceFixAll);
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|edit| edit.new_text.is_empty()));

    let parsed_missing = parse_tua_source("src/main.tua", "local Player = require(\"player\")\n")
        .expect("source should parse");
    let missing_require = diagnostics(
        &parsed_missing,
        DiagnosticsConfig {
            missing_requires: true,
            ..DiagnosticsConfig::default()
        },
    );
    let unsafe_actions = code_actions_parsed_source_with_kinds(
        &parsed_missing,
        &missing_require,
        range_line(1),
        std::path::Path::new("src"),
        "tua",
        &[AssistKind::SourceFixAll],
    );

    assert!(unsafe_actions.is_empty());
}

fn diagnostics(parsed: &ParsedSource, config: DiagnosticsConfig) -> Vec<Diagnostic> {
    check_parsed_source_with_diagnostics_config(parsed, &config)
        .err()
        .unwrap_or_default()
}

fn assert_action_rewrite(actions: &[CodeAction], title: &str, new_text: &str) {
    let action = actions
        .iter()
        .find(|action| action.title == title)
        .unwrap_or_else(|| panic!("{title} should be present in {actions:?}"));
    let edit = action
        .edit
        .changes
        .values()
        .next()
        .and_then(|edits| edits.first())
        .expect("action should have an edit");

    assert_eq!(edit.new_text, new_text);
}

fn range(line: usize, column: usize) -> CodeActionRange {
    CodeActionRange {
        start: SourcePosition { line, column },
        end: SourcePosition { line, column },
    }
}

fn range_span(
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
) -> CodeActionRange {
    CodeActionRange {
        start: SourcePosition {
            line: start_line,
            column: start_column,
        },
        end: SourcePosition {
            line: end_line,
            column: end_column,
        },
    }
}

fn range_line(line: usize) -> CodeActionRange {
    CodeActionRange {
        start: SourcePosition { line, column: 1 },
        end: SourcePosition { line, column: 200 },
    }
}

fn action_document_text<'a>(action: &'a Assist, path: &str) -> &'a str {
    &action.edit.changes[&std::path::PathBuf::from(path)][0].new_text
}
