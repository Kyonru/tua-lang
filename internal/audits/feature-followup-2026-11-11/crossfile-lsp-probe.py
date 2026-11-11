#!/usr/bin/env python3
"""Minimal stdio LSP cross-file class probe. Pass a disposable project path."""
import json
import pathlib
import queue
import subprocess
import sys
import threading
import time

project = pathlib.Path(sys.argv[1]).resolve()
server = pathlib.Path(__file__).resolve().parents[3] / "target/release/tua"
proc = subprocess.Popen([str(server), "lsp", "--stdio"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
received = queue.Queue()

def read():
    while True:
        headers = {}
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            if line == b"\r\n":
                break
            key, value = line.decode().split(":", 1)
            headers[key.lower()] = value.strip()
        received.put(json.loads(proc.stdout.read(int(headers["content-length"]))))

threading.Thread(target=read, daemon=True).start()
counter = 0
def send(method, params, request=True):
    global counter
    message = {"jsonrpc": "2.0", "method": method, "params": params}
    if request:
        counter += 1
        message["id"] = counter
    data = json.dumps(message).encode()
    proc.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
    proc.stdin.flush()
    if not request:
        return None
    until = time.time() + 20
    while time.time() < until:
        response = received.get(timeout=max(0.1, until - time.time()))
        if response.get("id") == counter:
            return response
    raise TimeoutError(method)

root = project.as_uri()
send("initialize", {"processId": None, "rootUri": root, "capabilities": {}, "workspaceFolders": [{"uri": root, "name": project.name}]})
send("initialized", {}, False)
file = project / "src/cross_file.tua"
if not file.exists():
    file = project / "src/main.tua"
uri = file.as_uri()
original = file.read_text()
text = original.replace("print(agent)\n", "print(agent.)\n")
send("textDocument/didOpen", {"textDocument": {"uri": uri, "languageId": "tua", "version": 1, "text": text}}, False)
for method, position in [("textDocument/completion", {"line": 3, "character": 12}),
                         ("textDocument/hover", {"line": 1, "character": 8})]:
    response = send(method, {"textDocument": {"uri": uri}, "position": position})
    result = response.get("result")
    if method.endswith("completion"):
        items = result.get("items", []) if isinstance(result, dict) else result or []
        result = [item.get("label") for item in items[:50]]
    print(method, json.dumps(result, ensure_ascii=False)[:2500])
text = original.replace("print(agent)\n", "agent.\n")
send("textDocument/didChange", {"textDocument": {"uri": uri, "version": 2}, "contentChanges": [{"text": text}]}, False)
response = send("textDocument/completion", {"textDocument": {"uri": uri}, "position": {"line": 3, "character": 6}})
result = response.get("result")
items = result.get("items", []) if isinstance(result, dict) else result or []
print("bare completion", json.dumps([item.get("label") for item in items[:50]], ensure_ascii=False))
variants = {
    "direct constructor": 'local Agent = require("classes.agent").Agent\nlocal agent = Agent.new("pilot")\nprint(agent)\nagent.\n',
    "factory return": 'local factory = require "factory"\nlocal agent = factory.spawn("pilot")\nprint(agent)\nagent.\n',
    "service return": 'local service = require "service"\nlocal agent = service.current()\nprint(agent)\nagent.\n',
    "annotated second hop": 'local service = require "service_annotated"\nlocal agent = service.current()\nprint(agent)\nagent.\n',
}
for version, (label, source) in enumerate(variants.items(), 3):
    send("textDocument/didChange", {"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": source}]}, False)
    response = send("textDocument/completion", {"textDocument": {"uri": uri}, "position": {"line": 3, "character": 6}})
    result = response.get("result")
    items = result.get("items", []) if isinstance(result, dict) else result or []
    print(label, json.dumps([item.get("label") for item in items[:50]], ensure_ascii=False))
send("shutdown", None)
send("exit", None, False)
proc.wait(timeout=5)
