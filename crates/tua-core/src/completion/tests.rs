use super::*;
use crate::source_text::position_at;
use crate::Fixture;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = r#"type Vec2 = { x: number, y: number }
type Player = { pos: Vec2, hp: number, alive?: boolean }

local player: Player = {
  pos = { x = 0, y = 0 },
  hp = 10,
}

local payload: any = {}
local value = player.hp
local partial = player.alive
local nested = player.pos.x
local typed: Pl = nil
"#;

#[test]
fn completes_builtin_ecs_module_and_documented_members() {
    let identifier = complete_tua_source(
        "src/main.tua",
        "ec",
        crate::SourcePosition { line: 1, column: 3 },
    )
    .expect("ECS identifier completion should succeed");
    let ecs = identifier
        .iter()
        .find(|completion| completion.label == "ecs")
        .expect("built-in ecs module should complete");

    assert_eq!(ecs.kind, CompletionKind::Module);
    assert!(ecs
        .documentation
        .as_deref()
        .is_some_and(|documentation| { documentation.contains("data-oriented ECS helpers") }));

    let source = "local world = ecs.\n";
    let members = complete_tua_source("src/main.tua", source, position_after(source, "ecs."))
        .expect("ECS member completion should succeed");
    let world = members
        .iter()
        .find(|completion| completion.label == "world")
        .expect("ecs.world should complete");

    assert!(labels(&members).contains(&"component".to_owned()));
    assert!(labels(&members).contains(&"system".to_owned()));
    assert_eq!(
        world.documentation.as_deref(),
        Some("Create an empty ECS world that owns entities and component storage.")
    );
}

#[test]
fn completes_type_aliases_in_annotation_positions() {
    let completions = complete_at("typed: Pl").expect("alias completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(completions[0].detail.as_deref(), Some("type alias"));
}

#[test]
fn completes_native_callback_fields_with_function_details() {
    let source = r#"type Handler = (value: number) -> string
type Hooks = {
  onValue: Handler,
}

local hooks: Hooks = nil
hooks.on
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "hooks.on"))
            .expect("callback field completion should succeed");

    assert_eq!(labels(&completions), vec!["onValue".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("field: (value: number) -> string")
    );
}

#[test]
fn completes_lua_doc_annotation_tags_after_marker() {
    let fixture = Fixture::parse(
        r#"---@p<|cursor|>
-- expect completions: param, type
"#,
    );
    let source = fixture.source("src/main.tua");
    let completions = complete_tua_source("src/main.tua", source, fixture.position("cursor"))
        .expect("annotation tag completion should succeed");

    assert_eq!(labels(&completions), fixture.expected_list("completions"));
    assert_eq!(completions[0].kind, CompletionKind::Keyword);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function parameter annotation")
    );

    let overload = complete_tua_source(
        "src/main.tua",
        "---@o",
        crate::SourcePosition { line: 1, column: 6 },
    )
    .expect("overload annotation completion should succeed");
    assert_eq!(
        labels(&overload),
        vec!["overload".to_owned(), "version".to_owned()]
    );

    let vararg = complete_tua_source(
        "src/main.tua",
        "---@v",
        crate::SourcePosition { line: 1, column: 6 },
    )
    .expect("vararg annotation completion should succeed");
    assert_eq!(
        labels(&vararg),
        vec![
            "vararg".to_owned(),
            "version".to_owned(),
            "overload".to_owned()
        ]
    );
}

#[test]
fn completes_lua_doc_type_names_in_tua_files() {
    let source = r#"type Player = { hp: number }
---@param player Pl
local function damage(player)
end
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "player Pl"))
            .expect("annotation type completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
}

#[test]
fn completes_lua_doc_type_names_in_lua_files() {
    let source = r#"---@alias Player { hp: number }
---@return Pl
local function makePlayer()
end
"#;
    let completions = complete_tua_source(
        "src/player.lua",
        source,
        position_after(source, "return Pl"),
    )
    .expect("Lua annotation type completion should succeed");

    assert_eq!(labels(&completions), vec!["Player".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
}

#[test]
fn completes_love_builtin_type_names() {
    let source = "local player: Im\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "Im"))
        .expect("LOVE type completion should succeed");
    let canvas_source = "local target: Can\n";
    let canvas = complete_tua_source(
        "src/main.tua",
        canvas_source,
        position_after(canvas_source, "Can"),
    )
    .expect("LOVE object type completion should succeed");

    assert_eq!(
        labels(&completions),
        vec![
            "Image".to_owned(),
            "ImageData".to_owned(),
            "CompressedImageData".to_owned()
        ]
    );
    assert_eq!(completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(completions[0].detail.as_deref(), Some("built-in type"));
    assert_eq!(labels(&canvas), vec!["Canvas".to_owned()]);
}

#[test]
fn completes_builtin_vec2_type_name_constructor_and_fields() {
    let type_source = "local position: Ve\n";
    let type_completions = complete_tua_source(
        "src/main.tua",
        type_source,
        position_after(type_source, "Ve"),
    )
    .expect("Vec2 type completion should succeed");
    let constructor_source = "local position = ve\n";
    let constructor_completions = complete_tua_source(
        "src/main.tua",
        constructor_source,
        position_after(constructor_source, "ve"),
    )
    .expect("vec2 constructor completion should succeed");
    let field_source = r#"
local position = vec2(100, 200)
position.
"#;
    let field_completions = complete_tua_source(
        "src/main.tua",
        field_source,
        position_after(field_source, "position."),
    )
    .expect("Vec2 field completion should succeed");

    assert_eq!(
        labels(&type_completions),
        vec!["Vec2".to_owned(), "BezierCurve".to_owned()]
    );
    assert_eq!(type_completions[0].kind, CompletionKind::TypeAlias);
    assert_eq!(type_completions[0].detail.as_deref(), Some("built-in type"));
    assert_eq!(labels(&constructor_completions), vec!["vec2".to_owned()]);
    assert_eq!(constructor_completions[0].kind, CompletionKind::Function);
    assert_eq!(
        constructor_completions[0].detail.as_deref(),
        Some("function vec2(x: number, y: number): Vec2")
    );
    let field_labels = labels(&field_completions);
    assert!(field_labels.contains(&"x".to_owned()));
    assert!(field_labels.contains(&"y".to_owned()));
    assert!(field_labels.contains(&"length".to_owned()));
    assert!(field_labels.contains(&"normalized".to_owned()));
    assert!(field_labels.contains(&"dot".to_owned()));
    let x = field_completions
        .iter()
        .find(|completion| completion.label == "x")
        .expect("x should complete");
    assert_eq!(x.detail.as_deref(), Some("field: number"));
}

#[test]
fn completes_builtin_signal_type_constructor_and_methods() {
    let type_source = "local events: Sig\n";
    let type_completions = complete_tua_source(
        "src/main.tua",
        type_source,
        position_after(type_source, "Sig"),
    )
    .expect("SignalBus type completion should succeed");
    let constructor_source = "local events = sig\n";
    let constructor_completions = complete_tua_source(
        "src/main.tua",
        constructor_source,
        position_after(constructor_source, "sig"),
    )
    .expect("signal constructor completion should succeed");
    let field_source = r#"
local events = signal()
events:
"#;
    let field_completions = complete_tua_source(
        "src/main.tua",
        field_source,
        position_after(field_source, "events:"),
    )
    .expect("SignalBus method completion should succeed");

    assert!(labels(&type_completions).contains(&"SignalBus".to_owned()));
    assert!(labels(&constructor_completions).contains(&"signal".to_owned()));

    let field_labels = labels(&field_completions);
    assert!(field_labels.contains(&"on".to_owned()));
    assert!(field_labels.contains(&"once".to_owned()));
    assert!(field_labels.contains(&"declare".to_owned()));
    assert!(field_labels.contains(&"emit".to_owned()));
    assert!(field_labels.contains(&"off".to_owned()));
    assert!(field_labels.contains(&"clear".to_owned()));

    let emit = field_completions
        .iter()
        .find(|completion| completion.label == "emit")
        .expect("emit should complete");
    assert_eq!(
        emit.detail.as_deref(),
        Some("function SignalBus:emit(name: string, ...: any): nil")
    );
    assert!(emit
        .documentation
        .as_deref()
        .is_some_and(|documentation| documentation.contains("Emit a named signal")));
    assert_eq!(
        emit.insert_text.as_deref(),
        Some("emit(${1:name}, ${2:...})$0")
    );
    assert!(emit.is_snippet);
}

#[test]
fn completes_builtin_object_pool_workflow_methods_and_item_fields() {
    let constructor_source = "local bullets = objectP\n";
    let constructor_completions = complete_tua_source(
        "src/main.tua",
        constructor_source,
        position_after(constructor_source, "objectP"),
    )
    .expect("objectPool constructor completion should succeed");
    let source = r#"
type Bullet = { x: number, active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { x = 0, active = false }
  end,
})
bullets:
"#;
    let methods = complete_tua_source("src/main.tua", source, position_after(source, "bullets:"))
        .expect("ObjectPool method completion should succeed");
    let item_source = source.replace("bullets:\n", "local bullet = bullets:acquire()\nbullet.\n");
    let item_fields = complete_tua_source(
        "src/main.tua",
        &item_source,
        position_after(&item_source, "bullet."),
    )
    .expect("pooled item field completion should succeed");

    assert!(labels(&constructor_completions).contains(&"objectPool".to_owned()));
    let method_labels = labels(&methods);
    for expected in [
        "acquire",
        "release",
        "prewarm",
        "clear",
        "owns",
        "activeCount",
        "availableCount",
        "capacity",
    ] {
        assert!(
            method_labels.contains(&expected.to_owned()),
            "missing {expected}"
        );
    }
    let release = methods
        .iter()
        .find(|completion| completion.label == "release")
        .expect("release should complete");
    assert!(release
        .documentation
        .as_deref()
        .is_some_and(|documentation| documentation.contains("duplicate releases")));
    assert!(labels(&item_fields).contains(&"x".to_owned()));
    assert!(labels(&item_fields).contains(&"active".to_owned()));
}

#[test]
fn completes_signal_event_names_from_existing_signal_calls() {
    let source = r#"
local events = signal()
local ui = signal()

ui:on("ui.ready", function() end)
events:on("player.hit", function() end)
events:on("player.dead", function() end)
events:emit("")
"#;
    let completions = complete_tua_source(
        "src/events.tua",
        source,
        position_after_last(source, "emit(\""),
    )
    .expect("SignalBus event completion should succeed");

    assert_eq!(
        labels(&completions),
        vec![
            "player.dead".to_owned(),
            "player.hit".to_owned(),
            "ui.ready".to_owned()
        ]
    );
    assert!(completions
        .iter()
        .all(|completion| completion.kind == CompletionKind::Value));
    assert!(completions
        .iter()
        .all(|completion| completion.detail.as_deref() == Some("SignalBus event")));
}

#[test]
fn completes_signal_event_names_from_declare_calls() {
    let source = r#"
local events = signal()
events:declare("player.hit", function() end)
events:emit("")
"#;
    let completions = complete_tua_source(
        "src/events.tua",
        source,
        position_after_last(source, "emit(\""),
    )
    .expect("SignalBus declare event completion should succeed");

    assert_eq!(labels(&completions), vec!["player.hit".to_owned()]);
}

#[test]
fn recovers_signal_event_names_for_typoed_string_arguments() {
    let source = r#"
local events = signal()
events:on("player.hit", function() end)
events:emit("plyer.h")
"#;
    let completions = complete_tua_source(
        "src/events.tua",
        source,
        position_after_last(source, "\"plyer.h"),
    )
    .expect("SignalBus event typo recovery should succeed");

    assert_eq!(labels(&completions), vec!["player.hit".to_owned()]);
    assert_eq!(completions[0].filter_text.as_deref(), Some("plyer.h"));
}

#[test]
fn signal_event_completion_skips_pattern_methods() {
    let source = r#"
local events = signal()
events:on("player.hit", function() end)
events:emitPattern("pla")
"#;
    let completions = complete_tua_source(
        "src/events.tua",
        source,
        position_after_last(source, "\"pla"),
    )
    .expect("SignalBus pattern completion should stay quiet");

    assert!(completions.is_empty(), "{completions:?}");
}

#[test]
fn completes_signal_event_names_from_project_sources() {
    let source = r#"
local events = signal()
events:emit("")
"#;
    let project_source = r#"
local bus = signal()
bus:on("enemy.spawn", function() end)
"#;
    let parsed = parse_tua_source("src/events.tua", source).expect("source should parse");
    let (environment, diagnostics) = build_type_environment_with_diagnostics(&parsed);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let completions = complete_source_with_parsed_environment_and_project_sources(
        source,
        &parsed,
        position_after_last(source, "emit(\""),
        &environment,
        &[project_source],
    );

    assert_eq!(labels(&completions), vec!["enemy.spawn".to_owned()]);
}

#[test]
fn generates_function_documentation_snippet_from_tua_signature() {
    let source = r#"type Player = { hp: number }
---
local function damage(player: Player, amount: number): Player
end
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "---"))
        .expect("documentation snippet completion should succeed");

    assert_eq!(labels(&completions), vec!["document function".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Snippet);
    assert!(completions[0].is_snippet);
    assert_eq!(
        completions[0].insert_text.as_deref(),
        Some(
            "@param player Player ${1:description}\n---@param amount number ${2:description}\n---@return Player ${3:description}"
        )
    );
}

#[test]
fn generates_function_documentation_snippet_from_lua_signature() {
    let source = r#"---
local function play(source, ...)
end
"#;
    let completions = complete_tua_source("src/audio.lua", source, position_after(source, "---"))
        .expect("Lua documentation snippet completion should succeed");

    assert_eq!(labels(&completions), vec!["document function".to_owned()]);
    assert_eq!(
        completions[0].insert_text.as_deref(),
        Some("@param source any ${1:description}\n---@param ... any ${2:description}")
    );
}

#[test]
fn annotation_completions_work_while_source_is_temporarily_invalid() {
    let source = "local broken =\n---@r\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "---@r"))
        .expect("lossy annotation completion should succeed");

    assert_eq!(
        labels(&completions),
        vec![
            "return".to_owned(),
            "param".to_owned(),
            "vararg".to_owned(),
            "version".to_owned(),
            "overload".to_owned(),
            "generic".to_owned()
        ]
    );
}

#[test]
fn completes_table_fields_after_dot_access() {
    let completions = complete_at("player.").expect("field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["hp".to_owned(), "alive".to_owned(), "pos".to_owned()]
    );
    assert!(completions
        .iter()
        .all(|item| item.kind == CompletionKind::Field));
}

#[test]
fn field_completion_ranking_degrades_to_alphabetical_without_usage() {
    let source = r#"type Player = { zeta: number, alpha: number, middle: number }
local player: Player = {}
player.
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "player."))
            .expect("field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["alpha".to_owned(), "middle".to_owned(), "zeta".to_owned()]
    );
}

#[test]
fn project_member_usage_ranks_field_completions() {
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local sprite: Sprite = {}
local current = sprite.pause
"#;
    let project_source = r#"
sprite.stop()
sprite.stop()
sprite.play()
"#;
    let parsed = parse_tua_source("src/main.tua", source.to_owned())
        .expect("source should parse for completion");
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);
    let completions = complete_source_with_parsed_environment_and_project_sources(
        source,
        &parsed,
        position_after(source, "sprite."),
        &environment,
        &[project_source],
    );

    assert_eq!(
        labels(&completions),
        vec!["stop".to_owned(), "play".to_owned(), "pause".to_owned()]
    );
}

#[test]
fn adaptive_stats_rank_field_completions_without_current_usage() {
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local sprite: Sprite = {}
local current = sprite.pause
"#;
    let parsed = parse_tua_source("src/main.tua", source.to_owned())
        .expect("source should parse for completion");
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);
    let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(Path::new("src/main.tua"));
    let mut adaptive_stats = CompletionAdaptiveStats::default();

    adaptive_stats.replace_file_observations(
        file_scope_hash,
        r#"
sprite.stop()
sprite.stop()
sprite.play()
"#,
        100,
    );

    let completions = complete_source_with_parsed_environment_and_ranking(
        source,
        &parsed,
        position_after(source, "sprite."),
        &environment,
        &[],
        Some(&adaptive_stats),
        file_scope_hash,
    );

    assert_eq!(
        labels(&completions),
        vec!["stop".to_owned(), "play".to_owned(), "pause".to_owned()]
    );
}

#[test]
fn sequence_ranking_reorders_candidates_when_enabled() {
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local stop = 1
local stopAgain = stop
local sprite: Sprite = {}
local current = sprite.pause
"#;
    let completion_source = source.replace("sprite.pause", "sprite.");
    let parsed = parse_tua_source("src/main.tua", source.to_owned())
        .expect("source should parse for completion");
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);
    let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(Path::new("src/main.tua"));
    let default_completions = complete_source_with_parsed_environment_and_ranking(
        &completion_source,
        &parsed,
        position_after(&completion_source, "sprite."),
        &environment,
        &[],
        None,
        file_scope_hash,
    );
    let config = CompletionSequenceRankingConfig {
        enabled: true,
        ..CompletionSequenceRankingConfig::default()
    };
    let ranked = complete_source_with_parsed_environment_and_ranking_inputs(
        &completion_source,
        &parsed,
        position_after(&completion_source, "sprite."),
        &environment,
        CompletionRankingInputs::from_sources(&[], None, file_scope_hash, &config),
    );

    assert_eq!(
        labels(&default_completions),
        vec!["pause".to_owned(), "play".to_owned(), "stop".to_owned()]
    );
    assert_eq!(
        labels(&ranked),
        vec!["stop".to_owned(), "pause".to_owned(), "play".to_owned()]
    );
}

#[test]
fn sequence_ranking_timeout_preserves_existing_order() {
    let source = r#"type Sprite = { pause: number, play: number, stop: number }
local stop = 1
local stopAgain = stop
local sprite: Sprite = {}
local current = sprite.pause
"#;
    let completion_source = source.replace("sprite.pause", "sprite.");
    let parsed = parse_tua_source("src/main.tua", source.to_owned())
        .expect("source should parse for completion");
    let (environment, _) = build_type_environment_with_diagnostics(&parsed);
    let file_scope_hash = CompletionAdaptiveStats::file_scope_hash(Path::new("src/main.tua"));
    let config = CompletionSequenceRankingConfig {
        enabled: true,
        timeout_ms: 0,
        ..CompletionSequenceRankingConfig::default()
    };
    let completions = complete_source_with_parsed_environment_and_ranking_inputs(
        &completion_source,
        &parsed,
        position_after(&completion_source, "sprite."),
        &environment,
        CompletionRankingInputs::from_sources(&[], None, file_scope_hash, &config),
    );

    assert_eq!(
        labels(&completions),
        vec!["pause".to_owned(), "play".to_owned(), "stop".to_owned()]
    );
}

#[test]
fn multi_token_completion_is_disabled_by_default() {
    let source = "local velocity = \n";
    let completions = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        source,
        position_after(source, "local velocity = "),
        0,
        &CompletionMultiTokenConfig::default(),
    );

    assert!(completions.is_empty());
}

#[test]
fn multi_token_completion_requires_low_confidence_or_advanced_path() {
    let source = "local velocity = \n";
    let enabled = CompletionMultiTokenConfig {
        enabled: true,
        ..CompletionMultiTokenConfig::default()
    };
    let advanced = CompletionMultiTokenConfig {
        enabled: true,
        advanced: true,
        ..CompletionMultiTokenConfig::default()
    };
    let position = position_after(source, "local velocity = ");

    let high_confidence = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        source,
        position,
        10,
        &enabled,
    );
    let low_confidence = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        source,
        position,
        0,
        &enabled,
    );
    let advanced_suggestions = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        source,
        position,
        10,
        &advanced,
    );

    assert!(high_confidence.is_empty());
    assert_eq!(labels(&low_confidence), vec!["vec2(0, 0)".to_owned()]);
    assert_eq!(labels(&advanced_suggestions), vec!["vec2(0, 0)".to_owned()]);
}

#[test]
fn multi_token_completion_rejects_unparsable_insertions() {
    let source = "local velocity = \n)\n";
    let config = CompletionMultiTokenConfig {
        enabled: true,
        advanced: true,
        ..CompletionMultiTokenConfig::default()
    };
    let completions = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        source,
        position_after(source, "local velocity = "),
        0,
        &config,
    );

    assert!(completions.is_empty());
}

#[test]
fn multi_token_completion_supports_expression_tail_and_method_chain_contexts() {
    let config = CompletionMultiTokenConfig {
        enabled: true,
        advanced: true,
        ..CompletionMultiTokenConfig::default()
    };
    let tail_source = "return score\n";
    let chain_source = "local result = audioSource()\n";
    let tail = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        tail_source,
        position_after(tail_source, "return score"),
        0,
        &config,
    );
    let chain = crate::completion::multitoken::complete_multi_token_source(
        Path::new("src/main.tua"),
        chain_source,
        position_after(chain_source, "audioSource()"),
        0,
        &config,
    );

    assert_eq!(labels(&tail), vec!["or nil".to_owned()]);
    assert_eq!(labels(&chain), vec![":play()".to_owned()]);
}

#[test]
fn filters_table_fields_by_partial_prefix() {
    let completions = complete_at("player.a").expect("field completion should succeed");

    assert_eq!(labels(&completions), vec!["alive".to_owned()]);
}

#[test]
fn completes_nested_table_fields_when_shape_is_known() {
    let completions = complete_at("player.pos.").expect("nested field completion should succeed");

    assert_eq!(labels(&completions), vec!["x".to_owned(), "y".to_owned()]);
}

#[test]
fn completes_lua_keywords_and_globals_in_expression_positions() {
    let source = "local value = ma\n";
    let position = position_after(source, "ma");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("Lua baseline completion should succeed");

    assert!(labels(&completions).contains(&"math".to_owned()));
}

#[test]
fn completes_tua_and_lua_statement_snippets() {
    for (source, needle, label, insert_text) in [
        ("cl", "cl", "class", "class ${1:Name} {\n  ${0}\n}"),
        ("ty", "ty", "type", "type ${1:Name} = {\n  ${0}\n}"),
        (
            "fun",
            "fun",
            "function",
            "function ${1:name}(${2})\n  ${0}\nend",
        ),
        ("loc", "loc", "local", "local ${1:name} = ${0:nil}"),
        (
            "spr",
            "spr",
            "spriteSheet",
            "local ${1:Player} = spriteSheet({\n  image = \"${2:assets/player.png}\",\n  frame = { width = ${3:16}, height = ${4:16} },\n  animations = {\n    ${5:idle} = { from = ${6:0}, to = ${7:3}, fps = ${8:8} },\n  },\n})",
        ),
        (
            "sig",
            "sig",
            "signal",
            "local ${1:signals} = signal()\n${1:signals}:on(\"${2:event}\", function(${3})\n  ${0}\nend)",
        ),
    ] {
        let completions =
            complete_tua_source("src/main.tua", source, position_after(source, needle))
                .expect("statement snippet completion should succeed");
        let completion = completions
            .iter()
            .find(|completion| completion.label == label)
            .unwrap_or_else(|| panic!("{label} snippet should complete"));

        assert_eq!(completion.kind, CompletionKind::Snippet);
        assert!(completion.is_snippet);
        assert_eq!(completion.insert_text.as_deref(), Some(insert_text));
    }
}

#[test]
fn ranks_nearby_locals_before_syntax_snippets() {
    let source = r#"local classFar = 1
local classNear = 2
local value = cl
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after(source, "local value = cl"),
    )
    .expect("identifier completion should succeed");
    let labels = labels(&completions);
    let class_index = labels
        .iter()
        .position(|label| label == "class")
        .expect("class snippet should complete");

    assert_eq!(labels[0], "classNear");
    assert_eq!(labels[1], "classFar");
    assert!(class_index > 1, "{labels:?}");
}

#[test]
fn parsed_identifier_completion_excludes_out_of_scope_locals() {
    let source = r#"do
  local insideChoice = 1
end

local visibleChoice = 2
local value = Choice
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after(source, "local value = Choice"),
    )
    .expect("identifier completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"visibleChoice".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"insideChoice".to_owned()), "{labels:?}");
}

#[test]
fn parsed_identifier_completion_is_reproducible() {
    let source = r#"local classFar = 1
local classNear = 2
local value = cl
"#;
    let position = position_after(source, "local value = cl");
    let first = complete_tua_source("src/main.tua", source, position)
        .expect("first completion should succeed");
    let second = complete_tua_source("src/main.tua", source, position)
        .expect("second completion should succeed");

    assert_eq!(labels(&first), labels(&second));
}

#[test]
fn parsed_identifier_completion_prioritizes_visible_module_aliases() {
    let source = r#"local classAssets = require("assets")
local value = cl
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after(source, "local value = cl"),
    )
    .expect("identifier completion should succeed");
    let labels = labels(&completions);
    let class_index = labels
        .iter()
        .position(|label| label == "class")
        .expect("class snippet should complete");

    assert_eq!(labels[0], "classAssets");
    assert!(class_index > 0, "{labels:?}");
}

#[test]
fn completes_love_asset_loader_globals() {
    let source = "local player = \n";
    let position = position_after(source, "= ");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("asset loader completion should succeed");

    for (label, detail) in [
        ("image", "function image(path: string): Image"),
        ("sound", "function sound(path: string): Source"),
        ("music", "function music(path: string): Source"),
        ("font", "function font(path: string, size: number): Font"),
        ("shader", "function shader(path: string): Shader"),
        ("imageData", "function imageData(path: string): ImageData"),
        (
            "canvas",
            "function canvas(width: number, height: number): Canvas",
        ),
        (
            "quad",
            "function quad(x: number, y: number, width: number, height: number, sw: number, sh: number): Quad",
        ),
        (
            "spriteBatch",
            "function spriteBatch(image: Image, size: number): SpriteBatch",
        ),
        (
            "particles",
            "function particles(image: Image, count: number): ParticleSystem",
        ),
    ] {
        let completion = completions
            .iter()
            .find(|completion| completion.label == label)
            .unwrap_or_else(|| panic!("{label} helper should complete"));

        assert_eq!(completion.kind, CompletionKind::Function);
        assert_eq!(completion.detail.as_deref(), Some(detail));
    }
}

#[test]
fn completes_extended_lua_global_items() {
    let underscore_source = "local value = _\n";
    let underscore = complete_tua_source(
        "src/main.tua",
        underscore_source,
        position_after(underscore_source, "_"),
    )
    .expect("underscore globals should complete");
    let raw_source = "local value = raw\n";
    let raw = complete_tua_source(
        "src/main.tua",
        raw_source,
        position_after(raw_source, "raw"),
    )
    .expect("raw globals should complete");
    let warn_source = "local value = wa\n";
    let warn = complete_tua_source(
        "src/main.tua",
        warn_source,
        position_after(warn_source, "wa"),
    )
    .expect("warn should complete");
    let arg_source = "local value = ar\n";
    let arg = complete_tua_source("src/main.tua", arg_source, position_after(arg_source, "ar"))
        .expect("arg should complete");

    assert!(labels(&underscore).contains(&"_ENV".to_owned()));
    assert!(labels(&underscore).contains(&"_G".to_owned()));
    assert!(labels(&raw).contains(&"rawlen".to_owned()));
    assert!(labels(&warn).contains(&"warn".to_owned()));
    assert!(labels(&arg).contains(&"arg".to_owned()));
}

#[test]
fn type_aliases_do_not_complete_as_runtime_values() {
    let source = "type Player = { hp: number }\nlocal value = Pl\n";
    let position = position_after(source, "Pl");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("runtime completion should succeed");

    assert!(!labels(&completions).contains(&"Player".to_owned()));
}

#[test]
fn completes_lua_standard_library_fields() {
    let source = "local value = math.si\n";
    let position = position_after(source, "math.si");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("Lua field completion should succeed");

    assert_eq!(labels(&completions), vec!["sin".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function math.sin(x: number): number")
    );
    assert_eq!(
        completions[0].documentation.as_deref(),
        Some("Returns the sine.")
    );
}

#[test]
fn completes_lua_fields_when_source_is_temporarily_invalid() {
    let source = "local broken =\nlocal value = math.si\n";
    let position = position_after(source, "math.si");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("lossy Lua completion should succeed");

    assert_eq!(labels(&completions), vec!["sin".to_owned()]);
}

#[test]
fn completes_love_fields() {
    let source = "love.graphics.pr\n";
    let position = position_after(source, "love.graphics.pr");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("LÖVE field completion should succeed");

    assert_eq!(
        labels(&completions),
        vec![
            "print".to_owned(),
            "printf".to_owned(),
            "present".to_owned()
        ]
    );
}

#[test]
fn completes_love_function_calls_as_signature_snippets() {
    let source = "love.graphics.cir\n";
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after(source, "love.graphics.cir"),
    )
    .expect("LÖVE function completion should succeed");
    let circle = completions
        .iter()
        .find(|completion| completion.label == "circle")
        .expect("circle should complete");

    assert_eq!(circle.kind, CompletionKind::Function);
    assert!(circle.is_snippet);
    assert_eq!(
        circle.insert_text.as_deref(),
        Some(r#"circle("${1|fill,line|}", ${2:x}, ${3:y}, ${4:radius})$0"#)
    );
    assert!(circle
        .documentation
        .as_deref()
        .unwrap_or_default()
        .contains("[Open LOVE wiki](https://love2d.org/wiki/love.graphics.circle)"));
}

#[test]
fn completes_love_callbacks_as_function_declaration_snippets() {
    let source = "function love.up\n";
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "up"))
        .expect("LOVE callback completion should succeed");
    let update = completions
        .iter()
        .find(|completion| completion.label == "update")
        .expect("update callback should complete");

    assert_eq!(update.kind, CompletionKind::Function);
    assert!(update.is_snippet);
    assert_eq!(update.insert_text.as_deref(), Some("update(dt)\n  $0\nend"));

    let expression_source = "local callback = love.up\n";
    let expression_completions = complete_tua_source(
        "src/main.tua",
        expression_source,
        position_after(expression_source, "up"),
    )
    .expect("LOVE expression completion should succeed");
    let expression_update = expression_completions
        .iter()
        .find(|completion| completion.label == "update")
        .expect("update should complete");

    assert!(!expression_update.is_snippet);
    assert_eq!(expression_update.insert_text, None);

    let generated_source = "function love.file\n";
    let generated_completions = complete_tua_source(
        "src/main.tua",
        generated_source,
        position_after(generated_source, "file"),
    )
    .expect("generated LOVE callback completion should succeed");
    let filedropped = generated_completions
        .iter()
        .find(|completion| completion.label == "filedropped")
        .expect("filedropped callback should complete");

    assert!(filedropped.is_snippet);
    assert_eq!(
        filedropped.insert_text.as_deref(),
        Some("filedropped(file)\n  $0\nend")
    );
    let documentation = filedropped.documentation.as_deref().unwrap_or_default();
    assert!(documentation.contains("**Callback parameters**"));
    assert!(documentation.contains("`file: DroppedFile`"));
    assert!(documentation.contains("The unopened File object"));
}

#[test]
fn completes_love_conf_config_fields_for_callback_parameter() {
    let source = r#"
function love.conf(t)
  local title = t.window.ti
  local physics = t.modules.ph
end
"#;
    let window = complete_tua_source(
        "src/conf.tua",
        source,
        position_after(source, "t.window.ti"),
    )
    .expect("love.conf window completion should succeed");
    let modules = complete_tua_source(
        "src/conf.tua",
        source,
        position_after(source, "t.modules.ph"),
    )
    .expect("love.conf modules completion should succeed");

    assert_eq!(labels(&window), vec!["title".to_owned()]);
    assert_eq!(window[0].detail.as_deref(), Some("field optional: string"));
    assert_eq!(
        labels(&modules),
        vec!["physics".to_owned(), "graphics".to_owned()]
    );
    assert_eq!(
        modules[0].detail.as_deref(),
        Some("field optional: boolean")
    );
}

#[test]
fn completes_love_api_generated_fields() {
    let modules = complete_tua_source(
        "src/main.tua",
        "love.\n",
        position_after("love.\n", "love."),
    )
    .expect("LÖVE module completion should succeed");
    let graphics = complete_tua_source(
        "src/main.tua",
        "love.graphics.newCan\n",
        position_after("love.graphics.newCan\n", "love.graphics.newCan"),
    )
    .expect("LÖVE graphics completion should succeed");
    let physics = complete_tua_source(
        "src/main.tua",
        "love.physics.newWo\n",
        position_after("love.physics.newWo\n", "love.physics.newWo"),
    )
    .expect("LÖVE physics completion should succeed");

    let module_labels = labels(&modules);
    for module in [
        "audio",
        "data",
        "event",
        "filesystem",
        "font",
        "graphics",
        "image",
        "joystick",
        "keyboard",
        "math",
        "mouse",
        "physics",
        "sound",
        "system",
        "thread",
        "timer",
        "touch",
        "video",
        "window",
    ] {
        assert!(
            module_labels.contains(&module.to_owned()),
            "{module_labels:?}"
        );
    }
    assert_eq!(labels(&graphics), vec!["newCanvas".to_owned()]);
    assert_eq!(labels(&physics), vec!["newWorld".to_owned()]);
}

#[test]
fn completes_generated_love_type_methods_from_module_catalog() {
    let source = r#"
local file = love.filesystem.newFile("save.txt")
file.op
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "file.op"))
            .expect("generated LÖVE type method completion should succeed");

    assert_eq!(labels(&completions), vec!["open".to_owned()]);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function File:open(mode: FileMode): boolean, string")
    );
}

#[test]
fn completes_love_image_methods_for_inferred_asset_loader_locals() {
    let source = r#"
local player = image "assets/player.png"
player.getWi
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getWi"))
        .expect("inferred image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getWidth".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Image:getWidth(): number")
    );
}

#[test]
fn love_object_method_dot_completion_prefers_colon_snippet() {
    let source = r#"
local player = image "assets/player.png"
player.setF
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "setF"))
        .expect("LOVE object method completion should succeed");
    let completion = completions
        .iter()
        .find(|completion| completion.label == "setFilter")
        .expect("setFilter should complete");

    assert_eq!(
        completion.insert_text.as_deref(),
        Some(r#"setFilter("${1|linear,nearest|}")$0"#)
    );
    assert!(completion.is_snippet);
    assert_eq!(completion.additional_text_edits.len(), 1);
    assert_eq!(completion.additional_text_edits[0].new_text, ":");
    assert_eq!(completion.additional_text_edits[0].span.start.line, 3);
    assert_eq!(completion.additional_text_edits[0].span.start.column, 7);
}

#[test]
fn completes_love_image_methods_for_direct_love_constructor_locals() {
    let source = r#"
local player = love.graphics.newImage("assets/player.png")
player:
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "player:"))
            .expect("direct LOVE constructor completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"getWidth".to_owned()), "{labels:?}");
    assert!(labels.contains(&"setFilter".to_owned()), "{labels:?}");
}

#[test]
fn completes_common_love_object_methods_for_direct_constructor_locals() {
    let canvas_source = r#"
local canvas = love.graphics.newCanvas()
canvas.getW
"#;
    let font_source = r#"
local font = love.graphics.newFont("assets/sniglet.otf", 16)
font.getW
"#;
    let quad_source = r#"
local quad = love.graphics.newQuad(0, 0, 16, 16, 64, 64)
quad.getV
"#;
    let data_source = r#"
local data = love.image.newImageData(16, 16)
data.setP
"#;
    let shader_source = r#"
local shader = love.graphics.newShader("void effect() {}")
shader.se
"#;
    let batch_source = r#"
local batch = love.graphics.newSpriteBatch(love.graphics.newImage("assets/player.png"))
batch.add
"#;
    let world_source = r#"
local world = love.physics.newWorld()
world.setG
"#;
    let body_source = r#"
local world = love.physics.newWorld()
local body = love.physics.newBody(world, 0, 0)
body.setP
"#;

    let canvas = complete_tua_source(
        "src/main.tua",
        canvas_source,
        position_after(canvas_source, "canvas.getW"),
    )
    .expect("Canvas completion should succeed");
    let font = complete_tua_source(
        "src/main.tua",
        font_source,
        position_after(font_source, "font.getW"),
    )
    .expect("Font completion should succeed");
    let quad = complete_tua_source(
        "src/main.tua",
        quad_source,
        position_after(quad_source, "quad.getV"),
    )
    .expect("Quad completion should succeed");
    let data = complete_tua_source(
        "src/main.tua",
        data_source,
        position_after(data_source, "data.setP"),
    )
    .expect("ImageData completion should succeed");
    let shader = complete_tua_source(
        "src/main.tua",
        shader_source,
        position_after(shader_source, "shader.se"),
    )
    .expect("Shader completion should succeed");
    let batch = complete_tua_source(
        "src/main.tua",
        batch_source,
        position_after(batch_source, "batch.add"),
    )
    .expect("SpriteBatch completion should succeed");
    let world = complete_tua_source(
        "src/main.tua",
        world_source,
        position_after(world_source, "world.setG"),
    )
    .expect("World completion should succeed");
    let body = complete_tua_source(
        "src/main.tua",
        body_source,
        position_after(body_source, "body.setP"),
    )
    .expect("Body completion should succeed");

    assert_eq!(
        labels(&canvas),
        vec!["getWrap".to_owned(), "getWidth".to_owned()]
    );
    assert_eq!(
        labels(&font),
        vec!["getWrap".to_owned(), "getWidth".to_owned()]
    );
    assert_eq!(labels(&quad), vec!["getViewport".to_owned()]);
    assert_eq!(labels(&data), vec!["setPixel".to_owned()]);
    assert!(labels(&shader).contains(&"send".to_owned()));
    assert_eq!(
        labels(&batch),
        vec!["add".to_owned(), "addLayer".to_owned()]
    );
    assert_eq!(labels(&world), vec!["setGravity".to_owned()]);
    assert_eq!(labels(&body), vec!["setPosition".to_owned()]);
}

#[test]
fn completes_every_generated_love_object_method_with_colon_rewrite() {
    let cases = love_object_methods_by_type();

    assert!(
        crate::love_api_signatures::object_method_cases().len() >= 1_000,
        "expected broad generated LOVE object method coverage"
    );

    for (type_name, methods) in cases {
        let source = format!("local receiver: {type_name} | nil = nil\nreceiver.\n");
        let completions = complete_tua_source(
            "src/main.tua",
            &source,
            position_after(&source, "receiver."),
        )
        .expect("LOVE object method completion should succeed");
        let labels = labels(&completions);

        for method in methods {
            let completion = completions
                .iter()
                .find(|completion| completion.label == method)
                .unwrap_or_else(|| panic!("{type_name}:{method} should complete; got {labels:?}"));

            assert_eq!(completion.kind, CompletionKind::Function);
            assert!(
                completion.detail.as_deref().is_some_and(
                    |detail| detail.starts_with(&format!("function {type_name}:{method}("))
                ),
                "{type_name}:{method} should use generated method detail, got {:?}",
                completion.detail
            );
            assert!(
                completion
                    .additional_text_edits
                    .iter()
                    .any(|edit| edit.new_text == ":"),
                "{type_name}:{method} dot completion should rewrite the receiver separator to `:`"
            );
        }
    }
}

#[test]
fn completes_love_image_methods_after_colon_access() {
    let source = r#"
local player = image "assets/player.png"
player:getWi
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getWi"))
        .expect("colon image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getWidth".to_owned()]);
}

#[test]
fn completes_love_image_methods_for_inferred_function_return_locals() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

player.getH
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "getH"))
        .expect("function-return image method completion should succeed");

    assert_eq!(labels(&completions), vec!["getHeight".to_owned()]);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Image:getHeight(): number")
    );
}

#[test]
fn completes_love_image_methods_after_empty_dot_for_inferred_function_return_locals() {
    let source = r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local function loadJumpSound()
  return sound "assets/jump.wav"
end

local player = loadPlayerImage()

player.

return {
  loadPlayerImage = loadPlayerImage,
  loadJumpSound = loadJumpSound,
}
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after_last(source, "player."),
    )
    .expect("empty-dot image method completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"getWidth".to_owned()), "{labels:?}");
    assert!(labels.contains(&"getHeight".to_owned()), "{labels:?}");
    assert!(labels.contains(&"setFilter".to_owned()), "{labels:?}");
}

#[test]
fn completes_love_source_methods_for_inferred_asset_loader_locals() {
    let source = r#"
local jump = sound "assets/jump.wav"
jump.pl
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "pl"))
        .expect("inferred source method completion should succeed");

    assert_eq!(labels(&completions), vec!["play".to_owned()]);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("function Source:play(): boolean")
    );
}

#[test]
fn completes_love_enum_values_in_method_string_arguments() {
    let source = r#"
local player = image "assets/player.png"
player:setFilter("")
"#;
    let completions = complete_tua_source(
        "src/main.tua",
        source,
        position_after(source, "setFilter(\""),
    )
    .expect("FilterMode value completion should succeed");

    assert_eq!(labels(&completions), vec!["linear", "nearest"]);
    assert!(completions
        .iter()
        .all(|completion| completion.kind == CompletionKind::Value));
    assert!(completions
        .iter()
        .all(|completion| completion.detail.as_deref() == Some("FilterMode")));
}

#[test]
fn filters_love_enum_values_by_string_prefix() {
    let source = r#"
local player = image "assets/player.png"
player:setFilter("li")
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "\"li"))
        .expect("FilterMode prefix completion should succeed");

    assert_eq!(labels(&completions), vec!["linear"]);
}

#[test]
fn completes_generated_love_enum_values_from_module_catalog() {
    let blend_source = r#"
love.graphics.setBlendMode("scr")
"#;
    let source_type_source = r#"
love.audio.newSource("assets/jump.wav", "str")
"#;
    let message_box_source = r#"
love.window.showMessageBox("Title", "Body", "warn")
"#;
    let key_vararg_source = r#"
love.keyboard.isDown("a", "b", "")
"#;

    let blend = complete_tua_source(
        "src/main.tua",
        blend_source,
        position_after(blend_source, "\"scr"),
    )
    .expect("generated BlendMode completion should succeed");
    let source_type = complete_tua_source(
        "src/main.tua",
        source_type_source,
        position_after(source_type_source, "\"str"),
    )
    .expect("generated SourceType completion should succeed");
    let message_box = complete_tua_source(
        "src/main.tua",
        message_box_source,
        position_after(message_box_source, "\"warn"),
    )
    .expect("generated MessageBoxType completion should succeed");
    let key_vararg = complete_tua_source(
        "src/main.tua",
        key_vararg_source,
        position_after(key_vararg_source, "b\", \""),
    )
    .expect("generated KeyConstant vararg completion should succeed");

    assert_eq!(labels(&blend), vec!["screen"]);
    assert_eq!(labels(&source_type), vec!["stream"]);
    assert_eq!(labels(&message_box), vec!["warning"]);
    assert!(labels(&key_vararg).contains(&"space".to_owned()));
}

#[test]
fn completes_every_generated_love_enum_parameter() {
    let cases = crate::love_api_signatures::enum_completion_cases();
    let mut failures = Vec::new();

    assert!(
        cases.len() >= 100,
        "expected broad generated LOVE enum coverage, got {} cases",
        cases.len()
    );

    for case in cases {
        let (source, position) = love_enum_completion_source(case);
        let expected = expected_love_enum_labels(case.enum_types);
        let completions = match complete_tua_source("src/main.tua", &source, position) {
            Ok(completions) => completions,
            Err(diagnostics) => {
                failures.push(format!(
                    "{} argument {} ({}) failed to complete: {diagnostics:?}\n{source}",
                    case.target,
                    case.parameter_index,
                    case.enum_types.join(" | ")
                ));
                continue;
            }
        };
        let actual = labels(&completions);
        let missing = expected
            .iter()
            .filter(|label| !actual.contains(label))
            .cloned()
            .collect::<Vec<_>>();

        if !missing.is_empty() {
            failures.push(format!(
                "{} argument {} ({}) missing {:?}; got {:?}\n{source}",
                case.target,
                case.parameter_index,
                case.enum_types.join(" | "),
                missing,
                actual
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "LOVE enum completion regressions:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn recovers_love_enum_values_for_typoed_string_arguments() {
    let source = r#"
function love.update(dt)
  love.graphics.circle(
    "fillter",
    100,
    100,
    2
  )
end
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after(source, "\"fillter"))
            .expect("DrawMode typo recovery completion should succeed");

    assert_eq!(labels(&completions), vec!["fill", "line"]);
    assert!(completions
        .iter()
        .all(|completion| completion.detail.as_deref() == Some("DrawMode")));
    assert!(completions
        .iter()
        .all(|completion| completion.filter_text.as_deref() == Some("fillter")));
}

#[test]
fn completes_lua_doc_literal_union_values_in_string_arguments() {
    let source = r#"
---@param mode "static" | "stream"
local function load(mode)
end

load("st")
"#;
    let completions =
        complete_tua_source("src/main.tua", source, position_after_last(source, "\"st"))
            .expect("literal union value completion should succeed");

    assert_eq!(labels(&completions), vec!["static", "stream"]);
    assert_eq!(completions[0].kind, CompletionKind::Value);
    assert_eq!(completions[0].detail.as_deref(), Some(r#""static""#));
}

#[test]
fn completes_tua_literal_union_and_enum_values_in_string_arguments() {
    let source = r#"
type LoadMode = "static" | "stream"

enum FilterMode
  "linear"
  "nearest"
end

local function load(mode: LoadMode)
end

local function setFilter(mode: FilterMode)
end

load("st")
setFilter("ne")
"#;

    let load_completions =
        complete_tua_source("src/main.tua", source, position_after_last(source, "\"st"))
            .expect("literal union completion should succeed");
    let filter_completions =
        complete_tua_source("src/main.tua", source, position_after_last(source, "\"ne"))
            .expect("enum completion should succeed");

    assert_eq!(labels(&load_completions), vec!["static", "stream"]);
    assert_eq!(labels(&filter_completions), vec!["nearest", "linear"]);
}

#[test]
fn completes_love_enum_values_in_incomplete_string_arguments() {
    let source = r#"
local player = image "assets/player.png"
player:setFilter("ne
"#;
    let completions = complete_tua_source("src/main.tua", source, position_after(source, "\"ne"))
        .expect("lossy FilterMode value completion should succeed");

    assert_eq!(labels(&completions), vec!["nearest"]);
}

#[test]
fn completes_local_lua_symbols() {
    let source = r#"local score = 0
local function spawn_player(x, y)
  local player = {}
  return sco
end
"#;
    let position = position_after(source, "return sco");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("local symbol completion should succeed");

    assert!(labels(&completions).contains(&"score".to_owned()));
}

#[test]
fn local_symbols_shadow_lua_globals_for_field_completion() {
    let source = "local math: any = {}\nlocal value = math.\n";
    let position = position_after(source, "math.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("shadowed Lua globals should stay dynamic");

    assert!(completions.is_empty());
}

#[test]
fn completes_require_paths_from_current_directory() {
    let root = temp_project_dir("require_completion");
    let src = root.join("src");
    let systems = src.join("systems");

    fs::create_dir_all(&systems).expect("test dirs should be created");
    fs::write(src.join("main.tua"), "local player = require(\"pla\")\n")
        .expect("main source should be written");
    fs::write(src.join("player.tua"), "return {}\n").expect("player source should be written");
    fs::write(systems.join("physics.lua"), "return {}\n").expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = fs::read_to_string(&main_path).expect("main source should be readable");
    let completions = complete_tua_source(
        &main_path,
        &source,
        position_after(&source, "require(\"pla"),
    )
    .expect("require path completion should succeed");

    assert_eq!(labels(&completions), vec!["player".to_owned()]);

    let source = "local physics = require(\"systems.\")\n";
    let completions = complete_tua_source(&main_path, source, position_after(source, "systems."))
        .expect("nested require path completion should succeed");

    assert_eq!(labels(&completions), vec!["systems.physics".to_owned()]);

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_import_paths_for_child_and_project_modules() {
    let root = temp_project_dir("import_completion");
    let src = root.join("src");
    let systems = src.join("systems");
    let shared = src.join("shared");

    fs::create_dir_all(&systems).expect("systems dir should be created");
    fs::create_dir_all(&shared).expect("shared dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        systems.join("main.tua"),
        "local player = import(\"./pla\")\n",
    )
    .expect("main source should be written");
    fs::write(systems.join("player.tua"), "return {}\n").expect("player source should be written");
    fs::write(shared.join("math.lua"), "return {}\n").expect("Lua module should be written");
    fs::write(shared.join("init.tua"), "return {}\n").expect("init module should be written");

    let main_path = systems.join("main.tua");
    let source = fs::read_to_string(&main_path).expect("main source should be readable");
    let completions = complete_tua_source(
        &main_path,
        &source,
        position_after(&source, "import(\"./pla"),
    )
    .expect("import path completion should succeed");

    assert_eq!(labels(&completions), vec!["./player".to_owned()]);

    let source = "local module = import(\"sha\")\n";
    let completions =
        complete_tua_source(&main_path, source, position_after(source, "import(\"sha"))
            .expect("project import path completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["shared".to_owned(), "shared.math".to_owned()]
    );

    let source = "local shared = import(\"../shared\")\n";
    let completions = complete_tua_source(&main_path, source, position_after(source, "../shared"))
        .expect("parent import path completion should succeed");

    assert!(completions.is_empty());

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_fields_from_required_tua_and_lua_modules() {
    let root = temp_project_dir("required_module_fields");
    let src = root.join("src");
    let game = root.join("game");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::create_dir_all(&game).expect("game dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\nout = \"game\"\n")
        .expect("config should be written");
    fs::write(
        src.join("player.tua"),
        r#"
type Player = {
  hp?: number,
  pos?: { x: number },
}

local player: Player = {}

function player.damage(amount: number)
end

return player
"#,
    )
    .expect("Tua module should be written");
    fs::write(
        game.join("utils.lua"),
        r#"
local utils = {}

function utils.clamp(value, min, max)
end

utils.version = "1"

return utils
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local player: any = require("player")
local hp = player.hp

local utils = require("utils")
local value = utils.clamp
"#;

    let player_completions =
        complete_tua_source(&main_path, source, position_after(source, "player."))
            .expect("required Tua module completion should succeed");
    let utils_completions =
        complete_tua_source(&main_path, source, position_after(source, "utils."))
            .expect("required Lua module completion should succeed");

    assert_eq!(
        labels(&player_completions),
        vec!["hp".to_owned(), "damage".to_owned(), "pos".to_owned()]
    );
    assert_eq!(
        labels(&utils_completions),
        vec!["clamp".to_owned(), "version".to_owned()]
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_nested_fields_from_required_modules() {
    let root = temp_project_dir("nested_required_module_fields");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("settings.lua"),
        r#"
return {
  audio = {
    volume = 1,
    muted = false,
  },
}
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local settings = require("settings")
local volume = settings.audio.volume
"#;
    let completions = complete_tua_source(
        &main_path,
        source,
        position_after(source, "settings.audio."),
    )
    .expect("nested required module completion should succeed");

    assert_eq!(
        labels(&completions),
        vec!["volume".to_owned(), "muted".to_owned()]
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_required_module_fields_with_lua_doc_documentation() {
    let root = temp_project_dir("annotated_required_module_completion");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("src dir should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("audio.lua"),
        r#"---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    )
    .expect("Lua module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local Audio = require("audio")
local value = Audio.play
"#;
    let completions = complete_tua_source(&main_path, source, position_after(source, "Audio."))
        .expect("annotated module completion should succeed");

    assert_eq!(labels(&completions), vec!["play".to_owned()]);
    assert_eq!(completions[0].kind, CompletionKind::Function);
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("field: (source: string) -> boolean")
    );
    assert_eq!(
        completions[0].documentation.as_deref(),
        Some("audio source\nsuccess flag")
    );

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_love_methods_for_required_module_function_return_locals() {
    let root = temp_project_dir("required_module_return_completion");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("test dirs should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("assets.tua"),
        r#"
local function loadJumpSound()
  return sound "assets/jump.wav"
end

return {
  loadJumpSound = loadJumpSound,
}
"#,
    )
    .expect("assets module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local Assets = require("assets")

local a = Assets.loadJumpSound()

a:
"#;
    let completions = complete_tua_source(&main_path, source, position_after(source, "a:"))
        .expect("required module return completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"play".to_owned()), "{labels:?}");
    assert!(labels.contains(&"setVolume".to_owned()), "{labels:?}");
    assert!(labels.contains(&"stop".to_owned()), "{labels:?}");

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completes_required_module_fields_through_copied_module_alias() {
    let root = temp_project_dir("required_module_alias_copy_completion");
    let src = root.join("src");

    fs::create_dir_all(&src).expect("test dirs should be created");
    fs::write(root.join("tua.toml"), "source = \"src\"\n").expect("config should be written");
    fs::write(
        src.join("assets.tua"),
        r#"
local function loadJumpSound()
  return sound "assets/jump.wav"
end

return {
  loadJumpSound = loadJumpSound,
}
"#,
    )
    .expect("assets module should be written");

    let main_path = src.join("main.tua");
    let source = r#"
local Assets = require("assets")
local a = Assets

a.
"#;
    let completions = complete_tua_source(&main_path, source, position_after(source, "a."))
        .expect("copied module alias completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"loadJumpSound".to_owned()), "{labels:?}");

    fs::remove_dir_all(root).expect("test dir should be removed");
}

#[test]
fn completion_remains_quiet_for_any_values() {
    let source = format!("{SOURCE}local dynamic = payload.\n");
    let position = position_after(&source, "payload.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("completion should not fail for any values");

    assert!(completions.is_empty());
}

#[test]
fn completion_remains_quiet_for_unknown_dynamic_values() {
    let source = "local value = missing.\n";
    let position = position_after(source, "missing.");
    let completions = complete_tua_source("src/main.tua", source, position)
        .expect("completion should not fail for unknown values");

    assert!(completions.is_empty());
}

#[test]
fn completion_includes_index_fields_but_hides_metamethods() {
    let index_shape =
        crate::TableShape::from_fields([("length", crate::TableField::required(Type::number()))]);
    let add = crate::FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let meta = Type::table([
        (
            "__index",
            crate::TableField::required(Type::Table(index_shape)),
        ),
        ("__add", crate::TableField::required(Type::Function(add))),
    ]);
    let table =
        crate::TableShape::from_fields([("x", crate::TableField::required(Type::number()))])
            .with_metatable_facts(crate::types::MetatableFacts::from_type(&meta));
    let parsed =
        crate::parse_tua_source("src/main.tua", "").expect("empty source should parse for context");
    let environment = TypeEnvironment::from_parsed(&parsed)
        .expect("empty source should build a type environment");
    let mut fields = std::collections::BTreeMap::new();

    collect_shape_fields(&environment, &table, &mut fields);

    assert!(fields.contains_key("x"), "{fields:?}");
    assert!(fields.contains_key("length"), "{fields:?}");
    assert!(!fields.contains_key("__add"), "{fields:?}");
    assert!(!fields.contains_key("__index"), "{fields:?}");
}

#[test]
fn completes_fields_from_visible_index_function_facts() {
    let source = r#"
local meta = {
  __index = function(_, key)
    if key == "length" then
      return 0
    end

    return nil
  end,
}

local vectorA = { x = 10, y = 20 }
setmetatable(vectorA, meta)

vectorA.
"#;
    let completions =
        complete_tua_source("src/vector.tua", source, position_after(source, "vectorA."))
            .expect("__index function completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"length".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"__index".to_owned()), "{labels:?}");
}

#[test]
fn completes_self_index_class_style_metatable_methods() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:withdraw(amount)
end

local account = setmetatable({}, Account)

account:
"#;
    let completions = complete_tua_source(
        "src/account.tua",
        source,
        position_after(source, "account:"),
    )
    .expect("self-index metatable completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"withdraw".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"__index".to_owned()), "{labels:?}");
}

#[test]
fn completes_self_index_constructor_instance_methods() {
    let source = r#"
local Account = {}
Account.__index = Account

function Account:new(balance)
  local instance = { balance = balance }
  return setmetatable(instance, self)
end

function Account:withdraw(amount)
end

local account = Account:new(100)

account:
"#;
    let completions = complete_tua_source(
        "src/account.tua",
        source,
        position_after(source, "account:"),
    )
    .expect("constructor instance completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"withdraw".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"__index".to_owned()), "{labels:?}");
}

#[test]
fn completes_metatable_index_chain_inherited_methods() {
    let source = r#"
local Base = {}
Base.__index = Base

function Base:describe(): string
  return "base"
end

local Mid = {}
Mid.__index = Mid
setmetatable(Mid, { __index = Base })

function Mid:level(): number
  return 2
end

local Leaf = {}
Leaf.__index = Leaf
setmetatable(Leaf, { __index = Mid })

local leaf = setmetatable({ name = "sprout" }, Leaf)

leaf:
"#;
    let completions = complete_tua_source(
        "src/inheritance.tua",
        source,
        position_after(source, "leaf:"),
    )
    .expect("inherited method completion should succeed");
    let labels = labels(&completions);

    assert!(labels.contains(&"describe".to_owned()), "{labels:?}");
    assert!(labels.contains(&"level".to_owned()), "{labels:?}");
    assert!(!labels.contains(&"__index".to_owned()), "{labels:?}");
}

fn complete_at(needle: &str) -> Result<Vec<Completion>, Vec<Diagnostic>> {
    complete_tua_source("src/main.tua", SOURCE, position_after(SOURCE, needle))
}

fn labels(completions: &[Completion]) -> Vec<String> {
    completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect()
}

fn love_enum_completion_source(
    case: &crate::love_api_signatures::LoveEnumCompletionCase,
) -> (String, SourcePosition) {
    const CURSOR: &str = "__tua_enum_completion_cursor__";

    let arguments = (0..=case.parameter_index)
        .map(|index| {
            if index == case.parameter_index {
                format!("\"{CURSOR}\"")
            } else {
                "nil".to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let source = if let Some((type_name, method)) = case.target.split_once(':') {
        format!("local receiver: {type_name} = nil\nreceiver:{method}({arguments})\n")
    } else {
        format!("{}({arguments})\n", case.target)
    };
    let position = position_at(&source, CURSOR);
    let source = source.replace(CURSOR, "");

    (source, position)
}

fn expected_love_enum_labels(enum_types: &[&str]) -> Vec<String> {
    let mut labels = Vec::new();

    for type_name in enum_types {
        for item in crate::lua::enum_value_items(type_name, "") {
            if !labels.iter().any(|label| label == item.label) {
                labels.push(item.label.to_owned());
            }
        }
    }

    labels
}

fn love_object_methods_by_type() -> std::collections::BTreeMap<&'static str, Vec<&'static str>> {
    let mut cases: std::collections::BTreeMap<&'static str, Vec<&'static str>> =
        std::collections::BTreeMap::new();

    for case in crate::love_api_signatures::object_method_cases() {
        let methods = cases.entry(case.type_name).or_default();

        if !methods.contains(&case.method_name) {
            methods.push(case.method_name);
        }
    }

    cases
}

fn position_after(source: &str, needle: &str) -> SourcePosition {
    let start = position_at(source, needle);

    SourcePosition {
        line: start.line,
        column: start.column + needle.chars().count(),
    }
}

fn position_after_last(source: &str, needle: &str) -> SourcePosition {
    let end = source.rfind(needle).expect("needle should exist in source") + needle.len();
    let prefix = &source[..end];
    let line = prefix
        .chars()
        .filter(|character| *character == '\n')
        .count()
        + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, line)| line)
        .chars()
        .count()
        + 1;

    SourcePosition { line, column }
}

fn temp_project_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();

    path.push(format!("tua_{name}_{}", std::process::id()));

    if path.exists() {
        fs::remove_dir_all(&path).expect("stale test dir should be removed");
    }

    path
}
