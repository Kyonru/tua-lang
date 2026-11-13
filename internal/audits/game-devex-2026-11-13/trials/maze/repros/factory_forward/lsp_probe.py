"""Probe imported-class receiver surfaces through the actual Tua LSP."""

import json
import os
import selectors
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent
TUA = Path(__file__).resolve().parents[7] / "target/debug/tua"


def send(proc, value):
    data = json.dumps(value, separators=(",", ":")).encode()
    proc.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
    proc.stdin.flush()


def read_responses(proc, wanted, timeout=12):
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ)
    buffer = b""
    found = {}
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline and not wanted.issubset(found):
        if not selector.select(max(0, deadline - time.monotonic())):
            continue
        chunk = os.read(proc.stdout.fileno(), 65536)
        if not chunk:
            break
        buffer += chunk
        while b"\r\n\r\n" in buffer:
            header, remainder = buffer.split(b"\r\n\r\n", 1)
            length = next((int(line.split(b":", 1)[1]) for line in header.split(b"\r\n")
                           if line.lower().startswith(b"content-length:")), None)
            if length is None or len(remainder) < length:
                break
            message = json.loads(remainder[:length])
            buffer = remainder[length:]
            if "id" in message:
                found[message["id"]] = message
    return found


proc = subprocess.Popen([str(TUA), "lsp"], cwd=ROOT, stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    send(proc, {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "processId": None, "rootUri": ROOT.as_uri(), "capabilities": {},
    }})
    init = read_responses(proc, {1})
    if 1 not in init:
        raise RuntimeError("initialize did not respond")
    send(proc, {"jsonrpc": "2.0", "method": "initialized", "params": {}})
    requests = {}
    for index, name in enumerate(("main", "annotated", "local")):
        path = ROOT / "src" / f"{name}.tua"
        source = path.read_text()
        send(proc, {"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": path.as_uri(), "languageId": "tua", "version": 1, "text": source},
        }})
        line_number, line = next((number, text) for number, text in enumerate(source.splitlines())
                                 if "guard:level()" in text)
        for offset, (method, character) in enumerate((
            ("textDocument/completion", line.index("guard:level()") + len("guard:")),
            ("textDocument/hover", line.index("guard:level()") + len("guard:") + 1),
            ("textDocument/definition", line.index("guard:level()") + len("guard:") + 1),
        )):
            request_id = 2 + index * 3 + offset
            requests[request_id] = (name, method)
            send(proc, {"jsonrpc": "2.0", "id": request_id, "method": method, "params": {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line_number, "character": character},
            }})
    responses = read_responses(proc, set(requests))
    for request_id, (name, method) in requests.items():
        response = responses.get(request_id)
        result = response.get("result") if response else None
        if method.endswith("completion"):
            items = result if isinstance(result, list) else (result or {}).get("items", [])
            summary = [item.get("label") for item in items if "level" in item.get("label", "")]
        elif method.endswith("hover"):
            content = (result or {}).get("contents", {})
            summary = str(content.get("value", content))[:240] if result else None
        else:
            summary = result
        print(json.dumps({"file": name, "method": method, "result": summary, "responded": response is not None}))
    provider = ROOT / "src" / "actors.tua"
    provider_source = provider.read_text()
    send(proc, {"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": {"uri": provider.as_uri(), "languageId": "tua", "version": 1, "text": provider_source},
    }})
    edited = provider_source.replace('level(): number { return 1 }', 'level(): string { return "one" }')
    for version, source in ((2, edited), (3, provider_source)):
        send(proc, {"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": provider.as_uri(), "version": version},
            "contentChanges": [{"text": source}],
        }})
        main_path = ROOT / "src" / "main.tua"
        send(proc, {"jsonrpc": "2.0", "id": 20 + version, "method": "textDocument/hover", "params": {
            "textDocument": {"uri": main_path.as_uri()},
            "position": {"line": 2, "character": 22},
        }})
        edit_response = read_responses(proc, {20 + version})
        value = ((edit_response.get(20 + version) or {}).get("result") or {}).get("contents", {})
        print(json.dumps({"provider_version": version, "method": "textDocument/hover",
                          "result": value.get("value", value)}))
    send(proc, {"jsonrpc": "2.0", "id": 99, "method": "shutdown", "params": None})
    read_responses(proc, {99}, timeout=2)
    send(proc, {"jsonrpc": "2.0", "method": "exit", "params": {}})
finally:
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    if proc.returncode:
        print("server stderr:", proc.stderr.read().decode(errors="replace")[:300], file=sys.stderr)
