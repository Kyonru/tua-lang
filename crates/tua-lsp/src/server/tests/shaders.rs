use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tua_core::shader_validation::{
    ShaderError, ShaderValidationOutcome, ShaderValidationRequest, ShaderValidator,
};

struct FakeValidator(Arc<AtomicUsize>);
impl ShaderValidator for FakeValidator {
    fn validate(
        &mut self,
        requests: &[ShaderValidationRequest],
    ) -> Result<Vec<ShaderValidationOutcome>, String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(requests
            .iter()
            .map(|request| {
                if request.source.contains("broken") {
                    ShaderValidationOutcome::Invalid(vec![ShaderError {
                        stage: None,
                        line: Some(2),
                        message: "invalid shader".into(),
                    }])
                } else {
                    ShaderValidationOutcome::Valid
                }
            })
            .collect())
    }
}

fn run_shader_chunks(chunks: Vec<(Duration, Vec<Value>)>, calls: Arc<AtomicUsize>) -> Vec<Value> {
    let chunks = chunks
        .into_iter()
        .map(|(delay, messages)| (delay, encode_messages(messages)))
        .collect();
    let mut output = Vec::new();
    super::super::run_with_shader_validator(
        DelayedChunks::new(chunks),
        &mut output,
        Some(Box::new(FakeValidator(calls))),
    )
    .unwrap();
    read_output_messages(&output)
}

#[test]
fn shader_worker_uses_overlays_clears_errors_and_reuses_unchanged_results() {
    let temp = TempProject::new("shader-worker");
    let source = "local effect = shader \"effect.glsl\"\nprint(effect)\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", source);
    temp.write("effect.glsl", "vec4 effect() {}");
    let mut initialize = initialize_with_root(1, temp.uri());
    initialize["params"]["initializationOptions"] = json!({"workspaceTrusted": true});
    let calls = Arc::new(AtomicUsize::new(0));
    let shader_uri = temp.uri_for("effect.glsl");
    let output = run_shader_chunks(
        vec![
            (
                Duration::ZERO,
                vec![
                    initialize,
                    did_open(&temp.uri_for("main.tua"), source),
                    did_open(&shader_uri, "vec4 effect() { broken }"),
                ],
            ),
            (
                Duration::from_millis(1200),
                vec![did_change(
                    &temp.uri_for("main.tua"),
                    &format!("{source}\n"),
                    2,
                )],
            ),
            (
                Duration::from_millis(900),
                vec![did_change(&shader_uri, "vec4 effect() {}", 2)],
            ),
            (
                Duration::from_millis(900),
                vec![json!({"jsonrpc":"2.0","method":"exit"})],
            ),
        ],
        calls.clone(),
    );
    let publications: Vec<_> = output
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == shader_uri
        })
        .collect();
    assert!(
        publications
            .iter()
            .any(|message| message["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["code"] == "TL2016")),
        "{output:?}"
    );
    assert!(publications.last().unwrap()["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        calls.load(Ordering::Relaxed),
        2,
        "unchanged shaders must reuse results"
    );
}

#[test]
fn untrusted_workspace_never_calls_shader_validator() {
    let temp = TempProject::new("untrusted-shaders");
    temp.write("tua.toml", "source = \".\"\n");
    temp.write(
        "main.tua",
        "local effect = shader \"effect.glsl\"\nprint(effect)\n",
    );
    temp.write("effect.glsl", "vec4 effect() { broken }");
    let calls = Arc::new(AtomicUsize::new(0));
    run_shader_chunks(
        vec![
            (Duration::ZERO, vec![initialize_with_root(1, temp.uri())]),
            (
                Duration::from_millis(800),
                vec![json!({"jsonrpc":"2.0","method":"exit"})],
            ),
        ],
        calls.clone(),
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn hover_completes_while_shader_compilation_is_blocked() {
    use std::io::Write;
    use std::sync::mpsc;
    struct BlockingValidator {
        started: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
    }
    impl ShaderValidator for BlockingValidator {
        fn validate(
            &mut self,
            requests: &[ShaderValidationRequest],
        ) -> Result<Vec<ShaderValidationOutcome>, String> {
            self.started.send(()).unwrap();
            self.release
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| "hover waited for shader compilation".to_owned())?;
            Ok(vec![ShaderValidationOutcome::Valid; requests.len()])
        }
    }
    struct Input {
        current: Cursor<Vec<u8>>,
        next: Option<Vec<u8>>,
        started: mpsc::Receiver<()>,
    }
    impl Read for Input {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            if self.current.position() as usize == self.current.get_ref().len() {
                if let Some(next) = self.next.take() {
                    self.started
                        .recv_timeout(Duration::from_secs(5))
                        .expect("shader worker must start");
                    self.current = Cursor::new(next);
                }
            }
            self.current.read(bytes)
        }
    }
    struct Output {
        bytes: Vec<u8>,
        release: mpsc::SyncSender<()>,
    }
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if String::from_utf8_lossy(bytes).contains("\"id\":2") {
                let _ = self.release.try_send(());
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let temp = TempProject::new("shader-latency");
    let source = "local effect = shader \"effect.glsl\"\nprint(effect)\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", source);
    temp.write("effect.glsl", "vec4 effect() {}");
    let mut initialize = initialize_with_root(1, temp.uri());
    initialize["params"]["initializationOptions"] = json!({"workspaceTrusted": true});
    let (start_tx, start_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let input = Input {
        current: Cursor::new(encode_messages(vec![
            initialize,
            did_open(&temp.uri_for("main.tua"), source),
        ])),
        next: Some(encode_messages(vec![
            json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":temp.uri_for("main.tua")},"position":{"line":1,"character":7}}}),
        ])),
        started: start_rx,
    };
    let mut output = Output {
        bytes: Vec::new(),
        release: release_tx,
    };
    let started = Instant::now();
    super::super::run_with_shader_validator(
        input,
        &mut output,
        Some(Box::new(BlockingValidator {
            started: start_tx,
            release: release_rx,
        })),
    )
    .unwrap();
    let responses = read_output_messages(&output.bytes);
    assert!(response_with_id(&responses, 2).get("result").is_some());
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "hover must release the validator before its timeout"
    );
}

#[test]
fn uniform_editor_features_return_shader_locations_and_live_overlays() {
    let temp = TempProject::new("uniform-editor");
    let source =
        "local effect = shader \"effect.glsl\"\neffect:send(\"tint\", {})\neffect:send(\"\", {})\n";
    let shader = "/* comment */ extern vec4 tint;\r\n#ifdef VERTEX\r\nuniform vec4 tint;\r\n#endif\r\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return tint; }\r\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", source);
    temp.write("effect.glsl", shader);
    let uri = temp.uri_for("main.tua");
    let shader_uri = temp.uri_for("effect.glsl");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(definition_request(2, &uri, source, "tint", 1));
    let locations = response_with_id(server.output(), 2)["result"]
        .as_array()
        .unwrap();
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0]["uri"], shader_uri);
    assert_eq!(
        locations[0]["range"],
        json!({"start":{"line":0,"character":26},"end":{"line":0,"character":30}})
    );
    assert_eq!(
        locations[1]["range"]["start"],
        json!({"line":2,"character":13})
    );
    server.send(hover_request(3, &uri, source, "tint"));
    assert_hover_contains(server.output(), 3, "extern vec4 tint;");
    server.send(completion_request(4, &uri, source, "effect:send(\""));
    assert_completion_labels(server.output(), 4, ["tint"]);

    server.send(did_open(&shader_uri, "extern number replacement;\n"));
    server.send(completion_request(5, &uri, source, "effect:send(\""));
    assert_completion_labels(server.output(), 5, ["replacement"]);
    server.send(did_change(&shader_uri, "extern vec2 moved;\n", 2));
    server.send(completion_request(6, &uri, source, "effect:send(\""));
    assert_completion_labels(server.output(), 6, ["moved"]);
    server.send(did_close(&shader_uri));
    server.send(completion_request(7, &uri, source, "effect:send(\""));
    assert_completion_labels(server.output(), 7, ["tint"]);
}

#[test]
fn lua_shader_uniform_editor_features_share_tua_metadata() {
    let temp = TempProject::new("lua-uniform-editor");
    let source =
        "local effect = love.graphics.newShader(\"effect.glsl\")\neffect:sendColor(\"tint\", {})\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.lua", source);
    temp.write("effect.glsl", "extern vec4 tint;\n");
    let uri = temp.uri_for("main.lua");
    let mut server = TestServer::new();
    server.send(initialize_with_root(1, temp.uri()));
    server.send(did_open(&uri, source));
    server.send(hover_request(2, &uri, source, "tint"));
    assert_hover_contains(server.output(), 2, "Unread uniform");
    server.send(definition_request(3, &uri, source, "tint", 1));
    assert_eq!(
        response_with_id(server.output(), 3)["result"]["uri"],
        temp.uri_for("effect.glsl")
    );
}

#[test]
fn shader_value_diagnostics_follow_unsaved_uniform_types() {
    let temp = TempProject::new("shader-value-diagnostics");
    let source = "local effect = shader \"effect.glsl\"\neffect:send(\"tint\", 1)\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", source);
    temp.write(
        "effect.glsl",
        "extern vec4 tint;\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return tint; }\n",
    );
    let uri = temp.uri_for("main.tua");
    let shader_uri = temp.uri_for("effect.glsl");
    let mut server = TestServer::new();
    server.send(initialize_with_pull_diagnostics(1, temp.uri(), false));
    server.send(did_open(&uri, source));
    server.send(text_document_diagnostic_request(2, &uri));
    let response = response_with_id(server.output(), 2);
    let errors: Vec<_> = response["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "TL2015")
        .collect();
    assert_eq!(errors.len(), 1, "{response}");
    assert_eq!(errors[0]["severity"], 1);
    assert_eq!(
        errors[0]["range"]["start"],
        json!({"line":1,"character":20})
    );
    server.send(did_open(&shader_uri, "extern number tint;\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return c * tint; }\n"));
    server.send(text_document_diagnostic_request(3, &uri));
    assert!(!response_with_id(server.output(), 3)["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "TL2015"));
}

#[test]
fn inline_shader_worker_maps_errors_and_clears_them_after_unsaved_edits() {
    let temp = TempProject::new("inline-shader-worker");
    let source = "local effect = love.graphics.newShader([[\nvec4 effect() {\nbroken\n}\n]])\nprint(effect)\n";
    temp.write("tua.toml", "source = \".\"\n");
    temp.write("main.tua", source);
    let uri = temp.uri_for("main.tua");
    let mut initialize = initialize_with_root(1, temp.uri());
    initialize["params"]["initializationOptions"] = json!({"workspaceTrusted": true});
    let calls = Arc::new(AtomicUsize::new(0));
    let output = run_shader_chunks(
        vec![
            (Duration::ZERO, vec![initialize, did_open(&uri, source)]),
            (
                Duration::from_millis(1200),
                vec![did_change(
                    &uri,
                    &source.replace("broken", "return vec4(1.0);"),
                    2,
                )],
            ),
            (
                Duration::from_millis(1200),
                vec![json!({"jsonrpc":"2.0","method":"exit"})],
            ),
        ],
        calls.clone(),
    );
    let publications: Vec<_> = output
        .iter()
        .filter(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
        .collect();
    assert!(
        publications.iter().any(|m| m["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "TL2016" && d["range"]["start"]["line"] == 2)),
        "{output:?}"
    );
    assert!(!publications.last().unwrap()["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "TL2016"));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}
