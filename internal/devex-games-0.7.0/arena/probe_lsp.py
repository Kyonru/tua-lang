"""Probe hover on the imported generic Box method using the real Tua LSP."""

import json
import os
import pathlib
import select
import subprocess
import time

root = pathlib.Path(__file__).resolve().parent
source = root / "src/main.tua"
binary = root.parents[2] / "target/debug/tua"
process = subprocess.Popen(
    [str(binary), "lsp"],
    cwd=root,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
)
buffer = b""


def send(message):
    body = json.dumps(message).encode()
    process.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    process.stdin.flush()


def receive(target_id, timeout=8):
    global buffer
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if b"\r\n\r\n" in buffer:
            headers, body = buffer.split(b"\r\n\r\n", 1)
            length = None
            for line in headers.split(b"\r\n"):
                if line.lower().startswith(b"content-length:"):
                    length = int(line.split(b":", 1)[1])
            if length is not None and len(body) >= length:
                message = json.loads(body[:length])
                buffer = body[length:]
                if message.get("id") == target_id:
                    return message
                continue
        ready, _, _ = select.select([process.stdout], [], [], max(0, deadline - time.monotonic()))
        if ready:
            chunk = os.read(process.stdout.fileno(), 65536)
            if not chunk:
                break
            buffer += chunk
    raise RuntimeError(f"LSP request {target_id} timed out")


try:
    send(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"processId": None, "rootUri": root.as_uri(), "capabilities": {}},
        }
    )
    receive(1)
    send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": source.as_uri(),
                    "languageId": "tua",
                    "version": 1,
                    "text": source.read_text(),
                }
            },
        }
    )
    lines = source.read_text().splitlines()
    line = next(i for i, value in enumerate(lines) if "score:set(score:get() + dt)" in value)
    column = lines[line].index("get()") + 1
    send(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/hover",
            "params": {
                "textDocument": {"uri": source.as_uri()},
                "position": {"line": line, "character": column},
            },
        }
    )
    hover = receive(2)
    send(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {"uri": source.as_uri()},
                "position": {"line": line, "character": lines[line].index("score:set") + len("score:")},
                "context": {"triggerKind": 1},
            },
        }
    )
    completion = receive(3).get("result")
    items = completion.get("items", []) if isinstance(completion, dict) else completion or []
    send(
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "textDocument/definition",
            "params": {
                "textDocument": {"uri": source.as_uri()},
                "position": {"line": line, "character": column},
            },
        }
    )
    definition = receive(4).get("result")
    print(
        json.dumps(
            {
                "line": line,
                "column": column,
                "hover": hover.get("result"),
                "completion_methods": [item["label"] for item in items if item.get("label") in ("get", "set")],
                "definition": definition,
            },
            indent=2,
        )
    )
finally:
    process.kill()
    process.wait(timeout=2)
