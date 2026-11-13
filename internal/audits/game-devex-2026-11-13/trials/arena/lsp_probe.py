#!/usr/bin/env python3
"""Probe the real Tua server against an unsaved arena edit, via stdio LSP."""

import json
from pathlib import Path
import queue
import subprocess
import threading
import time

project = Path(__file__).resolve().parent
server = Path(__file__).resolve().parents[5] / "target/debug/tua"
process = subprocess.Popen(
    [str(server), "lsp", "--stdio"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)
responses = queue.Queue()


def read_messages():
    while True:
        headers = {}
        while True:
            line = process.stdout.readline()
            if not line:
                return
            if line == b"\r\n":
                break
            key, value = line.decode().split(":", 1)
            headers[key.lower()] = value.strip()
        responses.put(json.loads(process.stdout.read(int(headers["content-length"]))))


threading.Thread(target=read_messages, daemon=True).start()
next_id = 0


def send(method, params, request=True):
    global next_id
    message = {"jsonrpc": "2.0", "method": method, "params": params}
    if request:
        next_id += 1
        message["id"] = next_id
    body = json.dumps(message).encode()
    process.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    process.stdin.flush()
    if not request:
        return None
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        reply = responses.get(timeout=max(0.1, deadline - time.monotonic()))
        if reply.get("id") == next_id:
            return reply
    raise TimeoutError(method)


root = project.as_uri()
send("initialize", {
    "processId": None,
    "rootUri": root,
    "capabilities": {},
    "workspaceFolders": [{"uri": root, "name": project.name}],
})
send("initialized", {}, False)
path = project / "src/main.tua"
uri = path.as_uri()
original = path.read_text()
send("textDocument/didOpen", {"textDocument": {
    "uri": uri, "languageId": "tua", "version": 1, "text": original,
}}, False)


def probe(name, method, text, position):
    send("textDocument/didChange", {"textDocument": {
        "uri": uri, "version": probe.version,
    }, "contentChanges": [{"text": text}]}, False)
    probe.version += 1
    start = time.monotonic()
    reply = send(method, {"textDocument": {"uri": uri}, "position": position})
    elapsed = round((time.monotonic() - start) * 1000, 2)
    result = reply.get("result")
    if method.endswith("completion"):
        items = result.get("items", []) if isinstance(result, dict) else result or []
        result = [item.get("label") for item in items]
    print(json.dumps({"name": name, "method": method, "ms": elapsed,
                      "result": result}, ensure_ascii=False, default=str))


probe.version = 2
direct_line = "Game.selected.value."
text = original + "\n" + direct_line
probe("direct imported generic chain", "textDocument/completion", text,
      {"line": len(text.splitlines()) - 1, "character": len(direct_line)})
probe_line = "local trialActor = Game.selected.value\ntrialActor."
text = original + "\n" + probe_line
probe("imported generic member", "textDocument/completion", text,
      {"line": len(text.splitlines()) - 1, "character": len("trialActor.")})
hover_line = "local trialActor = Game.selected.value"
text = original + "\n" + hover_line
probe("imported generic hover", "textDocument/hover", text,
      {"line": len(text.splitlines()) - 1, "character": len("local trialActor = Game.selected.va")})
probe("imported generic value end hover", "textDocument/hover", text,
      {"line": len(text.splitlines()) - 1, "character": 37})
probe("imported generic root hover", "textDocument/hover", text,
      {"line": len(text.splitlines()) - 1, "character": 21})
probe("imported generic field hover", "textDocument/hover", text,
      {"line": len(text.splitlines()) - 1, "character": 27})
local_line = "local trialActor = Game.selected.value\nprint(trialActor)"
text = original + "\n" + local_line
probe("inferred local hover", "textDocument/hover", text,
      {"line": len(text.splitlines()) - 1, "character": 9})
call_line = 'Game.events:emit("player.hit", '
text = original + "\n" + call_line
probe("event payload signature", "textDocument/signatureHelp", text,
      {"line": len(text.splitlines()) - 1, "character": len(call_line)})
source_line = next(i for i, line in enumerate(original.splitlines()) if "Game.attack()" in line)
probe("imported function definition", "textDocument/definition", original,
      {"line": source_line, "character": original.splitlines()[source_line].index("Game.attack") + len("Game.att")})
send("shutdown", None)
send("exit", None, False)
process.wait(timeout=5)
