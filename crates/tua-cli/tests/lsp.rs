use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};

mod support;

use support::{tua, wait_for_child_output};

#[test]
fn lsp_initializes_and_publishes_open_and_change_diagnostics() {
    let uri = "file:///tmp/tua-lsp-main.tua";
    let mut child = tua()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tua lsp");

    {
        let stdin = child.stdin.as_mut().expect("lsp stdin should be piped");

        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
        );
        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "tua",
                        "version": 1,
                        "text": "local hp: number = \"full\"\n"
                    }
                }
            }),
        );
        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "version": 2
                    },
                    "contentChanges": [
                        {
                            "text": "local hp: number = 10\n"
                        }
                    ]
                }
            }),
        );
        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": {
                        "uri": uri
                    },
                    "position": {
                        "line": 0,
                        "character": 6
                    }
                }
            }),
        );
        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            }),
        );
        write_message(
            stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "exit"
            }),
        );
    }

    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(
        output.status.success(),
        "tua lsp failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected lsp stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let messages = read_output_messages(&output.stdout);

    assert!(messages.iter().any(|message| {
        message["id"] == 1 && message["result"]["capabilities"]["textDocumentSync"] == 1
    }));
    assert!(messages
        .iter()
        .any(|message| message["id"] == 2 && message["result"].is_null()));
    assert!(messages.iter().any(|message| {
        message["id"] == 3
            && message["result"]["contents"]["value"]
                .as_str()
                .is_some_and(|contents| contents.contains("local hp: number"))
    }));

    let publications = messages
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        })
        .collect::<Vec<_>>();

    assert_eq!(publications.len(), 2);

    let open_diagnostics = publications[0]["params"]["diagnostics"]
        .as_array()
        .expect("open diagnostics should be an array");

    assert_eq!(open_diagnostics.len(), 1);
    assert_eq!(open_diagnostics[0]["code"], "TL1003");
    assert!(open_diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message should be a string")
        .contains("type mismatch: expected number, found string"));

    let change_diagnostics = publications[1]["params"]["diagnostics"]
        .as_array()
        .expect("change diagnostics should be an array");

    assert!(change_diagnostics.is_empty());
}

fn write_message(writer: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).expect("message should serialize");

    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).expect("failed to write header");
    writer.write_all(&body).expect("failed to write body");
    writer.flush().expect("failed to flush message");
}

fn read_output_messages(mut output: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();

    while !output.is_empty() {
        let header_end = output
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("output should contain a header terminator");
        let header = std::str::from_utf8(&output[..header_end]).expect("header should be UTF-8");
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;

                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("valid content length"))
            })
            .expect("content length should exist");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        let message =
            serde_json::from_slice(&output[body_start..body_end]).expect("valid JSON response");

        messages.push(message);
        output = &output[body_end..];
    }

    messages
}
