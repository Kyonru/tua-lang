use super::*;

const SHADER: &str = "extern number alpha;\nextern vec4 tint;\nextern vec3 unused;\nextern vec4 ripple1, ripple2;\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return tint * alpha + ripple1 + ripple2; }\n";
fn shader_host(temp: &TempProject, fixture: &Fixture) -> AnalysisHost {
    temp.write("effect.glsl", SHADER);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().to_path_buf()]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    host.refresh_asset_index_from_disk();
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    host
}

#[test]
fn shader_uniform_completion_ranks_and_filters_names() {
    let temp = TempProject::new("shader-completion");
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect = shader "effect.glsl"
effect:send("<|all|>")
effect:sendColor("<|color|>")
effect:send("ri<|prefix|>" .. i)
"#,
    );
    let host = shader_host(&temp, &fixture);
    let snapshot = host.snapshot();
    let all = snapshot.completion("main.tua", fixture.position("all"));
    assert_eq!(
        completion_labels(&all),
        ["alpha", "ripple1", "ripple2", "tint", "unused"]
    );
    assert_eq!(all.last().unwrap().detail.as_deref(), Some("vec3 (unread)"));
    let color = snapshot.completion("main.tua", fixture.position("color"));
    assert_eq!(
        completion_labels(&color),
        ["ripple1", "ripple2", "tint", "alpha", "unused"]
    );
    let prefix = snapshot.completion("main.tua", fixture.position("prefix"));
    assert_eq!(completion_labels(&prefix), ["ripple1", "ripple2"]);
    assert!(prefix
        .iter()
        .all(|item| item.insert_text.as_deref() == Some("ripple")));
}

#[test]
fn shader_uniform_hover_and_definitions_share_stage_metadata() {
    let temp = TempProject::new("shader-navigation");
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect = love.graphics.newShader("effect.glsl")
effect:send("ti<|hover|>nt", {})
effect:send("rip<|pattern|>ple" .. i, {})
effect:send("un<|unused|>used", {})
"#,
    );
    let mut host = shader_host(&temp, &fixture);
    let shader = temp.write(
        "effect.glsl",
        format!("{SHADER}\n#ifdef VERTEX\nextern vec4 tint;\n#endif\n"),
    );
    host.refresh_asset_index_from_disk();
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("main.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("extern vec4 tint;"));
    assert!(hover.contents.contains("at least 4 numeric components"));
    assert!(hover.contents.contains("effect.glsl:2"));
    let definitions = snapshot.definitions("main.tua", fixture.position("hover"));
    assert_eq!(definitions.len(), 2);
    assert!(definitions
        .iter()
        .all(|definition| definition.path == shader && definition.kind == DefinitionKind::Asset));
    assert_eq!(
        definitions[0].span.start,
        SourcePosition {
            line: 2,
            column: 13
        }
    );
    let pattern = snapshot
        .hover("main.tua", fixture.position("pattern"))
        .unwrap();
    assert!(pattern.contents.contains("ripple1") && pattern.contents.contains("ripple2"));
    assert!(snapshot
        .hover("main.tua", fixture.position("unused"))
        .unwrap()
        .contents
        .contains("Unread uniform"));
}

#[test]
fn shader_uniform_cache_tracks_disk_and_overlay_changes_without_request_io() {
    let temp = TempProject::new("shader-cache");
    let fixture = Fixture::parse(
        "-- file: main.tua\nlocal effect = shader \"effect.glsl\"\neffect:send(\"<|complete|>\")\n",
    );
    let mut host = shader_host(&temp, &fixture);
    let path = temp.path().join("effect.glsl");
    let before = host.snapshot();
    temp.write("effect.glsl", "extern number replacement;");
    assert_eq!(
        before
            .completion("main.tua", fixture.position("complete"))
            .len(),
        5
    );
    host.refresh_asset_index_from_disk();
    assert_eq!(
        completion_labels(
            &host
                .snapshot()
                .completion("main.tua", fixture.position("complete"))
        ),
        ["replacement"]
    );
    host.set_shader_source_overlay(&path, Some("extern vec2 overlay;".into()));
    assert_eq!(
        completion_labels(
            &host
                .snapshot()
                .completion("main.tua", fixture.position("complete"))
        ),
        ["overlay"]
    );
    host.refresh_asset_index_from_disk();
    assert_eq!(
        completion_labels(
            &host
                .snapshot()
                .completion("main.tua", fixture.position("complete"))
        ),
        ["overlay"]
    );
    host.set_shader_source_overlay(&path, None);
    assert_eq!(
        completion_labels(
            &host
                .snapshot()
                .completion("main.tua", fixture.position("complete"))
        ),
        ["replacement"]
    );
}

#[test]
fn shader_uniform_features_respect_shadowing_and_unknown_receivers() {
    let temp = TempProject::new("shader-shadow");
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect = shader "effect.glsl"
do
  local effect = {}
  effect:send("<|shadow|>")
end
effect:send("<|outer|>")
unknown:send("<|unknown|>")
"#,
    );
    let host = shader_host(&temp, &fixture);
    let snapshot = host.snapshot();
    assert!(!snapshot
        .completion("main.tua", fixture.position("shadow"))
        .iter()
        .any(|item| item.label == "tint"));
    assert!(!snapshot
        .completion("main.tua", fixture.position("unknown"))
        .iter()
        .any(|item| item.label == "tint"));
    assert_eq!(
        snapshot
            .completion("main.tua", fixture.position("outer"))
            .len(),
        5
    );
}

#[test]
fn shader_uniform_completion_defers_parameters_and_reassignments() {
    let temp = TempProject::new("shader-reassigned");
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect: any = shader "effect.glsl"
local function draw(effect: any)
  effect:send("<|parameter|>")
end
effect = {}
effect:send("<|reassigned|>")
"#,
    );
    let host = shader_host(&temp, &fixture);
    for marker in ["parameter", "reassigned"] {
        assert!(
            !host
                .snapshot()
                .completion("main.tua", fixture.position(marker))
                .iter()
                .any(|item| item.label == "tint"),
            "{marker}"
        );
    }
}

#[test]
fn shader_uniform_completion_works_while_a_call_is_unfinished() {
    let temp = TempProject::new("shader-incomplete");
    let fixture = Fixture::parse(
        "-- file: main.tua\nlocal effect = shader \"effect.glsl\"\neffect:send(\"ti<|complete|>",
    );
    let host = shader_host(&temp, &fixture);
    assert_eq!(
        completion_labels(
            &host
                .snapshot()
                .completion("main.tua", fixture.position("complete"))
        ),
        ["tint"]
    );
}

#[test]
fn shader_uniform_completion_only_targets_method_name_arguments() {
    let temp = TempProject::new("shader-arguments");
    let fixture = Fixture::parse(
        "-- file: main.tua\nlocal effect = shader \"effect.glsl\"\neffect.send(\"<|dot|>\")\neffect:send(\"tint\", \"<|value|>\")\n",
    );
    let host = shader_host(&temp, &fixture);
    for marker in ["dot", "value"] {
        assert!(!host
            .snapshot()
            .completion("main.tua", fixture.position(marker))
            .iter()
            .any(|item| item.label == "tint"));
    }
}

#[test]
fn shader_uniform_checker_uses_the_same_overlay_as_completion() {
    let temp = TempProject::new("shader-checker-overlay");
    let fixture = Fixture::parse(
        "-- file: main.tua\nlocal effect = shader \"effect.glsl\"\neffect:send(\"ti<|name|>nt\", {})\n",
    );
    let mut host = shader_host(&temp, &fixture);
    let missing = |snapshot: AnalysisSnapshot| {
        snapshot
            .diagnostics("main.tua")
            .iter()
            .any(|d| d.message.contains("has no uniform"))
    };
    assert!(!missing(host.snapshot()));
    host.set_shader_source_overlay(
        temp.path().join("effect.glsl"),
        Some("extern number replacement;".into()),
    );
    assert!(missing(host.snapshot()));
    assert!(host
        .snapshot()
        .completion("main.tua", fixture.position("name"))
        .iter()
        .all(|item| item.label != "tint"));
    host.set_shader_source_overlay(temp.path().join("effect.glsl"), None);
    assert!(!missing(host.snapshot()));
}

#[test]
fn shader_value_checks_use_project_module_return_types() {
    for extension in ["tua", "lua"] {
        let temp = TempProject::new("shader-value-module");
        let fixture = Fixture::parse(&format!(
            "-- file: main.tua\nlocal values = require(\"values\")\nlocal effect = shader \"effect.glsl\"\neffect:send(\"tint\", values.scalar())\neffect:send(\"tint\", values.vector())\n-- file: values.{extension}\nlocal M = {{}}\nfunction M.scalar() return 1 end\nfunction M.vector() return {{ 1, 1, 1, 1 }} end\nreturn M\n"
        ));
        temp.write("effect.glsl", SHADER);
        let mut host = AnalysisHost::new();
        host.set_source_roots([temp.path().to_path_buf()]);
        host.set_asset_roots([temp.path().to_path_buf()]);
        for file in fixture.files() {
            let path = temp.write(file.path.to_str().unwrap(), &file.source);
            let kind = if path.extension().is_some_and(|e| e == "lua") {
                AnalysisSourceKind::Lua
            } else {
                AnalysisSourceKind::Tua
            };
            host.set_file_with_kind(path, file.source.clone(), kind);
        }
        host.refresh_asset_index_from_disk();
        let diagnostics = host.snapshot().diagnostics(temp.path().join("main.tua"));
        let lines: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == Some(DiagnosticCode::ASSET_SHADER_VALUE))
            .map(|d| d.line)
            .collect();
        assert_eq!(lines, [3], "{extension}: {diagnostics:#?}");
    }
}
