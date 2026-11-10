"""Protocol-level hover/completion smoke; run from this project directory."""

import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parent
SERVER = ROOT.parents[2] / "target/debug/tua"
SOURCE = ROOT / "src/main.tua"


def frame(message):
    body = json.dumps(message, separators=(",", ":")).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


def messages(data):
    while data:
        header, data = data.split(b"\r\n\r\n", 1)
        length = int(header.split(b":", 1)[1].strip())
        yield json.loads(data[:length])
        data = data[length:]


uri = SOURCE.as_uri()
requests = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": ROOT.as_uri(), "capabilities": {}}},
    {"jsonrpc": "2.0", "method": "initialized", "params": {}},
    {"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": uri, "languageId": "tua", "version": 1, "text": SOURCE.read_text()}}},
    {"jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": {"textDocument": {"uri": uri}, "position": {"line": 54, "character": 8}}},
    {"jsonrpc": "2.0", "id": 3, "method": "textDocument/hover", "params": {"textDocument": {"uri": uri}, "position": {"line": 38, "character": 25}}},
    {"jsonrpc": "2.0", "id": 4, "method": "textDocument/completion", "params": {"textDocument": {"uri": uri}, "position": {"line": 38, "character": 25}}},
    {"jsonrpc": "2.0", "id": 5, "method": "shutdown", "params": None},
    {"jsonrpc": "2.0", "method": "exit"},
]

proc = subprocess.run(
    [str(SERVER), "lsp"],
    input=b"".join(map(frame, requests)),
    capture_output=True,
    timeout=15,
)
assert proc.returncode == 0, proc.stderr.decode()
responses = {item.get("id"): item for item in messages(proc.stdout) if "id" in item}
for request_id in (2, 3):
    result = responses[request_id].get("result")
    contents = result.get("contents") if result else None
    value = contents.get("value") if isinstance(contents, dict) else contents
    print(f"hover {request_id}: {value}")
completion = responses[4].get("result")
items = completion.get("items", []) if isinstance(completion, dict) else completion or []
print("completion labels:", [item.get("label") for item in items][:20])
for item in items:
    if item.get("label") in {"get", "value"}:
        print("completion item:", {key: item.get(key) for key in ("label", "kind", "insertText", "textEdit")})
