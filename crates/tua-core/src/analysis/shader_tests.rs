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
fn shader_locals_assigned_in_another_function_share_editor_features_and_checks() {
    let temp = TempProject::new("shader-load-draw");
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect: any
function love.load()
  effect = love.graphics.newShader("effect.glsl")
end
function love.draw()
  effect:send("<|draw|>")
  effect:send("tnit", {})
end
"#,
    );
    let host = shader_host(&temp, &fixture);
    let snapshot = host.snapshot();
    assert_eq!(
        snapshot
            .completion("main.tua", fixture.position("draw"))
            .len(),
        5
    );
    let diagnostics = snapshot.diagnostics("main.tua");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.line == 7 && d.code == Some(DiagnosticCode::ASSET_SHADER_UNIFORM)),
        "{diagnostics:#?}"
    );

    // Competing or unknown assignments leave the receiver unresolved.
    for other in ["{}", "love.graphics.newShader(\"effect.glsl\")"] {
        let fixture = Fixture::parse(&format!(
            "-- file: main.tua\nlocal effect: any\nfunction love.load()\n  effect = love.graphics.newShader(\"effect.glsl\")\nend\nfunction reset()\n  effect = {other}\nend\nfunction love.draw()\n  effect:send(\"<|draw|>\")\nend\n"
        ));
        let host = shader_host(&temp, &fixture);
        assert!(
            host.snapshot()
                .completion("main.tua", fixture.position("draw"))
                .iter()
                .all(|item| item.label != "tint"),
            "{other}"
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

#[test]
fn unsent_shader_uniform_lint_is_opt_in_and_tracks_project_sends() {
    let temp = TempProject::new("shader-unsent");
    let fixture = Fixture::parse("-- file: main.tua\nlocal effect = shader \"effect.glsl\"\nlove.graphics.setShader(effect)\n");
    let mut host = shader_host(&temp, &fixture);
    let unsent = |host: &AnalysisHost| {
        host.snapshot()
            .diagnostics("main.tua")
            .into_iter()
            .filter(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_UNSENT_UNIFORM))
            .collect::<Vec<_>>()
    };
    assert!(unsent(&host).is_empty());
    let mut config = host.config().clone();
    config.shaders.warn_unsent = true;
    host.set_config(config);
    let warnings = unsent(&host);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    for name in ["alpha", "ripple1", "ripple2", "tint"] {
        assert!(warnings[0].message.contains(name));
    }
    host.set_file("other.tua", "local other = shader \"effect.glsl\"\nother:send(\"alpha\", 1)\nother:sendColor(\"tint\", {})\nother:send(\"ripple\" .. i, {})\n");
    assert!(
        unsent(&host).is_empty(),
        "sends in another file and static patterns count"
    );
    host.set_file(
        "other.tua",
        "local other = shader \"effect.glsl\"\nreturn other\n",
    );
    assert!(
        unsent(&host).is_empty(),
        "escaping in another file suppresses the asset"
    );
    host.set_file("other.tua", "");
    assert_eq!(
        unsent(&host).len(),
        1,
        "new snapshots invalidate aggregate usage"
    );
}

#[test]
fn unsent_shader_uniform_lint_suppresses_dynamic_or_escaped_uses() {
    for suffix in [
        "return effect",
        "unknown(effect)",
        "local alias = effect",
        "effect = other",
        "effect:send(dynamic, 1)",
        "effect:unknown()",
        "local another = shader path",
        "local effect = 1",
        "holder.effect = effect",
        "local current = love.graphics.getShader()",
    ] {
        let temp = TempProject::new("shader-unsent-escape");
        let fixture = Fixture::parse(&format!(
            "-- file: main.tua\nlocal effect = shader \"effect.glsl\"\n{suffix}\n"
        ));
        let mut host = shader_host(&temp, &fixture);
        let mut config = host.config().clone();
        config.shaders.warn_unsent = true;
        host.set_config(config);
        assert!(
            !host
                .snapshot()
                .diagnostics("main.tua")
                .iter()
                .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_UNSENT_UNIFORM)),
            "{suffix}"
        );
    }
}

#[test]
fn unsent_shader_uniform_lint_ignores_initializers_and_unread_uniforms() {
    let temp = TempProject::new("shader-unsent-initialized");
    let fixture = Fixture::parse("-- file: main.tua\nlocal effect = shader \"effect.glsl\"\nlove.graphics.setShader(effect)\n");
    let mut host = shader_host(&temp, &fixture);
    temp.write("effect.glsl", "uniform float scale = 1.0;\nextern number unread;\nvec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c * scale; }\n");
    host.refresh_asset_index_from_disk();
    let mut config = host.config().clone();
    config.shaders.warn_unsent = true;
    host.set_config(config);
    assert!(!host
        .snapshot()
        .diagnostics("main.tua")
        .iter()
        .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_UNSENT_UNIFORM)));
}

#[test]
fn inline_shader_queries_and_checks_share_literal_source_locations() {
    let mut host = AnalysisHost::new();
    let fixture = Fixture::parse(
        r#"-- file: main.tua
local effect = love.graphics.newShader([=[
extern number intensity;
vec4 effect(vec4 c, Image t, vec2 uv, vec2 p) { return c * intensity; }
]=])
effect:send("inten<|uniform|>sity", {})
effect:send("<|complete|>", 1)
"#,
    );
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    assert_eq!(
        completion_labels(&snapshot.completion("main.tua", fixture.position("complete"))),
        ["intensity"]
    );
    let hover = snapshot
        .hover("main.tua", fixture.position("uniform"))
        .unwrap();
    assert!(hover.contents.contains("extern number intensity"));
    let definitions = snapshot.definitions("main.tua", fixture.position("uniform"));
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].path, PathBuf::from("main.tua"));
    assert_eq!(
        definitions[0].span.start,
        SourcePosition {
            line: 2,
            column: 15
        }
    );
    let diagnostics = snapshot.diagnostics("main.tua");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_VALUE)),
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_MISSING)),
        "{diagnostics:?}"
    );
    let requests = snapshot
        .inline_shader_validation_requests(|_| crate::ShadersConfig::default())
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].source_origin,
        Some(SourcePosition { line: 2, column: 1 })
    );
    let errors = crate::shader_validation::validation_diagnostics(
        &requests,
        &[crate::shader_validation::ShaderValidationOutcome::Invalid(
            vec![crate::shader_validation::ShaderError {
                stage: None,
                line: Some(2),
                message: "bad GLSL".into(),
            }],
        )],
    );
    assert_eq!(errors[0].file, PathBuf::from("main.tua"));
    assert_eq!(errors[0].line, 3);
}

#[test]
fn inline_shaders_respect_crlf_initial_newlines_offsets_and_unsaved_changes() {
    let mut host = AnalysisHost::new();
    host.set_file("main.tua", "local s = love.graphics.newShader([[\r\nextern number n;\r\nvec4 effect() { return vec4(n); }\r\n]])\r\ns:send(\"n\", 1)\r\n");
    let snapshot = host.snapshot();
    let definitions = snapshot.definitions("main.tua", SourcePosition { line: 5, column: 9 });
    assert_eq!(
        definitions[0].span.start,
        SourcePosition {
            line: 2,
            column: 15
        }
    );
    host.set_file("main.tua", "local s = love.graphics.newShader([[extern number x; vec4 effect() { return vec4(x); }]])\ns:send(\"x\", 1)\n");
    let snapshot = host.snapshot();
    let definitions = snapshot.definitions("main.tua", SourcePosition { line: 2, column: 9 });
    assert_eq!(
        definitions[0].span.start,
        SourcePosition {
            line: 1,
            column: 51
        }
    );
    let requests = snapshot
        .inline_shader_validation_requests(|_| crate::ShadersConfig::default())
        .unwrap();
    assert!(requests[0].source.contains("number x"));
    assert_eq!(
        requests[0].source_origin,
        Some(SourcePosition {
            line: 1,
            column: 37
        })
    );
}

#[test]
fn inline_shader_missing_entry_point_and_shadowing_are_conservative() {
    let mut host = AnalysisHost::new();
    host.set_file("main.tua", "local s = love.graphics.newShader([[extern number n;]])\nlocal function f(s: any) s:send(\"unknown\", {}) end\n");
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("main.tua");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_ENTRY_POINT)),
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_SHADER_UNIFORM)),
        "{diagnostics:?}"
    );
    assert!(snapshot
        .inline_shader_validation_requests(|_| crate::ShadersConfig::default())
        .unwrap()
        .is_empty());
}

#[test]
fn inline_shader_completion_survives_an_unfinished_send_and_lua_uses_same_model() {
    for path in ["main.tua", "main.lua"] {
        let mut host = AnalysisHost::new();
        let source = "local s = love.graphics.newShader([[extern number strength; vec4 effect() { return vec4(strength); }]])\ns:send(\"str";
        host.set_file(path, source);
        let items = host.snapshot().completion(
            path,
            SourcePosition {
                line: 2,
                column: 12,
            },
        );
        assert_eq!(completion_labels(&items), ["strength"], "{path}");
    }
}

#[test]
fn inline_shader_multi_source_calls_are_not_mistaken_for_asset_paths() {
    let mut host = AnalysisHost::new();
    host.set_file("main.tua", "local s = love.graphics.newShader([[// pixel\nvec4 effect() {}]], [[// vertex\nvec4 position() {}]])\nprint(s)\n");
    let snapshot = host.snapshot();
    assert!(!snapshot
        .diagnostics("main.tua")
        .iter()
        .any(|d| d.code == Some(crate::DiagnosticCode::ASSET_MISSING)));
    assert!(snapshot
        .inline_shader_validation_requests(|_| crate::ShadersConfig::default())
        .unwrap()
        .is_empty());
}
