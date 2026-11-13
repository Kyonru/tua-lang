#!/usr/bin/env python3
"""Compare SignalBus signature help at same-file and imported call sites."""
import json
from pathlib import Path
import queue
import subprocess
import threading
import time

project = Path(__file__).resolve().parent
server = project.parents[6] / "target/debug/tua"
process = subprocess.Popen([str(server), "lsp", "--stdio"], stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
replies = queue.Queue()


def reader():
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
        replies.put(json.loads(process.stdout.read(int(headers["content-length"]))))


threading.Thread(target=reader, daemon=True).start()
request_id = 0


def send(method, params, request=True):
    global request_id
    payload = {"jsonrpc": "2.0", "method": method, "params": params}
    if request:
        request_id += 1
        payload["id"] = request_id
    body = json.dumps(payload).encode()
    process.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    process.stdin.flush()
    if not request:
        return None
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        reply = replies.get(timeout=max(0.1, deadline - time.monotonic()))
        if reply.get("id") == request_id:
            return reply
    raise TimeoutError(method)


root = project.as_uri()
send("initialize", {"processId": None, "rootUri": root, "capabilities": {},
                    "workspaceFolders": [{"uri": root, "name": project.name}]})
send("initialized", {}, False)

for name, filename, prefix in [
    ("imported", "main.tua", 'bus:emit("hit", '),
    ("direct-module-property", "direct.tua", 'EventModule.events:emit("hit", '),
    ("same-file", "events.tua", 'events:emit("hit", '),
]:
    path = project / "src" / filename
    text = path.read_text() + "\n" + prefix
    uri = path.as_uri()
    send("textDocument/didOpen", {"textDocument": {"uri": uri,
         "languageId": "tua", "version": 1, "text": text}}, False)
    position = {"line": len(text.splitlines()) - 1, "character": len(prefix)}
    response = send("textDocument/signatureHelp", {"textDocument": {"uri": uri},
                                                    "position": position})
    print(json.dumps({"case": name, "position": position,
                      "result": response.get("result")}, ensure_ascii=False))

send("shutdown", None)
send("exit", None, False)
process.wait(timeout=5)
