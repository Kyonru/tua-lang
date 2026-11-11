#!/usr/bin/env python3
"""Request a compiler-produced module graph for the disposable host project."""
import json
import pathlib
import queue
import subprocess
import sys
import threading

project = pathlib.Path(sys.argv[1]).resolve()
output = pathlib.Path(sys.argv[2]).resolve()
server = pathlib.Path(sys.argv[3]).resolve()
process = subprocess.Popen([str(server), "lsp", "--stdio"], stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
responses = queue.Queue()

def receive():
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

threading.Thread(target=receive, daemon=True).start()

def request(identifier, method, params):
    message = {"jsonrpc": "2.0", "id": identifier, "method": method, "params": params}
    data = json.dumps(message).encode()
    process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
    process.stdin.flush()
    while True:
        answer = responses.get(timeout=120)
        if answer.get("id") == identifier:
            if "error" in answer:
                raise RuntimeError(answer["error"])
            return answer["result"]

root = project.as_uri()
request(1, "initialize", {"processId": None, "rootUri": root,
                          "capabilities": {}, "workspaceFolders": [{"uri": root, "name": project.name}]})
graph = request(2, "tua/moduleGraph", None)
output.write_text(json.dumps(graph))
request(3, "shutdown", None)
exit_message = b'{"jsonrpc":"2.0","method":"exit","params":null}'
process.stdin.write(f"Content-Length: {len(exit_message)}\r\n\r\n".encode() + exit_message)
process.stdin.flush()
process.wait(timeout=5)
