use super::*;
use crate::{DiagnosticCode, DiagnosticSeverity, Fixture, SemanticTokenModifier};
use std::env;
use std::fs;

#[test]
fn snapshots_share_parsed_and_last_good_analysis_storage() {
    let mut host = AnalysisHost::new();
    let path = PathBuf::from("main.tua");
    host.set_file(&path, "local value: number = 1\nreturn value\n");
    host.snapshot();

    let cached_file = host
        .cache
        .borrow()
        .files
        .get(&path)
        .expect("analysis file should be cached")
        .file
        .clone();
    let parsed = cached_file
        .parsed
        .as_ref()
        .expect("source should parse successfully");
    let cache = host.cache.borrow();
    let cached_parse = cache
        .parsed_files
        .values()
        .next()
        .expect("parsed source should be cached")
        .as_ref()
        .as_ref()
        .expect("cached source should parse successfully");
    assert!(Arc::ptr_eq(parsed, cached_parse));
    drop(cache);

    let last_good = host.last_good_files.borrow();
    let last_good_file = last_good
        .get(&path)
        .expect("valid file should be retained for recovery");
    assert!(Arc::ptr_eq(&cached_file.0, &last_good_file.0));
    drop(last_good);

    host.open_document(&path, "local value: number = \n");
    host.snapshot();
    let last_good = host.last_good_files.borrow();
    let retained_file = last_good
        .get(&path)
        .expect("invalid edit should preserve the previous valid file");
    assert!(Arc::ptr_eq(&cached_file.0, &retained_file.0));
}

#[test]
fn cached_snapshots_share_large_immutable_project_indexes() {
    let mut host = AnalysisHost::new();
    host.set_file("main.tua", "return 1\n");
    let snapshot = host.snapshot();
    let cached_snapshot = host
        .cached_snapshot()
        .expect("snapshot should be cached for the current revision");

    assert!(Arc::ptr_eq(&snapshot.vfs, &cached_snapshot.vfs));
    assert!(Arc::ptr_eq(
        &snapshot.call_hierarchy_symbols,
        &cached_snapshot.call_hierarchy_symbols
    ));
    assert!(Arc::ptr_eq(
        &snapshot.call_hierarchy_calls,
        &cached_snapshot.call_hierarchy_calls
    ));
}

#[test]
fn snapshot_reports_invalid_primitive_operator() {
    let mut host = AnalysisHost::new();
    host.set_file(
        "operators.tua",
        "local enabled: boolean = true\nlocal n: number = enabled + 1\n",
    );
    assert!(host
        .snapshot()
        .diagnostics("operators.tua")
        .iter()
        .any(|d| d.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)
            && d.message.contains("operator")));
}

#[test]
fn busted_types_are_available_only_in_test_sources_across_editor_surfaces() {
    let mut config = ProjectConfig::default();
    config.test.source = PathBuf::from("tests");
    let mut host = AnalysisHost::with_config(config);
    host.set_source_roots([PathBuf::from("src"), PathBuf::from("tests")]);
    let test_source = concat!(
        "describe(1, function() end)\n",
        "assert.is_true(\"wrong\")\n",
        "assert.are.equal(1, 1)\n",
    );
    host.set_file("tests/sample_spec.tua", test_source);
    host.set_file("tests/lua_spec.lua", "it(1, function() end)\n");
    host.set_file("tests/completion_spec.tua", "descr");
    host.set_file("src/main.tua", "describe(\"not a test\", function() end)\n");
    host.set_file("src/completion.tua", "descr");

    let snapshot = host.snapshot();
    let test_path = PathBuf::from("tests/sample_spec.tua");
    let test_environment = snapshot
        .type_environment(&test_path)
        .expect("test file should have a type environment");
    assert!(test_environment.has_busted_globals());
    assert!(!snapshot
        .type_environment("src/main.tua")
        .expect("production file should have a type environment")
        .has_busted_globals());

    let diagnostics = snapshot.pipeline_diagnostics(&test_path);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH) && diagnostic.line == 1
        }),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH) && diagnostic.line == 2
        }),
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_UNKNOWN_NAME)
                && diagnostic.message.contains("describe")
        }),
        "{diagnostics:?}"
    );

    let hover = snapshot
        .hover(&test_path, SourcePosition { line: 1, column: 2 })
        .expect("Busted global hover should be available");
    assert!(
        hover
            .documentation
            .as_deref()
            .is_some_and(|documentation| documentation.contains("Busted test suite")),
        "{hover:?}"
    );

    let signature = snapshot
        .signature_help(
            &test_path,
            SourcePosition {
                line: 1,
                column: 10,
            },
        )
        .expect("Busted signature should be available");
    assert!(
        signature.signatures[0].label.contains("name: string"),
        "{signature:?}"
    );

    let assertion_signature = snapshot
        .signature_help(
            &test_path,
            SourcePosition {
                line: 3,
                column: 18,
            },
        )
        .expect("LuaAssert signature should be available");
    assert!(
        assertion_signature.signatures[0]
            .label
            .contains("expected: any"),
        "{assertion_signature:?}"
    );

    let test_global_completions = snapshot.completion(
        "tests/completion_spec.tua",
        SourcePosition { line: 1, column: 6 },
    );
    assert!(test_global_completions
        .iter()
        .any(|completion| completion.label == "describe"));

    let completion_source = "assert.is_";
    host.open_document(&test_path, completion_source);
    let completions = host.snapshot().completion(
        &test_path,
        SourcePosition {
            line: 1,
            column: 11,
        },
    );
    assert!(completions
        .iter()
        .any(|completion| completion.label == "is_true"));

    let lua_diagnostics = snapshot.pipeline_diagnostics("tests/lua_spec.lua");
    assert!(
        lua_diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH) && diagnostic.line == 1
        }),
        "{lua_diagnostics:?}"
    );

    let production_completions =
        snapshot.completion("src/main.tua", SourcePosition { line: 1, column: 2 });
    assert!(!production_completions
        .iter()
        .any(|completion| completion.label == "describe"));
    let production_global_completions =
        snapshot.completion("src/completion.tua", SourcePosition { line: 1, column: 6 });
    assert!(!production_global_completions
        .iter()
        .any(|completion| completion.label == "describe"));
    let production_diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    assert!(
        production_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("undefined variable `describe`")),
        "{production_diagnostics:?}"
    );
}
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempProject {
    path: PathBuf,
}

#[test]
fn analysis_snapshots_are_send_and_sync() {
    fn assert_send_and_sync<T: Send + Sync>() {}

    assert_send_and_sync::<AnalysisSnapshot>();
}

#[test]
fn detached_ephemeral_clone_keeps_vfs_allocator_while_resetting_caches() {
    let path = PathBuf::from("src/main.tua");
    let mut coordinator = AnalysisHost::new();
    coordinator.set_file(&path, "return 1\n");
    let _ = coordinator.snapshot();
    assert!(coordinator.cached_snapshot().is_some());

    let mut detached = coordinator.detached_clone_for_ephemeral_analysis();
    assert!(detached.cached_snapshot().is_none());
    assert!(!Arc::ptr_eq(
        &coordinator.completion_cache,
        &detached.completion_cache
    ));
    assert!(!Arc::ptr_eq(
        &coordinator.completion_cache_stats,
        &detached.completion_cache_stats
    ));
    assert!(!Arc::ptr_eq(
        &coordinator.definition_cache,
        &detached.definition_cache
    ));
    assert!(!Arc::ptr_eq(
        &coordinator.module_path_index_cache,
        &detached.module_path_index_cache
    ));
    assert!(!Arc::ptr_eq(
        &coordinator.snapshot_cache,
        &detached.snapshot_cache
    ));

    coordinator.set_file(&path, "return 2\n");
    let coordinator_version = coordinator.vfs.source_version(&path).unwrap();
    detached.open_document(&path, "return 3\n");
    let detached_version = detached.vfs.source_version(&path).unwrap();

    assert_ne!(coordinator_version, detached_version);
    assert!(detached_version > coordinator_version);
}

impl TempProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "tua-analysis-cache-{name}-{}-{unique}",
            process::id()
        ));

        fs::create_dir_all(&path).expect("failed to create temp project");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, path: &str, source: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(path);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }

        fs::write(&path, source).expect("failed to write file");
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn snapshot_prefers_open_document_over_file_contents() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\nprint(value)\n");
    host.open_document(&path, "local value: number = \"open\"\nprint(value)\n");

    let snapshot = host.snapshot();
    assert!(snapshot.is_open_document(&path));
    assert_eq!(
        snapshot.source(&path),
        Some("local value: number = \"open\"\nprint(value)\n")
    );
    assert_eq!(snapshot.diagnostics(&path).len(), 1);
}

#[test]
fn a_lua_parameter_reassigned_from_its_own_method_call_does_not_recurse() {
    // `s = s:gsub(...)` in an unannotated Lua function made name inference
    // resolve `s` through its own assignment until the stack overflowed, so
    // `tua check` crashed on projects that vendor anim8.lua.
    let module = PathBuf::from("src/strings.lua");
    let main = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        &module,
        "local function trim(str)\n  str = str:gsub('%s', '')\n  str = str:upper()\n  return str\nend\nreturn trim\n",
    );
    host.set_file(
        &main,
        "local trim = require(\"strings\")\nprint(trim(\" a \"))\n",
    );

    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics(&main);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let _ = snapshot.pipeline_diagnostics(&module);
}

#[test]
fn pipeline_diagnostics_reject_luau_syntax_at_the_parse_boundary() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"while running do
  continue
end
"#,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::SYNTAX_UNSUPPORTED_DIALECT))
        .expect("full project diagnostics should include Tua dialect validation");

    assert_eq!(diagnostic.line, 2);
    assert_eq!(diagnostic.column, 3);
    assert!(diagnostic.message.contains("`continue`"));
}

#[test]
fn current_file_field_completion_answers_known_lua_receivers() {
    let path = PathBuf::from("src/main.tua");
    let source = "local value = table.\n";
    let position = SourcePosition {
        line: 1,
        column: source.find("table.").expect("receiver should exist") + "table.".len() + 1,
    };
    let mut host = AnalysisHost::new();

    host.open_document(&path, source);

    let completions = host
        .current_file_field_completion(&path, position)
        .expect("Lua receivers should be authoritative without a project snapshot");
    let labels = completions
        .iter()
        .map(|completion| completion.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(labels, ["concat", "insert", "maxn", "remove", "sort"]);
}

#[test]
fn module_path_completion_uses_snapshot_files_instead_of_the_live_filesystem() {
    let project = TempProject::new("snapshot-module-completion");
    let source_root = project.path().join("src");
    let main_path = source_root.join("main.tua");
    let open_candidate = source_root.join("candidate_open.tua");
    project.write("tua.toml", "source = \"src\"\n");
    let disk_candidate = project.write("src/candidate_disk.tua", "return {}\n");
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local required = require("candidate_<|require|>")
local imported = import("candidate_<|import|>")
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([source_root]);
    host.open_document(&main_path, fixture.source("src/main.tua"));
    host.open_document(&open_candidate, "return {}\n");

    assert!(disk_candidate.exists());
    assert!(!open_candidate.exists());

    let snapshot = host.snapshot();

    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("require"))),
        ["candidate_open"]
    );
    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("import"))),
        ["candidate_open"]
    );
}

#[test]
fn module_path_completion_uses_workspace_scoped_require_root_inventory() {
    let fixture = Fixture::parse(
        r#"-- file: game-a/src/main.tua
local feedback = import("fee<|vendor|>")
local requiredFeedback = require("fee<|require_vendor|>")
local runtime = import("run<|output|>")
local helpers = import("help<|associated|>")
local other = import("oth<|other|>")
"#,
    );
    let main_path = PathBuf::from("game-a/src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("game-a/src"), PathBuf::from("game-b/src")]);
    host.set_module_path_workspaces([
        ProjectModuleWorkspace::new(
            "game-a",
            [
                "game-a/src".into(),
                "game-a/build".into(),
                "game-a/vendor".into(),
            ],
        ),
        ProjectModuleWorkspace::new("game-b", ["game-b/src".into()]),
    ]);
    host.set_indexed_module_paths([
        PathBuf::from("game-a/vendor/feel/init.lua"),
        PathBuf::from("game-a/build/runtime.lua"),
        PathBuf::from("game-a/vendor/helpers.luau"),
        PathBuf::from("game-b/src/other.tua"),
    ]);
    host.open_document(&main_path, fixture.source("game-a/src/main.tua"));

    let snapshot = host.snapshot();

    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("vendor"))),
        ["feel"]
    );
    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("require_vendor"))),
        ["feel"]
    );
    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("output"))),
        ["runtime"]
    );
    assert_eq!(
        completion_labels(&snapshot.completion(&main_path, fixture.position("associated"))),
        ["helpers"]
    );
    assert!(snapshot
        .completion(&main_path, fixture.position("other"))
        .is_empty());
}

#[test]
fn type_explanation_reports_initializer_widening_and_field_resolution() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Player = { hp: number }

local mode = "play"
local modeCopy = <|mode|>mode

local player: Player = { hp = 100 }
local health = player.<|field|>hp
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let mode = snapshot
        .explain_type("src/main.tua", fixture.position("mode"))
        .expect("an inferred local should have a type explanation");
    let field = snapshot
        .explain_type("src/main.tua", fixture.position("field"))
        .expect("a typed field should have a type explanation");

    assert_eq!(mode.expression, "mode");
    assert_eq!(mode.ty, Type::string());
    assert!(mode.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Inference
            && step.ty == Some(Type::string_literal("play"))
    }));
    assert!(mode.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Widening && step.ty == Some(Type::string())
    }));

    assert_eq!(field.expression, "player.hp");
    assert_eq!(field.ty, Type::number());
    assert!(field
        .steps
        .iter()
        .any(|step| step.kind == TypeExplanationStepKind::Annotation));
    assert!(field.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Field && step.ty == Some(Type::number())
    }));
}

#[test]
fn type_explanation_identifies_explicit_any_propagation() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local payload: any = {}
local value = payload.<|field|>missing
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let explanation = host
        .snapshot()
        .explain_type("src/main.tua", fixture.position("field"))
        .expect("an any-backed field should still explain its dynamic type");

    assert_eq!(explanation.ty, Type::Any);
    assert!(explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Dynamic && step.title == "Explicit `any` annotation"
    }));
    assert!(explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Dynamic && step.detail.contains("receiver is `any`")
    }));
}

#[test]
fn type_explanation_links_call_inference_to_function_origin() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local function makeName(): string
  return "Ada"
end

local name = makeName()
local copy = <|name|>name
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let explanation = host
        .snapshot()
        .explain_type("src/main.tua", fixture.position("name"))
        .expect("a typed call result should have an explanation");
    let inference = explanation
        .steps
        .iter()
        .find(|step| step.kind == TypeExplanationStepKind::Inference)
        .expect("the call result should include an inference step");

    assert_eq!(explanation.ty, Type::string());
    assert_eq!(inference.path.as_deref(), Some(Path::new("src/main.tua")));
    assert_eq!(inference.span.map(|span| span.start.line), Some(1));
}

#[test]
fn type_explanation_identifies_missing_required_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Missing = require("missing")
local value = Missing.<|field|>value
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let explanation = host
        .snapshot()
        .explain_type("src/main.tua", fixture.position("field"))
        .expect("a missing module binding should have an explanation");

    assert_eq!(explanation.ty, Type::Any);
    assert!(explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Unresolved && step.title == "Missing module"
    }));
}

#[test]
fn type_explanation_uses_indexed_exports_for_required_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = require("player")
local player = Player.<|field|>new()
-- file: src/player.tua
local function new(): { hp: number }
  return { hp = 100 }
end

return {
  new = new,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let explanation = host
        .snapshot()
        .explain_type("src/main.tua", fixture.position("field"))
        .expect("an indexed module export should have an explanation");

    assert_ne!(explanation.ty, Type::Any);
    assert!(explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Module
            && step.title == "Resolved module"
            && step.ty.as_ref().is_some_and(|ty| !matches!(ty, Type::Any))
    }));
    assert!(explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Field
            && step.title == "Resolved member `new`"
            && step.ty.as_ref().is_some_and(|ty| !matches!(ty, Type::Any))
    }));
    assert!(!explanation.steps.iter().any(|step| {
        step.kind == TypeExplanationStepKind::Dynamic && step.title == "Initializer is dynamic"
    }));
}

#[test]
fn signal_graph_collects_typed_calls_across_files() {
    let mut host = AnalysisHost::new();
    let listeners = PathBuf::from("src/listeners.tua");
    let emitters = PathBuf::from("src/emitters.tua");

    host.set_file(
        &listeners,
        r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: string, damage: number) end)
"#,
    );
    host.set_file(
        &emitters,
        r#"local events = signal()
events:emit("player.hit", "Ada", 12)
events:has("player.hit")
"#,
    );

    let graph = host.snapshot().signal_graph();

    assert_eq!(graph.events.len(), 2);
    assert!(graph.events.iter().all(|event| event.name == "player.hit"));
    assert_eq!(
        graph
            .events
            .iter()
            .flat_map(|event| event.calls.iter())
            .map(|call| call.role)
            .collect::<Vec<_>>(),
        vec![
            crate::SignalGraphCallRole::Emit,
            crate::SignalGraphCallRole::Query,
            crate::SignalGraphCallRole::Declaration,
            crate::SignalGraphCallRole::Listener,
        ]
    );
    assert_eq!(
        graph
            .events
            .iter()
            .flat_map(|event| event.calls.iter())
            .find(|call| call.role == crate::SignalGraphCallRole::Declaration)
            .expect("declaration should be present")
            .payload
            .as_ref()
            .expect("declaration should expose a payload")
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter.type_label.as_str()))
            .collect::<Vec<_>>(),
        vec![("player", "string"), ("damage", "number")]
    );
    let emit_event = graph
        .events
        .iter()
        .find(|event| {
            event
                .calls
                .iter()
                .any(|call| call.role == crate::SignalGraphCallRole::Emit)
        })
        .expect("emitter bus should be present");
    assert_eq!(
        emit_event.resolved_payload,
        Some(vec![
            crate::SignalGraphParameter {
                name: "value1".to_owned(),
                type_label: "string".to_owned(),
            },
            crate::SignalGraphParameter {
                name: "value2".to_owned(),
                type_label: "number".to_owned(),
            },
        ])
    );
    assert_eq!(
        emit_event
            .calls
            .iter()
            .find(|call| call.role == crate::SignalGraphCallRole::Emit)
            .and_then(|call| call.inferred_emit_payload.as_ref())
            .map(|parameters| parameters
                .iter()
                .map(|parameter| parameter.type_label.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["string", "number"])
    );
    assert!(!emit_event.schema_conflict);
}

#[test]
fn signal_graph_exposes_conflicting_schema_variants_without_resolving_one() {
    let mut host = AnalysisHost::new();

    host.set_file(
        "src/events.tua",
        r#"local events = signal()
events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: number, damage: number) end)
events:emit("player.hit", "Ada", 12)
"#,
    );

    let graph = host.snapshot().signal_graph();
    let event = graph.events.first().expect("event should be indexed");

    assert!(event.schema_conflict);
    assert_eq!(event.resolved_payload, None);
    assert_eq!(
        event
            .calls
            .iter()
            .filter_map(|call| call.payload.as_ref())
            .map(|payload| payload[0].type_label.as_str())
            .collect::<Vec<_>>(),
        vec!["string", "number"]
    );
    assert_eq!(event.calls[0].insertion_position.line, 2);
    assert_eq!(event.calls[0].indentation, "");
}

#[test]
fn signal_event_queries_share_one_bus_aware_index() {
    let fixture = Fixture::parse(
        r#"local events = signal()
local ui = signal()

events:declare("<|declaration|>player.hit", function(player: string, damage: number) end)
events:emit("<|usage|>player.hit", "Ada", 12)
ui:declare("player.hit", function(message: string) end)
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source(&path));
    let snapshot = host.snapshot();
    let usage = fixture.position("usage");
    let declaration = fixture.position("declaration");

    let hover = snapshot
        .hover(&path, usage)
        .expect("event string should have hover");
    assert_eq!(
        hover.contents,
        "SignalBus event \"player.hit\"(player: string, damage: number)"
    );

    let definition = snapshot
        .definition(&path, usage)
        .expect("event string should resolve to its declaration");
    assert_eq!(definition.kind, DefinitionKind::SignalEvent);
    assert_eq!(definition.span.start, declaration);

    let references = snapshot.references(&path, usage, true);
    assert_eq!(references.len(), 2);
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::SignalEvent));

    let target = snapshot
        .prepare_rename(&path, usage)
        .expect("event string should prepare rename");
    assert_eq!(target.kind, crate::RenameKind::SignalEvent);
    assert_eq!(target.placeholder, "player.hit");

    let rename = snapshot
        .rename(&path, usage, "player.damaged")
        .expect("event name should rename");
    assert_eq!(rename.edits.len(), 2);
    assert!(rename
        .edits
        .iter()
        .all(|edit| edit.new_text == "player.damaged"));
}

#[test]
fn escaped_signal_event_names_match_their_lua_runtime_values() {
    let source = r#"local events = signal()
events:declare("player.hit", function(player: string) end)
events:emit("player\046hit", "Ada")
"#;
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", source);
    let snapshot = host.snapshot();
    let usage = crate::source_text::position_at(source, "player\\046hit");
    let hover = snapshot
        .hover("src/main.tua", usage)
        .expect("escaped event string should match its decoded declaration");
    assert_eq!(
        hover.contents,
        "SignalBus event \"player.hit\"(player: string)"
    );
    assert_eq!(
        snapshot
            .definition("src/main.tua", usage)
            .expect("event definition should resolve")
            .kind,
        DefinitionKind::SignalEvent
    );
}

#[test]
fn signal_hover_infers_payload_from_emit_when_no_schema_is_declared() {
    let source = r#"local events = signal()
events:emit("player.hit", vec2(10, 20))
"#;
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", source);

    let hover = host
        .snapshot()
        .hover(
            "src/main.tua",
            crate::source_text::position_at(source, "player.hit"),
        )
        .expect("event name should hover from its emitted payload");

    assert_eq!(
        hover.contents,
        "SignalBus event \"player.hit\"(value1: Vec2)"
    );
}

#[test]
fn signal_handler_parameter_hover_and_completion_use_emitted_payloads() {
    let source = r#"type Player = { hp: number, name: string }
local events = signal()
local player: Player = { hp = 10, name = "Ada" }
events:on("duck.hit", function(target)
  print(target.hp)
end)
events:emit("duck.hit", player)
"#;
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", source);
    let snapshot = host.snapshot();

    let hover = snapshot
        .hover(
            "src/main.tua",
            crate::source_text::position_at(source, "target.hp"),
        )
        .expect("handler parameter should use emitted payload type");
    assert_eq!(hover.contents, "parameter target: Player");

    let target = crate::source_text::position_at(source, "target.hp");
    let completions = snapshot.completion(
        "src/main.tua",
        SourcePosition {
            line: target.line,
            column: target.column + "target.".len(),
        },
    );
    assert!(completions.iter().any(|item| item.label == "hp"));
}

#[test]
fn signal_event_payload_conflicts_are_diagnosed_per_bus() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"local events = signal()
local ui = signal()

events:declare("player.hit", function(player: string, damage: number) end)
events:on("player.hit", function(player: string, damage: string) end)
ui:declare("player.hit", function(message: string) end)
"#,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let conflicts = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::DOMAIN_SIGNAL_SCHEMA))
        .collect::<Vec<_>>();

    assert_eq!(conflicts.len(), 2);
    assert!(conflicts
        .iter()
        .all(|diagnostic| diagnostic.message.contains("player.hit")));
}

#[test]
fn shared_required_signal_bus_links_event_queries_across_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/signals.tua
return signal()
-- file: src/listeners.tua
local events: SignalBus = require("signals")
events:declare("<|declaration|>player.hit", function(player: string, damage: number) end)
-- file: src/emitters.tua
local events: SignalBus = require("signals")
events:emit("<|usage|>player.hit", "Ada", <|signature|>12)
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let snapshot = host.snapshot();
    let emitter = PathBuf::from("src/emitters.tua");
    let listener = PathBuf::from("src/listeners.tua");
    let usage = fixture.position("usage");

    let definition = snapshot
        .definition(&emitter, usage)
        .expect("required SignalBus event should resolve across files");
    assert_eq!(definition.path, listener);
    assert_eq!(definition.span.start, fixture.position("declaration"));

    let references = snapshot.references(&emitter, usage, true);
    assert_eq!(references.len(), 2);

    let help = snapshot
        .signature_help(&emitter, fixture.position("signature"))
        .expect("required SignalBus event should expose project payload signature");
    assert_eq!(
        help.signatures[0].label,
        "function events:emit(\"player.hit\", player: string, damage: number): nil"
    );
}

#[test]
fn shared_required_signal_bus_reports_cross_file_schema_conflicts() {
    let mut host = AnalysisHost::new();

    host.set_file("src/signals.tua", "return signal()\n");
    host.set_file(
        "src/first.tua",
        r#"local events: SignalBus = require("signals")
events:declare("player.hit", function(player: string, damage: number) end)
"#,
    );
    host.set_file(
        "src/second.tua",
        r#"local events: SignalBus = require("signals")
events:on("player.hit", function(player: string, damage: string) end)
"#,
    );

    let snapshot = host.snapshot();

    for path in ["src/first.tua", "src/second.tua"] {
        let diagnostics = snapshot.pipeline_diagnostics(path);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::DOMAIN_SIGNAL_SCHEMA)
                && diagnostic.message.contains("player.hit")
        }));
    }

    let emitter = Fixture::parse(
        r#"local events: SignalBus = require("signals")
events:emit("player.hit", "Ada", <|signature|>12)
"#,
    );
    host.set_file("src/emitter.tua", emitter.source("src/main.tua"));
    let snapshot = host.snapshot();
    let help = snapshot
        .signature_help("src/emitter.tua", emitter.position("signature"))
        .expect("conflicted event should retain generic signature help");

    assert_eq!(
        help.signatures[0].label,
        "function SignalBus:emit(name: string, ...: any): nil"
    );
}

#[test]
fn imported_signal_bus_payloads_are_checked_for_aliases_and_module_fields() {
    let fixture = Fixture::parse(
        r#"-- file: src/events.tua
local events = signal()
events:on("hit", function(damage: number) end)
return { events = events }
-- file: src/alias.tua
local EventModule = require("events")
local bus = EventModule.events
bus:emit("hit", "bad")
-- file: src/direct.tua
local EventModule = require("events")
EventModule.events:emit("hit", "bad")
EventModule.events:emit("hit", <|direct_signature|>1)
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for path in ["src/alias.tua", "src/direct.tua"] {
        let diagnostics = snapshot.pipeline_diagnostics(path);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{path} must reject the imported wrong payload: {diagnostics:#?}"
        );
    }
    let help = snapshot
        .signature_help("src/direct.tua", fixture.position("direct_signature"))
        .expect("direct module field should retain its payload signature");
    assert!(
        help.signatures[0].label.contains("damage: number"),
        "direct module field signature: {help:#?}"
    );

    host.set_file(
        "src/events.tua",
        "local events = signal()\nevents:on(\"hit\", function(damage: string) end)\nreturn { events = events }\n",
    );
    let refreshed = host.snapshot();
    assert!(
        !refreshed
            .pipeline_diagnostics("src/alias.tua")
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "editing the provider should refresh the imported payload schema"
    );
    assert!(
        refreshed
            .pipeline_diagnostics("src/direct.tua")
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "the numeric direct-property call should become invalid after the schema edit"
    );
}

#[test]
fn audio_snapshot_music_fields_complete_and_hover_with_their_types() {
    let fixture = Fixture::parse(
        r#"local audio = audioSystem({})
local snapshot = audio:snapshot()
local state = snapshot.music.<|hover|>state
local pending = snapshot.music.<|members|>pending
local unknown = snapshot.<|top_members|>musicState
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", fixture.source("src/main.tua"));
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("music state should hover");
    assert!(hover.contents.contains("string"), "{hover:#?}");
    assert!(hover.contents.contains("nil"), "{hover:#?}");
    let completions = snapshot.completion("src/main.tua", fixture.position("members"));
    assert!(
        completions.iter().any(|item| item.label == "pending"),
        "music fields should complete: {completions:#?}"
    );
    let top_level = snapshot.completion("src/main.tua", fixture.position("top_members"));
    assert!(
        top_level.iter().any(|item| item.label == "music")
            && !top_level.iter().any(|item| item.label == "musicState"),
        "snapshot and subscription state fields must remain distinct: {top_level:#?}"
    );
}

#[test]
fn shared_required_signal_bus_ranks_event_completion_before_nearby_other_bus() {
    let fixture = Fixture::parse(
        r#"-- file: src/signals.tua
return signal()
-- file: src/listeners.tua
local events: SignalBus = require("signals")
events:declare("player.hit", function(player: string, damage: number) end)
-- file: src/emitters.tua
local events: SignalBus = require("signals")
local ui = signal()
ui:declare("ui.ready", function() end)
events:emit("<|completion|>")
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let emitter = PathBuf::from("src/emitters.tua");
    let completions = host
        .snapshot()
        .completion(&emitter, fixture.position("completion"));
    let labels = completions
        .iter()
        .map(|completion| completion.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(labels.first(), Some(&"player.hit"));
    assert!(labels.contains(&"ui.ready"));
}

#[test]
fn open_documents_drive_hover_and_closed_documents_fall_back_to_disk() {
    let path = PathBuf::from("src/main.tua");
    let disk = Fixture::parse(
        r#"type Player = { hp: number }
local player: <|hover|>Player = { hp = 10 }
"#,
    );
    let open = Fixture::parse(
        r#"type Enemy = { hp: number }
local enemy: <|hover|>Enemy = { hp = 5 }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_file(&path, disk.source("src/main.tua"));
    host.open_document(&path, open.source("src/main.tua"));

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover(&path, open.position("hover"))
        .expect("open hover should resolve");

    assert!(hover.contents.contains("type Enemy"));

    host.close_document(&path);

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover(&path, disk.position("hover"))
        .expect("disk hover should resolve after close");

    assert!(hover.contents.contains("type Player"));
}

#[test]
fn snapshot_answers_parse_diagnostics_hover_completion_and_definition() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type <|alias_def|>Player<|/alias_def|> = { hp: number }

local player: <|hover|>Player = { hp = 10 }
local broken: number = "nope"

player.hp = 11
-- file: src/complete.tua
type Player = { hp: number }
local typed: Pl<|complete|> = nil
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let completion_path = PathBuf::from("src/complete.tua");
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let parsed = snapshot
        .parse(&path)
        .expect("file should be cached")
        .expect("source should parse");
    let diagnostics = snapshot.diagnostics(&path);
    let hover = snapshot
        .hover(&path, fixture.position("hover"))
        .expect("alias hover should resolve");
    let completions = snapshot.completion(&completion_path, fixture.position("complete"));
    let definition = snapshot
        .definition(&path, fixture.position("hover"))
        .expect("alias definition should resolve");

    assert_eq!(parsed.path, path);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("type mismatch")));
    assert!(hover.contents.contains("type Player"));
    assert!(completions
        .iter()
        .any(|completion| completion.label == "Player"));
    assert_eq!(definition.span, fixture.range("alias_def"));
}

#[test]
fn runtime_module_facades_preserve_types_across_forwarding_layers() {
    let fixture = Fixture::parse(
        r#"-- file: src/leaf.tua
return {
  value = 42,
  nested = { label = "ready" },
}
-- file: src/direct.tua
return require("leaf")
-- file: src/facade.tua
local Next = require("direct")
return {
  value = Next.value,
  nested = Next.nested,
}
-- file: src/facade_two.tua
local Next = require("facade")
return {
  value = Next.value,
  nested = Next.nested,
}
-- file: src/main.tua
local Facade = require("facade_two")
local wrong: string = Facade.value
local value = Facade.<|hover|>value
local label = Facade.nested.label
local picked = Facade.va<|complete|>
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let path = PathBuf::from("src/main.tua");
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics(&path);
    let hover = snapshot
        .hover(&path, fixture.position("hover"))
        .expect("forwarded number should hover");
    let completions = completion_labels(&snapshot.completion(&path, fixture.position("complete")));

    assert_eq!(
        snapshot
            .module_graph()
            .export("src/facade_two.tua", "value")
            .map(|entry| &entry.field.ty),
        Some(&Type::number())
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("expected string, found number")
        }),
        "{diagnostics:?}"
    );
    assert!(hover.contents.contains("number"), "{hover:?}");
    assert!(completions.contains(&"value".to_owned()), "{completions:?}");

    assert_eq!(
        snapshot
            .module_graph()
            .export("src/direct.tua", "value")
            .map(|entry| &entry.field.ty),
        Some(&Type::number())
    );
    assert!(matches!(
        snapshot
            .module_graph()
            .export("src/facade_two.tua", "nested")
            .map(|entry| &entry.field.ty),
        Some(Type::Table(_))
    ));
}

#[test]
fn dynamic_runtime_module_facades_remain_conservative() {
    let fixture = Fixture::parse(
        r#"-- file: src/dynamic.tua
local name = "leaf"
return require(name)
-- file: src/main.tua
local Dynamic = require("dynamic")
local value: string = Dynamic.value
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let snapshot = host.snapshot();

    assert!(snapshot
        .module_graph()
        .export("src/dynamic.tua", "value")
        .is_none());
    assert!(!snapshot
        .diagnostics("src/main.tua")
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn projected_require_returns_do_not_forward_the_whole_module() {
    let fixture = Fixture::parse(
        r#"-- file: src/leaf.tua
return {
  value = 42,
  nested = { label = "ready" },
}
-- file: src/projected.tua
return require("leaf").nested
-- file: src/main.tua
local Projected = require("projected")
local value: string = Projected.value
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let snapshot = host.snapshot();

    assert!(snapshot
        .module_graph()
        .export("src/projected.tua", "value")
        .is_none());
    assert!(!snapshot
        .diagnostics("src/main.tua")
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn whole_module_forwarding_replaces_provisional_any_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/leaf.tua
return { value = 42 }
-- file: src/z_mapped.tua
local Leaf = require("leaf")
return { value = Leaf.value }
-- file: src/a_whole.tua
local Mapped = require("z_mapped")
return Mapped
-- file: src/main.tua
local Forwarded = require("a_whole")
local wrong: string = Forwarded.value
local value = Forwarded.<|hover|>value
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let path = PathBuf::from("src/main.tua");
    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics(&path);
    let hover = snapshot
        .hover(&path, fixture.position("hover"))
        .expect("forwarded number should hover");

    assert_eq!(
        snapshot
            .module_graph()
            .export("src/a_whole.tua", "value")
            .map(|entry| &entry.field.ty),
        Some(&Type::number())
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("expected string, found number")
        }),
        "{diagnostics:?}"
    );
    assert!(hover.contents.contains("number"), "{hover:?}");
}

#[test]
fn nested_builtin_configuration_fields_hover_and_navigate_to_virtual_schemas() {
    let fixture = Fixture::parse(
        r#"local Player = spriteSheet({
  <|sprite_image|>image = "assets/player.png",
  frame = { <|frame_width|>width = 128, height = 128 },
  animations = {
    green = {
      from = 0,
      to = 3,
      <|animation_row|>row = 2,
      fps = 8,
      onLoop = function(animation, loops)
        if <|sprite_loops|>loops > 1 then
          animation:pause()
        end
      end,
    },
  },
})

local light = stateMachine({
  <|machine_initial|>initial = "green",
  events = {
    { <|event_name|>name = "warn", from = "green", to = "yellow" },
  },
})

type Bullet = { active: boolean }
local bullets = objectPool({
  <|pool_create|>create = function(): any
    return {}
  end,
  reset = function(bullet: Bullet)
    <|pool_bullet|>bullet.active = false
  end,
  <|pool_initial|>initial = 64,
})

type CustomFrame = {
  <|custom_width_definition|>width: number,
  height: number,
  left?: number,
  top?: number,
  border?: number,
}
local customFrame: CustomFrame = {
  <|custom_width_reference|>width = 32,
  height = 32,
}
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source("src/main.tua"));
    let snapshot = host.snapshot();

    for (marker, field, ty, builtin) in [
        ("sprite_image", "image", "string", "SpriteSheetConfig.image"),
        ("frame_width", "width", "number", "SpriteFrameConfig.width"),
        (
            "animation_row",
            "row",
            "number",
            "SpriteAnimationConfig.row",
        ),
        (
            "machine_initial",
            "initial",
            "string",
            "StateMachineOptions.initial",
        ),
        ("event_name", "name", "string", "StateMachineEvent.name"),
        (
            "pool_create",
            "create",
            "() -> any",
            "ObjectPoolOptions.create",
        ),
        (
            "pool_initial",
            "initial",
            "number",
            "ObjectPoolOptions.initial",
        ),
    ] {
        let hover = snapshot
            .hover(&path, fixture.position(marker))
            .unwrap_or_else(|| panic!("{field} should have contextual field hover"));
        assert!(
            hover.contents.contains(field) && hover.contents.contains(ty),
            "unexpected {field} hover: {}",
            hover.contents
        );

        let definition = snapshot
            .definition(&path, fixture.position(marker))
            .unwrap_or_else(|| panic!("{field} should navigate to its builtin schema"));
        let uri = definition.path.to_string_lossy();
        assert_eq!(uri, format!("tua:/builtins/{builtin}.tua"));
        let document = crate::builtin_definition_document(&uri)
            .unwrap_or_else(|| panic!("{builtin} should have a virtual document"));
        assert_eq!(text_at_span(&document, definition.span), field);
    }

    let loops = snapshot
        .hover(&path, fixture.position("sprite_loops"))
        .expect("sprite callback parameters should be contextually typed");
    assert_eq!(loops.contents, "parameter loops: number");

    let bullet = snapshot
        .hover(&path, fixture.position("pool_bullet"))
        .expect("object-pool callback annotations should remain visible");
    assert_eq!(bullet.contents, "parameter bullet: Bullet");

    let custom_definition = snapshot.definition(&path, fixture.position("custom_width_reference"));
    assert!(
        custom_definition
            .as_ref()
            .is_none_or(|definition| definition.path == path),
        "a structurally similar user type must not borrow a builtin schema identity: \
         {custom_definition:?}"
    );
}

#[test]
fn helper_derived_names_and_nested_callback_expressions_keep_source_identity() {
    let fixture = Fixture::parse(
        r#"type GameState = { score: number }
local game = store({ <|score_definition|>score = 0 })
game:watch(
  <|score_reference|>"score",
  function(score: number, previous: number, state: GameState)
    print("score", previous, <|score_parameter|>score, state.score)
  end
)

local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    <|green_definition|>green = { from = 0, to = 3, row = 2, fps = 8 },
  },
})
local green = Player.animations.<|green_reference|>green

local light = stateMachine({
  initial = "green",
  events = {
    { name = <|warn_definition|>"warn", from = "green", to = <|red_definition|>"red" },
  },
  callbacks = {
    <|callback_reference|>onenterred = function(
      self: StateMachine,
      event: "warn",
      from: "green" | "red",
      to: "green" | "red"
    )
      print(<|event_parameter|>event, from, to, self.current)
    end,
  },
})
light:<|warn_reference|>warn()
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source("src/main.tua"));
    let snapshot = host.snapshot();

    let score_hover = snapshot
        .hover(&path, fixture.position("score_reference"))
        .expect("watch key should have derived field hover");
    assert_eq!(score_hover.contents, "(field) score: number");

    let parameter_hover = snapshot
        .hover(&path, fixture.position("score_parameter"))
        .expect("nested callback parameter should have parameter hover");
    assert_eq!(parameter_hover.contents, "parameter score: number");

    let callback_hover = snapshot
        .hover(&path, fixture.position("callback_reference"))
        .expect("derived callback key should have an exact function hover");
    assert!(
        callback_hover.contents.contains("event: \"warn\"")
            && callback_hover.contents.contains("\"green\" | \"red\""),
        "{}",
        callback_hover.contents
    );

    let event_hover = snapshot
        .hover(&path, fixture.position("event_parameter"))
        .expect("nested state-machine callback parameter should hover");
    assert_eq!(event_hover.contents, "parameter event: \"warn\"");

    for (reference, definition) in [
        ("score_reference", "score_definition"),
        ("green_reference", "green_definition"),
        ("warn_reference", "warn_definition"),
        ("callback_reference", "red_definition"),
    ] {
        let target = snapshot
            .definition(&path, fixture.position(reference))
            .unwrap_or_else(|| panic!("{reference} should retain a source definition"));
        assert_eq!(target.path, path);
        assert_eq!(target.span.start, fixture.position(definition));
    }
}

#[test]
fn snapshot_reuses_cached_project_artifacts_until_revision_changes() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\n");

    let first_revision = host.revision();
    let _first = host.snapshot();
    let stats_after_first = host.cache_stats();
    let _second = host.snapshot();

    assert_eq!(host.revision(), first_revision);
    assert_eq!(host.cache_stats(), stats_after_first);

    host.open_document(&path, "local value: number = 2\n");
    assert!(host.revision() > first_revision);

    let _third = host.snapshot();
    assert!(host.cache_stats().invalidations > stats_after_first.invalidations);
}

#[test]
fn snapshots_reuse_module_path_index_until_its_inputs_change() {
    let main_path = PathBuf::from("src/main.tua");
    let player_path = PathBuf::from("src/player.tua");
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(&main_path, "local value = 1\n");

    let first = host.snapshot();

    host.set_file(&main_path, "local value = 2\n");
    let content_update = host.snapshot();

    assert!(Arc::ptr_eq(
        &first.module_path_index,
        &content_update.module_path_index
    ));

    host.set_file(&player_path, "return {}\n");
    let path_update = host.snapshot();

    assert!(!Arc::ptr_eq(
        &content_update.module_path_index,
        &path_update.module_path_index
    ));
}

#[test]
fn setting_identical_file_source_does_not_bump_revision() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value: number = 1\n");
    host.snapshot();

    let revision = host.revision();
    let stats = host.cache_stats();

    host.set_file(&path, "local value: number = 1\n");
    host.snapshot();

    assert_eq!(host.revision(), revision);
    assert_eq!(host.cache_stats(), stats);
}

#[test]
fn snapshot_clones_share_warmed_definition_cache() {
    let fixture = Fixture::parse(
        r#"type <|alias_def|>Player<|/alias_def|> = { hp: number }
local player: <|alias_ref|>Player = { hp = 10 }
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source("src/main.tua"));

    let snapshot = host.snapshot();
    assert!(snapshot
        .definition_cache
        .lock()
        .expect("definition cache lock poisoned")
        .is_empty());

    let definition = snapshot
        .definition(&path, fixture.position("alias_ref"))
        .expect("alias definition should resolve");
    assert_eq!(definition.span, fixture.range("alias_def"));
    let cache_len = snapshot
        .definition_cache
        .lock()
        .expect("definition cache lock poisoned")
        .len();
    assert!(cache_len > 0);

    let warm_snapshot = host.snapshot();

    assert!(Arc::ptr_eq(
        &snapshot.definition_cache,
        &warm_snapshot.definition_cache
    ));
    assert_eq!(
        warm_snapshot
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .len(),
        cache_len
    );

    let repeated = warm_snapshot
        .definition(&path, fixture.position("alias_ref"))
        .expect("cached alias definition should resolve");

    assert_eq!(repeated.span, fixture.range("alias_def"));
    assert_eq!(
        warm_snapshot
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .len(),
        cache_len
    );
}

#[test]
fn warm_file_builds_only_the_requested_file_and_snapshot_reuses_it() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &main,
        r#"type Player = { hp: number }
local player: Player = { hp = 10 }
local hp = player.hp
"#,
    );
    host.set_file(
        &other,
        r#"type Enemy = { hp: number }
local enemy: Enemy = { hp = 5 }
local hp = enemy.hp
"#,
    );

    assert!(host.warm_file(&other, AnalysisWarmPriority::High));
    let after_warm = host.cache_stats();

    assert_eq!(after_warm.parse_misses, 1);
    assert_eq!(after_warm.hir_misses, 1);
    assert!(host.cached_snapshot().is_none());

    let snapshot = host.snapshot();

    assert_eq!(
        host.cache_stats().parse_misses,
        2,
        "the snapshot should build the remaining cold file"
    );
    assert_eq!(host.cache_stats().parse_hits, 1);
    assert!(snapshot.semantic_index(&main).is_some());
    assert!(snapshot.semantic_index(&other).is_some());
}

#[test]
fn stale_warmed_file_is_not_adopted_after_an_edit() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "local value = 1\n");
    let warmed = host.clone();
    assert!(warmed.warm_file(&path, AnalysisWarmPriority::Low));

    host.set_file(&path, "local value = 2\n");

    assert!(!host.adopt_warmed_file(&warmed, &path));
    assert!(host.cached_snapshot().is_none());
}

#[test]
fn valid_files_keep_cached_semantics_when_type_diagnostics_exist() {
    let fixture = Fixture::parse(
        r#"type Player = { hp: number }
local player: Player = { hp = 10 }
local unresolved: Missing = nil
local hp = player.<|complete|>h
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, fixture.source(&path).to_owned());

    let snapshot = host.snapshot();
    let semantic_file = snapshot
        .semantic_file(&path)
        .expect("valid source should have semantic facts");

    assert!(!semantic_file.hir.symbols.is_empty());
    assert!(!semantic_file.scope.declarations().is_empty());
    assert!(snapshot.type_environment(&path).is_some());
    assert!(snapshot
        .diagnostics(&path)
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_UNKNOWN_NAME)));
    assert!(
        completion_labels(&snapshot.completion(&path, fixture.position("complete")))
            .contains(&"hp".to_owned())
    );
    assert_eq!(
        snapshot
            .semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .len(),
        1
    );
}

#[test]
fn snapshot_keeps_project_inputs() {
    let mut host = AnalysisHost::with_config(ProjectConfig {
        source: PathBuf::from("game"),
        extension: "tua".to_owned(),
        ..ProjectConfig::default()
    });

    host.set_source_roots([PathBuf::from("game")]);
    host.set_builtin_catalogs(BuiltinCatalogs {
        lua: true,
        love: false,
    });

    let snapshot = host.snapshot();

    assert_eq!(snapshot.config().source, PathBuf::from("game"));
    assert_eq!(snapshot.source_roots(), &[PathBuf::from("game")]);
    assert!(!snapshot.builtin_catalogs().love);
}

#[test]
fn meta_declarations_resolve_runtime_module_prefixes_without_becoming_source_roots() {
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_type_libraries([ProjectTypeLibrary::new(
        "lib/feel/types",
        "feel",
        "lib.feel",
    )]);
    host.set_file_with_kind(
        "src/main.tua",
        "local love = require(\"lib.feel.love\")\nreturn love\n",
        AnalysisSourceKind::Tua,
    );
    host.set_file_with_kind(
        "lib/feel/types/love.lua",
        "---@meta feel.love\nreturn { supported = true }\n",
        AnalysisSourceKind::Lua,
    );

    let snapshot = host.snapshot();

    assert_eq!(
        snapshot
            .module_graph()
            .resolve_require("src/main.tua", "lib.feel.love"),
        Some(&RequireResolution::Resolved {
            path: PathBuf::from("lib/feel/types/love.lua"),
        })
    );
    assert_eq!(snapshot.source_roots(), &[PathBuf::from("src")]);
}

#[test]
fn declaration_library_drives_recursive_discriminated_table_completion() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local feel = require("lib.feel")

feel.define("field", {
  { <|field|>dummy },
})

feel.define("literal", {
  { kind = "an<|literal|>" },
})

feel.define("animate", {
  { kind = "animate", <|animate|>dummy },
})

feel.define("nested", {
  { kind = "parallel", steps = {
    { <|nested|>dummy },
  } },
})

feel.play("button.press", {}, {
  emit = function(event, ctx)
    local copy = <|callback_hover|>event
    event.<|callback|>
    ctx.target.<|callback_nested|>
  end,
  <|options|>dummy
})

-- file: lib/feel/types/feel.lua
---@meta feel

---@class FeelAnimateStep
---@field kind "animate"
---@field duration number
---@field to table<string, number>
---@field ease? string

---@class FeelEmitStep
---@field kind "emit"
---@field event string
---@field payload? table

---@class FeelParallelStep
---@field kind "parallel"
---@field steps FeelSequenceInput[]

---@alias FeelStep FeelAnimateStep|FeelEmitStep|FeelParallelStep|table
---@alias FeelStepInput FeelStep|fun(ctx: FeelContext)|string|number|boolean|nil
---@alias FeelSequenceInput string|FeelStepInput|FeelStepInput[]|nil|false

---@class FeelTarget
---@field values table<string, number>

---@class FeelContext
---@field target? FeelTarget

---@class FeelEvent
---@field kind string
---@field payload? any
---@field step FeelStep

---@class FeelPlayOptions
---@field restart? boolean
---@field key? string
---@field emit? fun(event: FeelEvent, ctx: FeelContext)

local feel = {}

---@param name string
---@param sequence FeelSequenceInput
function feel.define(name, sequence) end

---@overload fun(name: string, target?: FeelTarget, options?: FeelPlayOptions)
---@overload fun(sequence: FeelSequenceInput, target?: FeelTarget, options?: FeelPlayOptions)
---@param nameOrSequence FeelSequenceInput
---@param target? FeelTarget
---@param options? FeelPlayOptions
function feel.play(nameOrSequence, target, options) end

return feel
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_type_libraries([ProjectTypeLibrary::new(
        "lib/feel/types",
        "feel",
        "lib.feel",
    )]);
    for file in fixture.files() {
        host.set_file_with_kind(
            &file.path,
            file.source.clone(),
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("lua")
            {
                AnalysisSourceKind::Lua
            } else {
                AnalysisSourceKind::Tua
            },
        );
    }

    let snapshot = host.snapshot();
    let field = completion_labels(&snapshot.completion("src/main.tua", fixture.position("field")));
    let literal =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("literal")));
    let animate =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("animate")));
    let nested =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("nested")));
    let options =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("options")));
    let callback =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("callback")));
    let callback_nested = completion_labels(
        &snapshot.completion("src/main.tua", fixture.position("callback_nested")),
    );
    let callback_hover = snapshot
        .hover("src/main.tua", fixture.position("callback_hover"))
        .expect("expected callback parameter type should provide hover");

    assert!(field.contains(&"duration".to_owned()), "{field:?}");
    assert!(field.contains(&"event".to_owned()), "{field:?}");
    assert_eq!(literal, vec!["animate".to_owned()]);
    assert!(animate.contains(&"duration".to_owned()), "{animate:?}");
    assert!(animate.contains(&"to".to_owned()), "{animate:?}");
    assert!(!animate.contains(&"event".to_owned()), "{animate:?}");
    assert!(
        nested.is_empty(),
        "recursive alias back-edges should degrade to any: {nested:?}"
    );
    assert!(options.contains(&"restart".to_owned()), "{options:?}");
    assert!(options.contains(&"key".to_owned()), "{options:?}");
    assert!(!options.contains(&"emit".to_owned()), "{options:?}");
    assert!(callback.contains(&"kind".to_owned()), "{callback:?}");
    assert!(callback.contains(&"payload".to_owned()), "{callback:?}");
    assert!(
        callback_nested.contains(&"values".to_owned()),
        "{callback_nested:?}"
    );
    assert!(callback_hover.contents.contains("kind: string"));
}

#[test]
fn pipeline_diagnostics_report_literal_missing_modules_by_default_without_duplicates() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::with_config(ProjectConfig::default());
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(&path, "local Missing = require(\"missing\")\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let missing_requires = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MISSING_REQUIRE))
        .collect::<Vec<_>>();

    assert_eq!(missing_requires.len(), 1, "{diagnostics:?}");
}

#[test]
fn pipeline_diagnostics_report_malformed_static_imports_by_default() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::with_config(ProjectConfig::default());
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(&path, "local Player = import(\"../player\")\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);

    let module_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                Some(DiagnosticCode::LINT_BAD_REQUIRE | DiagnosticCode::LINT_MISSING_REQUIRE)
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(module_diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        module_diagnostics[0].code,
        Some(DiagnosticCode::LINT_BAD_REQUIRE)
    );
    assert_eq!(module_diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(module_diagnostics[0]
        .message
        .contains("cannot traverse parent directories"));
}

#[test]
fn pipeline_diagnostics_allow_missing_module_guidance_to_be_disabled() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();
    config.diagnostics.missing_requires = false;
    let mut host = AnalysisHost::with_config(config);
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(&path, "local Missing = require(\"missing\")\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.code != Some(DiagnosticCode::LINT_MISSING_REQUIRE) }),
        "{diagnostics:?}"
    );
}

#[test]
fn pipeline_diagnostics_report_distinct_runtime_keys_for_one_module() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::with_config(ProjectConfig::default());
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        &path,
        "local Imported = import(\"./shared\")\nlocal Required = require(\"shared.init\")\nreturn { Imported, Required }\n",
    );
    host.set_file("src/shared/init.tua", "return { enabled = true }\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let spelling = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::LINT_CANONICAL_MODULE_SPELLING)
        })
        .collect::<Vec<_>>();

    assert_eq!(spelling.len(), 1, "{diagnostics:?}");
    assert!(spelling[0].message.contains("`shared.init`"));
    assert!(spelling[0].message.contains("`shared`"));
}

#[test]
fn canonical_module_spelling_respects_the_standard_rules_switch() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();
    config.diagnostics.rules = false;
    let mut host = AnalysisHost::with_config(config);
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        &path,
        "local Imported = import(\"./shared\")\nlocal Required = require(\"shared.init\")\nreturn { Imported, Required }\n",
    );
    host.set_file("src/shared/init.tua", "return { enabled = true }\n");

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(DiagnosticCode::LINT_CANONICAL_MODULE_SPELLING)),
        "{diagnostics:?}"
    );
}

#[test]
fn pipeline_diagnostics_resolve_imports_from_existing_disk_siblings() {
    let project = TempProject::new("disk-sibling-import");
    let main = project.write(
        "src/imports.tua",
        "local Functions = import(\"./functions\")\n",
    );

    project.write("src/functions.tua", "return {}\n");

    let mut host = AnalysisHost::new();
    host.set_source_roots([project.path().join("src")]);
    host.set_file(
        &main,
        fs::read_to_string(&main).expect("source should be readable"),
    );

    let diagnostics = host.snapshot().pipeline_diagnostics(&main);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::LINT_MISSING_REQUIRE)),
        "{diagnostics:?}"
    );
}

#[test]
fn pipeline_diagnostics_use_configured_asset_roots() {
    let temp = TempProject::new("asset-roots");
    let path = temp.write(
        "src/main.tua",
        "local player = image \"assets/player.png\"\nreturn player\n",
    );

    temp.write("assets/player.png", "");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    host.set_file(
        &path,
        fs::read_to_string(&path).expect("source should be readable"),
    );

    assert!(host.snapshot().pipeline_diagnostics(&path).is_empty());
}

#[test]
fn snapshot_surfaces_love_asset_metadata_in_hover_and_completion() {
    let temp = TempProject::new("asset-metadata");
    let source = r#"local player = image "assets/<|hover|>player.png"
local next = image "assets/p<|complete|>"
local raw = love.graphics.newImage("assets/<|raw_hover|>player.png")
local rawNext = love.graphics.newImage("assets/p<|raw_complete|>")
"#;
    let fixture = Fixture::parse(&format!("-- file: {}\n{source}", "src/main.tua"));

    temp.write("assets/player.png", png_header(64, 32));
    temp.write("assets/jump.wav", b"placeholder");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("asset literal should hover");
    let raw_hover = snapshot
        .hover("src/main.tua", fixture.position("raw_hover"))
        .expect("native asset literal should hover");
    let completions = snapshot.completion("src/main.tua", fixture.position("complete"));
    let raw_completions = snapshot.completion("src/main.tua", fixture.position("raw_complete"));

    assert!(hover.contents.contains("asset `assets/player.png`"));
    assert!(hover.contents.contains("LOVE type: Image"));
    assert!(hover.contents.contains("status: valid"));
    assert!(hover.contents.contains("dimensions: 64x32"));
    assert!(raw_hover.contents.contains("asset `assets/player.png`"));
    assert!(raw_hover.contents.contains("LOVE type: Image"));
    assert_eq!(
        completion_labels(&completions),
        vec!["assets/player.png".to_owned()]
    );
    assert_eq!(
        completion_labels(&raw_completions),
        vec!["assets/player.png".to_owned()]
    );
    assert_eq!(
        completions[0].detail.as_deref(),
        Some("image asset path -> Image")
    );
}

#[test]
fn snapshot_resolves_asset_definitions_and_references_from_cached_metadata() {
    let temp = TempProject::new("asset-navigation");
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local player = image "assets/<|definition|>player.png"
-- file: src/ui.lua
local portrait = love.graphics.newImage("assets/<|reference|>player.png")
"#,
    );
    let asset_path = temp.write("assets/player.png", png_header(16, 16));
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("definition"))
        .expect("asset definition should resolve");
    let references = snapshot.references("src/main.tua", fixture.position("definition"), true);

    assert_eq!(definition.path, asset_path);
    assert_eq!(definition.kind, DefinitionKind::Asset);
    assert_eq!(
        definition.span,
        SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 1, column: 1 },
        }
    );
    assert_eq!(references.len(), 2);
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::Asset));
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.path.as_path())
            .collect::<Vec<_>>(),
        [Path::new("src/main.tua"), Path::new("src/ui.lua")]
    );
}

#[test]
fn asset_inventory_groups_usage_and_reports_unused_assets() {
    let temp = TempProject::new("asset-inventory");
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local player = image "assets/player.png"
-- file: src/ui.tua
local portrait = love.graphics.newImage('assets/player.png')
"#,
    );
    let player_path = temp.write("assets/player.png", png_header(16, 16));
    let unused_path = temp.write("assets/unused.wav", b"placeholder");
    temp.write("build/assets/player.png", png_header(16, 16));
    let mut host = AnalysisHost::new();
    host.set_output_roots([temp.path().join("build")]);
    host.set_file(
        temp.path().join("build/main.lua"),
        "local player = love.graphics.newImage(\"assets/player.png\")\n",
    );

    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    host.refresh_asset_index_from_disk();

    let inventory = host.snapshot().asset_inventory();
    assert_eq!(inventory.assets.len(), 2);
    let player = inventory
        .assets
        .iter()
        .find(|asset| asset.path == player_path)
        .expect("used image should be inventoried");
    let unused = inventory
        .assets
        .iter()
        .find(|asset| asset.path == unused_path)
        .expect("unused sound should be inventoried");

    assert_eq!(player.category, crate::AssetCategory::Image);
    assert_eq!(player.references.len(), 2);
    assert_eq!(unused.category, crate::AssetCategory::Audio);
    assert!(unused.references.is_empty());
    assert_eq!(inventory.diagnostics.len(), 1);
    assert_eq!(inventory.diagnostics[0].file, unused_path);
    assert_eq!(
        inventory.diagnostics[0].code,
        Some(DiagnosticCode::ASSET_UNUSED)
    );
    assert_eq!(
        inventory.diagnostics[0].severity,
        crate::DiagnosticSeverity::Hint
    );
}

#[test]
fn asset_file_rename_edits_update_static_references_and_preserve_quotes() {
    let temp = TempProject::new("asset-rename");
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local player = image "assets/player.png"
-- file: src/ui.tua
local portrait = love.graphics.newImage('assets/player.png')
"#,
    );
    let old_path = temp.write("assets/player.png", png_header(16, 16));
    let new_path = temp.path().join("assets/hero.png");
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    host.refresh_asset_index_from_disk();

    let edits = host
        .snapshot()
        .asset_file_rename_edits(&old_path, &new_path);

    assert_eq!(edits.len(), 2);
    assert_eq!(
        edits
            .iter()
            .map(|edit| (edit.path.as_path(), edit.new_text.as_str()))
            .collect::<Vec<_>>(),
        [
            (Path::new("src/main.tua"), "\"assets/hero.png\""),
            (Path::new("src/ui.tua"), "'assets/hero.png'")
        ]
    );
}

#[test]
fn pipeline_diagnostics_report_inconsistent_asset_path_casing() {
    let temp = TempProject::new("asset-case-diagnostic");
    let source = r#"local player = image "Assets/Player.PNG"
"#;
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    temp.write("assets/player.png", png_header(16, 16));
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    host.set_file(&path, source);
    host.refresh_asset_index_from_disk();

    let diagnostics = host.snapshot().pipeline_diagnostics(&path);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Some(DiagnosticCode::ASSET_PATH_CASE))
        .expect("path casing mismatch should be diagnosed");

    assert_eq!(diagnostic.severity, crate::DiagnosticSeverity::Warning);
    assert!(diagnostic.message.contains("assets/player.png"));
}

#[test]
fn snapshot_filters_love_asset_path_completion_by_helper_kind() {
    let temp = TempProject::new("asset-kind-completion");
    let source = r#"local player = image "assets/<|image|>"
local jump = sound "assets/<|sound|>"
local ui = font("assets/<|font|>", 18)
local wave = shader "assets/<|shader|>"
"#;
    let fixture = Fixture::parse(&format!("-- file: {}\n{source}", "src/main.tua"));

    temp.write("assets/player.png", png_header(16, 16));
    temp.write("assets/jump.wav", b"placeholder");
    temp.write("assets/ui.ttf", b"placeholder");
    temp.write("assets/wave.glsl", b"placeholder");

    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();

    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("image"))),
        vec!["assets/player.png".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("sound"))),
        vec!["assets/jump.wav".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("font"))),
        vec!["assets/ui.ttf".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("shader"))),
        vec!["assets/wave.glsl".to_owned()]
    );
}

#[test]
fn refreshed_asset_index_updates_asset_diagnostics_and_completion_without_source_change() {
    let temp = TempProject::new("asset-index-refresh");
    let source = r#"-- file: src/main.tua
local player = image "assets/player.png"
local next = image "assets/p<|complete|>"
"#;
    let fixture = Fixture::parse(source);
    let mut host = AnalysisHost::new();

    host.set_asset_roots([temp.path().to_path_buf()]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    host.refresh_asset_index_from_disk();

    let before = host.snapshot();
    assert!(before
        .pipeline_diagnostics("src/main.tua")
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::ASSET_MISSING)));
    assert!(before
        .completion("src/main.tua", fixture.position("complete"))
        .is_empty());

    temp.write("assets/player.png", png_header(24, 12));
    host.refresh_asset_index_from_disk();

    let after = host.snapshot();
    let diagnostics = after.pipeline_diagnostics("src/main.tua");
    let completions = after.completion("src/main.tua", fixture.position("complete"));

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::ASSET_MISSING)),
        "{diagnostics:?}"
    );
    assert_eq!(
        completion_labels(&completions),
        vec!["assets/player.png".to_owned()]
    );
}

#[test]
fn pipeline_diagnostics_leave_formatting_to_the_formatter() {
    let mut host = AnalysisHost::with_config(ProjectConfig::default());

    host.set_file_with_kind(
        "src/legacy.lua",
        "local value=1   \nreturn { value=value }\n",
        AnalysisSourceKind::Lua,
    );

    assert!(host
        .snapshot()
        .pipeline_diagnostics("src/legacy.lua")
        .is_empty());
}

#[test]
fn pipeline_diagnostics_run_jit_guidance_from_project_config() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();

    config.jit.enabled = true;

    let mut host = AnalysisHost::with_config(config);
    host.set_file(
        &path,
        r#"
local player = { hp = 10 }
player.name = "late"
"#,
    );

    let diagnostics = host.snapshot().diagnostics(&path);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_JIT_TABLE_SHAPE_DRIFT)));
}

#[test]
fn ecs_domain_support_is_enabled_by_default_and_can_be_disabled() {
    let path = PathBuf::from("src/ecs.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&path, "ecs.\n");

    let snapshot = host.snapshot();

    assert!(
        completion_labels(&snapshot.completion(&path, SourcePosition { line: 1, column: 5 }))
            .contains(&"world".to_owned())
    );

    let mut config = ProjectConfig::default();
    config.ecs.enabled = false;
    let mut disabled = AnalysisHost::with_config(config);
    disabled.set_file(&path, "ecs.\n");

    assert!(completion_labels(
        &disabled
            .snapshot()
            .completion(&path, SourcePosition { line: 1, column: 5 })
    )
    .is_empty());

    disabled.set_file(&path, "local value = ec\n");
    assert!(!completion_labels(&disabled.snapshot().completion(
        &path,
        SourcePosition {
            line: 1,
            column: 17
        }
    ))
    .contains(&"ecs".to_owned()));
}

#[test]
fn sprite_domain_reports_missing_assets_and_invalid_ranges() {
    let temp = TempProject::new("sprite-domain-diagnostics");
    let path = temp.path().join("src/sprites.tua");
    let source = r#"local Player = spriteSheet({
  image = "assets/missing.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 5, to = 3, fps = 0 },
    idle = { from = 0, to = 1, fps = 8 },
  },
})
"#;
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_asset_roots([temp.path().to_path_buf()]);
    host.set_file(&path, source);

    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics(&path);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
            && diagnostic.message.contains("missing sprite image asset")
            && diagnostic.line == 2
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
            && diagnostic.message.contains("invalid frame range")
            && diagnostic.line == 5
    }));
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
                && diagnostic.message.contains("fps must be greater than zero")
                && diagnostic.line == 5
        }),
        "{diagnostics:#?}"
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
            && diagnostic.message.contains("defined more than once")
            && diagnostic.line == 6
    }));
}

#[test]
fn sprite_inspection_uses_cached_declarations_assets_and_diagnostics() {
    let temp = TempProject::new("sprite-inspection");
    let path = temp.path().join("src/sprites.tua");
    let image_path = temp.write("src/assets/player.png", b"placeholder");
    let source = r#"local Player = spriteSheet({
  image = "assets/player.png",
  frame = { width = 16, height = 16 },
  animations = {
    idle = { from = 0, to = 3, fps = 8 },
    idle = { from = 4, to = 2, fps = 12 },
  },
})
local selected = Player
"#;
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&path, source);
    host.refresh_asset_index_from_disk();

    let snapshot = host.snapshot();
    let inspections = snapshot.inspect(&path, None);
    let InspectorItem::SpriteSheet(sprite) = &inspections[0] else {
        panic!("expected sprite sheet inspection");
    };

    assert_eq!(sprite.name, "Player");
    assert_eq!(
        sprite.frame.map(|frame| (frame.width, frame.height)),
        Some((16, 16))
    );
    assert_eq!(sprite.animations.len(), 2);
    assert_eq!(
        sprite
            .image
            .as_ref()
            .and_then(|image| image.resolved_path.as_ref()),
        Some(&image_path)
    );
    assert!(sprite.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
            && diagnostic.message.contains("defined more than once")
    }));
    assert!(sprite.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE)
            && diagnostic.message.contains("invalid frame range")
    }));
    assert_eq!(
        snapshot
            .inspect(
                &path,
                Some(SourcePosition {
                    line: 9,
                    column: 20,
                }),
            )
            .len(),
        1,
        "sprite usages should resolve back to their inspected declaration"
    );
}

#[test]
fn incomplete_sprite_configuration_is_shared_by_diagnostics_and_inspection() {
    let path = PathBuf::from("src/sprites.tua");
    let source = "local Player = spriteSheet({})\n";
    let mut host = AnalysisHost::new();

    host.set_file(&path, source);

    let snapshot = host.snapshot();
    let pipeline_messages = snapshot
        .pipeline_diagnostics(&path)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::DOMAIN_SPRITE))
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    let inspections = snapshot.inspect(&path, None);
    let InspectorItem::SpriteSheet(sprite) = &inspections[0] else {
        panic!("expected sprite sheet inspection");
    };
    let inspection_messages = sprite
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();

    assert_eq!(inspection_messages, pipeline_messages);
    assert!(inspection_messages
        .iter()
        .any(|message| message.contains("image string path")));
    assert!(inspection_messages
        .iter()
        .any(|message| message.contains("frame width and height")));
    assert!(inspection_messages
        .iter()
        .any(|message| message.contains("at least one animation")));
}

#[test]
fn state_machine_inspection_reuses_static_states_transitions_and_source_spans() {
    let path = PathBuf::from("src/flow.tua");
    let source = r#"local flow = stateMachine({
  initial = "idle",
  events = {
    { name = "start", from = "idle", to = "running" },
    { name = "reset", from = { "running", "paused" }, to = "idle" },
    { name = "stop", from = "*", to = "idle" },
  },
})
local selected = flow
"#;
    let mut host = AnalysisHost::new();

    host.set_file(&path, source);

    let snapshot = host.snapshot();
    let inspections = snapshot.inspect(&path, None);
    let machine = inspections
        .iter()
        .find_map(|item| match item {
            InspectorItem::StateMachine(machine) => Some(machine),
            _ => None,
        })
        .expect("state machine inspection");

    assert_eq!(machine.name, "flow");
    assert_eq!(machine.initial, "idle");
    assert_eq!(
        machine
            .states
            .iter()
            .map(|state| (state.name.as_str(), state.initial))
            .collect::<Vec<_>>(),
        vec![("idle", true), ("paused", false), ("running", false)]
    );
    assert_eq!(machine.transitions.len(), 3);
    assert_eq!(machine.transitions[0].event, "start");
    assert_eq!(machine.transitions[0].from, ["idle"]);
    assert_eq!(machine.transitions[0].to, "running");
    assert_eq!(machine.transitions[1].from, ["running", "paused"]);
    assert!(machine.transitions[2].wildcard_from);
    assert_eq!(machine.transitions[2].to, "idle");
    assert!(machine.transitions[0].event_span.start.line > 1);

    let usage_inspections = snapshot.inspect(
        &path,
        Some(SourcePosition {
            line: 9,
            column: 18,
        }),
    );
    assert!(usage_inspections.iter().any(
        |item| matches!(item, InspectorItem::StateMachine(machine) if machine.name == "flow")
    ));
}

#[test]
fn ecs_domain_support_flows_through_diagnostics_and_semantic_surfaces() {
    let path = PathBuf::from("src/ecs.tua");
    let completion_path = PathBuf::from("src/ecs_completion.tua");
    let mut config = ProjectConfig::default();

    config.ecs.enabled = true;

    let mut host = AnalysisHost::with_config(config);
    host.set_file(
        &path,
        r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local entity = world:create()
world:add(entity, Position, { x = 1, y = "slow" })
ecs.system(world, { Position, Missing }, function() end)
"#,
    );
    host.set_file(
        &completion_path,
        r#"local world = ecs.world()
local api = ecs.w
local call = world.a
"#,
    );

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics(&path);
    let ecs_labels = completion_labels(&snapshot.completion(
        &completion_path,
        SourcePosition {
            line: 2,
            column: 18,
        },
    ));
    let world_labels = completion_labels(&snapshot.completion(
        &completion_path,
        SourcePosition {
            line: 3,
            column: 21,
        },
    ));
    let hover = snapshot
        .hover(&path, SourcePosition { line: 2, column: 7 })
        .expect("world local should hover");
    let signature = snapshot
        .signature_help(
            &path,
            SourcePosition {
                line: 4,
                column: 11,
            },
        )
        .expect("world:add should have signature help");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE)
            && diagnostic.message.contains("expected number")
    }));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::DOMAIN_ECS_QUERY)));
    assert!(ecs_labels.contains(&"world".to_owned()));
    assert!(world_labels.contains(&"add".to_owned()));
    assert_eq!(hover.contents, "local world: EcsWorld");
    assert_eq!(
        signature.signatures[0].label,
        "function world:add(entity: EcsEntityId, component: EcsComponent, value: any): any"
    );
}

#[test]
fn pipeline_diagnostics_apply_default_inline_ignores() {
    let mut host = AnalysisHost::with_config(ProjectConfig::default());

    host.set_file(
        "src/main.tua",
        r#"
-- tua-ignore-next-line TL1003
local value: number = "legacy"

-- tua-ignore-next-line TL4018
local clean = true
"#,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics("src/main.tua");

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED)));
}

#[test]
fn object_pool_graph_tracks_lifecycle_and_warns_for_visible_unreleased_acquire() {
    let mut host = AnalysisHost::new();

    host.set_file(
        "src/pool.tua",
        r#"type Bullet = { active: boolean }
local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
})

local leaked = bullets:acquire()
local released = bullets:acquire()
bullets:release(released)
"#,
    );

    let snapshot = host.snapshot();
    let graph = snapshot.object_pool_graph();
    let diagnostics = snapshot.pipeline_diagnostics("src/pool.tua");

    assert_eq!(graph.pools.len(), 1);
    assert_eq!(graph.pools[0].name, "bullets");
    assert_eq!(graph.pools[0].item_type_label, "Bullet");
    assert_eq!(
        graph
            .operations
            .iter()
            .map(|operation| operation.kind)
            .collect::<Vec<_>>(),
        vec![
            crate::ObjectPoolOperationKind::Acquire,
            crate::ObjectPoolOperationKind::Acquire,
            crate::ObjectPoolOperationKind::Release,
        ]
    );
    assert_eq!(graph.insights.len(), 1);
    assert_eq!(graph.insights[0].item_name, "leaked");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::DOMAIN_OBJECT_POOL_LIFECYCLE)
            && diagnostic.message.contains("leaked")
    }));
}

#[test]
fn object_pool_handoff_suppression_is_local_to_one_acquisition() {
    let mut host = AnalysisHost::new();
    host.set_file(
        "src/pool.tua",
        r#"type Particle = { active: boolean }
local effects = objectPool({
  create = function(): Particle
    return { active = false }
  end,
})

-- An ECS cleanup system owns the eventual release of this particle.
-- tua-ignore-next-line TL5007
local transferred = effects:acquire()
local forgotten = effects:acquire()
"#,
    );

    let diagnostics = host.snapshot().pipeline_diagnostics("src/pool.tua");
    let pool_warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::DOMAIN_OBJECT_POOL_LIFECYCLE))
        .collect::<Vec<_>>();
    assert_eq!(pool_warnings.len(), 1, "{diagnostics:#?}");
    assert!(pool_warnings[0].message.contains("forgotten"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == Some(DiagnosticCode::LINT_INLINE_IGNORE_UNUSED) }));
}

#[test]
fn object_pool_graph_ignores_non_pool_object_pool_overrides() {
    let mut host = AnalysisHost::new();

    host.set_file(
        "src/pool.tua",
        r#"local function objectPool()
  return "custom"
end

local notPool = objectPool()
"#,
    );

    let graph = host.snapshot().object_pool_graph();

    assert!(graph.pools.is_empty());
    assert!(graph.operations.is_empty());
    assert!(graph.insights.is_empty());
}

fn completion_labels(completions: &[Completion]) -> Vec<String> {
    completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect()
}

#[test]
fn snapshot_recovers_semantics_when_field_chain_statement_is_incomplete() {
    let fixture = Fixture::parse(
        r#"-- file: src/conf.tua
function love.conf(t)
  local bad: number = "oops"
  local title = t.window.<|hover|>title
  local identity = t.identity -- Optional config fields retain nil during recovery.
  t.modules.font<|complete|>
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics("src/conf.tua");
    let hover = snapshot
        .hover("src/conf.tua", fixture.position("hover"))
        .expect("hover should recover before incomplete field chain");
    let completions = snapshot.completion("src/conf.tua", fixture.position("complete"));
    let hints = snapshot.inlay_hints("src/conf.tua");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "{diagnostics:?}"
    );
    assert!(hover.contents.contains("title"), "{}", hover.contents);
    assert_eq!(completion_labels(&completions), vec!["font".to_owned()]);
    assert_eq!(
        hints
            .iter()
            .map(|hint| hint.label.as_str())
            .collect::<Vec<_>>(),
        vec![": string | nil", ": string | nil"]
    );
    assert_eq!(
        snapshot
            .degraded_facts
            .lock()
            .expect("degraded semantic facts lock poisoned")
            .len(),
        1,
        "degraded hover, completion, and inlay hints should share one cached environment"
    );
}

#[test]
fn snapshot_inlay_hints_recover_from_a_bare_identifier_statement() {
    let valid = r#"type Player = { name: string }

local function makePlayer(name: string): Player
  return { name = name }
end

local player = makePlayer("Ada")
local score = 1

return { player = player, score = score }
"#;
    let invalid = valid.replace(
        "local score = 1\n\nreturn",
        "local score = 1\nplayer\nreturn",
    );
    let mut valid_host = AnalysisHost::new();
    let mut invalid_host = AnalysisHost::new();

    valid_host.set_file("src/main.tua", valid);
    invalid_host.set_file("src/main.tua", invalid);

    let valid_hints = valid_host.snapshot().inlay_hints("src/main.tua");
    let invalid_hints = invalid_host.snapshot().inlay_hints("src/main.tua");

    assert!(!valid_hints.is_empty());
    assert_eq!(invalid_hints, valid_hints);
}

#[test]
fn bare_identifier_recovery_preserves_lua_structure_keywords() {
    assert!(line_is_recoverable_incomplete_edit("player"));
    assert!(line_is_recoverable_incomplete_edit("player.position"));
    assert!(!line_is_recoverable_incomplete_edit("end"));
    assert!(!line_is_recoverable_incomplete_edit("return"));
}

#[test]
fn snapshot_combines_current_completion_context_with_last_good_semantics() {
    let valid = r#"type Player = {
  hp: number,
}

local player: Player = { hp = 100 }
local health = player.hp
"#;
    let invalid = Fixture::parse(
        r#"-- file: src/main.tua
type Player = {
  hp: number,
}

local <|hover|>player: Player = { hp = 100 }
local health = player.hp
local unfinished = (
local nextPlayer: Pl<|complete|>
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, invalid.source("src/main.tua"));

    let snapshot = host.snapshot();

    assert!(matches!(snapshot.parse(&path), Some(Err(_))));
    assert_eq!(
        snapshot
            .hover(&path, invalid.position("hover"))
            .expect("hover should use the last valid semantic snapshot")
            .contents,
        "local player: Player"
    );
    assert!(
        completion_labels(&snapshot.completion(&path, invalid.position("complete")))
            .contains(&"Player".to_owned())
    );
    assert!(snapshot
        .inlay_hints(&path)
        .iter()
        .any(|hint| hint.label == ": number"));
}

#[test]
fn love_conf_parameter_type_does_not_escape_callback_body() {
    let fixture = Fixture::parse(
        r#"-- file: src/conf.tua
function love.conf(t)
  t.window.title = "inside"
end

local outside = t.window.<|hover|>title
local field = t.<|complete|>
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();

    assert!(snapshot
        .hover("src/conf.tua", fixture.position("hover"))
        .is_none());
    assert!(snapshot
        .completion("src/conf.tua", fixture.position("complete"))
        .is_empty());
}

#[test]
fn fast_diagnostics_include_current_file_errors_without_slow_asset_validation() {
    let path = PathBuf::from("src/main.tua");
    let mut config = ProjectConfig::default();
    config.diagnostics.undefined_variables = true;
    let mut host = AnalysisHost::with_config(config);

    host.set_file(
        &path,
        r#"
type Broken = MissingType
local hp: number = "full"
missing()
local sprite = image "missing.png"
"#,
    );

    let diagnostics = host.fast_diagnostics(&path);
    let codes = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert!(codes.contains(&DiagnosticCode::TYPE_MISMATCH));
    assert!(codes.contains(&DiagnosticCode::LINT_UNDEFINED_VARIABLE));
    assert!(!codes.contains(&DiagnosticCode::ASSET_MISSING));
}

#[test]
fn fast_diagnostics_defer_type_import_names_to_project_analysis() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"
import type Shared from "./types"
import type { Player as NamedPlayer } from "./types"

local first: Shared.Player = { hp = 10 }
local second: NamedPlayer = first
local missing: MissingType = second
"#,
    );

    let diagnostics = host.fast_diagnostics(&path);

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| { diagnostic.code == Some(DiagnosticCode::TYPE_UNKNOWN_NAME) })
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["unknown type `MissingType`"]
    );
}

#[test]
fn fast_diagnostics_defer_mismatches_when_project_types_can_change_the_answer() {
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        "src/main.tua",
        r#"import type { Player } from "./types"
local player: Player = { hp = "high" }
return player
"#,
    );
    host.set_file("src/types.tua", "export type Player = { hp: number }\n");

    let fast = host.fast_diagnostics("src/main.tua");
    assert!(
        fast.iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "project-dependent mismatch must not flash in fast diagnostics: {fast:#?}"
    );

    let settled = host.snapshot().diagnostics("src/main.tua");
    assert!(
        settled
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "the project-aware pass must still report the real mismatch: {settled:#?}"
    );
}

#[test]
fn imported_type_alias_can_name_a_runtime_class_instance() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { DialogueInstance } from "./dialogue"
local DialogueModule = require("dialogue")
local dialogue: DialogueInstance = DialogueModule.Dialogue.new("Hello")
local _ = dialogue.up<|members|>
dialogue:update(<|signature|>2)
dialogue.<|field|>text = "Goodbye"
-- file: src/dialogue.tua
final class Dialogue {
  text: string

  init(text: string) {
    self.text = text
  }

  update(dt: number): string {
    return self.text
  }
}

export type DialogueInstance = Dialogue
return { Dialogue = Dialogue }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:#?}"
    );
    let hover = snapshot
        .hover("src/main.tua", fixture.position("field"))
        .expect("imported class instance field should hover");

    assert_eq!(hover.contents, "(field) text: string");

    let completions = snapshot.completion("src/main.tua", fixture.position("members"));
    assert!(
        completions
            .iter()
            .any(|completion| completion.label == "update"),
        "imported instance completion should include class methods: {completions:#?}"
    );
    let signature = snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .expect("imported instance method should provide signature help");
    assert!(signature.signatures[0].label.contains("update(dt: number)"));
}

#[test]
fn inferred_imported_class_instances_keep_their_method_receiver_identity() {
    let fixture = Fixture::parse(
        r#"-- file: src/actors.tua
final class Guard {
  level(): number { return 1 }
}
export type GuardInstance = Guard
return { Guard = Guard }
-- file: src/rules.tua
import type { GuardInstance } from "./actors"
local Actors = require("actors")
local function make(): GuardInstance
  return Actors.Guard.new()
end
return { make = make }
-- file: src/main.tua
local Actors = require("actors")
local Rules = require("rules")
local direct = Actors.Guard.new()
local forwarded = Rules.make()
local a = direct:level()
local b = forwarded:level()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "inferred imported instances should keep their receiver: {diagnostics:#?}"
    );
}

#[test]
fn imported_child_alias_upcasts_with_a_separate_protocol_bound() {
    let fixture = Fixture::parse(
        r#"-- file: src/actors.tua
export protocol Alertable { raise(amount: number): nil }
open class Actor { label(): string { return "actor" } }
final class Guard extends Actor implements Alertable {
  raise(amount: number): nil {}
  override label(): string { return "guard" }
}
export type ActorInstance = Actor
export type GuardInstance = Guard
return { Guard = Guard }
-- file: src/main.tua
import type { ActorInstance, Alertable, GuardInstance } from "./actors"
local Actors = require("actors")
local function alarm<T: Alertable>(subject: T): T
  subject:raise(1)
  return subject
end
local function throughAlias(guard: GuardInstance): string
  alarm(guard)
  local actors: ActorInstance[] = { guard }
  return actors[1]:label()
end
assert(throughAlias(Actors.Guard.new()) == "guard")
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "the unrelated protocol bound must not affect subclass upcasts: {diagnostics:#?}"
    );
}

#[test]
fn unrelated_imported_class_does_not_upcast_through_a_protocol_bound() {
    let fixture = Fixture::parse(
        r#"-- file: src/actors.tua
export protocol Alertable { raise(amount: number): nil }
open class Actor {}
final class Guard extends Actor implements Alertable {
  raise(amount: number): nil {}
}
final class Stranger {}
export type ActorInstance = Actor
export type StrangerInstance = Stranger
return { Stranger = Stranger }
-- file: src/main.tua
import type { ActorInstance, Alertable, StrangerInstance } from "./actors"
local Actors = require("actors")
local function alarm<T: Alertable>(subject: T): T
  return subject
end
local stranger: StrangerInstance = Actors.Stranger.new()
local actors: ActorInstance[] = { stranger }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "an unrelated class must remain incompatible: {diagnostics:#?}"
    );
}

#[test]
fn imported_generic_class_alias_uses_runtime_constructor_and_shared_members() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { BoxInstance as ImportedBox } from "./box"
local BoxModule = require("box")
local box = BoxModule.Box.new(3)
local typed: <|alias|>ImportedBox<number> = box
local _value: number = box.<|field_definition|>value
local _method_value: number = typed:<|method_definition|>get(<|signature|>)
local _ = box.ge<|members|>
local _shown = box.<|hover|>value
-- file: src/box.tua
final class Box<T> {
  value: T

  init(value: T) {
    self.value = value
  }

  get(): T {
    return self.value
  }
}

export type BoxInstance<T> = Box<T>
return { Box = Box }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    let environment = snapshot.project_type_environment("src/main.tua").unwrap();
    assert!(
        environment
            .generic_alias_definition_for_local_name("ImportedBox")
            .is_some(),
        "the project environment should retain the imported alias spelling"
    );
    assert!(environment.generic_alias_definitions().any(|definition| {
        matches!(definition.body.as_ref(), Type::GenericClassApplication { name, .. } if name == "Box")
    }), "the imported generic instance alias must retain its producer class identity");
    assert!(
        diagnostics.is_empty(),
        "imported generic class members should retain their producer identity: {diagnostics:#?}"
    );
    assert_eq!(
        snapshot
            .hover("src/main.tua", fixture.position("hover"))
            .expect("imported generic field should hover")
            .contents,
        "(field) value: number"
    );
    let alias_hover = snapshot
        .hover("src/main.tua", fixture.position("alias"))
        .expect("an imported generic alias should show its declaration signature");
    assert_eq!(alias_hover.contents, "type ImportedBox<T> = Box<T>");
    let method_definition = snapshot
        .definition("src/main.tua", fixture.position("method_definition"))
        .expect("an imported generic class method should navigate to its declaration");
    assert_eq!(method_definition.path, PathBuf::from("src/box.tua"));
    assert_eq!(
        text_at_span(fixture.source("src/box.tua"), method_definition.span),
        "get"
    );
    let field_definition = snapshot
        .definition("src/main.tua", fixture.position("field_definition"))
        .expect("an imported generic class field should navigate to its declaration");
    assert_eq!(field_definition.path, PathBuf::from("src/box.tua"));
    assert_eq!(
        text_at_span(fixture.source("src/box.tua"), field_definition.span),
        "value"
    );
    let completions = snapshot.completion("src/main.tua", fixture.position("members"));
    assert!(
        completions.iter().any(|item| {
            item.label == "get"
                && item
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("number"))
        }),
        "imported generic member completion should show the instantiated type: {completions:#?}"
    );
    let signature = snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .expect("imported generic method should provide signature help");
    assert!(
        signature.signatures[0].label.contains("get(): number"),
        "signature should use the instantiated imported class type: {signature:#?}"
    );
}

#[test]
fn nested_imported_generic_class_field_hover_matches_completion() {
    let fixture = Fixture::parse(
        r#"-- file: src/game.tua
final class Defender {
  speed: number
  init() { self.speed = 3 }
}
final class Slot<T> {
  value: T
  init(value: T) { self.value = value }
}
local selected = Slot.new(Defender.new())
return { selected = selected }
-- file: src/main.tua
local Game = require("game")
local chosen = Game.selected.<|hover|>value
local speed = Game.selected.value.<|members|>speed
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("the nested imported field should hover");
    assert!(hover.contents.contains("Defender"), "{hover:#?}");
    assert_eq!(
        hover
            .span
            .map(|span| text_at_span(fixture.source("src/main.tua"), span)),
        Some("value".to_owned())
    );
    let completions = snapshot.completion("src/main.tua", fixture.position("members"));
    assert!(
        completions.iter().any(|item| item.label == "speed"),
        "the same field should keep its member completions: {completions:#?}"
    );

    host.set_file(
        "src/game.tua",
        fixture
            .source("src/game.tua")
            .replace("speed: number", "speed: string")
            .replace("self.speed = 3", "self.speed = \"fast\""),
    );
    let refreshed = host.snapshot();
    let completions = refreshed.completion("src/main.tua", fixture.position("members"));
    assert!(
        completions.iter().any(|item| {
            item.label == "speed"
                && item
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("string"))
        }),
        "an imported field edit should refresh its nested member type: {completions:#?}"
    );
}

#[test]
fn reexported_generic_class_metadata_refreshes_after_unsaved_provider_edits() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Types from "./barrel"
local Bundle = require("barrel")
local box = Bundle.Box.new(3)
local typed: Types.BoxInstance<number> = box
local _value: number = typed:get()
local _shown = typed.<|hover|>get
-- file: src/barrel.tua
export type { BoxInstance } from "./box"
local Provider = require("box")
return { Box = Provider.Box }
-- file: src/box.tua
final class Box<T> {
  value: T
  init(value: T) { self.value = value }
  get(): T { return self.value }
}
export type BoxInstance<T> = Box<T>
return { Box = Box }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let initial = host.snapshot();
    assert!(
        initial.diagnostics("src/main.tua").is_empty(),
        "namespace type alias plus runtime re-export should resolve: {:#?}",
        initial.diagnostics("src/main.tua")
    );
    assert!(initial
        .hover("src/main.tua", fixture.position("hover"))
        .is_some());

    host.set_file(
        "src/box.tua",
        r#"final class Box<T> {
  value: T
  init(value: T) { self.value = value }
  get(): string { return "ready" }
}
export type BoxInstance<T> = Box<T>
return { Box = Box }
"#,
    );
    let edited = host.snapshot();
    let diagnostics = edited.diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("expected number, found string")
        }),
        "unsaved provider signature edit should refresh consumer diagnostics: {diagnostics:#?}"
    );
    assert!(
        edited
            .hover("src/main.tua", fixture.position("hover"))
            .is_some(),
        "unsaved provider edit should retain imported member hover"
    );
}

#[test]
fn imported_record_aliases_accept_values_with_nested_exported_aliases() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"

local function unit(): Shared.Unit
  return { position = { x = 1, y = 2 }, color = { 1, 0.5, 0 } }
end

local function consume(value: Shared.Unit): Shared.Unit
  return value
end

local local_unit: Shared.Unit = {
  position = { x = 3, y = 4 },
  color = { 0, 0.5, 1 },
}

local units: Shared.Unit[] = {
  { position = { x = 5, y = 6 }, color = { 1, 1, 1 } },
}

return consume(unit()), local_unit, units
-- file: src/types.tua
export type Point = { x: number, y: number }
export type Color = number[]
export type Unit = { position: Point, color: Color }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "nested exported aliases should resolve before structural assignability: {diagnostics:#?}"
    );
}

#[test]
fn selected_imported_return_aliases_survive_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Types from "./types"
local Pieces = require("pieces")

local piece = Pieces.spawn()
local function consumeKind(kind: Types.Kind)
  return kind
end

consumeKind(piece.kind)
local invalid: number = piece.<|complete|><|hover|>kind
return invalid
-- file: src/pieces.tua
import type { Piece } from "./types"

local function spawn(): Piece
  return { kind = "I", x = 1 }
end

return { spawn = spawn }
-- file: src/types.d.tua
export type Kind = "I" | "O"
export type Piece = { kind: Kind, x: number }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let fast_diagnostics = host.fast_diagnostics(Path::new("src/main.tua"));
    assert!(
        fast_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "fast diagnostics must defer selected imported aliases: {fast_diagnostics:#?}"
    );

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("selected imported return field should hover");
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    let mismatches = snapshot
        .diagnostics("src/main.tua")
        .into_iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH))
        .collect::<Vec<_>>();

    assert_eq!(hover.contents, "(field) kind: \"I\" | \"O\"");
    assert!(labels.contains(&"kind".to_owned()), "{labels:?}");
    assert!(labels.contains(&"x".to_owned()), "{labels:?}");
    assert_eq!(
        mismatches.len(),
        1,
        "expected a number mismatch: {mismatches:#?}"
    );
}

#[test]
fn project_resolved_constructor_contracts_are_not_widened_in_the_checker() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Types from "./types"
local Game = require("game")

local game = Game.newGame()
local function consume(value: Types.Game)
  return value
end

return consume(game)
-- file: src/game.tua
import type Types from "./types"

local function newGame(): Types.Game
  return { mode = "playing" }
end

return { newGame = newGame }
-- file: src/types.d.tua
export type GameMode = "playing" | "paused" | "gameover"
export type Game = { mode: GameMode }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "the checker should retain the project-resolved literal contract: {diagnostics:#?}"
    );
}

#[test]
fn indexed_imported_literal_union_contracts_remain_narrow() {
    let fixture = Fixture::parse(
        r#"-- file: src/pieces.tua
import type { PieceKind } from "./types"

local kinds: PieceKind[] = { "I", "O" }
local kind = kinds[1]

local function consume(value: PieceKind)
  return value
end

return consume(kind)
-- file: src/types.d.tua
export type PieceKind = "I" | "O"
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/pieces.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "indexed declared unions should retain their item contract: {diagnostics:#?}"
    );
}

#[test]
fn named_imported_union_inside_record_return_preserves_all_members() {
    let fixture = Fixture::parse(
        r#"-- file: src/ledger.tua
import type { Category, Expense } from "./types"

local function make(category: Category): Expense
  return { category = category }
end

return { make = make }
-- file: src/types.d.tua
export type Category = "food" | "travel" | "tools"
export type Expense = { category: Category }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/ledger.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "all imported literal-union members should be accepted in an imported record: {diagnostics:#?}"
    );
}

#[test]
fn namespace_imported_declaration_records_accept_array_literals() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Types from "./types"

local function points(kind: Types.Kind): Types.Point[]
  if kind == "A" then
    return { { x = 1 } }
  end
  return { { x = 2 } }
end

return points("A")
-- file: src/types.d.tua
export type Kind = "A" | "B"
export type Point = { x: number }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "namespace declaration aliases should resolve for array items: {diagnostics:#?}"
    );
}

#[test]
fn fast_diagnostics_include_inferred_local_reassignment_errors() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"
local score = 10
score = "high"
"#,
    );

    let diagnostics = host.fast_diagnostics(&path);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
            && diagnostic.message.contains("expected number, found string")
    }));
}

#[test]
fn cache_reuses_unedited_files_across_snapshots() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&main, "local main: number = 1\n");
    host.set_file(&other, "local other: number = 2\n");

    let first = host.snapshot();
    assert!(first.parse(&main).is_some());
    assert!(first.parse(&other).is_some());
    assert_eq!(host.cache_stats().parse_misses, 2);
    assert_eq!(host.cache_stats().parse_hits, 0);

    let stats_after_first = host.cache_stats();
    let second = host.snapshot();
    assert!(second.parse(&main).is_some());
    assert!(second.parse(&other).is_some());

    // A warm host snapshot is memoized by revision, so unchanged snapshots do
    // not need to touch the per-file query cache at all.
    assert_eq!(host.cache_stats(), stats_after_first);
}

#[test]
fn current_file_queries_do_not_build_unrelated_files() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &main,
        r#"type Player = { hp: number }
local function spawn()
end
"#,
    );
    host.set_file(&other, "local other: number = 2\n");

    let symbols = host.document_symbols(&main);
    let stats_after_symbols = host.cache_stats();

    assert!(symbols.iter().any(|symbol| symbol.name == "spawn"));
    assert_eq!(stats_after_symbols.parse_misses, 1);
    assert_eq!(stats_after_symbols.hir_misses, 1);

    let tokens = host.semantic_tokens(&main);
    let stats_after_tokens = host.cache_stats();

    assert!(!tokens.is_empty());
    assert_eq!(stats_after_tokens.parse_misses, 1);
    assert_eq!(stats_after_tokens.hir_misses, 1);
    assert_eq!(stats_after_tokens.parse_hits, 1);
}

#[test]
fn editing_one_file_does_not_reparse_unrelated_files() {
    let main = PathBuf::from("src/main.tua");
    let other = PathBuf::from("src/other.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&main, "local main: number = 1\n");
    host.set_file(&other, "local other: number = 2\n");
    host.snapshot();

    host.set_file(&main, "local main: number = 3\n");
    host.snapshot();

    let stats = host.cache_stats();

    assert_eq!(stats.parse_misses, 3);
    assert_eq!(stats.parse_hits, 1);
    assert_eq!(stats.invalidations, 1);
}

#[test]
fn module_dependency_graph_reports_relationships_cycles_exports_and_inclusion_paths() {
    let temp = TempProject::new("module-dependency-graph");
    let main = temp.write(
        "src/main.tua",
        r#"local Player = require("player")
local Missing = import("missing")
return { player = Player, missing = Missing }
"#,
    );
    let player = temp.write(
        "src/player.tua",
        r#"local Shared = require("shared")
return { hp = 100, shared = Shared }
"#,
    );
    let shared = temp.write(
        "src/shared.lua",
        r#"local Player = require("player")
return { ready = true, player = Player }
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    for path in [&main, &player, &shared] {
        host.set_file(path, fs::read_to_string(path).expect("module should read"));
    }

    let graph = host.snapshot().module_dependency_graph();
    let main_module = graph
        .modules
        .iter()
        .find(|module| module.path == main)
        .expect("main module should be included");
    let player_module = graph
        .modules
        .iter()
        .find(|module| module.path == player)
        .expect("player module should be included");
    let shared_module = graph
        .modules
        .iter()
        .find(|module| module.path == shared)
        .expect("shared module should be included");

    assert_eq!(graph.modules.len(), 3);
    assert_eq!(graph.edges.len(), 4);
    assert!(main_module.is_root);
    assert_eq!(main_module.unresolved_dependency_count, 1);
    assert_eq!(player_module.reverse_dependency_count, 2);
    assert!(player_module.in_cycle);
    assert!(shared_module.in_cycle);
    assert_eq!(shared_module.source_kind, AnalysisSourceKind::Lua);
    assert_eq!(graph.cycles, vec![vec![player.clone(), shared.clone()]]);
    assert!(graph.edges.iter().any(|edge| {
        edge.source == main
            && edge.specifier == "missing"
            && matches!(edge.resolution, RequireResolution::Missing { .. })
    }));
    assert!(player_module
        .exports
        .iter()
        .any(|export| export.name == "hp" && export.type_label == "number"));
    assert_eq!(
        player_module
            .inclusion_path
            .as_ref()
            .expect("player should have an inclusion path")
            .steps
            .iter()
            .map(|step| step.specifier.as_str())
            .collect::<Vec<_>>(),
        ["player"]
    );
    assert_eq!(
        shared_module
            .inclusion_path
            .as_ref()
            .expect("shared should have an inclusion path")
            .steps
            .iter()
            .map(|step| step.specifier.as_str())
            .collect::<Vec<_>>(),
        ["player", "shared"]
    );
}

#[test]
fn changing_exported_module_invalidates_known_importers() {
    let temp = TempProject::new("module-dependency");

    temp.write("tua.toml", "source = \"src\"\n");
    let main = temp.write(
        "src/main.tua",
        r#"
local Player = require("player")
return Player.new()
"#,
    );
    let player = temp.write(
        "src/player.tua",
        r#"
local M = {}
function M.new()
  return { hp = 100 }
end
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );
    host.snapshot();

    fs::write(
        &player,
        r#"
local M = {}
function M.new()
  return { hp = "full" }
end
return M
"#,
    )
    .expect("player should update");
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );

    let after_update = host.cache_stats();
    assert_eq!(after_update.invalidations, 2);
    assert_eq!(after_update.dependent_invalidations, 1);
    assert_eq!(after_update.require_surface_invalidations, 0);
    assert_eq!(after_update.export_surface_invalidations, 1);
    assert_eq!(after_update.type_surface_invalidations, 1);

    host.snapshot();

    let after_snapshot = host.cache_stats();
    assert_eq!(after_snapshot.parse_misses, 3);
    assert_eq!(after_snapshot.parse_hits, 2);
    assert_eq!(after_snapshot.diagnostics_misses, 4);
}

#[test]
fn changing_exported_module_invalidates_transitive_importers() {
    let temp = TempProject::new("transitive-module-dependency");
    let main = temp.write(
        "src/main.tua",
        "local Facade = require(\"facade\")\nreturn { new = Facade.new }\n",
    );
    let facade = temp.write(
        "src/facade.tua",
        "local Model = require(\"model\")\nreturn { new = Model.new }\n",
    );
    let model = temp.write(
        "src/model.tua",
        "local M = {}\nfunction M.new()\n  return { hp = 100 }\nend\nreturn M\n",
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file(
        &facade,
        fs::read_to_string(&facade).expect("facade should read"),
    );
    host.set_file(
        &model,
        fs::read_to_string(&model).expect("model should read"),
    );
    host.snapshot();

    let surface_changed = host.update_file_with_kind(
        &model,
        "local M = {}\nfunction M.new()\n  return { hp = \"full\" }\nend\nreturn M\n",
        AnalysisSourceKind::Tua,
    );
    let stats = host.cache_stats();

    assert!(surface_changed);
    assert_eq!(stats.invalidations, 3);
    assert_eq!(stats.dependent_invalidations, 2);
    assert_eq!(stats.require_surface_invalidations, 0);
    assert_eq!(stats.export_surface_invalidations, 1);
    assert_eq!(stats.type_surface_invalidations, 1);
}

#[test]
fn changing_one_exported_type_invalidates_only_consumers_of_that_type() {
    let temp = TempProject::new("selective-type-dependency");
    let player_user = temp.write(
        "src/player_user.tua",
        "import type { Player } from \"./types\"\nlocal value: Player\n",
    );
    let room_user = temp.write(
        "src/room_user.tua",
        "import type { Room } from \"./types\"\nlocal value: Room\n",
    );
    let types = temp.write(
        "src/types.d.tua",
        "export type Player = { hp: number }\nexport type Room = { name: string }\n",
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    for path in [&player_user, &room_user, &types] {
        host.set_file(path, fs::read_to_string(path).expect("source should read"));
    }
    host.snapshot();

    let changed = host.update_file_with_kind(
        &types,
        "export type Player = { hp: string }\nexport type Room = { name: string }\n",
        AnalysisSourceKind::Tua,
    );
    let stats = host.cache_stats();

    assert!(changed);
    assert_eq!(stats.invalidations, 2);
    assert_eq!(stats.dependent_invalidations, 1);
    assert_eq!(stats.type_surface_invalidations, 1);
}

#[test]
fn changing_required_module_body_without_surface_change_keeps_importers_cached() {
    let temp = TempProject::new("module-body-dependency");

    temp.write("tua.toml", "source = \"src\"\n");
    let main = temp.write(
        "src/main.tua",
        r#"
local Player = require("player")
local player = Player.new()
return player.hp
"#,
    );
    let player = temp.write(
        "src/player.tua",
        r#"
local M = {}
function M.new()
  return { hp = 100 }
end
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );
    host.snapshot();

    fs::write(
        &player,
        r#"
local M = {}
function M.new()
  local base = 100
  return { hp = base }
end
return M
"#,
    )
    .expect("player should update");
    host.set_file(
        &player,
        fs::read_to_string(&player).expect("player should read"),
    );

    let after_update = host.cache_stats();
    assert_eq!(after_update.invalidations, 1);
    assert_eq!(after_update.dependent_invalidations, 0);
    assert_eq!(after_update.require_surface_invalidations, 0);
    assert_eq!(after_update.export_surface_invalidations, 0);
    assert_eq!(after_update.type_surface_invalidations, 0);

    host.snapshot();

    let after_snapshot = host.cache_stats();
    assert_eq!(after_snapshot.diagnostics_hits, 1);
    assert_eq!(after_snapshot.diagnostics_misses, 3);
}

#[test]
fn changing_inferred_lua_module_body_without_surface_change_keeps_importers_cached() {
    let temp = TempProject::new("lua-module-body-dependency");
    let main = temp.write(
        "src/main.tua",
        "local Player = require(\"player\")\nlocal player = Player.new()\nreturn player.x\n",
    );
    let player = temp.write(
        "src/player.lua",
        "local M = {}\nfunction M.new()\n  return { x = 0 }\nend\nreturn M\n",
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&main, fs::read_to_string(&main).expect("main should read"));
    host.set_file_with_kind(
        &player,
        fs::read_to_string(&player).expect("player should read"),
        AnalysisSourceKind::Lua,
    );
    host.snapshot();

    host.set_file_with_kind(
        &player,
        "local M = {}\nfunction M.new()\n  local initial = 0\n  return { x = initial }\nend\nreturn M\n",
        AnalysisSourceKind::Lua,
    );

    let stats = host.cache_stats();
    assert_eq!(stats.invalidations, 1);
    assert_eq!(stats.dependent_invalidations, 0);
    assert_eq!(stats.export_surface_invalidations, 0);
    assert_eq!(stats.type_surface_invalidations, 0);
}

#[test]
fn changing_require_targets_records_a_require_surface_invalidation() {
    let temp = TempProject::new("module-require-surface");
    let main = temp.write("src/main.tua", "return require(\"player\")\n");
    let player = temp.write(
        "src/player.tua",
        "local dependency = require(\"stats\")\nreturn { dependency = dependency }\n",
    );
    let stats_module = temp.write("src/stats.tua", "return {}\n");
    let health_module = temp.write("src/health.tua", "return {}\n");
    let mut host = AnalysisHost::new();

    host.set_source_roots([temp.path().join("src")]);
    for path in [&main, &player, &stats_module, &health_module] {
        host.set_file(path, fs::read_to_string(path).expect("fixture should read"));
    }
    host.snapshot();

    let surface_changed = host.update_file_with_kind(
        &player,
        "local dependency = require(\"health\")\nreturn { dependency = dependency }\n",
        AnalysisSourceKind::Tua,
    );
    let cache_stats = host.cache_stats();

    assert!(!surface_changed);
    assert_eq!(cache_stats.require_surface_invalidations, 1);
    assert_eq!(cache_stats.invalidations, 1);
    assert_eq!(cache_stats.dependent_invalidations, 0);
}

#[test]
fn cache_stores_derived_query_products() {
    let path = PathBuf::from("src/module.tua");
    let mut host = AnalysisHost::new();

    host.set_file(
        &path,
        r#"
type Player = { hp: number }

local M = {}

function M.new(): Player
  return { hp = 100 }
end

return M
"#,
    );

    let snapshot = host.snapshot();

    assert!(snapshot.type_environment(&path).is_some());
    assert!(snapshot.scope_map(&path).is_some());
    let semantic_file = snapshot
        .semantic_file(&path)
        .expect("semantic file should include parsed source");
    assert!(!semantic_file.hir.symbols.is_empty());
    assert!(!semantic_file.scope.declarations().is_empty());
    assert!(snapshot.module_exports(&path).is_some());
    assert!(snapshot
        .symbol_index_for_path(&path)
        .iter()
        .any(|symbol| symbol.name == "Player"));
    assert!(snapshot.require_edges(&path).is_empty());

    let stats = host.cache_stats();
    assert_eq!(stats.hir_misses, 1);
    assert_eq!(stats.scope_misses, 1);
    assert_eq!(stats.type_environment_misses, 1);
    assert_eq!(stats.diagnostics_misses, 1);
    assert_eq!(stats.module_export_misses, 1);
    assert_eq!(stats.require_edge_misses, 1);
    assert_eq!(stats.symbol_index_misses, 1);
}

#[test]
fn reverse_reference_index_collects_local_and_project_global_references() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local <|local_def|>value = 1
local other = value
-- file: src/globals.tua
function <|global_def|>greet(name: string): string
  return name
end

local own = greet("Local")
-- file: src/use.tua
local message = greet("Ada")
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let local_target = snapshot
        .definition("src/main.tua", fixture.position("local_def"))
        .expect("local definition should resolve");
    let global_target = snapshot
        .definition("src/globals.tua", fixture.position("global_def"))
        .expect("global function definition should resolve");
    let local_references = snapshot
        .reverse_reference_index()
        .references(&local_target, true)
        .expect("local references should be indexed");
    let global_references = snapshot
        .reverse_reference_index()
        .references(&global_target, true)
        .expect("global references should be indexed");

    assert_eq!(local_references.len(), 2);
    assert!(global_references
        .iter()
        .any(|reference| reference.path == Path::new("src/use.tua")));
}

#[test]
fn module_alias_references_use_file_scope_without_building_reverse_index() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local <|def|>player = require("player")
player.load()
-- file: src/player.tua
local M = {}
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let references = snapshot.references("src/main.tua", fixture.position("def"), true);

    assert_eq!(references.len(), 2);
    assert!(references
        .iter()
        .all(|reference| reference.kind == ReferenceKind::Module));
    assert!(
        snapshot.reverse_reference_index.get().is_none(),
        "module alias references should not build the project-wide reverse-reference index"
    );
}

#[test]
fn inferred_local_table_field_definition_and_references_use_constructor_key() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local lookupObject = {
  <|field_def|>count<|/field_def|> = 0,
  y = function(self: any, amount: number): number
    self.count = self.count + amount

    return self.count
  end,
}

lookupObject.<|field_ref|>count<|/field_ref|> = 1
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("field_ref"))
        .expect("inferred table field should resolve to constructor key");
    let references = snapshot.references("src/main.tua", fixture.position("field_ref"), true);

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(definition.span, fixture.range("field_def"));
    assert!(references
        .iter()
        .any(|reference| reference.span == fixture.range("field_def")));
    assert!(references
        .iter()
        .any(|reference| reference.span == fixture.range("field_ref")));
}

#[test]
fn snapshot_completion_uses_is_metatable_narrowing_inside_branch() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = {
  health = 0,
}
Player.__index = Player

local value: any = {}

if value is Player then
  local health = value.h<|complete|>ealth
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let labels = completion_labels(
        &host
            .snapshot()
            .completion("src/main.tua", fixture.position("complete")),
    );

    assert!(labels.contains(&"health".to_owned()), "{labels:?}");
}

#[test]
fn hover_uses_nilable_post_guard_narrowing() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Piece = { x: number }
local active: Piece | nil = nil
local function update()
  if not active then
    return
  end
  local current = active
  return <|hover|>current.x
end
return update
"#,
    );
    let mut host = AnalysisHost::new();
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let hover = host
        .snapshot()
        .hover("src/main.tua", fixture.position("hover"))
        .expect("the guarded value should hover");
    assert!(hover.contents.contains("{ x: number }"), "{hover:#?}");
    assert!(!hover.contents.contains("| nil"), "{hover:#?}");
}

#[test]
fn snapshot_completion_uses_class_constructor_instance_fields() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
final class Particle {
  x: number
  speed: number = 120

  init(x: number) {
    self.x = x
  }

  final update(dt: number) {
    self.x += self.speed * dt
  }
}

local particle = Particle.new(1)
particle.s<|complete|>peed
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let labels = completion_labels(
        &host
            .snapshot()
            .completion("src/main.tua", fixture.position("complete")),
    );

    assert!(labels.contains(&"speed".to_owned()), "{labels:?}");
}

#[test]
fn snapshot_preserves_exported_class_constructor_and_member_types() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Classes = require("classes")
local manager = Classes.BeatManager.new()
local first = manager.<|hover|>steps[1]
manager:<|complete|>add(1)
manager:add("wrong")
-- file: src/classes.tua
final class BeatManager {
  steps: number[]

  init() {
    self.steps = {}
  }

  add(step: number): number {
    table.insert(self.steps, step)
    return step
  }
}

return {
  BeatManager = BeatManager,
}
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("exported class field should hover");

    assert!(labels.contains(&"add".to_owned()), "{labels:?}");
    assert_eq!(hover.contents, "(field) steps: number[]");
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)),
        "{diagnostics:?}"
    );
}

#[test]
fn snapshot_navigates_and_renames_local_class_symbols() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
open class <|class_def|>Entity<|/class_def|> {
  <|field_def|>health<|/field_def|>: number = 10

  <|method_def|>damage<|/method_def|>(amount: number) {
    self.<|field_self|>health<|/field_self|> -= amount
  }
}

local entity = <|class_value|>Entity<|/class_value|>.new()
local typed: <|class_type|>Entity<|/class_type|> = entity
print(entity.<|field_use|>health<|/field_use|>)
entity:<|method_use|>damage<|/method_use|>(1)
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let class_references =
        snapshot.references("src/main.tua", fixture.position("class_value"), true);
    let field_references = snapshot.references("src/main.tua", fixture.position("field_use"), true);
    let method_references =
        snapshot.references("src/main.tua", fixture.position("method_use"), true);

    assert_reference_ranges(
        &class_references,
        &fixture,
        &["class_def", "class_value", "class_type"],
    );
    assert_reference_ranges(
        &field_references,
        &fixture,
        &["field_def", "field_self", "field_use"],
    );
    assert_reference_ranges(&method_references, &fixture, &["method_def", "method_use"]);

    let class_rename = snapshot
        .rename("src/main.tua", fixture.position("class_value"), "Actor")
        .expect("class should rename");
    let field_rename = snapshot
        .rename("src/main.tua", fixture.position("field_use"), "hitPoints")
        .expect("class field should rename");
    let method_rename = snapshot
        .rename("src/main.tua", fixture.position("method_use"), "hurt")
        .expect("class method should rename");

    assert_edit_ranges(
        &class_rename,
        &fixture,
        &["class_def", "class_value", "class_type"],
    );
    assert_edit_ranges(
        &field_rename,
        &fixture,
        &["field_def", "field_self", "field_use"],
    );
    assert_edit_ranges(&method_rename, &fixture, &["method_def", "method_use"]);
}

#[test]
fn snapshot_keeps_same_named_class_navigation_file_local() {
    let fixture = Fixture::parse(
        r#"-- file: src/first.tua
open class <|first_def|>Player<|/first_def|> {
  name: string = "first"
}
local first = <|first_value|>Player<|/first_value|>.new()
local firstTyped: <|first_type|>Player<|/first_type|> = first
-- file: src/second.tua
open class <|second_def|>Player<|/second_def|> {
  name: string = "second"
}
local second = <|second_value|>Player<|/second_value|>.new()
local secondTyped: <|second_type|>Player<|/second_type|> = second
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let references = snapshot.references("src/first.tua", fixture.position("first_value"), true);
    let rename = snapshot
        .rename(
            "src/first.tua",
            fixture.position("first_value"),
            "FirstPlayer",
        )
        .expect("nominal class identity should make rename unambiguous");

    assert_reference_ranges(
        &references,
        &fixture,
        &["first_def", "first_value", "first_type"],
    );
    assert_edit_ranges(
        &rename,
        &fixture,
        &["first_def", "first_value", "first_type"],
    );
}

#[test]
fn snapshot_resolves_and_renames_exported_class_instance_members() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local State = require("state")
local other = State.<|class_use|>Player<|/class_use|>.new()
print(State.player.<|field_use|>health<|/field_use|>)
State.player:<|method_use|>damage<|/method_use|>(1)
-- file: src/state.tua
open class <|class_def|>Player<|/class_def|> {
  <|field_def|>health<|/field_def|>: number = 10

  <|method_def|>damage<|/method_def|>(amount: number) {
    self.<|field_self|>health<|/field_self|> -= amount
  }
}

local player = <|class_local|>Player<|/class_local|>.new()
return {
  <|class_export_key|>Player<|/class_export_key|> = <|class_export_value|>Player<|/class_export_value|>,
  player = player,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let class_references = snapshot.references("src/main.tua", fixture.position("class_use"), true);
    let field_definition = snapshot
        .definition("src/main.tua", fixture.position("field_use"))
        .expect("exported instance field should resolve");
    let method_definition = snapshot
        .definition("src/main.tua", fixture.position("method_use"))
        .expect("exported instance method should resolve");

    assert_eq!(field_definition.path, PathBuf::from("src/state.tua"));
    assert_eq!(field_definition.span, fixture.range("field_def"));
    assert_eq!(method_definition.path, PathBuf::from("src/state.tua"));
    assert_eq!(method_definition.span, fixture.range("method_def"));

    assert_reference_ranges(
        &class_references,
        &fixture,
        &[
            "class_def",
            "class_local",
            "class_export_key",
            "class_export_value",
            "class_use",
        ],
    );

    let field_references = snapshot.references("src/main.tua", fixture.position("field_use"), true);
    let method_references =
        snapshot.references("src/main.tua", fixture.position("method_use"), true);

    assert_reference_ranges(
        &field_references,
        &fixture,
        &["field_def", "field_self", "field_use"],
    );
    assert_reference_ranges(&method_references, &fixture, &["method_def", "method_use"]);

    let field_rename = snapshot
        .rename("src/main.tua", fixture.position("field_use"), "hitPoints")
        .expect("exported instance field should rename");
    let method_rename = snapshot
        .rename("src/main.tua", fixture.position("method_use"), "hurt")
        .expect("exported instance method should rename");
    let class_rename = snapshot
        .rename("src/main.tua", fixture.position("class_use"), "Actor")
        .expect("exported class should rename");

    assert_edit_ranges(
        &class_rename,
        &fixture,
        &[
            "class_def",
            "class_local",
            "class_export_key",
            "class_export_value",
            "class_use",
        ],
    );
    assert_edit_ranges(
        &field_rename,
        &fixture,
        &["field_def", "field_self", "field_use"],
    );
    assert_edit_ranges(&method_rename, &fixture, &["method_def", "method_use"]);
}

#[test]
fn snapshot_preserves_nominal_class_identity_across_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Classes = require("classes")
Classes.acceptA(Classes.A.new())
Classes.acceptA(Classes.Child.new())
Classes.acceptA(Classes.B.new())
-- file: src/classes.tua
open class A {
  value: number = 1
}

final class Child extends A {
}

final class B {
  value: number = 1
}

local function acceptA(value: A): A
  return value
end

return {
  A = A,
  Child = Child,
  B = B,
  acceptA = acceptA,
}
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    let mismatches = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH))
        .collect::<Vec<_>>();

    assert_eq!(mismatches.len(), 1, "{diagnostics:?}");
    assert!(
        mismatches[0].message.contains("expected A, found B"),
        "{diagnostics:?}"
    );
}

#[test]
fn snapshot_definition_uses_indexed_module_exports_without_reading_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Player = require("player")
local player = Player.<|usage|>new()
-- file: src/player.tua
local function <|def|>new<|/def|>()
  return { hp = 100 }
end

return {
  new = new,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("usage"))
        .expect("required module export should resolve from snapshot index");

    assert_eq!(definition.path, PathBuf::from("src/player.tua"));
    assert_eq!(definition.span, fixture.range("def"));
}

#[test]
fn snapshot_definition_resolves_each_required_module_from_its_own_binding() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Left = require("left")
local Right = require("right")
local value = Right.<|usage|>get()
-- file: src/left.tua
local function get()
  return 1
end
return { get = get }
-- file: src/right.tua
local function <|def|>get<|/def|>()
  return "right"
end
return { get = get }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let definition = host
        .snapshot()
        .definition("src/main.tua", fixture.position("usage"))
        .expect("second required module export should resolve from its own binding");

    assert_eq!(definition.path, PathBuf::from("src/right.tua"));
    assert_eq!(definition.span, fixture.range("def"));
}

#[test]
fn snapshot_hover_uses_indexed_module_exports_without_reading_files() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Audio = require("audio")
local ok = Audio.<|hover|>play("theme")
-- file: src/audio.lua
---@param source string audio source
---@return boolean success flag
local function play(source)
  return true
end

return {
  play = play,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("required module export hover should resolve from snapshot index");

    assert!(
        hover
            .contents
            .contains("function Audio.play(source: string): boolean"),
        "unexpected hover contents: {}",
        hover.contents
    );
    assert_eq!(
        hover.documentation,
        Some("audio source\nsuccess flag".to_owned())
    );
}

#[test]
fn generic_function_schemes_survive_imports_and_reexports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local api = require("api")
local relay = require("relay")
local _number_value: number = api.identity(5)
local _text_value: string = api.identity("text")
local copied = api.identity
local _copied_value: string = copied("copied")
local _hovered: string = copied(<|hover_arg|>"hover")
local _relayed: number = relay.identity(7)
local _signature = relay.identity(<|signature|>7)
-- file: src/api.tua
local Module = {}
function Module.identity<T>(value: T): T return value end
return Module
-- file: src/relay.tua
local api = require("api")
return { identity = api.identity }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");

    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover_arg"))
        .expect("the call result should retain its generic instantiation");
    assert!(hover.contents.contains("string"), "{hover:?}");

    let signature = snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .expect("imported generic function should provide signature help");
    assert!(
        signature.signatures[0].label.contains("<T>")
            || signature.signatures[0].label.contains("value: number"),
        "{signature:?}"
    );
}

#[test]
fn snapshot_infers_richer_unannotated_lua_module_surfaces() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Factory = require("factory")
local logger = require("logger")
local player = Factory.create("Ada")
local score: number = <|player_hover|>player.score
local message, ok = logger("ready")
local checked: boolean = <|ok_hover|>ok
-- file: src/factory.lua
local Module = {}
local Public = Module
Public.settings = {}
Public.settings.video = {}
Public.settings.video.fullscreen = false
Public.create = function(name)
  return { name = name, score = 0, active = true }
end
return Module
-- file: src/logger.lua
return function(message)
  return message, true
end
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file.path.extension().and_then(|value| value.to_str()) == Some("lua") {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };
        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let player = snapshot
        .hover("src/main.tua", fixture.position("player_hover"))
        .expect("factory result should hover");
    let ok = snapshot
        .hover("src/main.tua", fixture.position("ok_hover"))
        .expect("second module return should hover");

    assert_eq!(
        player.contents,
        "local player: { active: boolean, name: any, score: number }"
    );
    assert_eq!(ok.contents, "local ok: boolean");
    assert!(snapshot
        .diagnostics("src/main.tua")
        .iter()
        .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn snapshot_hover_preserves_inferred_ecs_types_through_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local ECS = require("ecs_state")
local entity = ECS.<|player_hover|>player
local world = ECS.<|world_hover|>world
local component = ECS.<|component_hover|>Position
local position = world:get(ECS.player, ECS.Position)
local x: number = <|position_hover|>position.x
world:destroy(ECS.player)
-- file: src/ecs_state.tua
local Position = ecs.component("Position", { x = "number", y = "number" })
local world = ecs.world()
local player = world:create()

return {
  Position = Position,
  world = world,
  player = player,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("player_hover"))
        .expect("ECS-derived module export should hover");
    let world_hover = snapshot
        .hover("src/main.tua", fixture.position("world_hover"))
        .expect("exported ECS world should hover");
    let component_hover = snapshot
        .hover("src/main.tua", fixture.position("component_hover"))
        .expect("exported ECS component should hover");
    let position_hover = snapshot
        .hover("src/main.tua", fixture.position("position_hover"))
        .expect("ECS component payload should hover");

    assert_eq!(hover.contents, "(field) ECS.player: EcsEntityId");
    assert_eq!(world_hover.contents, "(field) ECS.world: EcsWorld");
    assert_eq!(
        component_hover.contents,
        "(field) ECS.Position: EcsComponent"
    );
    assert_eq!(
        position_hover.contents,
        "local position: { x: number, y: number } | nil"
    );
    let hint_labels = snapshot
        .inlay_hints("src/main.tua")
        .into_iter()
        .map(|hint| hint.label)
        .collect::<Vec<_>>();
    assert!(
        hint_labels.contains(&": EcsEntityId".to_owned()),
        "{hint_labels:?}"
    );
    assert!(
        hint_labels.contains(&": EcsWorld".to_owned()),
        "{hint_labels:?}"
    );
    assert!(
        hint_labels.contains(&": EcsComponent".to_owned()),
        "{hint_labels:?}"
    );
    assert!(
        hint_labels.contains(&": { x: number, y: number } | nil".to_owned()),
        "{hint_labels:?}"
    );
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)),
        "exported ECS entity type should satisfy world methods: {diagnostics:#?}"
    );
}

#[test]
fn project_ecs_diagnostics_accept_imported_components_and_pooled_payloads() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
local ECS = require("ecs_state")

local payloads = objectPool({
  create = function(): Shared.Bullet
    return { x = 0, y = 0, dx = 0, dy = 0 }
  end,
})
local payload = payloads:acquire()

ECS.world:add(
  ECS.player,
  ECS.Bullet,
  payload
)

local movement = ecs.system(
  ECS.world,
  {
    ECS.Bullet,
  },
  function(_, bullet: Shared.Bullet, dt: number)
    bullet.x += bullet.dx * dt
  end
)
-- file: src/ecs_state.tua
local Bullet = ecs.component(
  "Bullet",
  { x = "number", y = "number", dx = "number", dy = "number" }
)
local world = ecs.world()
local player = world:create()

return {
  Bullet = Bullet,
  world = world,
  player = player,
}
-- file: src/types.tua
export type Bullet = {
  x: number,
  y: number,
  dx: number,
  dy: number,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().pipeline_diagnostics("src/main.tua");

    assert!(
        diagnostics.iter().all(|diagnostic| !matches!(
            diagnostic.code,
            Some(DiagnosticCode::DOMAIN_ECS_COMPONENT_VALUE | DiagnosticCode::DOMAIN_ECS_QUERY)
        )),
        "imported components and typed pooled payloads should be accepted: {diagnostics:#?}"
    );
}

#[test]
fn snapshot_diagnostics_follow_deprecated_lua_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Api = require("api")
do
  local Api = { oldApi = function() end }
  Api.oldApi()
end
Api.<|usage|>oldApi()
-- file: src/api.lua
---@deprecated use newApi
local function oldApi() end

return {
  oldApi = oldApi,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    let deprecated = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_DEPRECATED_API))
        .collect::<Vec<_>>();

    assert_eq!(deprecated.len(), 1, "{diagnostics:#?}");
    assert_eq!(
        deprecated[0].message,
        "API `Api.oldApi` is deprecated; use newApi"
    );
    assert_eq!(deprecated[0].line, fixture.position("usage").line);

    let usage = fixture.position("usage");
    let completion = snapshot
        .completion("src/main.tua", usage)
        .into_iter()
        .find(|completion| completion.label == "oldApi")
        .expect("deprecated export should complete");
    assert_eq!(
        completion
            .deprecation
            .as_ref()
            .and_then(|deprecation| deprecation.guidance.as_deref()),
        Some("use newApi")
    );
    let token = snapshot
        .semantic_tokens("src/main.tua")
        .into_iter()
        .find(|token| token.span.start == usage)
        .expect("deprecated member should have a semantic token");
    assert!(
        token.modifiers.contains(&SemanticTokenModifier::Deprecated),
        "{token:#?}"
    );
    let hover = snapshot
        .hover("src/main.tua", usage)
        .expect("deprecated export should hover");
    assert!(
        hover
            .documentation
            .as_deref()
            .is_some_and(|documentation| documentation.contains("**Deprecated.** use newApi")),
        "{hover:#?}"
    );
}

#[test]
fn snapshot_diagnostics_follow_nodiscard_lua_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Api = require("api")
do
  local Api = { acquire = function() return 1 end }
  Api.acquire()
end
Api.<|discarded|>acquire()
local handle = Api.acquire()
print(handle)
-- file: src/api.lua
---@nodiscard
local function acquire()
  return 1
end

return {
  acquire = acquire,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };
        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    let nodiscard = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::LINT_MUST_USE_RESULT))
        .collect::<Vec<_>>();

    assert_eq!(nodiscard.len(), 1, "{diagnostics:#?}");
    assert_eq!(nodiscard[0].message, "result of `Api.acquire` must be used");
    assert_eq!(nodiscard[0].line, fixture.position("discarded").line);
}

#[test]
fn snapshot_diagnostics_reject_unknown_members_on_exact_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local BoardModule = require("board")
BoardModule.rotate()
BoardModule.<|missing|>missingFunction()
-- file: src/board.tua
local function rotate()
end

return {
  rotate = rotate,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    let unknown_exports = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_UNKNOWN_MODULE_EXPORT))
        .collect::<Vec<_>>();

    assert_eq!(unknown_exports.len(), 1, "{diagnostics:#?}");
    assert_eq!(unknown_exports[0].line, fixture.position("missing").line);
    assert_eq!(
        unknown_exports[0].column,
        fixture.position("missing").column
    );
    assert_eq!(
        unknown_exports[0].message,
        "module `board` has no exported member `missingFunction`"
    );

    let completion_position = SourcePosition {
        line: 2,
        column: "BoardModule.".len() + 1,
    };
    assert!(snapshot
        .completion("src/main.tua", completion_position)
        .iter()
        .any(|completion| completion.label == "rotate"));
}

#[test]
fn snapshot_unknown_module_members_stay_permissive_for_dynamic_and_shadowed_values() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Missing = require("not-installed")
Missing.anything()

local Dynamic = require("dynamic")
Dynamic.anything()

local Proxy = require("proxy")
Proxy.generatedAtRuntime()

local Board = require("board")
do
  local Board: any = plugin
  Board.anything()
end
-- file: src/dynamic.lua
return load("return plugin")()
-- file: src/proxy.lua
local Proxy = { known = true }
return setmetatable(Proxy, {
  __index = function(_, key)
    return key
  end,
})
-- file: src/board.tua
return {
  rotate = function() end,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };
        host.set_file_with_kind(&file.path, &file.source, kind);
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.code != Some(DiagnosticCode::TYPE_UNKNOWN_MODULE_EXPORT)
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn snapshot_hover_prefers_project_type_aliases_in_type_annotations() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local LuaDocTypes = require("lua_doc_types")

local player: Types.<|player_type|>Player = nil
local enemy: LuaDocTypes.<|enemy_type|>Enemy = nil
-- file: src/types.tua
export type Player = { hp: number }

return {}
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let player = snapshot
        .hover("src/main.tua", fixture.position("player_type"))
        .expect("required tua alias should hover");
    let enemy = snapshot
        .hover("src/main.tua", fixture.position("enemy_type"))
        .expect("required lua doc alias should hover");

    assert_eq!(player.contents, "type Types.Player = { hp: number }");
    assert_eq!(
        enemy.contents,
        "type LuaDocTypes.Enemy = { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert!(snapshot.diagnostics("src/main.tua").is_empty());
}

#[test]
fn snapshot_navigates_project_types_in_lua_doc_annotations_across_edits() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local LuaDocTypes = require("lua_doc_types")

---@param enemy LuaDocTypes.<|param_type|>Enemy
---@return LuaDocTypes.<|return_type|>Enemy
local function clone(enemy) return enemy end
-- file: src/lua_doc_types.lua
---@class Enemy
local Enemy = {}
---@class Boss
local Boss = {}
return { Enemy = Enemy, Boss = Boss }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };
        host.set_file_with_kind(&file.path, &file.source, kind);
    }

    let snapshot = host.snapshot();
    for marker in ["param_type", "return_type"] {
        let definition = snapshot
            .definition("src/main.tua", fixture.position(marker))
            .expect("LuaDoc type reference should navigate to its project declaration");
        assert_eq!(definition.kind, DefinitionKind::TypeAlias);
        assert_eq!(definition.path, PathBuf::from("src/lua_doc_types.lua"));
        assert_eq!(
            snapshot
                .type_definition("src/main.tua", fixture.position(marker))
                .expect("LuaDoc type definition should use project resolution"),
            definition
        );
    }

    let edited = fixture
        .source("src/main.tua")
        .replace("LuaDocTypes.Enemy", "LuaDocTypes.Boss");
    host.set_file_with_kind("src/main.tua", edited.clone(), AnalysisSourceKind::Tua);
    let edited_snapshot = host.snapshot();
    let param_line = edited.lines().nth(2).expect("parameter tag should exist");
    let edit_position = SourcePosition {
        line: 3,
        column: param_line
            .find("LuaDocTypes.Boss")
            .expect("edited alias should be present")
            + "LuaDocTypes.".len()
            + 1,
    };
    let definition = edited_snapshot
        .definition("src/main.tua", edit_position)
        .expect("unsaved LuaDoc type edit should resolve the new project alias");
    assert_eq!(definition.kind, DefinitionKind::TypeAlias);
    assert_eq!(definition.path, PathBuf::from("src/lua_doc_types.lua"));
    assert_eq!(
        edited_snapshot.type_definition("src/main.tua", edit_position),
        Some(definition)
    );
}

#[test]
fn snapshot_uses_project_types_for_local_hover_and_field_completion() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local LuaDocTypes = require("lua_doc_types")

local enemyFromLuaDocModule: LuaDocTypes.Enemy = {
  <|literal|>
}

local a = enemyFromLuaDocModule
a.<|field|>
local copied = <|hover|>enemyFromLuaDocModule
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("annotated project local should hover");
    let literal_labels = snapshot
        .completion("src/main.tua", fixture.position("literal"))
        .into_iter()
        .map(|completion| completion.label)
        .collect::<Vec<_>>();
    let field_labels = snapshot
        .completion("src/main.tua", fixture.position("field"))
        .into_iter()
        .map(|completion| completion.label)
        .collect::<Vec<_>>();

    assert_eq!(
        hover.contents,
        "local enemyFromLuaDocModule: {\n  hp: number,\n  name: string,\n  type: \"friendly\" | \"mad\"\n}"
    );
    assert!(literal_labels.contains(&"hp".to_owned()));
    assert!(literal_labels.contains(&"name".to_owned()));
    assert!(literal_labels.contains(&"type".to_owned()));
    assert!(field_labels.contains(&"hp".to_owned()));
    assert!(field_labels.contains(&"name".to_owned()));
}

#[test]
fn snapshot_preserves_store_callback_state_through_nested_module_exports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Globals = require("global")

Globals.game:update(function(<|hover|>state)
  state.<|complete|>
end)
-- file: src/global.tua
local game = store({ score = 0, paused = false })

return {
  game = game,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("Store callback parameter should hover through a module field");
    let fields =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));

    assert_eq!(
        hover.contents,
        "parameter state: { paused: boolean, score: number }"
    );
    assert!(fields.contains(&"paused".to_owned()), "{fields:?}");
    assert!(fields.contains(&"score".to_owned()), "{fields:?}");
}

#[test]
fn lua_explicit_runtime_imports_keep_domain_inference() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.lua
local makeStore = require("tuastore")
local StateMachine = require("tuastatemachine")
local Baton = require("baton")

local game = makeStore({ score = 0, paused = false })
game:update(function(<|state_hover|>state)
  state.<|state_complete|>
end)

local <|flow_hover|>flow = StateMachine.create({
  initial = "menu",
  events = {
    { name = "start", from = "menu", to = "playing" },
  },
})
local <|controls_hover|>controls = Baton.new({ controls = { move = { "key:left", "key:right" } } })
flow:start()
controls:get("move")
"#,
    );
    let mut host = AnalysisHost::new();
    let file = fixture
        .file("src/main.lua")
        .expect("fixture should contain Lua file");
    host.set_file_with_kind(&file.path, file.source.clone(), AnalysisSourceKind::Lua);

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.lua", fixture.position("state_hover"))
        .expect("explicitly imported Store callback should be typed");
    let fields =
        completion_labels(&snapshot.completion("src/main.lua", fixture.position("state_complete")));
    let flow_hover = snapshot
        .hover("src/main.lua", fixture.position("flow_hover"))
        .expect("explicitly imported state machine should be typed");
    let controls_hover = snapshot
        .hover("src/main.lua", fixture.position("controls_hover"))
        .expect("explicitly imported input should be typed");

    assert_eq!(
        hover.contents,
        "parameter state: { paused: boolean, score: number }"
    );
    assert!(fields.contains(&"paused".to_owned()), "{fields:?}");
    assert!(fields.contains(&"score".to_owned()), "{fields:?}");
    assert_eq!(flow_hover.contents, "local flow: StateMachine");
    assert!(
        flow_hover
            .documentation
            .as_deref()
            .is_some_and(|documentation| {
                documentation.contains("`\"menu\" | \"playing\"`")
                    && documentation.contains("`\"start\"`")
            }),
        "{flow_hover:?}"
    );
    assert_eq!(controls_hover.contents, "local controls: Input");
    assert!(
        controls_hover
            .documentation
            .as_deref()
            .is_some_and(|documentation| documentation.contains("`move`")),
        "{controls_hover:?}"
    );
    assert!(snapshot
        .pipeline_diagnostics("src/main.lua")
        .iter()
        .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::LINT_MISSING_REQUIRE)));
}

#[test]
fn snapshot_uses_imported_return_alias_fields_for_inferred_local_completion_and_definition() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Functions = require("functions")

local makePlayer = Functions.makePlayer
local player = makePlayer("static import")
local direct = Functions.makePlayer("direct import")
local hp = player.<|complete|>h
local directHp = direct.<|direct_complete|>h
player.<|hp_ref|>hp = 12
-- file: src/functions.tua
type Player = {
  name: string,
  <|hp_def|>hp<|/hp_def|>: number,
  alive?: boolean,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
    alive = true,
  }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    assert!(snapshot
        .semantic_facts
        .lock()
        .expect("semantic facts lock poisoned")
        .is_empty());

    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    let facts = snapshot
        .semantic_facts
        .lock()
        .expect("semantic facts lock poisoned")
        .get(&normalize_path(Path::new("src/main.tua")))
        .cloned()
        .expect("completion should warm shared semantic facts");

    assert!(facts.project_inference.field_origins.contains_key("player"));
    assert!(facts.project_inference.field_origins.contains_key("direct"));

    let direct_labels = completion_labels(
        &snapshot.completion("src/main.tua", fixture.position("direct_complete")),
    );
    let definition = snapshot
        .definition("src/main.tua", fixture.position("hp_ref"))
        .expect("imported return alias field should resolve to provider alias field");

    assert_eq!(
        snapshot
            .semantic_facts
            .lock()
            .expect("semantic facts lock poisoned")
            .len(),
        1
    );
    assert!(labels.contains(&"hp".to_owned()), "{labels:?}");
    assert!(labels.contains(&"name".to_owned()), "{labels:?}");
    assert!(direct_labels.contains(&"hp".to_owned()));
    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(definition.path, PathBuf::from("src/functions.tua"));
    assert_eq!(definition.span, fixture.range("hp_def"));
}

#[test]
fn snapshot_type_definition_resolves_alias_origins_and_field_shapes() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Player = {
  <|hp_def|>hp<|/hp_def|>: number,
}

local <|decl|>player: <|annotation|>Player = { hp = 10 }
local copied = <|usage|>player
player.<|hp_ref|>hp = 12
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();

    for marker in ["decl", "annotation", "usage"] {
        let definition = snapshot
            .type_definition("src/main.tua", fixture.position(marker))
            .expect("player type definition should resolve to alias");

        assert_eq!(definition.kind, DefinitionKind::TypeAlias);
        assert_eq!(definition.path, PathBuf::from("src/main.tua"));
        assert_eq!(source_text(&fixture, &definition), "Player");
    }

    let field = snapshot
        .type_definition("src/main.tua", fixture.position("hp_ref"))
        .expect("field type definition should resolve to shape origin");

    assert_eq!(field.kind, DefinitionKind::TableField);
    assert_eq!(field.path, PathBuf::from("src/main.tua"));
    assert_eq!(field.span, fixture.range("hp_def"));
}

#[test]
fn snapshot_type_definition_resolves_required_module_aliases() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Types = require("types")
local player: Types.<|type_ref|>Player = { hp = 10 }
-- file: src/types.tua
export type Player = {
  hp: number,
}

return {}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .type_definition("src/main.tua", fixture.position("type_ref"))
        .expect("required module type alias should resolve");

    assert_eq!(definition.kind, DefinitionKind::TypeAlias);
    assert_eq!(definition.path, PathBuf::from("src/types.tua"));
    assert_eq!(source_text(&fixture, &definition), "Player");
}

#[test]
fn snapshot_hover_and_definition_resolve_type_only_imports() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"
import type { Player as NamedPlayer } from "./types"
local player: Shared.<|type_ref|>Player = { hp = 10 }
local other: Shared.Pla<|complete|> = player
local named: <|named_ref|>NamedPlayer = player
local namedTyping: NamedP<|named_complete|> = player
local runtime = Shared.Pla<|runtime_complete|>
return player.hp
-- file: src/types.tua
export type <|type_def|>Player<|/type_def|> = { hp: number }
type PlayerPrivate = { hp: number }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("type_ref"))
        .expect("type-only alias should hover");
    let definition = snapshot
        .type_definition("src/main.tua", fixture.position("type_ref"))
        .expect("type-only alias should navigate");
    let named_definition = snapshot
        .type_definition("src/main.tua", fixture.position("named_ref"))
        .expect("named type import should navigate");

    assert_eq!(hover.contents, "type Shared.Player = { hp: number }");
    assert_eq!(definition.kind, DefinitionKind::TypeAlias);
    assert_eq!(definition.path, PathBuf::from("src/types.tua"));
    assert_eq!(definition.span, fixture.range("type_def"));
    assert_eq!(named_definition.path, PathBuf::from("src/types.tua"));
    assert_eq!(named_definition.span, fixture.range("type_def"));
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete"))),
        vec!["Player".to_owned()]
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("named_complete"))),
        vec!["NamedPlayer".to_owned()]
    );
    assert!(!completion_labels(
        &snapshot.completion("src/main.tua", fixture.position("runtime_complete"))
    )
    .contains(&"Player".to_owned()));
}

#[test]
fn type_only_imports_resolve_inside_anonymous_function_contracts() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type Shared from "./types"

local bullets = objectPool({
  create = function(): Shared.Bullet
    return { x = 0, y = 0, dx = 0, dy = 0, active = false }
  end,
  reset = function(bullet: Shared.Bullet)
    bullet.x = 0
    bullet.y = 0
    bullet.dx = 0
    bullet.dy = 0
    bullet.active = false
  end,
})

local bullet = bullets:acquire()
bullet.<|complete|>
bullets:release(bullet)
-- file: src/types.tua
export type Bullet = {
  x: number,
  y: number,
  dx: number,
  dy: number,
  active: boolean,
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");

    assert!(
        !diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                Some(DiagnosticCode::TYPE_MISMATCH | DiagnosticCode::TYPE_UNKNOWN_NAME)
            )
        }),
        "anonymous callback contracts should resolve imported aliases: {diagnostics:#?}"
    );
    let mut labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    labels.sort();
    assert_eq!(
        labels,
        vec![
            "active".to_owned(),
            "dx".to_owned(),
            "dy".to_owned(),
            "x".to_owned(),
            "y".to_owned(),
        ]
    );
}

#[test]
fn game_module_queries_share_exported_array_and_function_types() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Bullets = require("bullets")
local spawnFn = Bullets.<|complete|>spawn
Bullets.<|spawn_use|>spawn(10, 20)
local bullet = Bullets.active[1]
local damage = bullet.<|damage_use|>damage
print(damage)
print(spawnFn)
-- file: src/bullets.tua
import type Shared from "./types"
local active: Shared.Bullet[] = {}

local function <|spawn_def|>spawn<|/spawn_def|>(x: number, y: number)
  table.insert(active, { x = x, y = y, damage = 1, active = true })
end

local function update(dt: number)
  for _, bullet in ipairs(active) do
    bullet.x += dt
  end
end

return { active = active, spawn = spawn, update = update }
-- file: src/types.tua
export type Bullet = {
  x: number,
  y: number,
  damage: number,
  active: boolean,
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    let hover = snapshot
        .hover("src/main.tua", fixture.position("damage_use"))
        .expect("exported array element field should hover");
    let definition = snapshot
        .definition("src/main.tua", fixture.position("spawn_use"))
        .expect("exported function should navigate");
    let references = snapshot.references("src/bullets.tua", fixture.position("spawn_def"), true);

    assert!(labels.contains(&"active".to_owned()), "{labels:?}");
    assert!(labels.contains(&"spawn".to_owned()), "{labels:?}");
    assert!(labels.contains(&"update".to_owned()), "{labels:?}");
    assert_eq!(hover.contents, "(field) damage: number");
    assert_eq!(definition.path, PathBuf::from("src/bullets.tua"));
    assert_eq!(definition.span, fixture.range("spawn_def"));
    assert!(references.len() >= 3, "{references:#?}");
    assert!(
        snapshot
            .diagnostics("src/main.tua")
            .iter()
            .all(|diagnostic| diagnostic.severity != crate::DiagnosticSeverity::Error),
        "game module consumer should stay diagnostic-clean"
    );
}

#[test]
fn snapshot_implementation_resolves_functions_and_stays_conservative() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Functions = require("functions")

type Account = { balance: number }

local function <|local_def|>damage<|/local_def|>(player: Account)
  return player.balance
end

function <|method_def|>Account:deposit<|/method_def|>(amount: number)
  self.balance = self.balance + amount
end

local account: Account = { balance = 1 }
<|local_call|>damage(account)
account:<|method_call|>deposit(1)
Functions.<|module_call|>makePlayer()

local payload: any = {}
payload.<|any_call|>missing()

local Meta = {}
Meta.__index = Meta
function Meta:onlyRuntime() end
local runtime = setmetatable({}, Meta)
runtime:<|meta_call|>onlyRuntime()

function <|first_global|>run() end
function <|second_global|>run() end
<|global_call|>run()
-- file: src/functions.tua
local function <|module_def|>makePlayer<|/module_def|>()
  return {}
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let local =
        only_definition(snapshot.implementation("src/main.tua", fixture.position("local_call")));
    let method =
        only_definition(snapshot.implementation("src/main.tua", fixture.position("method_call")));
    let module =
        only_definition(snapshot.implementation("src/main.tua", fixture.position("module_call")));

    assert_eq!(local.path, PathBuf::from("src/main.tua"));
    assert_eq!(local.span, fixture.range("local_def"));
    assert_eq!(method.path, PathBuf::from("src/main.tua"));
    assert_eq!(method.span, fixture.range("method_def"));
    assert_eq!(module.path, PathBuf::from("src/functions.tua"));
    assert_eq!(module.span, fixture.range("module_def"));
    assert!(snapshot
        .implementation("src/main.tua", fixture.position("any_call"))
        .is_empty());
    assert!(snapshot
        .implementation("src/main.tua", fixture.position("meta_call"))
        .is_empty());
    assert!(snapshot
        .implementation("src/main.tua", fixture.position("global_call"))
        .is_empty());
}

#[test]
fn protocol_method_navigation_resolves_requirement_and_all_implementations() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.tua
export protocol <|protocol_name|>Updatable<|/protocol_name|> {
  <|requirement|>update<|/requirement|>(dt: number): nil
}
export protocol Schedulable {
  update(dt: number): nil
}
-- file: src/actors.tua
import type { Updatable, Schedulable } from "./contracts"
final class Particle implements Updatable, Schedulable {
  <|particle|>update<|/particle|>(dt: number): nil {
  }
}
final class Clock implements Updatable {
  <|clock|>update<|/clock|>(dt: number): nil {
  }
}
-- file: src/main.tua
import type { Updatable, Schedulable } from "./contracts"
local function tick(value: <|type_use|>Updatable, dt: number)
  value:<|use|>update(dt)
end
local function schedule(value: Schedulable, dt: number)
  value:update(dt)
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("use"))
        .expect("a protocol method call should navigate to its requirement");
    assert_eq!(definition.path, PathBuf::from("src/contracts.tua"));
    assert_eq!(definition.span, fixture.range("requirement"));

    let type_definition = snapshot
        .definition("src/main.tua", fixture.position("type_use"))
        .expect("a protocol type reference should navigate to its declaration");
    assert_eq!(type_definition.path, PathBuf::from("src/contracts.tua"));
    assert_eq!(type_definition.span, fixture.range("protocol_name"));

    let implementations = snapshot.implementation("src/main.tua", fixture.position("use"));
    assert_eq!(implementations.len(), 2, "{implementations:#?}");
    assert!(implementations.iter().any(|definition| {
        definition.path == std::path::Path::new("src/actors.tua")
            && definition.span == fixture.range("particle")
    }));
    assert!(implementations.iter().any(|definition| {
        definition.path == std::path::Path::new("src/actors.tua")
            && definition.span == fixture.range("clock")
    }));

    let references =
        snapshot.references("src/contracts.tua", fixture.position("requirement"), true);
    assert_eq!(references.len(), 6, "{references:#?}");
    assert!(references.iter().any(|reference| {
        reference.path == std::path::Path::new("src/actors.tua")
            && reference.span == fixture.range("particle")
    }));
    assert!(references.iter().any(|reference| {
        reference.path == std::path::Path::new("src/actors.tua")
            && reference.span == fixture.range("clock")
    }));
    assert!(references.iter().any(|reference| {
        reference.path == std::path::Path::new("src/main.tua")
            && reference.span.start == fixture.position("use")
    }));

    let rename = snapshot
        .rename("src/contracts.tua", fixture.position("requirement"), "step")
        .expect("renaming a protocol requirement should include its implementations");
    assert_eq!(rename.edits.len(), 6, "{rename:#?}");
    let implementation_rename = snapshot
        .rename("src/actors.tua", fixture.position("particle"), "step")
        .expect("renaming an implementation should include its protocol contract");
    assert_eq!(
        implementation_rename.edits.len(),
        6,
        "{implementation_rename:#?}"
    );
}

#[test]
fn imported_protocol_methods_are_available_to_completion_and_hover() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.tua
export protocol Updatable {
  --- advances the actor by one frame
  ---@param dt elapsed seconds
  update(dt: number): nil
}
-- file: src/main.tua
import type { Updatable } from "./contracts"
local function tick(value: Updatable, dt: number)
  value:up<|complete|>date(dt)
  value:<|use|>update(dt)
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();

    let hover = snapshot.hover("src/main.tua", fixture.position("use"));
    let completions =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert_eq!(completions, ["update"]);
    let hover = hover.expect("protocol method should have a hover");
    assert!(hover.contents.contains("update"), "{hover:#?}");
    assert!(hover.contents.contains("number"), "{hover:#?}");
    assert!(hover.contents.contains("(method)"), "{hover:#?}");
    assert!(!hover.contents.contains("self: any"), "{hover:#?}");
    assert!(
        hover.documentation.as_deref().is_some_and(|documentation| {
            documentation.contains("advances the actor by one frame")
                && documentation.contains("@param")
                && documentation.contains("elapsed seconds")
        }),
        "protocol requirement docs should be visible from an imported method hover: {hover:#?}"
    );
}

#[test]
fn protocol_types_flow_through_inlays_and_type_explanations() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil
}
-- file: src/main.tua
import type { Updatable } from "./contracts"
local function tick(value: Updatable, dt: number)
  local current = value
  current:<|call|>update(dt)
end
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();

    let hints = snapshot.inlay_hints(&path);
    assert!(
        hints.iter().any(|hint| hint.label.contains("Updatable")),
        "inferred local hints should preserve the protocol type: {hints:#?}"
    );

    let explanation = snapshot
        .explain_type(&path, fixture.position("call"))
        .expect("protocol method should have a type explanation");
    assert!(
        explanation.ty.to_string().contains("number"),
        "protocol method explanation should retain its signature: {explanation:#?}"
    );
    assert!(
        explanation.steps.iter().any(|step| step
            .ty
            .as_ref()
            .is_some_and(|ty| ty.to_string().contains("number"))),
        "the explanation path should identify the protocol method's typed receiver: {explanation:#?}"
    );
}

#[test]
fn imported_protocol_completion_survives_incomplete_method_edit() {
    let valid = r#"import type { Updatable } from "./contracts"
local function tick(value: Updatable, dt: number)
  value:update(dt)
end
"#;
    let fixture = Fixture::parse(&valid.replace("value:update(dt)", "value:up<|complete|>"));
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        "src/contracts.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n}\n",
    );
    let path = PathBuf::from("src/main.tua");
    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, fixture.source("src/main.tua"));
    let snapshot = host.snapshot();
    let (path, file) = snapshot
        .files
        .iter()
        .find(|(path, _)| path.ends_with("src/main.tua"))
        .expect("main file should be in the snapshot");
    assert!(file.parsed.is_err(), "the incomplete edit should not parse");
    let degraded = snapshot
        .degraded_semantic_facts(path, file)
        .expect("last-good semantic facts should be reused");
    assert!(
        snapshot
            .type_database
            .resolve_type_name(&snapshot.module_graph, &degraded.parsed.path, "Updatable")
            .is_some(),
        "the imported protocol binding should survive an incomplete edit"
    );
    let completions =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert_eq!(completions, ["update"]);
}

#[test]
fn worker_snapshot_supplies_last_good_import_facts_to_coordinator_edits() {
    let valid = r#"import type { Updatable } from "./contracts"
local function tick(value: Updatable, dt: number)
  value:update(dt)
end
"#;
    let invalid = Fixture::parse(&valid.replace("value:update(dt)", "value:up<|complete|>"));
    let main_path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        "src/contracts.tua",
        "export protocol Updatable {\n  update(dt: number): nil\n}\n",
    );
    host.set_file(&main_path, valid);

    // LSP request workers analyze a clone. Its cached snapshot is shared with
    // the coordinator, while its local last-good map is not.
    let worker = host.clone();
    let _ = worker.snapshot();
    host.update_open_document(&main_path, invalid.source("src/main.tua"));

    let snapshot = host.snapshot();
    let completions =
        completion_labels(&snapshot.completion(&main_path, invalid.position("complete")));
    assert_eq!(completions, ["update"]);
}

#[test]
fn imported_protocol_conformance_is_checked_in_the_implementing_module() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.tua
export protocol Updatable {
  update(dt: number): nil
}
-- file: src/actors.tua
import type { Updatable } from "./contracts"
final class Valid implements Updatable {
  update(dt: number): nil {
  }
}
final class Missing implements Updatable {
}
final class Bad implements Updatable {
  update(dt: string): nil {
  }
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/actors.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Missing")
                && diagnostic.message.contains("missing method `update`")
        }),
        "project analysis should report a missing protocol method in the implementing module: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Bad.update")
                && diagnostic.message.contains("does not satisfy protocol")
        }),
        "project analysis should report an incompatible protocol method in the implementing module: {diagnostics:#?}"
    );
    let bad = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Bad.update"))
        .expect("the incompatible method diagnostic should exist");
    assert_eq!(bad.related_information.len(), 1, "{bad:#?}");
    assert_eq!(
        bad.related_information[0].file,
        PathBuf::from("src/contracts.tua")
    );
    assert_eq!(
        bad.related_information[0].span.start,
        SourcePosition { line: 2, column: 3 }
    );
    assert_eq!(
        bad.related_information[0].span.end,
        SourcePosition { line: 2, column: 9 }
    );
    let missing = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Missing"))
        .expect("the missing method diagnostic should exist");
    assert_eq!(missing.related_information.len(), 1, "{missing:#?}");
    assert_eq!(
        missing.related_information[0].file,
        PathBuf::from("src/contracts.tua")
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Valid")
                && diagnostic.message.contains("does not satisfy protocol")
        }),
        "valid conformance should remain accepted: {diagnostics:#?}"
    );

    host.set_file(
        "src/contracts.tua",
        "export protocol Updatable {\n  update(dt: string): nil\n}\n",
    );
    let changed = host.snapshot().diagnostics("src/actors.tua");
    assert!(
        changed.iter().any(|diagnostic| {
            diagnostic.message.contains("Valid.update")
                && diagnostic.message.contains("does not satisfy protocol")
        }),
        "editing an imported protocol should refresh conformance diagnostics: {changed:#?}"
    );
}

#[test]
fn inherited_override_diagnostics_link_to_imported_protocol_requirements() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.tua
export protocol Updatable {
  update(dt: string): nil
}
export protocol Steppable {
  update(dt: string): nil
}
-- file: src/actors.tua
import type { Updatable, Steppable } from "./contracts"
open class Parent implements Updatable, Steppable {
  update(dt: string): nil {}
}
final class Child extends Parent {
  override update(dt: number): nil {}
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/actors.tua");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("method `update` has a signature")
        })
        .expect("the incompatible inherited override should be diagnosed");
    assert!(
        diagnostic
            .message
            .contains("inherited protocol requirements `Updatable.update`"),
        "the override diagnostic should identify its imported protocol obligation: {diagnostic:#?}"
    );
    assert!(diagnostic.message.contains("`Steppable.update`"));
    assert_eq!(diagnostic.related_information.len(), 2, "{diagnostic:#?}");
    assert_eq!(
        diagnostic.related_information[0].file,
        PathBuf::from("src/contracts.tua")
    );
    assert_eq!(
        diagnostic.related_information[0].span.start,
        SourcePosition { line: 2, column: 3 }
    );
    assert_eq!(
        diagnostic.related_information[0].span.end,
        SourcePosition { line: 2, column: 9 }
    );
    assert_eq!(
        diagnostic.related_information[1].span.start,
        SourcePosition { line: 5, column: 3 }
    );
    assert_eq!(
        diagnostic.related_information[1].span.end,
        SourcePosition { line: 5, column: 9 }
    );
}

#[test]
fn cyclic_imported_protocol_signatures_preserve_conformance_identity() {
    let fixture = Fixture::parse(
        r#"-- file: src/a.tua
import type { B } from "./b"
export protocol A {
  f(value: B): nil
}
-- file: src/b.tua
import type { A } from "./a"
export protocol B {
  g(value: A): nil
}
-- file: src/actors.tua
import type { A } from "./a"
import type { B } from "./b"
final class Actor implements A, B {
  f(value: B): nil {}
  g(value: A): nil {}
}
final class Wrong implements A, B {
  f(value: A): nil {}
  g(value: B): nil {}
}
local function inspect(<|parameter|>value: B)
  value:g<|complete|>()
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/actors.tua");
    assert_eq!(
        completion_labels(&snapshot.completion("src/actors.tua", fixture.position("complete"))),
        ["g"],
        "cyclic protocol views should still expose their methods to completion"
    );
    let parameter_hover = snapshot
        .hover("src/actors.tua", fixture.position("parameter"))
        .expect("the parameter should retain its cyclic protocol type in hover");
    assert_eq!(
        parameter_hover.contents, "parameter value: B",
        "hover should preserve the protocol-facing type instead of displaying an expanded structural cycle"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == crate::DiagnosticSeverity::Error
                && diagnostic.message.contains("Actor.")
        }),
        "matching implementations of cyclic imported protocol signatures should conform: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Wrong.f")
                && diagnostic.message.contains("does not satisfy protocol `A.f`")
        }) && diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Wrong.g")
                && diagnostic.message.contains("does not satisfy protocol `B.g`")
        }),
        "wrong protocol parameter types must remain rejected around a recursive protocol cycle: {diagnostics:#?}"
    );

    host.set_file(
        "src/a.tua",
        "import type { B } from \"./b\"\nexport protocol A {\n  f(value: string): nil\n}\n",
    );
    let changed = host.snapshot();
    let changed_diagnostics = changed.diagnostics("src/actors.tua");
    assert!(
        changed_diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Actor.f")
                && diagnostic.message.contains("does not satisfy protocol `A.f`")
        }),
        "editing a protocol in a cyclic import graph should refresh dependent conformance: {changed_diagnostics:#?}"
    );
    assert_eq!(
        completion_labels(&changed.completion("src/actors.tua", fixture.position("complete"))),
        ["g"],
        "the other side of the cyclic graph should retain its completion surface"
    );

    host.remove_file("src/a.tua");
    let removed = host.snapshot();
    let removed_diagnostics = removed.diagnostics("src/actors.tua");
    assert!(
        removed_diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::LINT_MISSING_REQUIRE)
                && diagnostic.message.contains("import file not found: `./a`")
        }),
        "deleting one side of a cyclic protocol graph should invalidate the last-good export and report the missing import: {removed_diagnostics:#?}"
    );
    assert!(
        removed_diagnostics.iter().all(|diagnostic| {
            !diagnostic.message.contains("Actor.f")
                || !diagnostic.message.contains("does not satisfy protocol `A.f`")
        }),
        "deleted cyclic contracts must not leave stale conformance errors: {removed_diagnostics:#?}"
    );
}

#[test]
fn same_file_recursive_protocol_signatures_keep_conformance_and_editor_types() {
    let fixture = Fixture::parse(
        r#"protocol A {
  f(value: B): nil
}
protocol B {
  g(value: A): nil
}
final class Actor implements A, B {
  f(value: B): nil {}
  g(value: A): nil {}
}
final class Wrong implements A, B {
  f(value: A): nil {}
  g(value: B): nil {}
}
local function inspect(<|parameter|>value: B)
  value:g<|complete|>()
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", fixture.source("src/main.tua"));
    let snapshot = host.snapshot();

    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == crate::DiagnosticSeverity::Error
                && diagnostic.message.contains("Actor.")
        }),
        "matching same-file recursive protocol signatures should conform: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Wrong.f")
                && diagnostic.message.contains("does not satisfy protocol `A.f`")
        }) && diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Wrong.g")
                && diagnostic.message.contains("does not satisfy protocol `B.g`")
        }),
        "same-file cycle handling must still reject incompatible method signatures: {diagnostics:#?}"
    );
    assert_eq!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete"))),
        ["g"],
        "same-file protocol completion should expose the declared method"
    );
    let parameter_hover = snapshot
        .hover("src/main.tua", fixture.position("parameter"))
        .expect("the parameter should retain its same-file protocol type in hover");
    assert_eq!(
        parameter_hover.contents, "parameter value: B",
        "hover and completion should agree on the protocol surface"
    );
}

#[test]
fn same_file_recursive_protocol_method_returns_keep_nominal_identity() {
    let fixture = Fixture::parse(
        r#"protocol A {
  next(): B
}
protocol B {
  previous(): A
}
final class Value implements A, B {
  next(): Self { return self }
  previous(): Self { return self }
}
local a: A = Value.new()
local <|target|>target: B = a:next()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", fixture.source("src/main.tua"));
    let snapshot = host.snapshot();

    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("target")
        }),
        "a method returning a recursive protocol must retain its declared protocol identity: {diagnostics:#?}"
    );
    assert_eq!(
        snapshot
            .hover("src/main.tua", fixture.position("target"))
            .expect("the assignment target should expose its protocol type")
            .contents,
        "local target: B"
    );
}

#[test]
fn imported_recursive_protocol_method_returns_keep_nominal_identity() {
    let fixture = Fixture::parse(
        r#"-- file: src/a.tua
import type { B } from "./b"
export protocol A {
  next(): B
}
-- file: src/b.tua
import type { A } from "./a"
export protocol B {
  previous(): A
}
-- file: src/value.tua
import type { A } from "./a"
import type { B } from "./b"
final class Value implements A, B {
  next(): B { return self }
  previous(): A { return self }
}
-- file: src/main.tua
import type { A } from "./a"
import type { B } from "./b"
local Values = require("value")
local a: A = Values.Value.new()
local <|target|>target: B = a:next()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();

    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("target")
        }),
        "an imported recursive protocol method must preserve its declared return identity: {diagnostics:#?}"
    );
    assert_eq!(
        snapshot
            .hover("src/main.tua", fixture.position("target"))
            .expect("the assignment target should expose its imported protocol type")
            .contents,
        "local target: B"
    );
}

#[test]
fn imported_class_instances_keep_protocol_conformance_across_modules() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Readable {
  read(): number
}
-- file: src/entity.tua
import type { Readable } from "./contracts"
open class Entity implements Readable {
  read(): number {
    return 1
  }
}
final class Child extends Entity {
}
export type EntityInstance = Entity
export type ChildInstance = Child
return { Entity = Entity, Child = Child }
-- file: src/main.tua
import type { EntityInstance } from "./entity"
import type { ChildInstance } from "./entity"
import type { Readable } from "./contracts"
local Entities = require("entity")
local entity: EntityInstance = Entities.Entity.new()
local readable: Readable = entity
local child: ChildInstance = Entities.Child.new()
local inheritedReadable: Readable = child
local <|hover|>result = readable:read()
final class SameShape {
  read(): number {
    return 2
  }
}
local rejected: Readable = SameShape.new()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    let mismatches = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH))
        .collect::<Vec<_>>();
    assert_eq!(
        mismatches.len(),
        1,
        "only the same-shaped class without explicit conformance should fail: {diagnostics:#?}"
    );
    assert!(
        mismatches[0].message.contains("expected Readable")
            && mismatches[0].message.contains("found SameShape"),
        "the nonconforming class should retain its concrete type in the diagnostic: {mismatches:#?}"
    );
    assert_eq!(mismatches[0].line, 15);
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("expected Readable")
                && diagnostic.message.contains("found EntityInstance")
        }),
        "an exported instance whose class explicitly implements the imported protocol should be accepted: {diagnostics:#?}"
    );
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("the result of the imported protocol call should hover");
    assert_eq!(hover.contents, "local result: number");
}

#[test]
fn namespace_imported_protocol_conformance_uses_declaration_contracts() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil
}
-- file: src/actors.tua
import type Contracts from "./contracts"
final class Valid implements Contracts.Updatable {
  update(dt: number): nil {
  }
}
final class Missing implements Contracts.Updatable {
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/actors.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Missing")
                && diagnostic.message.contains("missing method `update`")
        }),
        "namespace-qualified conformance should use the exported declaration contract: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Valid")
                && diagnostic.message.contains("does not satisfy protocol")
        }),
        "valid namespace-qualified conformance should remain accepted: {diagnostics:#?}"
    );
}

#[test]
fn imported_protocol_self_returns_accept_conforming_class_instances() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Cloneable {
  clone(): Self
}
-- file: src/actors.tua
import type { Cloneable } from "./contracts"
final class Thing implements Cloneable {
  clone(): Self { return self }
}
local _view: Cloneable = Thing.new()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/actors.tua");
    assert!(
        !diagnostics.iter().any(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error),
        "imported protocol Self returns should accept the explicit class conformer: {diagnostics:#?}"
    );
}

#[test]
fn imported_structural_function_alias_is_not_a_protocol() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export type Callable = { run: (x: number) -> nil }
-- file: src/actors.tua
import type { Callable } from "./contracts"
final class Actor implements Callable {
  run(): nil {}
}
local _callable: Callable = Actor.new()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let diagnostics = host.snapshot().diagnostics("src/actors.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unknown protocol `Callable`")
        }),
        "an imported structural function alias must not become a nominal protocol: {diagnostics:#?}"
    );
}

#[test]
fn generic_protocol_bounds_keep_canonical_identity_through_imported_reexports() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil
}
-- file: src/relay.tua
import type { Updatable } from "./contracts"
export type { Updatable } from "./contracts"
-- file: src/api.tua
import type { Updatable } from "./relay"
local function tick<T: Updatable>(value: T): T
  value:update(1)
  return value
end
return { tick = tick }
-- file: src/actors.tua
import type { Updatable } from "./contracts"
final class Actor implements Updatable {
  update(dt: number): nil { return nil }
}
return { Actor = Actor }
-- file: src/lookalike.tua
final class Lookalike {
  update(dt: number): nil { return nil }
}
return { Lookalike = Lookalike }
-- file: src/main.tua
local api = require("api")
local actors = require("actors")
local lookalikes = require("lookalike")
local actor = actors.Actor.new()
local valid = api.tick(actor)
local invalid = api.tick(lookalikes.Lookalike.new())
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();

    let api_diagnostics = snapshot.diagnostics("src/api.tua");
    assert!(
        !api_diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == crate::DiagnosticSeverity::Error
                || diagnostic.code == Some(crate::DiagnosticCode::LINT_UNUSED_TYPE)
        }),
        "a generic function must be able to use its imported protocol bound in its body without false errors or unused-import warnings: {api_diagnostics:#?}"
    );

    let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("requires explicit conformance")
                && diagnostic.message.contains("Updatable")
                && diagnostic.message.contains("Lookalike")
        }),
        "the imported generic function must reject a same-shaped class using the canonical re-exported protocol identity: {diagnostics:#?}"
    );
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Actor") && diagnostic.message.contains("Updatable")
        }),
        "the explicit cross-file conformer should remain accepted: {diagnostics:#?}"
    );
}

#[test]
fn imported_protocol_bound_accepts_generic_class_methods() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Measurable { measure(): number }
-- file: src/box.tua
import type { Measurable } from "./contracts"
final class Box<T: Measurable> {
  item: T
  init(item: T) { self.item = item }
  get(): T { return self.item }
}
return { Box = Box }
-- file: src/main.tua
import type { Measurable } from "./contracts"
local boxes = require("box")
final class Meter implements Measurable {
  value: number
  init(value: number) { self.value = value }
  measure(): number { return self.value }
}
local box = boxes.Box.new(Meter.new(7))
local value: number = box:get():measure()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for path in ["src/box.tua", "src/main.tua"] {
        let errors = snapshot
            .pipeline_diagnostics(path)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error)
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "{path} should type-check: {errors:#?}");
    }

    let invalid_main = format!(
        "{}\nfinal class Lookalike {{ measure(): number {{ return 7 }} }}\nlocal invalid = boxes.Box.new(Lookalike.new())\n",
        fixture
            .files()
            .iter()
            .find(|file| file.path.as_path() == std::path::Path::new("src/main.tua"))
            .expect("main fixture")
            .source
    );
    host.set_file("src/main.tua", invalid_main);
    let errors = host
        .snapshot()
        .pipeline_diagnostics("src/main.tua")
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error)
        .collect::<Vec<_>>();
    assert!(
        errors.len() == 1
            && errors[0].message.contains("requires explicit conformance")
            && errors[0].message.contains("Lookalike"),
        "a same-shaped class without implements must still be rejected: {errors:#?}"
    );
}

#[test]
fn imported_child_classes_keep_inherited_protocol_bounds() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Updatable {
  update(dt: number): nil
}
-- file: src/actors.tua
import type { Updatable } from "./contracts"
open class Base implements Updatable {
  update(dt: number): nil { return nil }
}
final class Child extends Base {
}
export type ChildInstance = Child
final class Box<T: Updatable> {
  value: T
  init(value: T) { self.value = value }
  get(): T { return self.value }
}
export type ChildBox = Box<Child>
return { Child = Child, Box = Box }
-- file: src/api.tua
import type { Updatable } from "./contracts"
export function tick<T: Updatable>(value: T): T
  value:update(1)
  return value
end
return { tick = tick }
-- file: src/lookalike.tua
final class Lookalike {
  update(dt: number): nil { return nil }
}
return { Lookalike = Lookalike }
-- file: src/main.tua
import type { ChildInstance } from "./actors"
import type { ChildBox } from "./actors"
local actors = require("actors")
local api = require("api")
local lookalikes = require("lookalike")
local child: ChildInstance = actors.Child.new()
local _accepted = api.tick(child)
local _box = actors.Box.new(child)
local _typedBox: ChildBox = _box
local _invalidBox = actors.Box.new(lookalikes.Lookalike.new())
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();

    let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == crate::DiagnosticSeverity::Error)
        .collect::<Vec<_>>();
    assert!(
        errors.len() == 1
            && errors[0].message.contains("requires explicit conformance")
            && errors[0].message.contains("Updatable")
            && errors[0].message.contains("Lookalike"),
        "an imported child should retain its same-file parent's protocol conformance for bounded functions and classes, while a same-shaped lookalike is rejected: {diagnostics:#?}"
    );
}

#[test]
fn bounded_generic_body_uses_protocol_members_and_displays_the_constraint() {
    let fixture = Fixture::parse(
        r#"protocol Updatable {
  update(dt: number): nil
}

final class Actor implements Updatable {
  update(dt: number): nil { return nil }
}

function <|function_hover|>tick<T: <|bound|>Updatable<|/bound|>>(value: T): T
  value:<|completion|>update(1)
  return value
end

local result = tick(Actor.new())
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", fixture.source("src/main.tua"));
    let snapshot = host.snapshot();

    let completions =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("completion")));
    assert_eq!(completions, ["update"]);

    let hover = snapshot
        .hover("src/main.tua", fixture.position("function_hover"))
        .expect("generic declaration hover should be available");
    assert!(hover.contents.contains("T: Updatable"), "{hover:#?}");

    let definition = snapshot
        .definition("src/main.tua", fixture.position("bound"))
        .expect("protocol bound should navigate to its declaration");
    assert_eq!(definition.span.start.line, 1);
    assert_eq!(definition.span.start.column, 10);
    assert!(definition.path.ends_with("src/main.tua"));
}

#[test]
fn imported_protocol_arrays_reject_mutable_concrete_widening() {
    let fixture = Fixture::parse(
        r#"-- file: src/contracts.d.tua
export protocol Readable {
  read(): number
}
-- file: src/entities.tua
import type { Readable } from "./contracts"
final class Entity implements Readable {
  read(): number {
    return 1
  }
}
export type EntityInstance = Entity
return { Entity = Entity }
-- file: src/main.tua
import type { Readable } from "./contracts"
import type { EntityInstance } from "./entities"
local concrete: EntityInstance[] = {}
local view: Readable[] = concrete
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let diagnostics = snapshot.diagnostics("src/main.tua");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(DiagnosticCode::TYPE_MISMATCH)
                && diagnostic.message.contains("expected Readable[]")
                && diagnostic.message.contains("found EntityInstance[]")
        }),
        "the project semantic environment must preserve imported protocol identity when rejecting mutable widening: {diagnostics:#?}"
    );
}

#[test]
fn snapshot_recovers_imported_return_alias_field_completion_while_typing() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Functions = require("functions")

local makePlayer = Functions.makePlayer
local player = makePlayer("static import")
local hp = player.<|complete|>
-- file: src/functions.tua
type Player = {
  name: string,
  hp: number,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    hp = 100,
  }
end

return {
  makePlayer = makePlayer,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));

    assert!(labels.contains(&"hp".to_owned()), "{labels:?}");
    assert!(labels.contains(&"name".to_owned()), "{labels:?}");
}

#[test]
fn snapshot_caches_completion_and_shares_inferred_love_facts() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local function loadPlayerImage()
  return image "assets/player.png"
end

local <|hover|>player = loadPlayerImage()
player:<|complete|>setFilter(<|signature|>"nearest")
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("inferred LOVE local should hover");

    assert_eq!(hover.contents, "local player: Image");
    assert_eq!(
        snapshot
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .len(),
        0
    );

    let completions = snapshot.completion("src/main.tua", fixture.position("complete"));
    let labels = completions
        .iter()
        .map(|completion| completion.label.clone())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"setFilter".to_owned()));
    assert_eq!(
        snapshot
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .len(),
        1
    );
    assert_eq!(
        snapshot
            .completion("src/main.tua", fixture.position("complete"))
            .iter()
            .map(|completion| completion.label.clone())
            .collect::<Vec<_>>(),
        labels
    );
    assert_eq!(
        snapshot
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .len(),
        1
    );
    assert!(snapshot
        .resolve_completion(
            "src/main.tua",
            fixture.position("complete"),
            "setFilter",
            Some(CompletionKind::Function),
            None,
            None,
        )
        .is_some());
    assert_eq!(
        snapshot
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .len(),
        1
    );

    let hints = snapshot.inlay_hints("src/main.tua");

    assert!(hints.iter().any(|hint| hint.label == ": Image"));

    let signature = snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .expect("LOVE method signature should resolve");

    assert!(signature.signatures[0].label.contains("setFilter"));
}

#[test]
fn project_usage_index_updates_changed_files_and_isolates_completion_revisions() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Sprite = { pause: number, play: number, stop: number }
local sprite: Sprite = {}
local selected = sprite.<|member|>
-- file: src/events.tua
local events = signal()
events:emit("<|event|>")
"#,
    );
    let main_path = PathBuf::from("src/main.tua");
    let events_path = PathBuf::from("src/events.tua");
    let usage_path = PathBuf::from("src/usage.tua");
    let mut host = AnalysisHost::new();

    host.set_file(&main_path, fixture.source(&main_path));
    host.set_file(&events_path, fixture.source(&events_path));
    host.set_file(
        &usage_path,
        r#"local sprite = {}
sprite.unrelated()
sprite.unrelated()
sprite.stop()
sprite.stop()
sprite.play()

local events = signal()
events:on("enemy.spawn", function() end)
"#,
    );

    let first = host.snapshot();
    let first_members =
        completion_labels(&first.completion(&main_path, fixture.position("member")));
    let first_events =
        completion_labels(&first.completion(&events_path, fixture.position("event")));
    let first_graph = first.signal_graph();

    assert_eq!(first_members[0], "stop");
    assert!(!first_members.contains(&"unrelated".to_owned()));
    assert_eq!(first_graph.events.len(), 1, "{first_graph:?}");
    assert_eq!(first_graph.events[0].name, "enemy.spawn");
    assert_eq!(first_events, vec!["enemy.spawn".to_owned()]);
    assert_eq!(host.cache_stats().usage_index_misses, 3);

    host.set_file(
        &usage_path,
        r#"local sprite = {}
sprite.play()
sprite.play()
sprite.play()
sprite.stop()

local events = signal()
events:on("enemy.move", function() end)
"#,
    );

    let stale_members =
        completion_labels(&first.completion(&main_path, fixture.position("member")));
    assert_eq!(stale_members[0], "stop");

    let second = host.snapshot();
    let second_members =
        completion_labels(&second.completion(&main_path, fixture.position("member")));
    let second_events =
        completion_labels(&second.completion(&events_path, fixture.position("event")));
    let second_graph = second.signal_graph();

    assert!(second.revision() > first.revision());
    assert_eq!(second_members[0], "play");
    assert_eq!(second_events, vec!["enemy.move".to_owned()]);
    assert_eq!(second_graph.events.len(), 1);
    assert_eq!(second_graph.events[0].name, "enemy.move");

    let stats = host.cache_stats();
    assert_eq!(stats.usage_index_hits, 2);
    assert_eq!(stats.usage_index_misses, 4);
}

#[test]
fn snapshot_definition_cache_covers_the_whole_identifier_token() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type MetaVector = {
  x: number,
  y: number,
  length: number,
}

local function combineVector(left: MetaVector, right: MetaVector): MetaVector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
    length = left.length + right.<|lookup|>length,
  }
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let first_position = fixture.position("lookup");
    let second_position = SourcePosition {
        line: first_position.line,
        column: first_position.column + 4,
    };
    let first = snapshot
        .definition("src/main.tua", first_position)
        .expect("field definition should resolve");
    let cache_len = snapshot
        .definition_cache
        .lock()
        .expect("definition cache lock poisoned")
        .len();
    let second = snapshot
        .definition("src/main.tua", second_position)
        .expect("field definition should resolve from token cache");

    assert_eq!(first, second);
    assert_eq!(first.kind, DefinitionKind::TableField);
    assert_eq!(
        snapshot
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .len(),
        cache_len,
        "lookup inside the same identifier should reuse the warmed cache"
    );
}

#[test]
fn snapshot_completion_reuses_prefix_stable_cache_across_typing_snapshots() {
    let valid = r#"type Player = {
  health: number,
  score: number,
}

local player: Player = { health = 10, score = 1 }
"#;
    let base = Fixture::parse(&format!(
        "-- file: src/main.tua\n{valid}player.<|complete|>\n"
    ));
    let typed = Fixture::parse(&format!(
        "-- file: src/main.tua\n{valid}player.EA<|complete|>\n"
    ));
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, base.source("src/main.tua"));

    let first = host.snapshot();
    let base_labels = completion_labels(&first.completion(&path, base.position("complete")));

    assert!(base_labels.contains(&"health".to_owned()));
    assert!(base_labels.contains(&"score".to_owned()));

    let warmed_cache_len = first
        .completion_cache
        .lock()
        .expect("completion cache lock poisoned")
        .len();

    host.update_open_document(&path, typed.source("src/main.tua"));

    let second = host.snapshot();
    let typed_labels = completion_labels(&second.completion(&path, typed.position("complete")));
    let typed_completions = second.completion(&path, typed.position("complete"));

    assert_eq!(typed_labels, vec!["health".to_owned()]);
    assert_eq!(typed_completions[0].filter_text.as_deref(), Some("EA"));
    assert_eq!(
        second
            .completion_cache
            .lock()
            .expect("completion cache lock poisoned")
            .len(),
        warmed_cache_len,
        "typing a case-insensitive contains prefix should reuse the warmed player. completion cache"
    );
}

#[test]
fn snapshot_completion_reuses_prefix_cache_for_plain_identifiers() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Player = { hp: number }
local score = 1

local function damage(player: Player): Player
  return player
end

local typed: Pl = nil
local value = sco<|complete|>
local call = dam
---
local function heal(player: Player): Player
  return player
end
"#,
    );
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, fixture.source("src/main.tua"));

    let labels = completion_labels(
        &host
            .snapshot()
            .completion(&path, fixture.position("complete")),
    );

    assert!(
        labels.contains(&"score".to_owned()),
        "plain identifier completion should reuse stripped-prefix source: {labels:?}"
    );
}

#[test]
fn snapshot_definition_resolves_colon_self_field_to_type_shape_field() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
type Account = {
  name: string,
  <|field_def|>balance<|/field_def|>: number,
}

local Account = {}
Account.__index = Account

function Account:new(name, balance)
  local instance = {
    name = name,
    balance = balance,
  }

  return setmetatable(instance, self)
end

function Account:deposit(amount: number): number
  self.<|field_ref|>balance = self.balance + amount

  return self.balance
end
"#,
    );
    let mut host = AnalysisHost::new();

    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }

    let snapshot = host.snapshot();
    let first = snapshot
        .definition("src/main.tua", fixture.position("field_ref"))
        .expect("self field definition should resolve through synthetic receiver");
    let cache_len = snapshot
        .definition_cache
        .lock()
        .expect("definition cache lock poisoned")
        .len();
    let second = snapshot
        .definition("src/main.tua", fixture.position("field_ref"))
        .expect("repeated self field definition should stay cached");

    assert_eq!(first, second);
    assert_eq!(first.kind, DefinitionKind::TableField);
    assert_eq!(first.span, fixture.range("field_def"));
    assert_eq!(
        snapshot
            .definition_cache
            .lock()
            .expect("definition cache lock poisoned")
            .len(),
        cache_len
    );
}

#[test]
fn snapshot_completion_uses_recovered_semantics_after_incomplete_colon() {
    let valid = r#"type Account = {
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ balance = 50 }, Account)
"#;
    let invalid = Fixture::parse(&format!("{valid}account:<|complete|>\n"));
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, invalid.source("src/main.tua"));

    let completions = host
        .snapshot()
        .completion(&path, invalid.position("complete"));

    assert!(
        completion_labels(&completions)
            .iter()
            .any(|label| label == "deposit"),
        "incomplete colon access should preserve metatable method completion"
    );
}

#[test]
fn snapshot_completion_uses_current_source_with_last_good_semantics_after_unrecoverable_edit() {
    let valid = r#"type Player = {
  hp: number,
  name: string,
}

local player: Player = { hp = 100, name = "Ada" }
"#;
    let invalid = Fixture::parse(&format!(
        "-- file: src/main.tua\n{valid}local unfinished = (\nplayer.<|complete|>\n"
    ));
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, invalid.source("src/main.tua"));

    let completions = host
        .snapshot()
        .completion(&path, invalid.position("complete"));
    let labels = completion_labels(&completions);

    assert!(
        labels.contains(&"hp".to_owned()) && labels.contains(&"name".to_owned()),
        "field completion should use last good types at the current edit position: {labels:?}"
    );
}

#[test]
fn asset_index_refresh_preserves_last_good_semantics_for_invalid_open_documents() {
    let temp = TempProject::new("asset-refresh-last-good");
    let valid = r#"type Player = {
  hp: number,
}

local player: Player = { hp = 100 }
"#;
    let invalid = Fixture::parse(&format!(
        "-- file: src/main.tua\n{valid}local unfinished = (\nplayer.<|complete|>\n"
    ));
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_asset_roots([temp.path().to_path_buf()]);
    host.open_document(&path, valid);
    let _ = host.snapshot();
    host.update_open_document(&path, invalid.source("src/main.tua"));

    temp.write("assets/player.png", png_header(8, 8));
    host.refresh_asset_index_from_disk();

    let labels = completion_labels(
        &host
            .snapshot()
            .completion(&path, invalid.position("complete")),
    );

    assert!(
        labels.contains(&"hp".to_owned()),
        "asset refresh should not clear last-good semantic completions: {labels:?}"
    );
}

#[test]
fn snapshot_hovers_parseable_lua_doc_project_locals_as_class_shapes() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local LuaDocTypes = require("lua_doc_types")

local <|nil_decl_hover|>nilEnemy: LuaDocTypes.Enemy = nil
local <|table_decl_hover|>tableEnemy: LuaDocTypes.Enemy = {
  hp = 1,
  name = "hello",
}

local nilCopy = <|nil_hover|>nilEnemy
local tableCopy = <|table_hover|>tableEnemy
-- file: src/lua_doc_types.lua
---@class Enemy
---@field hp number
---@field name string
---@field type 'friendly' | 'mad'
local Enemy = {}

return {
  Enemy = Enemy,
}
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };

        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let snapshot = host.snapshot();
    let nil_hover = snapshot
        .hover("src/main.tua", fixture.position("nil_hover"))
        .expect("nil-initialized lua doc local should hover");
    let table_hover = snapshot
        .hover("src/main.tua", fixture.position("table_hover"))
        .expect("table-initialized lua doc local should hover");
    let nil_decl_hover = snapshot
        .hover("src/main.tua", fixture.position("nil_decl_hover"))
        .expect("nil-initialized lua doc local declaration should hover");
    let table_decl_hover = snapshot
        .hover("src/main.tua", fixture.position("table_decl_hover"))
        .expect("table-initialized lua doc local declaration should hover");

    assert_eq!(
        nil_hover.contents,
        "local nilEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        table_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        nil_decl_hover.contents,
        "local nilEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
    assert_eq!(
        table_decl_hover.contents,
        "local tableEnemy: { hp: number, name: string, type: \"friendly\" | \"mad\" }"
    );
}

#[test]
fn snapshot_workspace_symbols_update_after_file_changes() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    host.set_file(
        &path,
        r#"
type Player = { hp: number }
"#,
    );

    let first = host.snapshot();

    assert!(first
        .workspace_symbols("Player")
        .iter()
        .any(|symbol| symbol.name == "Player"));

    host.set_file(
        &path,
        r#"
type Enemy = { hp: number }
"#,
    );

    let second = host.snapshot();

    assert!(second
        .workspace_symbols("Enemy")
        .iter()
        .any(|symbol| symbol.name == "Enemy"));
    assert!(!second
        .workspace_symbols("Player")
        .iter()
        .any(|symbol| symbol.name == "Player"));
}

fn only_definition(definitions: Vec<Definition>) -> Definition {
    assert_eq!(definitions.len(), 1, "expected exactly one definition");

    definitions.into_iter().next().unwrap()
}

fn assert_reference_ranges(references: &[Reference], fixture: &Fixture, markers: &[&str]) {
    let mut actual = references
        .iter()
        .map(|reference| (reference.path.clone(), reference.span))
        .collect::<Vec<_>>();
    let mut expected = markers
        .iter()
        .map(|marker| {
            (
                fixture.marker_path(marker).to_path_buf(),
                fixture.range(marker),
            )
        })
        .collect::<Vec<_>>();

    actual.sort_by_key(path_span_key);
    expected.sort_by_key(path_span_key);

    assert_eq!(actual, expected);
}

fn assert_edit_ranges(edit: &RenameEdit, fixture: &Fixture, markers: &[&str]) {
    let mut actual = edit
        .edits
        .iter()
        .map(|edit| (edit.path.clone(), edit.span))
        .collect::<Vec<_>>();
    let mut expected = markers
        .iter()
        .map(|marker| {
            (
                fixture.marker_path(marker).to_path_buf(),
                fixture.range(marker),
            )
        })
        .collect::<Vec<_>>();

    actual.sort_by_key(path_span_key);
    expected.sort_by_key(path_span_key);

    assert_eq!(actual, expected);
}

fn path_span_key((path, span): &(PathBuf, SourceSpan)) -> (PathBuf, usize, usize, usize, usize) {
    (
        path.clone(),
        span.start.line,
        span.start.column,
        span.end.line,
        span.end.column,
    )
}

fn source_text(fixture: &Fixture, definition: &Definition) -> String {
    let path = definition
        .path
        .to_str()
        .expect("fixture paths should be UTF-8");
    let source = fixture.source(path);
    let line = source
        .lines()
        .nth(definition.span.start.line.saturating_sub(1))
        .expect("definition line should exist");
    let start = definition.span.start.column.saturating_sub(1);
    let len = definition
        .span
        .end
        .column
        .saturating_sub(definition.span.start.column);

    line.chars().skip(start).take(len).collect()
}

fn text_at_span(source: &str, span: SourceSpan) -> String {
    assert_eq!(
        span.start.line, span.end.line,
        "test helper expects one-line spans"
    );
    let line = source
        .lines()
        .nth(span.start.line.saturating_sub(1))
        .expect("span line should exist");

    line.chars()
        .skip(span.start.column.saturating_sub(1))
        .take(span.end.column.saturating_sub(span.start.column))
        .collect()
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(&0u32.to_be_bytes());

    bytes
}

#[test]
fn snapshot_diagnostics_warn_when_a_required_lua_module_does_not_parse() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local broken = require("<|module|>broken")
local luajit = require("luajit")
print(broken, luajit.sumOdd({ 1, 2, 3 }))
-- file: src/broken.lua
local M = {}
function M.value()
  return 1 +
end
return M
-- file: src/luajit.lua
local M = {}
function M.sumOdd(items)
  local total = 0
  for _, value in ipairs(items) do
    if value % 2 == 0 then goto continue end
    total = total + value
    ::continue::
  end
  return total
end
return M
"#,
    );
    let mut host = AnalysisHost::new();

    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        let kind = if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("lua")
        {
            AnalysisSourceKind::Lua
        } else {
            AnalysisSourceKind::Tua
        };
        host.set_file_with_kind(&file.path, file.source.clone(), kind);
    }

    let diagnostics = host.snapshot().diagnostics("src/main.tua");
    let unparsed = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_UNPARSED_MODULE))
        .collect::<Vec<_>>();

    assert_eq!(unparsed.len(), 1, "{diagnostics:#?}");
    assert_eq!(unparsed[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(unparsed[0].line, fixture.position("module").line);
    assert_eq!(unparsed[0].column, fixture.position("module").column);
    assert_eq!(
        unparsed[0].message,
        "module `broken` is untyped because src/broken.lua:3:12 does not parse: expected expression after binary operator"
    );
}
#[test]
fn input_budget_rejection_is_diagnostic_and_recovery_replaces_stale_data() {
    let mut host = AnalysisHost::new();
    host.set_file("main.tua", "local value = 1");
    host.open_document(
        "main.tua",
        " ".repeat(crate::input_limits::MAX_SOURCE_BYTES + 1),
    );
    let rejected = host.snapshot();
    assert!(rejected.source("main.tua").is_none());
    assert!(rejected.diagnostics("main.tua")[0]
        .message
        .contains("8 MiB"));
    host.open_document("main.tua", "local value = 2\nprint(value)");
    assert!(host.snapshot().diagnostics("main.tua").is_empty());
    host.input_sizes.insert(
        PathBuf::from("budget-fixture"),
        crate::input_limits::MAX_PROJECT_SOURCE_BYTES,
    );
    host.set_file("other.tua", "local other = 3\nprint(other)");
    assert!(host.snapshot().diagnostics("other.tua")[0]
        .message
        .contains("aggregate"));
    host.remove_file("budget-fixture");
    host.set_file("other.tua", "local other = 3\nprint(other)");
    assert!(host.snapshot().diagnostics("other.tua").is_empty());
}
#[test]
fn snapshot_clones_share_immutable_files_and_keep_old_revisions() {
    let mut host = AnalysisHost::new();
    host.set_file("shared.tua", "return 1");
    let old = host.snapshot();
    let cloned = old.clone();
    assert!(Arc::ptr_eq(&old.type_database, &cloned.type_database));
    assert!(Arc::ptr_eq(&old.module_graph, &cloned.module_graph));
    assert!(Arc::ptr_eq(&old.symbol_index, &cloned.symbol_index));
    assert!(Arc::ptr_eq(&old.usage_index, &cloned.usage_index));
    assert!(Arc::ptr_eq(
        &old.files[Path::new("shared.tua")].0,
        &cloned.files[Path::new("shared.tua")].0
    ));
    host.set_file("shared.tua", "return 2");
    let current = host.snapshot();
    assert_eq!(old.source("shared.tua"), Some("return 1"));
    assert_eq!(current.source("shared.tua"), Some("return 2"));
    assert!(!Arc::ptr_eq(
        &old.files[Path::new("shared.tua")].0,
        &current.files[Path::new("shared.tua")].0
    ));
}

#[path = "shader_tests.rs"]
mod shader_uniforms;

#[test]
fn signal_handler_inference_preserves_shadowed_parameter_types() {
    let source = r#"type Player = { hp: number }
local events = signal()
local player: Player = { hp = 10 }
events:on("hit", function(target)
  local function nested(target: string)
    print(target)
  end
end)
events:emit("hit", player)
"#;
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", source);
    let hover = host
        .snapshot()
        .hover(
            "src/main.tua",
            SourcePosition {
                line: 6,
                column: 11,
            },
        )
        .expect("nested parameter should hover");
    assert_eq!(hover.contents, "parameter target: string");
}

#[test]
fn event_payload_inference_follows_imported_module_property_aliases() {
    let mut host = AnalysisHost::new();
    host.set_file(
        "src/events.tua",
        "local enemies = signal()
return { enemies = enemies }
",
    );
    host.set_file(
        "src/combat.tua",
        r#"local Events = require "events"
local EnemyEvents = Events.enemies
EnemyEvents:emit("duck.hit", vec2(1, 2))
"#,
    );
    let source = r#"local Events = require "events"
local EnemyEvents = Events.enemies
EnemyEvents:on("duck.hit", function(position)
  print(position.x)
end)
"#;
    host.set_file("src/main.tua", source);
    let hover = host
        .snapshot()
        .hover(
            "src/main.tua",
            crate::source_text::position_at(source, "position.x"),
        )
        .expect("payload hover");
    assert_eq!(hover.contents, "parameter position: Vec2");
}

#[test]
fn editor_snapshot_defers_emission_validation_until_diagnostics() {
    let mut host = AnalysisHost::new();
    host.set_file(
        "src/main.tua",
        "local value = 1
print(value)
",
    );
    let snapshot = host.snapshot();
    assert!(snapshot
        .emission_diagnostics_cache
        .lock()
        .unwrap()
        .is_empty());
    snapshot.completion("src/main.tua", SourcePosition { line: 2, column: 8 });
    assert!(snapshot
        .emission_diagnostics_cache
        .lock()
        .unwrap()
        .is_empty());
    snapshot.pipeline_diagnostics("src/main.tua");
    assert_eq!(snapshot.emission_diagnostics_cache.lock().unwrap().len(), 1);
}

#[test]
fn lazy_emission_diagnostics_preserve_lowering_errors_and_refresh_after_edits() {
    let mut host = AnalysisHost::new();
    host.set_file(
        "src/main.tua",
        "if value is Array<string> then print(value) end\n",
    );
    let snapshot = host.snapshot();
    assert!(snapshot
        .pipeline_diagnostics("src/main.tua")
        .iter()
        .any(|d| d.code == Some(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)));
    host.set_file("src/main.tua", "if value is string then print(value) end\n");
    assert!(!host
        .snapshot()
        .pipeline_diagnostics("src/main.tua")
        .iter()
        .any(|d| d.code == Some(DiagnosticCode::TYPE_UNSUPPORTED_RUNTIME_CHECK)));
}

#[test]
fn event_bus_identity_tracks_alias_bindings_and_export_origins() {
    for (events, source, expected) in [
        (
            "",
            r#"local events = signal()
local E = events
E:emit("hit", 42)
events:on("hit", function(payload) print(payload) end)
"#,
            "number",
        ),
        (
            r#"local bus = signal()
bus:emit("hit", 42)
return { enemies = bus }
"#,
            r#"local Events = require "events"
local E = Events.enemies
E:on("hit", function(payload) print(payload) end)
"#,
            "number",
        ),
        (
            "return { enemies = signal() }",
            r#"local Events = require "events"
do
  local Events = { enemies = signal() }
  Events.enemies:emit("hit", "wrong")
end
Events.enemies:on("hit", function(payload) print(payload) end)
"#,
            "any",
        ),
        (
            "return { enemies = signal() }",
            r#"local Events = require "events"
local E = Events.enemies
E = signal()
E:emit("hit", "wrong")
Events.enemies:on("hit", function(payload) print(payload) end)
"#,
            "any",
        ),
        (
            "return { enemies = signal() }",
            r#"local Events = require "events"
Events = { enemies = signal() }
Events.enemies:emit("hit", "wrong")
local Other = require "events"
Other.enemies:on("hit", function(payload) print(payload) end)
"#,
            "any",
        ),
    ] {
        let mut host = AnalysisHost::new();
        host.set_file("src/events.tua", events);
        host.set_file("src/main.tua", source);
        let hover = host
            .snapshot()
            .hover(
                "src/main.tua",
                crate::source_text::position_at(source, "payload)"),
            )
            .expect("parameter hover");
        assert_eq!(
            hover.contents,
            format!("parameter payload: {expected}"),
            "{source}"
        );
    }
}

#[test]
fn snapshot_navigates_inferred_class_array_receivers() {
    let fixture = Fixture::parse(
        r#"-- file: src/actors.tua
open class Actor {
  hp: number = 100
  <|damage_def|>damage<|/damage_def|>(amount: number): boolean {
    self.hp -= amount
    return self.hp <= 0
  }
}
export type ActorInstance = Actor
return { Actor = Actor }
-- file: src/world.tua
import type { ActorInstance } from "./actors"
local actors: ActorInstance[] = {}
local target = actors[1]
target:<|indexed|>damage<|/indexed|>(1)
return { actors = actors }
-- file: src/main.tua
local world = require("world")
for _, actor in ipairs(world.actors) do
  actor:<|iterated|>damage<|/iterated|>(1)
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for (path, marker) in [("src/world.tua", "indexed"), ("src/main.tua", "iterated")] {
        let definition = snapshot
            .definition(path, fixture.position(marker))
            .expect("inferred class receiver should navigate");
        assert_eq!(definition.path, PathBuf::from("src/actors.tua"));
        assert_eq!(definition.span, fixture.range("damage_def"));
        assert!(snapshot
            .completion(path, fixture.position(marker))
            .iter()
            .any(|item| item.label == "damage"));
    }
}

#[test]
fn guarded_imported_instance_preserves_multi_returns_in_first_statement() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { LabelInstance } from "./label"
local title: LabelInstance? = nil
function love.draw()
  if title then
    local width, height = title:getSize()
    print(<|width|>width, <|height|>height)
  end
end
-- file: src/label.tua
final class Label {
  getSize(): (number, number) { return 10, 20 }
}
export type LabelInstance = Label
return { Label = Label }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for marker in ["width", "height"] {
        let hover = snapshot
            .hover("src/main.tua", fixture.position(marker))
            .unwrap();
        assert_eq!(hover.contents, format!("local {marker}: number"));
    }
}

#[test]
fn incomplete_event_payload_member_keeps_callback_scope() {
    for statement in [
        "print(payload.<|cursor|>)",
        "local label = payload.<|cursor|> .. \" hit\"",
    ] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/events.tua
return {{ combat = signal() }}
-- file: src/emitter.tua
local Events = require("events")
Events.combat:emit("hit", {{ amount = 12, name = "Slime" }})
-- file: src/main.tua
local Events = require("events")
Events.combat:on("hit", function(payload)
  {statement}
end)
"#
        ));
        let mut host = AnalysisHost::new();
        host.set_source_roots([PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snap = host.snapshot();
        let completions = snap.completion("src/main.tua", fixture.position("cursor"));
        assert!(
            completions.iter().any(|item| item.label == "amount"),
            "{statement}: {completions:?}"
        );
        assert!(completions.iter().any(|item| item.label == "name"));
    }
    let literal = "local label = 'target.)' -- target.)\n";
    assert_eq!(
        source_without_recoverable_incomplete_lines(literal),
        literal
    );
}

#[test]
fn incomplete_line_recovery_uses_class_placeholder_only_inside_class_bodies() {
    let table_source = "local options = {\n  pending\n}\n";
    let recovered_table = source_without_recoverable_incomplete_lines(table_source);
    assert!(!recovered_table.contains("do end"));
    assert!(recovered_table.contains("  \n"));

    let class_source = "final class Box {\n  inspect(value: number) {\n    value:\n  }\n}\n";
    let recovered_class = source_without_recoverable_incomplete_lines(class_source);
    assert!(recovered_class.contains("    do end\n"));
    assert_eq!(class_source.len(), recovered_class.len());
}

#[test]
fn incomplete_protocol_member_recovery_keeps_prior_contract_and_module_bindings() {
    let source = "local retained = true\nexport protocol Display {\n  render(value: { text: string }): number | nil\n  r";
    let recovered = super::recovery::parse_degraded_analysis_source(
        Path::new("src/display.tua"),
        source,
        AnalysisSourceKind::Tua,
    )
    .expect("editor recovery should remove the partial member and close the protocol body");

    assert!(
        recovered.ast.to_string().contains("retained"),
        "recovery must preserve declarations before the unfinished protocol member"
    );
    assert_eq!(recovered.protocol_declarations.len(), 1);
    assert_eq!(recovered.protocol_declarations[0].name, "Display");
    assert_eq!(recovered.protocol_declarations[0].methods.len(), 1);
    assert_eq!(recovered.protocol_declarations[0].methods[0].name, "render");
}

#[test]
fn incomplete_protocol_does_not_hide_later_protocol_in_degraded_analysis() {
    let source = r#"export protocol Updatable {
  update(dt: number): nil
export protocol Drawable {
  draw(): nil
}
local function render(value: Drawable): nil
  value:
end
"#;
    let recovered = super::recovery::parse_degraded_analysis_source(
        Path::new("src/drawable.tua"),
        source,
        AnalysisSourceKind::Tua,
    )
    .expect("editor recovery should separate the next protocol declaration");

    assert_eq!(
        recovered
            .protocol_declarations
            .iter()
            .map(|protocol| protocol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Updatable", "Drawable"]
    );
    assert_eq!(recovered.protocol_declarations[1].methods[0].name, "draw");
    assert!(recovered.ast.to_string().contains("render"));

    let path = PathBuf::from("src/drawable.tua");
    let mut host = AnalysisHost::new();
    host.open_document(&path, source);
    let snapshot = host.snapshot();
    let completions = snapshot.completion(&path, SourcePosition { line: 7, column: 9 });
    assert!(
        completions
            .iter()
            .any(|completion| completion.label == "draw"),
        "degraded completion should resolve the following protocol's members: {completions:#?}"
    );
}

#[test]
fn snapshot_chained_factory_uses_final_multiple_returns() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Factory = require("factory")
local width, height = Factory.canvas():getDimensions()
print(<|width|>width, <|height|>height)
-- file: src/factory.tua
local function canvas(): Canvas
  return love.graphics.newCanvas(320, 180)
end
return { canvas = canvas }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for marker in ["width", "height"] {
        let hover = snapshot
            .hover("src/main.tua", fixture.position(marker))
            .unwrap();
        assert_eq!(hover.contents, format!("local {marker}: number"));
    }
}

#[test]
fn lua_doc_multiple_returns_reach_imported_local_hovers() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Api = require("api")
local width, label = Api.measure()
print(<|width|>width, <|label|>label)
-- file: src/api.lua
---@return number, string Width and display label.
local function measure()
  return 10, "wide"
end
return { measure = measure }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for (marker, ty) in [("width", "number"), ("label", "string")] {
        let hover = snapshot
            .hover("src/main.tua", fixture.position(marker))
            .unwrap();
        assert_eq!(hover.contents, format!("local {marker}: {ty}"));
    }
}

#[test]
fn devex_event_fields_agree_between_hover_and_completion_during_edits() {
    for access in ["print(payload.<|member|>amount)", "payload.<|member|>"] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/events.tua
return {{ combat = signal() }}
-- file: src/emitter.tua
local Events = require("events")
Events.combat:emit("hit", {{ amount = 12, name = "Slime" }})
-- file: src/main.tua
local Events = require("events")
Events.combat:on("hit", function(payload)
  {access}
end)
"#
        ));
        let mut host = AnalysisHost::new();
        host.set_source_roots([PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snapshot = host.snapshot();
        let completions = snapshot.completion("src/main.tua", fixture.position("member"));
        assert!(
            completions.iter().any(|item| item.label == "amount"),
            "{access}: {completions:?}"
        );
        if access.contains("amount") {
            let hover = snapshot
                .hover("src/main.tua", fixture.position("member"))
                .unwrap();
            assert_eq!(hover.contents, "(field) amount: number");
            assert_eq!(hover.span.unwrap().start, fixture.position("member"));
        }
    }
}

#[test]
fn devex_imported_loop_receiver_agrees_across_editor_surfaces() {
    let fixture = Fixture::parse(
        r#"-- file: src/actors.tua
open class Actor {
  hp: number = 100
  update(dt: number): number { return self.hp + dt }
}
export type ActorInstance = Actor
return { Actor = Actor }
-- file: src/main.tua
import type { ActorInstance } from "./actors"
local actors: ActorInstance[] = {}
for _, actor in ipairs(actors) do
  print(<|hover|>actor)
  actor:<|completion|>update(<|signature|>1)
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("local actor: Actor"), "{hover:?}");
    assert!(snapshot
        .completion("src/main.tua", fixture.position("completion"))
        .iter()
        .any(|item| item.label == "update"));
    assert!(snapshot
        .signature_help("src/main.tua", fixture.position("signature"))
        .unwrap()
        .signatures[0]
        .label
        .contains("dt: number"));
    assert_eq!(
        snapshot
            .definition("src/main.tua", fixture.position("completion"))
            .unwrap()
            .path,
        PathBuf::from("src/actors.tua")
    );
}

#[test]
fn devex_class_instance_conformance_matrix() {
    for binding in [
        "local label = Labels.Label.new()",
        "local label: LabelInstance = Labels.Label.new()",
        "local label: LabelInstance? = Labels.Label.new()",
    ] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/main.tua
import type {{ LabelInstance }} from "./label"
local Labels = require("label")
{binding}
if label then
  local width = label:measure(1)
  print(<|width|>width)
  label:<|member|>measure(<|signature|>1)
end
-- file: src/label.tua
final class Label {{
  measure(scale: number): number {{ return 10 * scale }}
}}
export type LabelInstance = Label
return {{ Label = Label }}
"#
        ));
        let mut host = AnalysisHost::new();
        host.set_source_roots([PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snapshot = host.snapshot();
        assert_eq!(
            snapshot
                .hover("src/main.tua", fixture.position("width"))
                .unwrap()
                .contents,
            "local width: number",
            "{binding}"
        );
        assert!(
            snapshot
                .completion("src/main.tua", fixture.position("member"))
                .iter()
                .any(|item| item.label == "measure"),
            "{binding}"
        );
        assert!(
            snapshot
                .signature_help("src/main.tua", fixture.position("signature"))
                .unwrap()
                .signatures[0]
                .label
                .contains("scale: number"),
            "{binding}"
        );
        assert_eq!(
            snapshot
                .definition("src/main.tua", fixture.position("member"))
                .unwrap()
                .path,
            PathBuf::from("src/label.tua"),
            "{binding}"
        );
    }
}

#[test]
fn port_audit_class_parameter_members_use_source_coordinates() {
    for parameter_type in ["LocalItem", "ImportedItem"] {
        let fixture = Fixture::parse(&format!(
            r#"-- file: src/main.tua
import type {{ ImportedItem }} from "./item"
final class LocalItem {{
  dead: boolean = false
  velocity(scale: number): number {{ return scale }}
}}
final class Inspector {{
  inspect(item: {parameter_type}): number {{
    local n = item:<|completion|>velocity(<|signature|>1)
    if item.<|hover|>dead then return 0 end
    return n
  }}
}}
return Inspector
-- file: src/item.tua
final class Item {{
  dead: boolean = false
  velocity(scale: number): number {{ return scale }}
}}
export type ImportedItem = Item
return {{ Item = Item }}
"#
        ));
        let mut host = AnalysisHost::new();
        host.set_source_roots([PathBuf::from("src")]);
        for file in fixture.files() {
            host.set_file(&file.path, file.source.clone());
        }
        let snapshot = host.snapshot();
        assert!(
            snapshot
                .completion("src/main.tua", fixture.position("completion"))
                .iter()
                .any(|item| item.label == "velocity"),
            "{parameter_type}"
        );
        assert!(snapshot
            .hover("src/main.tua", fixture.position("hover"))
            .unwrap()
            .contents
            .contains("boolean"));
        assert!(snapshot
            .signature_help("src/main.tua", fixture.position("signature"))
            .unwrap()
            .signatures[0]
            .label
            .contains("scale: number"));
    }
}

#[test]
fn port_audit_class_loop_members_use_source_coordinates() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Layout = require("layout")
final class Viewer {
  draw() {
    local layers = Layout.layers()
    for _, layer in ipairs(layers) do
      print(layer.<|hover|>rotation)
      print(layer.<|completion|>rotation)
    end
    for index = 1, 3 do
      print(<|index|>index)
    end
  }
}
return Viewer
-- file: src/layout.tua
export type Layer = { rotation: number, y: number }
local function layers(): Layer[]
  return { { rotation = 1, y = 2 } }
end
return { layers = layers }
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    assert!(snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap()
        .contents
        .contains("number"));
    assert!(snapshot
        .completion("src/main.tua", fixture.position("completion"))
        .iter()
        .any(|item| item.label == "y"));
    assert_eq!(
        snapshot
            .hover("src/main.tua", fixture.position("index"))
            .unwrap()
            .contents,
        "local index: number"
    );
}

#[test]
fn declaration_contracts_keep_cross_file_returns_and_editor_members_separate() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local api = require("values")
local n = api.count()
local s = api.text()
print(<|number|>n, <|string|>s)
-- file: src/values.tua
type Pose = { x: number }
local function count()
  local p: number = 3
  return p
end
local function text()
  local p: string = "ok"
  return p
end
local function pose()
  local p: Pose = { x = 2 }
  print(p.<|members|>x)
  return function() return p.<|capture|>x end
end
local function parameter(p: string)
  print(<|parameter|>p)
end
return {count = count, text = text, pose = pose}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    for (path, marker, expected) in [
        ("src/main.tua", "number", "number"),
        ("src/main.tua", "string", "string"),
        ("src/values.tua", "capture", "number"),
        ("src/values.tua", "parameter", "string"),
    ] {
        let hover = snapshot
            .hover(path, fixture.position(marker))
            .expect(marker);
        assert!(hover.contents.contains(expected), "{marker}: {hover:?}");
    }
    assert!(snapshot
        .completion("src/values.tua", fixture.position("members"))
        .iter()
        .any(|c| c.label == "x"));
}

#[test]
fn nested_callable_module_exports_support_editor_and_checker() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
local Camera = require("vendor")
local value = Camera.smooth.<|hover|>damped(5)
local typed: number = value
print(Camera.smooth.<|complete|>damped(2))
Camera.smooth.damped("wrong")
local dynamic: any = require("vendor")
dynamic.unknown.member()
-- file: src/vendor.lua
local camera = {}
camera.__index = camera
camera.smooth = {}
---@param speed number
---@return number
function camera.smooth.damped(speed) return speed end
return setmetatable({smooth = camera.smooth}, {__call = function() return {} end})
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "{diagnostics:?}");
    assert_eq!(errors[0].code, Some(DiagnosticCode::TYPE_MISMATCH));
    assert!(snapshot
        .completion("src/main.tua", fixture.position("complete"))
        .iter()
        .any(|item| item.label == "damped"));
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("speed"), "{hover:?}");
}

#[test]
fn project_global_modules_keep_runtime_ownership_across_unsaved_edits() {
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file("src/main.tua", "Camera = require('vendor')\n");
    host.set_file(
        "src/vendor.lua",
        "return {smooth = {damped = function(n) return n end}}\n",
    );
    host.set_file("src/scene.tua", "local value = Camera.smooth.damped(5)\n");
    let check = |snapshot: &AnalysisSnapshot, builtin: bool| {
        let file = snapshot.file(Path::new("src/scene.tua")).unwrap();
        let parsed = file.parsed.as_ref().unwrap();
        let environment = snapshot.project_type_environment("src/scene.tua").unwrap();
        let emitted = crate::emit::emit_lua_artifact_with_mode_import_resolver_and_environment(
            parsed,
            crate::AnnotationMode::Off,
            None,
            false,
            Some(&environment),
        )
        .unwrap();
        assert_eq!(
            emitted.code.contains("require(\"tuacamera\")"),
            builtin,
            "{}",
            emitted.code
        );
        if !builtin {
            let errors = snapshot
                .pipeline_diagnostics("src/scene.tua")
                .into_iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .collect::<Vec<_>>();
            assert!(errors.is_empty(), "{errors:?}");
            if let Some(hover) = snapshot.hover(
                "src/scene.tua",
                SourcePosition {
                    line: 1,
                    column: 16,
                },
            ) {
                assert!(!hover.contents.contains("built-in module"), "{hover:?}");
            }
            assert!(!snapshot
                .completion(
                    "src/scene.tua",
                    SourcePosition {
                        line: 1,
                        column: 22
                    }
                )
                .iter()
                .any(|item| item.label == "follow"));
        }
    };
    let original = host.snapshot();
    check(&original, false);
    host.set_file("src/main.tua", "local Camera = require('vendor')\n");
    check(&host.snapshot(), true);
    check(&original, false);
    host.set_file("src/main.tua", "Camera = require('vendor')\n");
    check(&host.snapshot(), false);
    host.set_file(
        "src/scene.tua",
        "local Camera = require('vendor')\nlocal value: number = Camera.smooth.damped(5)\n",
    );
    let snapshot = host.snapshot();
    let environment = snapshot.project_type_environment("src/scene.tua").unwrap();
    assert!(!matches!(environment.local("Camera"), Some(Type::Any)));
    assert!(!snapshot
        .pipeline_diagnostics("src/scene.tua")
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error));
}

#[test]
fn project_global_tween_suppresses_injection_but_unrelated_globals_do_not() {
    for (declaration, expected_builtin) in [
        ("function tween() end", false),
        ("tween = {}", false),
        ("local tween = {}", true),
        ("other = {}", true),
    ] {
        let mut host = AnalysisHost::new();
        host.set_file("globals.lua", declaration);
        host.set_file("scene.tua", "tween.update(1)");
        let snapshot = host.snapshot();
        let parsed = snapshot
            .file(Path::new("scene.tua"))
            .unwrap()
            .parsed
            .as_ref()
            .unwrap();
        let environment = snapshot.project_type_environment("scene.tua").unwrap();
        let emitted = crate::emit::emit_lua_artifact_with_mode_import_resolver_and_environment(
            parsed,
            crate::AnnotationMode::Off,
            None,
            false,
            Some(&environment),
        )
        .unwrap();
        assert_eq!(
            emitted.code.contains("require(\"tuatween\")"),
            expected_builtin,
            "{declaration}: {}",
            emitted.code
        );
    }
}

#[test]
fn returned_native_instance_preserves_colon_call_receivers() {
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_file("src/viewer.tua", "final class Viewer {\n model: boolean = false\n update(dt: number) {}\n draw() {}\n}\nexport type ViewerInstance = Viewer\nreturn Viewer.new()\n");
    host.set_file("src/main.tua", "local viewer = require('viewer')\nfunction love.update(dt: number)\n if viewer.model then viewer:update(dt) end\nend\nfunction love.draw()\n if viewer.model then viewer:draw() end\nend\n");
    let snapshot = host.snapshot();
    let errors = snapshot
        .pipeline_diagnostics("src/main.tua")
        .into_iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn current_file_queries_reuse_a_worker_snapshot_without_rebuilding() {
    let mut coordinator = AnalysisHost::new();
    let path = PathBuf::from("main.tua");
    coordinator.set_file(&path, "local function before() end\n");
    let worker = coordinator.clone();
    worker.snapshot();
    assert_eq!(coordinator.cache_stats().parse_misses, 0);
    let symbols = coordinator.document_symbols(&path);
    assert!(symbols.iter().any(|symbol| symbol.name == "before"));
    assert_eq!(coordinator.cache_stats().parse_misses, 0);
    assert_eq!(coordinator.cache_stats().type_environment_misses, 0);

    coordinator.set_file(&path, "local function after() end\n");
    let symbols = coordinator.document_symbols(&path);
    assert!(symbols.iter().any(|symbol| symbol.name == "after"));
    assert!(!symbols.iter().any(|symbol| symbol.name == "before"));
    assert_eq!(coordinator.cache_stats().parse_misses, 1);
}

#[test]
fn worker_snapshot_reuse_checks_source_identity_for_divergent_edits() {
    let mut coordinator = AnalysisHost::new();
    let path = PathBuf::from("main.tua");
    coordinator.set_file(&path, "local function original() end\n");
    let mut worker = coordinator.clone();
    worker.set_file(&path, "local function worker_edit() end\n");
    worker.snapshot();
    coordinator.set_file(&path, "local function current_edit() end\n");
    let symbols = coordinator.document_symbols(&path);
    assert!(symbols.iter().any(|symbol| symbol.name == "current_edit"));
    assert!(!symbols.iter().any(|symbol| symbol.name == "worker_edit"));
}

#[test]
fn edits_adopt_worker_facts_and_invalidate_only_affected_dependencies() {
    let project = TempProject::new("worker-snapshot-handoff");
    let producer = project.write("src/producer.tua", "return {value = 1}\n");
    let consumer = project.write(
        "src/consumer.tua",
        "local api = require('producer')\nlocal value: number = api.value\n",
    );
    let unrelated = project.write("src/unrelated.tua", "local independent = 1\n");
    let mut coordinator = AnalysisHost::new();
    coordinator.set_source_roots([project.path().join("src")]);
    coordinator.set_file(&producer, "return {value = 1}\n");
    coordinator.set_file(
        &consumer,
        "local api = require('producer')\nlocal value: number = api.value\n",
    );
    coordinator.set_file(&unrelated, "local independent = 1\n");
    coordinator.clone().snapshot();
    assert_eq!(coordinator.cache_stats().parse_misses, 0);
    coordinator.open_document(&producer, "return {value = 'changed'}\n");
    let snapshot = coordinator.snapshot();
    assert!(snapshot
        .diagnostics(&consumer)
        .iter()
        .any(|d| d.code == Some(DiagnosticCode::TYPE_MISMATCH)));
    let stats = coordinator.cache_stats();
    assert_eq!(
        stats.parse_misses, 2,
        "producer and affected consumer only: {stats:?}"
    );
    assert_eq!(stats.dependent_invalidations, 1);
    coordinator.close_document(&producer);
    assert!(!coordinator
        .snapshot()
        .diagnostics(&consumer)
        .iter()
        .any(|d| d.code == Some(DiagnosticCode::TYPE_MISMATCH)));
}

#[test]
fn unchanged_overlays_preserve_worker_analysis_and_track_disk_changes() {
    let mut host = AnalysisHost::new();
    let path = PathBuf::from("main.tua");
    let original = "local function original() end\n";
    host.set_file(&path, original);
    host.clone().snapshot();
    host.open_document(&path, original);
    host.snapshot();
    assert_eq!(host.cache_stats().parse_misses, 0);
    assert!(host.vfs.is_open(&path));
    host.close_document(&path);
    host.snapshot();
    assert_eq!(host.cache_stats().parse_misses, 0);
    assert!(!host.vfs.is_open(&path));

    host.open_document(&path, original);
    host.set_file(&path, "local function disk_changed() end\n");
    assert!(host
        .document_symbols(&path)
        .iter()
        .any(|s| s.name == "original"));
    host.close_document(&path);
    let symbols = host.document_symbols(&path);
    assert!(symbols.iter().any(|s| s.name == "disk_changed"));
    assert!(!symbols.iter().any(|s| s.name == "original"));
}

#[test]
fn snapshot_reuse_keeps_background_warm_results_adoptable() {
    let mut coordinator = AnalysisHost::new();
    let path = PathBuf::from("main.tua");
    coordinator.set_file(&path, "local function ready() end\n");
    coordinator.clone().snapshot();
    let worker = coordinator.clone();
    assert!(worker.warm_file(&path, AnalysisWarmPriority::Low));
    assert_eq!(worker.cache_stats().parse_misses, 0);
    assert!(coordinator.adopt_warmed_file(&worker, &path));
    assert!(coordinator
        .document_symbols(&path)
        .iter()
        .any(|s| s.name == "ready"));
    coordinator.set_file(&path, "local function changed() end\n");
    assert!(!coordinator.adopt_warmed_file(&worker, &path));
}

#[test]
fn recovered_file_checking_preserves_environment_and_body_errors() {
    for ecs_enabled in [false, true] {
        for (declaration, expected) in [
            (
                "local value: MissingType = 1",
                DiagnosticCode::TYPE_UNKNOWN_NAME,
            ),
            (
                "local value: number = 'wrong'",
                DiagnosticCode::TYPE_MISMATCH,
            ),
        ] {
            let mut config = ProjectConfig::default();
            config.ecs.enabled = ecs_enabled;
            let mut host = AnalysisHost::with_config(config);
            host.set_file(
                "partial.tua",
                format!("{declaration}\nlocal item = {{ value = 1 }}\nitem.\n"),
            );
            let snapshot = host.snapshot();
            let diagnostics = snapshot.diagnostics("partial.tua");
            assert!(
                diagnostics.iter().any(|d| d.code == Some(expected)),
                "{diagnostics:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.code == Some(DiagnosticCode::SYNTAX_PARSE)),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn completion_defers_checker_and_diagnostics_cache_follows_file_identity() {
    for partial in [false, true] {
        let suffix = if partial { "item.\n" } else { "" };
        let source =
            format!("local value: number = 'wrong'\nlocal item = {{ field = 1 }}\n{suffix}");
        let mut host = AnalysisHost::new();
        host.set_file("lazy.tua", source.clone());
        let snapshot = host.snapshot();
        let file = snapshot.file(Path::new("lazy.tua")).unwrap();
        assert!(file.checker_diagnostics.get().is_none());
        snapshot.completion("lazy.tua", SourcePosition { line: 2, column: 6 });
        assert!(
            file.checker_diagnostics.get().is_none(),
            "completion must not check function bodies"
        );
        let diagnostics = snapshot.diagnostics("lazy.tua");
        assert!(file.checker_diagnostics.get().is_some());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == Some(DiagnosticCode::TYPE_MISMATCH)),
            "{diagnostics:?}"
        );
        assert_eq!(diagnostics, snapshot.diagnostics("lazy.tua"));
        host.set_file("lazy.tua", source.replace("'wrong'", "1"));
        let changed = host.snapshot();
        assert!(changed
            .file(Path::new("lazy.tua"))
            .unwrap()
            .checker_diagnostics
            .get()
            .is_none());
        assert!(!changed
            .diagnostics("lazy.tua")
            .iter()
            .any(|d| d.code == Some(DiagnosticCode::TYPE_MISMATCH)));
        assert_eq!(
            diagnostics,
            snapshot.diagnostics("lazy.tua"),
            "old snapshots retain their own diagnostics"
        );
    }
}

#[test]
fn indexed_receiver_completion_uses_project_types_and_unsaved_edits() {
    let temp = TempProject::new("indexed-receiver");
    temp.write("tua.toml", "source = 'src'\n");
    temp.write(
        "src/items.tua",
        "export type Item = { speed: number }\nreturn {}\n",
    );
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
import type { Item } from "items"
local items: Item[] = {}
local index = 1
print(items[index].<|complete|>speed)
"#,
    );
    let file = &fixture.files()[0];
    let path = temp.path.join("src/main.tua");
    temp.write("src/main.tua", &file.source);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path.join("src")]);
    host.set_file(&path, file.source.clone());
    host.set_file(
        temp.path.join("src/items.tua"),
        "export type Item = { speed: number }\nreturn {}\n",
    );
    let snapshot = host.snapshot();
    assert!(
        completion_labels(&snapshot.completion(&path, fixture.position("complete")))
            .contains(&"speed".into())
    );
    let hover = snapshot
        .hover(&path, fixture.position("complete"))
        .expect("indexed member hover");
    assert!(hover.contents.contains("number"), "{hover:?}");
    let partial = file.source.replace(".speed)", ".)");
    host.open_document(&path, partial);
    assert!(completion_labels(
        &host
            .snapshot()
            .completion(&path, fixture.position("complete"))
    )
    .contains(&"speed".into()));
    host.open_document(
        temp.path.join("src/items.tua"),
        "export type Item = { health: number }\nreturn {}\n",
    );
    let labels = completion_labels(
        &host
            .snapshot()
            .completion(&path, fixture.position("complete")),
    );
    assert!(labels.contains(&"health".into()), "{labels:?}");
    assert!(!labels.contains(&"speed".into()), "{labels:?}");
}

#[test]
fn indexed_receiver_completion_handles_nested_indexes_and_dynamic_values() {
    for (source, expected) in [
        (
            "local items: { speed: number }[] = {}\nprint(items[1].<|complete|>)",
            Some("speed"),
        ),
        (
            "local items: { speed: number }[][] = {}\nprint(items[1][1].<|complete|>)",
            Some("speed"),
        ),
        ("local items: any = {}\nprint(items[1].<|complete|>)", None),
    ] {
        let fixture = Fixture::parse(&format!("-- file: test.tua\n{source}"));
        let mut host = AnalysisHost::new();
        host.set_file("test.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("test.tua", fixture.position("complete")),
        );
        match expected {
            Some(label) => assert!(labels.contains(&label.to_owned()), "{labels:?}"),
            None => assert!(labels.is_empty(), "{labels:?}"),
        }
    }
}

#[test]
fn chained_receivers_share_completion_hover_and_signature_types() {
    let fixture = Fixture::parse(
        r#"-- file: chain.tua
type Result = { value: number, run: (amount: number) -> number }
local function make(): Result
  return { value = 1, run = function(amount: number): number return amount end }
end
print(make().<|complete|>value)
print(make().<|hover|>value)
print(make():run(<|signature|>1))
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("chain.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    let labels = completion_labels(&snapshot.completion("chain.tua", fixture.position("complete")));
    assert!(
        labels.contains(&"value".into()) && labels.contains(&"run".into()),
        "{labels:?}; {:?}",
        snapshot.pipeline_diagnostics("chain.tua")
    );
    let hover = snapshot
        .hover("chain.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("number"), "{hover:?}");
    let signature = snapshot
        .signature_help("chain.tua", fixture.position("signature"))
        .unwrap();
    assert!(
        signature.signatures[0].label.contains("amount: number"),
        "{signature:?}"
    );
}

#[test]
fn chained_love_receivers_keep_signatures_and_do_not_invent_void_results() {
    for (call, expected) in [
        (
            "love.graphics.newText(love.graphics.newFont(12), 'hi'):",
            Some("getWidth"),
        ),
        (
            "love.graphics.newText(love.graphics.newFont(12), 'hi'):set('bye'):",
            None,
        ),
    ] {
        let fixture = Fixture::parse(&format!("-- file: love.tua\n{call}<|complete|>\n"));
        let mut host = AnalysisHost::new();
        host.set_file("love.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("love.tua", fixture.position("complete")),
        );
        match expected {
            Some(label) => assert!(labels.contains(&label.to_owned()), "{labels:?}"),
            None => assert!(labels.is_empty(), "{labels:?}"),
        }
    }
    let fixture = Fixture::parse(
        "-- file: love.tua\nlove.graphics.newText(love.graphics.newFont(12), 'hi'):getWidth(<|signature|>\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("love.tua", fixture.files()[0].source.clone());
    let signature = host
        .snapshot()
        .signature_help("love.tua", fixture.position("signature"))
        .unwrap();
    assert!(
        signature
            .signatures
            .iter()
            .any(|s| s.label.contains("getWidth")),
        "{signature:?}"
    );
}

#[test]
fn chained_imported_factory_completion_tracks_unsaved_return_types() {
    let temp = TempProject::new("chained-factory");
    temp.write("tua.toml", "source = 'src'\n");
    let producer = "export type Item = { speed: number }\nlocal function make(): Item\n return { speed = 1 }\nend\nreturn { make = make }\n";
    temp.write("src/items.tua", producer);
    let fixture = Fixture::parse(
        "-- file: src/main.tua\nlocal items = require('items')\nprint(items.make().<|complete|>)\n",
    );
    let path = temp.path.join("src/main.tua");
    temp.write("src/main.tua", &fixture.files()[0].source);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path.join("src")]);
    host.set_file(&path, fixture.files()[0].source.clone());
    host.set_file(temp.path.join("src/items.tua"), producer);
    let old = host.snapshot();
    assert!(
        completion_labels(&old.completion(&path, fixture.position("complete")))
            .contains(&"speed".into())
    );
    host.set_file(
        temp.path.join("src/items.tua"),
        producer.replace("speed", "health"),
    );
    let labels = completion_labels(
        &host
            .snapshot()
            .completion(&path, fixture.position("complete")),
    );
    assert!(
        labels.contains(&"health".into()) && !labels.contains(&"speed".into()),
        "{labels:?}"
    );
    assert!(
        completion_labels(&old.completion(&path, fixture.position("complete")))
            .contains(&"speed".into())
    );
}

#[test]
fn recursive_imported_self_preserves_members_at_arbitrary_access_depth() {
    let temp = TempProject::new("recursive-imported-self");
    temp.write("tua.toml", "source = 'src'\n");
    let producer = "final class Unit {\n speed: number = 1\n followers: Self[] = {}\n same(): Self { return self }\n}\nexport type UnitInstance = Unit\nreturn { Unit = Unit }\n";
    temp.write("src/unit.tua", producer);
    for import in [
        "import type { UnitInstance } from 'unit'",
        "import type Types from 'unit'\ntype UnitInstance = Types.UnitInstance",
    ] {
        let fixture = Fixture::parse(&format!(
            "-- file: src/main.tua\n{import}\nlocal function inspect(unit: UnitInstance)\n print(unit.followers[1].followers[1].<|complete|>speed)\n print(unit:same():same().<|hover|>speed)\nend\n"
        ));
        let path = temp.path.join("src/main.tua");
        temp.write("src/main.tua", &fixture.files()[0].source);
        let mut host = AnalysisHost::new();
        host.set_source_roots([temp.path.join("src")]);
        host.set_file(&path, fixture.files()[0].source.clone());
        host.set_file(temp.path.join("src/unit.tua"), producer);
        let snapshot = host.snapshot();
        let labels = completion_labels(&snapshot.completion(&path, fixture.position("complete")));
        assert!(labels.contains(&"speed".into()), "{labels:?}");
        let hover = snapshot.hover(&path, fixture.position("hover")).unwrap();
        assert!(hover.contents.contains("number"), "{hover:?}");
        host.set_file(
            temp.path.join("src/unit.tua"),
            producer.replace("speed", "health"),
        );
        let labels = completion_labels(
            &host
                .snapshot()
                .completion(&path, fixture.position("complete")),
        );
        assert!(
            labels.contains(&"health".into()) && !labels.contains(&"speed".into()),
            "{labels:?}"
        );
    }
}

#[test]
fn cross_file_event_payload_uses_imported_method_return_and_unsaved_edits() {
    let temp = TempProject::new("event-method-result");
    temp.write("tua.toml", "source = 'src'\n");
    let producer = "export type Actor = { damage: (amount: number) -> boolean }\nreturn {}\n";
    temp.write("src/actor.tua", producer);
    temp.write("src/events.tua", "return { combat = signal() }\n");
    temp.write("src/world.tua", "import type { Actor } from 'actor'\nlocal events = require('events')\nlocal actors: Actor[] = {}\nlocal function hit(amount: number)\n local target = actors[1]\n local dead = target:damage(amount)\n events.combat:emit('hit', dead)\nend\nreturn { hit = hit }\n");
    let fixture = Fixture::parse(
        "-- file: src/main.tua\nlocal events = require('events')\nevents.combat:on('hit', function(dead)\n print(<|hover|>dead)\nend)\n",
    );
    let path = temp.path.join("src/main.tua");
    temp.write("src/main.tua", &fixture.files()[0].source);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path.join("src")]);
    for file in ["actor.tua", "events.tua", "world.tua", "main.tua"] {
        let path = temp.path.join("src").join(file);
        host.set_file(&path, fs::read_to_string(&path).unwrap());
    }
    let old = host.snapshot();
    let hover = old.hover(&path, fixture.position("hover")).unwrap();
    assert!(hover.contents.contains("boolean"), "{hover:?}");
    host.set_file(
        temp.path.join("src/actor.tua"),
        producer.replace("boolean", "string"),
    );
    let hover = host
        .snapshot()
        .hover(&path, fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("string"), "{hover:?}");
    assert!(old
        .hover(&path, fixture.position("hover"))
        .unwrap()
        .contents
        .contains("boolean"));
}

#[test]
fn class_self_parameters_and_guarded_captures_share_source_coordinates() {
    let fixture = Fixture::parse(
        r#"-- file: nodes.tua
final class Node {
 enabled: boolean = true
 parent: Self | nil
 inspect(other: Self) {
   print(other.<|parameter|>enabled)
   local parent = self.parent
   if parent then
     print(parent.<|guard|>enabled)
   end
 }
 later() {
   local parent = self.parent
   if not parent then return end
   local callback = function()
     print(parent.<|capture|>enabled)
   end
   callback()
 }
}
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("nodes.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    for marker in ["parameter", "guard", "capture"] {
        let labels = completion_labels(&snapshot.completion("nodes.tua", fixture.position(marker)));
        assert!(labels.contains(&"enabled".into()), "{marker}: {labels:?}");
        let hover = snapshot
            .hover("nodes.tua", fixture.position(marker))
            .unwrap();
        assert!(hover.contents.contains("boolean"), "{marker}: {hover:?}");
    }
}

#[test]
fn guarded_capture_keeps_non_nil_after_member_writes_but_not_reassignment() {
    for (write, expected) in [("item.enabled = true", true), ("item = nil", false)] {
        let fixture = Fixture::parse(&format!(
            "-- file: capture.tua\ntype Item = {{ enabled: boolean }}\nlocal function later(item: Item | nil)\n if not item or item.enabled then return end\n {write}\n local callback = function() print(item.<|complete|>enabled) end\nend\n"
        ));
        let mut host = AnalysisHost::new();
        host.set_file("capture.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("capture.tua", fixture.position("complete")),
        );
        assert_eq!(
            labels.contains(&"enabled".into()),
            expected,
            "{write}: {labels:?}"
        );
    }
}

#[test]
fn native_class_parameter_navigation_and_rename_preserve_original_spans() {
    for header in [
        "<|declaration|>amount: number = 2, label: string = 'x'",
        "\n    <|declaration|>amount: number = 2,\n    label: string = 'x'\n  ",
    ] {
        let fixture = Fixture::parse(&format!(
            "-- file: counter.tua\nfinal class Counter {{\n apply({header}): number {{\n   local function nested(amount: number): number return amount end\n   print(<|use|>amount, label)\n   return amount\n }}\n}}\n"
        ));
        let source = &fixture.files()[0].source;
        let mut host = AnalysisHost::new();
        host.set_file("counter.tua", source.clone());
        let snapshot = host.snapshot();
        let definition = snapshot
            .definition("counter.tua", fixture.position("use"))
            .expect("parameter definition");
        assert_eq!(definition.kind, DefinitionKind::Parameter);
        assert_eq!(definition.span.start, fixture.position("declaration"));
        let references = snapshot.references("counter.tua", fixture.position("use"), true);
        assert_eq!(references.len(), 3, "{references:?}");
        let rename = snapshot
            .rename("counter.tua", fixture.position("declaration"), "count")
            .expect("parameter rename");
        assert_eq!(rename.edits.len(), 3, "{rename:?}");
        for edit in rename.edits {
            let start = crate::source_text::byte_for_position(source, edit.span.start).unwrap();
            let end = crate::source_text::byte_for_position(source, edit.span.end).unwrap();
            assert_eq!(&source[start..end], "amount");
            assert_ne!(
                edit.span.start.line,
                fixture.position("use").line - 1,
                "nested shadow must remain unchanged"
            );
        }
    }
}

#[test]
fn side_effect_global_definition_follows_runtime_imports_without_guessing() {
    let temp = TempProject::new("side-effect-navigation");
    temp.write("tua.toml", "source = 'src'\n");
    temp.write("src/globals.tua", "function loadAssets() return true end\n");
    temp.write("src/bridge.tua", "require('globals')\nrequire('main')\n");
    temp.write(
        "src/unrelated.tua",
        "function loadAssets() return false end\n",
    );
    let fixture = Fixture::parse(
        "-- file: src/main.tua\nrequire('bridge')\n<|call|>loadAssets()\nlocal function shadow(loadAssets) <|shadow|>loadAssets() end\n",
    );
    let path = temp.path.join("src/main.tua");
    temp.write("src/main.tua", &fixture.files()[0].source);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path.join("src")]);
    for file in ["globals.tua", "bridge.tua", "unrelated.tua", "main.tua"] {
        let path = temp.path.join("src").join(file);
        host.set_file(&path, fs::read_to_string(&path).unwrap());
    }
    let old = host.snapshot();
    let definition = old.definition(&path, fixture.position("call")).unwrap();
    assert_eq!(definition.path, temp.path.join("src/globals.tua"));
    assert_eq!(
        old.definition(&path, fixture.position("shadow"))
            .unwrap()
            .kind,
        DefinitionKind::Parameter
    );
    host.set_file(
        temp.path.join("src/bridge.tua"),
        "require('globals')\nrequire('unrelated')\n",
    );
    assert!(host
        .snapshot()
        .definition(&path, fixture.position("call"))
        .is_none());
    assert_eq!(
        old.definition(&path, fixture.position("call")).unwrap(),
        definition
    );
}

#[test]
fn fluent_class_and_love_navigation_use_resolved_receivers() {
    let fixture = Fixture::parse(
        r#"-- file: fluent.tua
final class Label {
 value: string = ""
 setValue(value: string): Self { self.value = value; return self }
 <|declaration|>draw() { print(self.value) }
}
Label.new():setValue("hello"):<|call|>draw()
local function wrapper() return { label = Label.new() } end
wrapper().label:<|nested|>draw()
love.graphics.newText(love.graphics.newFont(12), "hi"):<|love|>getWidth()
love.graphics.newText(love.graphics.newFont(12), "hi"):set("bye"):<|void|>getWidth()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("fluent.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("fluent.tua", fixture.position("call"))
        .unwrap();
    assert_eq!(definition.span.start, fixture.position("declaration"));
    assert_eq!(
        snapshot.definition("fluent.tua", fixture.position("nested")),
        Some(definition.clone())
    );
    let references = snapshot.references("fluent.tua", fixture.position("call"), true);
    assert!(
        references
            .iter()
            .any(|reference| reference.span.start == fixture.position("call")),
        "{references:?}"
    );
    let rename = snapshot
        .rename("fluent.tua", fixture.position("call"), "render")
        .unwrap();
    assert!(
        rename
            .edits
            .iter()
            .any(|edit| edit.span.start == fixture.position("call")),
        "{rename:?}"
    );
    assert_eq!(
        snapshot
            .definition("fluent.tua", fixture.position("love"))
            .unwrap()
            .kind,
        DefinitionKind::Builtin
    );
    assert!(snapshot
        .definition("fluent.tua", fixture.position("void"))
        .is_none());
}

#[test]
fn anonymous_default_signature_and_body_hover_agree() {
    let fixture = Fixture::parse(
        "-- file: callbacks.tua\nlocal f = function(value: number = 3): number\n return <|hover|>value\nend\nf(<|signature|>)\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("callbacks.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("callbacks.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("number"), "{hover:?}");
    let signature = snapshot
        .signature_help("callbacks.tua", fixture.position("signature"))
        .unwrap();
    assert!(
        signature.signatures[0].label.contains("value?: number"),
        "{signature:?}"
    );
}

#[test]
fn assertion_narrowing_agrees_in_completion_hover_and_return_inference() {
    let fixture = Fixture::parse(
        "-- file: assertions.tua\ntype Item = { value: number }\nlocal function need(item: Item | nil)\n assert(item)\n print(item.<|complete|>value)\n return item\nend\nlocal <|hover|>result = need(nil)\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("assertions.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    assert!(completion_labels(
        &snapshot.completion("assertions.tua", fixture.position("complete"))
    )
    .contains(&"value".into()));
    let hover = snapshot
        .hover("assertions.tua", fixture.position("hover"))
        .unwrap();
    assert!(
        !hover.contents.contains("nil") && !hover.contents.contains("any"),
        "{hover:?}"
    );
}

#[test]
fn assertion_completion_does_not_narrow_shadowed_calls() {
    for declaration in [
        "local assert = function(value) return true end",
        "assert = function(value) return true end",
        "_G.assert = function(value) return true end",
    ] {
        let fixture = Fixture::parse(&format!(
            "-- file: assertions.tua\n{declaration}\nlocal function need(item: {{ value: number }} | nil)\n assert(item)\n print(item.<|complete|>value)\nend\n"
        ));
        let mut host = AnalysisHost::new();
        host.set_file("assertions.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("assertions.tua", fixture.position("complete")),
        );
        assert!(
            !labels.contains(&"value".into()),
            "{declaration}: {labels:?}"
        );
    }
}

#[test]
fn stable_member_guards_agree_across_queries() {
    let fixture = Fixture::parse(
        "-- file: paths.tua\ntype State = { target: { value: number } | nil }\nlocal function read(state: State)\n if state.target then\n  local value = state.target.<|complete|>value\n  return state.target\n end\n return { value = 0 }\nend\nlocal <|hover|>result = read({target = nil})\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("paths.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    assert!(
        completion_labels(&snapshot.completion("paths.tua", fixture.position("complete")))
            .contains(&"value".into())
    );
    let hover = snapshot
        .hover("paths.tua", fixture.position("hover"))
        .unwrap();
    assert!(
        !hover.contents.contains("nil") && !hover.contents.contains("any"),
        "{hover:?}"
    );
}

#[test]
fn table_sort_comparator_editor_context_matches_array_items() {
    let fixture = Fixture::parse(
        "-- file: sort.tua\ntype Item = { score: number }\nlocal items: Item[] = {}\ntable.sort(items, function(a, b)\n return a.<|complete|>score < <|hover|>b.score\nend)\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("sort.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    assert!(
        completion_labels(&snapshot.completion("sort.tua", fixture.position("complete")))
            .contains(&"score".into())
    );
    let hover = snapshot
        .hover("sort.tua", fixture.position("hover"))
        .unwrap();
    assert!(!hover.contents.contains("any"), "{hover:?}");
}

#[test]
fn table_sort_context_respects_library_and_nested_parameter_shadows() {
    for source in [
        "local table = { sort = function(list, callback) end }\nlocal items: { score: number }[] = {}\ntable.sort(items, function(a,b) return a.<|complete|>score end)",
        "table.sort = function(list, callback) end\nlocal items: { score: number }[] = {}\ntable.sort(items, function(a,b) return a.<|complete|>score end)",
        "local items: { score: number }[] = {}\ntable.sort(items, function(a,b) local f = function(a) return a.<|complete|>score end return true end)",
    ] {
        let fixture = Fixture::parse(&format!("-- file: sort.tua\n{source}\n"));
        let mut host = AnalysisHost::new();
        host.set_file("sort.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("sort.tua", fixture.position("complete")),
        );
        assert!(!labels.contains(&"score".into()), "{source}: {labels:?}");
    }
}

#[test]
fn love_image_file_signature_and_result_completion_agree() {
    let fixture = Fixture::parse(
        "-- file: files.tua\nlocal function load(file: File)\n local data = love.image.newImageData(<|signature|>file)\n return data:<|complete|>getWidth()\nend\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("files.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    let signature = snapshot
        .signature_help("files.tua", fixture.position("signature"))
        .unwrap();
    assert!(
        signature
            .signatures
            .iter()
            .any(|item| item.label.contains("File | Data")),
        "{signature:?}"
    );
    assert!(
        completion_labels(&snapshot.completion("files.tua", fixture.position("complete")))
            .contains(&"getWidth".into())
    );
}

#[test]
fn asset_metadata_and_completion_respect_loader_bindings() {
    let temp = TempProject::new("asset-binding");
    let image = temp.write("assets/player.png", png_header(16, 16));
    for source in [
        "local image = function(path) return path end\nlocal a = image 'assets/<|complete|>player.png'",
        "local function load(image) return image 'assets/<|complete|>player.png' end",
        "image = function(path) return path end\nlocal a = image 'assets/<|complete|>player.png'",
        "local love = { graphics = { newImage = function(path) return path end } }\nlocal a = love.graphics.newImage('assets/<|complete|>player.png')",
    ] {
        let fixture = Fixture::parse(&format!("-- file: src/main.tua\n{source}\n"));
        let mut host = AnalysisHost::new();
        host.set_asset_roots([temp.path().to_path_buf()]);
        host.set_file("src/main.tua", fixture.files()[0].source.clone());
        host.refresh_asset_index_from_disk();
        let snapshot = host.snapshot();
        assert!(
            snapshot
                .hover("src/main.tua", fixture.position("complete"))
                .is_none_or(|hover| !hover.contents.contains("asset `")),
            "{source}"
        );
        assert!(
            snapshot
                .definition("src/main.tua", fixture.position("complete"))
                .is_none_or(|definition| definition.kind != DefinitionKind::Asset),
            "{source}"
        );
        assert!(
            !completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")))
                .contains(&"assets/player.png".into()),
            "{source}"
        );
        assert!(
            snapshot
                .asset_inventory()
                .assets
                .iter()
                .all(|asset| asset.references.is_empty()),
            "{source}"
        );
        assert!(
            snapshot
                .asset_file_rename_edits(&image, temp.path().join("assets/hero.png"))
                .is_empty(),
            "{source}"
        );
    }
}

#[test]
fn asset_paths_survive_incomplete_strings_and_scope_exit() {
    let temp = TempProject::new("asset-recovery-binding");
    temp.write("assets/player.png", png_header(16, 16));
    for source in [
        "local value = image 'assets/<|complete|>",
        "do local image = function(path) return path end end\nlocal value = image 'assets/<|complete|>player.png'",
    ] {
        let fixture = Fixture::parse(&format!("-- file: src/main.tua\n{source}\n"));
        let mut host = AnalysisHost::new();
        host.set_asset_roots([temp.path().to_path_buf()]);
        host.set_file("src/main.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("src/main.tua", fixture.position("complete")),
        );
        assert!(
            labels.contains(&"assets/player.png".into()),
            "{source}: {labels:?}"
        );
    }
}

#[test]
fn love_conf_window_completion_tracks_initial_state_and_disable() {
    for (statement, expected) in [
        ("t.window.width = 320", true),
        ("t.window = nil", false),
        ("t.window = false", false),
        ("mutate(t)", false),
    ] {
        let fixture = Fixture::parse(&format!(
            "-- file: conf.tua\nfunction love.conf(t)\n {statement}\n local width = t.window.<|complete|>width\nend\n"
        ));
        let mut host = AnalysisHost::new();
        host.set_file("conf.tua", fixture.files()[0].source.clone());
        let labels = completion_labels(
            &host
                .snapshot()
                .completion("conf.tua", fixture.position("complete")),
        );
        assert_eq!(
            labels.contains(&"width".into()),
            expected,
            "{statement}: {labels:?}"
        );
    }
}

#[test]
fn bundled_hump_timer_declarations_support_typed_migration() {
    let fixture = Fixture::parse(
        "-- file: src/main.tua\nlocal Timer = require('libraries.hump.timer')\nlocal timer = Timer.new()\ntimer:<|complete|>after(0.5, function() end)\ntimer:during(1, function(dt, remaining)\n local elapsed = <|hover|>dt\nend)\n",
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    host.set_type_libraries([ProjectTypeLibrary::new(
        "types/hump",
        "hump",
        "libraries.hump",
    )]);
    host.set_file_with_kind(
        "types/hump/timer.lua",
        include_str!("../../../../examples/type-libraries/hump/timer.lua"),
        AnalysisSourceKind::Lua,
    );
    host.set_file("src/main.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert!(
        labels.contains(&"after".into()) && labels.contains(&"cancel".into()),
        "{labels:?}"
    );
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .unwrap();
    assert!(hover.contents.contains("number"), "{hover:?}");
}

#[test]
fn nested_default_reads_share_lexical_navigation_and_rename() {
    let fixture = Fixture::parse(
        "-- file: defaults.tua\nlocal <|declaration|>seed = 4\nlocal f = function(callback = function(value: number = <|use|>seed) return value end) return callback() end\nprint(f())\n",
    );
    let source = &fixture.files()[0].source;
    let mut host = AnalysisHost::new();
    host.set_file("defaults.tua", source.clone());
    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("defaults.tua", fixture.position("use"))
        .unwrap();
    assert_eq!(definition.span.start, fixture.position("declaration"));
    let rename = snapshot
        .rename("defaults.tua", fixture.position("declaration"), "base")
        .unwrap();
    assert_eq!(rename.edits.len(), 2, "{rename:?}");
    for edit in rename.edits {
        let start = crate::source_text::byte_for_position(source, edit.span.start).unwrap();
        let end = crate::source_text::byte_for_position(source, edit.span.end).unwrap();
        assert_eq!(&source[start..end], "seed");
    }
}

#[test]
fn unclosed_native_classes_recover_members_without_erasing_outer_scopes() {
    for closing in ["", " }"] {
        let fixture = Fixture::parse(&format!(
            "-- file: draft.tua\nlocal label = 'ready'\nfinal class Draft {{\n count: number = 0\n show() {{\n  if true then print(<|hover|>label) end\n  self.<|complete|>\n{closing}\n"
        ));
        let mut host = AnalysisHost::new();
        host.set_file("draft.tua", fixture.files()[0].source.clone());
        let snapshot = host.snapshot();
        let labels =
            completion_labels(&snapshot.completion("draft.tua", fixture.position("complete")));
        assert!(
            labels.contains(&"count".into()) && labels.contains(&"show".into()),
            "{closing}: {labels:?}"
        );
        let hover = snapshot
            .hover("draft.tua", fixture.position("hover"))
            .unwrap();
        assert!(
            hover.contents.contains("string") || hover.contents.contains("ready"),
            "{hover:?}"
        );
        assert!(snapshot
            .diagnostics("draft.tua")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unclosed class `Draft`")));
    }
    let literal = "local text = [=[final class Draft {]=] -- final class Other {\n";
    assert!(crate::classes::source_with_closed_class_bodies(literal).is_none());
}

#[test]
fn unclosed_protocols_keep_current_buffer_semantics_available() {
    let fixture = Fixture::parse(
        "-- file: draft.tua\nlocal label = 'ready'\nlocal function draw(value: Display): nil\n value:<|complete|>\nend\nprotocol Display {\n render(): string\n r\n",
    );
    let mut host = AnalysisHost::new();
    host.set_file("draft.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();

    let labels = completion_labels(&snapshot.completion("draft.tua", fixture.position("complete")));
    assert!(
        labels.contains(&"render".into()),
        "the partial protocol contract should serve current-file completion: {labels:?}"
    );
    let hover = snapshot
        .hover("draft.tua", SourcePosition { line: 1, column: 8 })
        .expect("an earlier local should still resolve in the recovered current buffer");
    assert!(hover.contents.contains("string"), "{hover:?}");
    assert!(snapshot
        .diagnostics("draft.tua")
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unclosed protocol `Display`")));
}

#[test]
fn incomplete_generic_class_completion_uses_current_and_last_good_facts() {
    let fixture = Fixture::parse(
        r#"-- file: draft.tua
protocol Renderable {
 render(): string
}
final class Draft<T: Renderable> implements Renderable {
 render(): string { return "draft" }
 inspect(value: T) {
  value:<|complete|>
 }
"#,
    );
    let mut host = AnalysisHost::new();
    let mut fresh_host = AnalysisHost::new();
    fresh_host.set_file("draft.tua", fixture.files()[0].source.clone());
    let fresh_snapshot = fresh_host.snapshot();
    let fresh_labels =
        completion_labels(&fresh_snapshot.completion("draft.tua", fixture.position("complete")));
    assert!(
        fresh_labels.contains(&"render".into()),
        "new generic class bodies should expose protocol-bound members before a valid snapshot exists: {fresh_labels:?}"
    );

    let valid = fixture.files()[0]
        .source
        .replace("  value:\n", "  value:render()\n")
        + "}\n";
    host.set_file("draft.tua", valid);
    let valid_snapshot = host.snapshot();
    assert!(
        valid_snapshot.semantic_file("draft.tua").is_some(),
        "valid setup source should parse: {:#?}",
        valid_snapshot.parse("draft.tua")
    );
    host.set_file("draft.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();

    let labels = completion_labels(&snapshot.completion("draft.tua", fixture.position("complete")));
    let last_good = snapshot
        .last_good_semantic_facts(Path::new("draft.tua"))
        .expect("the valid generic class snapshot should be retained");
    let cached_last_good = snapshot
        .last_good_semantic_facts(Path::new("draft.tua"))
        .expect("last-good facts should remain available for later editor requests");
    assert!(std::sync::Arc::ptr_eq(&last_good, &cached_last_good));
    assert!(
        labels.contains(&"render".into()),
        "a partially typed member in an edited generic class should use last-good protocol-bound facts: {labels:?}; diagnostics: {:#?}",
        snapshot.diagnostics("draft.tua")
    );
    assert!(snapshot
        .diagnostics("draft.tua")
        .iter()
        .any(|diagnostic| { diagnostic.message.contains("unclosed class `Draft`") }));
}

#[test]
fn cold_incomplete_imported_generic_receiver_keeps_member_completion() {
    let fixture = Fixture::parse(
        r#"-- file: src/model.tua
final class Jar<T> {
  item: T
  init(item: T) { self.item = item }
  get(): T { return self.item }
  label(): string { return "jar" }
}
export type JarInstance<T> = Jar<T>
return { Jar = Jar }
-- file: src/recipes.tua
import type { JarInstance } from "./model"
local Model = require("model")
local function seal(value: number): JarInstance<number>
  return Model.Jar.new(value)
end
return { seal = seal }
-- file: src/main.tua
local Recipes = require("recipes")
local potion: number | nil = 1
if potion then
  local jar = Recipes.seal(potion)
  jar:<|complete|>
end
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    assert!(snapshot
        .last_good_semantic_facts(Path::new("src/main.tua"))
        .is_none());
    let labels =
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")));
    assert!(labels.contains(&"get".to_owned()), "{labels:?}");
    assert!(labels.contains(&"label".to_owned()), "{labels:?}");
    assert!(snapshot
        .diagnostics("src/main.tua")
        .iter()
        .any(|diagnostic| { diagnostic.code == Some(DiagnosticCode::SYNTAX_PARSE) }));
}

#[test]
fn workspace_declaration_overlays_keep_project_ownership() {
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("first/src"), PathBuf::from("second/src")]);
    host.set_module_path_workspaces([
        ProjectModuleWorkspace::new("first", [PathBuf::from("first/src")])
            .with_type_libraries([ProjectTypeLibrary::new("first/types", "api", "engine")]),
        ProjectModuleWorkspace::new("third", [PathBuf::from("third/src")]),
        ProjectModuleWorkspace::new("second", [PathBuf::from("second/src")]).with_type_libraries([
            ProjectTypeLibrary::new("shared/second-types", "api", "engine"),
        ]),
    ]);
    host.set_type_libraries([
        ProjectTypeLibrary::new("first/types", "api", "engine"),
        ProjectTypeLibrary::new("shared/second-types", "api", "engine"),
    ]);
    host.set_file_with_kind(
        "first/types/api.lua",
        "---@meta api\nreturn {value = 1}\n",
        AnalysisSourceKind::Lua,
    );
    host.set_file_with_kind(
        "shared/second-types/api.lua",
        "---@meta api\nreturn {value = 'second'}\n",
        AnalysisSourceKind::Lua,
    );
    host.set_file(
        "first/src/main.tua",
        "local api = require('engine')\nlocal value: number = api.value\n",
    );
    host.set_file(
        "second/src/main.tua",
        "local api = require('engine')\nlocal value: string = api.value\n",
    );
    host.set_file(
        "third/src/main.tua",
        "local api = require('engine')\nreturn api\n",
    );
    let snapshot = host.snapshot();
    assert!(matches!(
        snapshot
            .module_graph()
            .resolve_require("third/src/main.tua", "engine"),
        Some(RequireResolution::Missing { .. })
    ));
    for (project, declaration) in [
        ("first", "first/types/api.lua"),
        ("second", "shared/second-types/api.lua"),
    ] {
        assert_eq!(
            snapshot
                .module_graph()
                .resolve_require(format!("{project}/src/main.tua"), "engine"),
            Some(&RequireResolution::Resolved {
                path: PathBuf::from(declaration)
            })
        );
        assert!(snapshot
            .diagnostics(format!("{project}/src/main.tua"))
            .iter()
            .all(|diagnostic| diagnostic.code != Some(DiagnosticCode::TYPE_MISMATCH)));
    }
}

#[test]
fn directly_returned_signal_buses_preserve_navigation_through_reexports() {
    let fixture = Fixture::parse(
        r#"-- file: src/events.tua
local bus = signal()
bus:declare("<|declaration|>score", function(value: number) end)
return bus
-- file: src/relay.tua
local bus = require("events")
return bus
-- file: src/main.tua
local bus = require("relay")
bus:emit("<|usage|>score", 1)
local unrelated = signal()
unrelated:emit("score", "other")
"#,
    );
    let mut host = AnalysisHost::new();
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }
    let snapshot = host.snapshot();
    let definition = snapshot
        .definition("src/main.tua", fixture.position("usage"))
        .unwrap();
    assert_eq!(definition.path, PathBuf::from("src/events.tua"));
    assert_eq!(definition.span.start, fixture.position("declaration"));
    let references = snapshot.references("src/main.tua", fixture.position("usage"), true);
    assert_eq!(references.len(), 2, "{references:?}");
}

#[test]
fn inline_require_field_extraction_preserves_exported_types() {
    let fixture = Fixture::parse(
        r#"-- file: src/provider.tua
return {inc = function(x: number): number return x + 1 end, game = store({score=1})}
-- file: src/main.tua
local inc = require("provider").inc
local n = inc(2)
local state = require("provider").game:get()
print(<|number|>n, <|state|>state)
"#,
    );
    let mut host = AnalysisHost::new();
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }
    let snapshot = host.snapshot();
    let number = snapshot
        .hover("src/main.tua", fixture.position("number"))
        .unwrap();
    assert!(number.contents.contains("number"), "{}", number.contents);
    assert!(snapshot
        .hover("src/main.tua", fixture.position("state"))
        .unwrap()
        .contents
        .contains("score: number"));
}

#[test]
fn concrete_generic_parent_substitution_reaches_completion_and_hover() {
    let fixture = Fixture::parse(
        r#"-- file: src/main.tua
open class Base<T> {
  value: T

  init(value: T) { self.value = value }
  get(): T { return self.value }
}

final class NumericBox extends Base<number> {
  init(value: number) { super(value) }
}

local box: NumericBox = NumericBox.new(42)
local value: number = box.<|complete|>value
local result = box:<|hover|>get()
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_file("src/main.tua", fixture.files()[0].source.clone());
    let snapshot = host.snapshot();

    assert!(
        completion_labels(&snapshot.completion("src/main.tua", fixture.position("complete")))
            .contains(&"value".into()),
        "inherited fields should complete on a specialized generic subclass"
    );
    let hover = snapshot
        .hover("src/main.tua", fixture.position("hover"))
        .expect("the inherited generic method call should have hover");
    assert!(
        hover.contents.contains("number") && !hover.contents.contains("T"),
        "inherited method hover should show the applied parent argument: {hover:?}"
    );
}

#[test]
fn imported_ecs_component_retains_its_payload_schema() {
    let fixture = Fixture::parse(
        r#"-- file: src/components.tua
local Position = ecs.component("Position", {x="number"})
return {Position=Position}
-- file: src/main.tua
local C = require("components")
local world = ecs.world()
local entity = world:create()
world:add(entity, C.Position, {x=1})
local p = world:get(entity, C.Position)
print(<|payload|>p)
"#,
    );
    let mut host = AnalysisHost::new();
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("payload"))
        .unwrap();
    assert!(hover.contents.contains("x: number"), "{}", hover.contents);
    assert!(hover.contents.contains("nil"), "{}", hover.contents);
}

#[test]
fn absolute_project_imported_ecs_component_retains_payload_schema() {
    let temp = TempProject::new("absolute-imported-ecs-component");
    let provider = temp.write(
        "src/components.tua",
        "local Position = ecs.component(\"Position\", { x = \"number\" })\nreturn { Position = Position }\n",
    );
    let source = "local C = require \"components\"\nlocal world = ecs.world()\nlocal entity = world:create()\nlocal position = world:get(entity, C.Position)\nprint(position)\nlocal unrelated: any = nil\n";
    let consumer = temp.write("src/main.tua", source);
    let mut host = AnalysisHost::new();
    host.set_source_roots([temp.path().join("src")]);
    host.set_file(&provider, fs::read_to_string(&provider).unwrap());
    host.set_file(&consumer, source);

    let snapshot = host.snapshot();
    let parsed = snapshot.file(&consumer).unwrap().parsed.as_ref().unwrap();
    assert!(snapshot
        .project_type_for_field_chain(parsed, &["C".into(), "Position".into()])
        .is_some());
    let hover = snapshot
        .hover(&consumer, SourcePosition { line: 5, column: 8 })
        .expect("the local should have hover");
    assert!(hover.contents.contains("x: number"), "{hover:#?}");
    let unrelated = host
        .current_file_hover(&consumer, SourcePosition { line: 6, column: 8 })
        .expect("an unrelated local should retain the fast hover path");
    assert!(unrelated.contents.contains("local unrelated: any"));
}

#[test]
fn imported_ecs_world_and_component_keep_get_result_schema() {
    let fixture = Fixture::parse(
        r#"-- file: src/components.tua
local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
return { Position = Position, world = world }
-- file: src/main.tua
local Shared = require("components")
local entity = Shared.world:create()
local added = Shared.world:add(entity, Shared.Position, { x = 1 })
added.x = "wrong"
local position = Shared.world:get(entity, Shared.Position)
print(<|payload|>position)
position.x = "wrong"
local removed = Shared.world:remove(entity, Shared.Position)
removed.x = "wrong"
"#,
    );
    let mut host = AnalysisHost::new();
    host.set_source_roots([PathBuf::from("src")]);
    for file in fixture.files() {
        host.set_file(&file.path, file.source.clone());
    }
    let snapshot = host.snapshot();
    let hover = snapshot
        .hover("src/main.tua", fixture.position("payload"))
        .expect("the imported ECS result should have hover");
    assert!(hover.contents.contains("x: number"), "{hover:#?}");
    let diagnostics = snapshot.pipeline_diagnostics("src/main.tua");
    for field in ["added.x", "position.x", "removed.x"] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == Some(crate::DiagnosticCode::TYPE_MISMATCH)
                    && diagnostic.message.contains(field)
            }),
            "the imported component field `{field}` must reject a string assignment: {diagnostics:#?}"
        );
    }
}

#[test]
fn inline_require_inference_respects_shadowed_loader() {
    let fixture = Fixture::parse(
        r#"-- file: src/provider.tua
return {inc = function(x: number): number return x + 1 end}
-- file: src/main.tua
local require = function(name: string): {inc: (x: string) -> string}
  return {inc = function(x: string): string return x end}
end
local result = require("provider").inc("yes")
print(<|result|>result)
"#,
    );
    let mut host = AnalysisHost::new();
    for file in fixture.files() {
        host.set_file(&file.path, &file.source);
    }
    let hover = host
        .snapshot()
        .hover("src/main.tua", fixture.position("result"))
        .unwrap();
    assert!(hover.contents.contains("string"), "{hover:?}");
}
