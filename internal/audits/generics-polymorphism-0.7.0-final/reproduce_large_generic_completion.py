#!/usr/bin/env python3
"""Reproduce large imported generic-alias completion through the Tua LSP."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import select
import subprocess
import tempfile
import time


def send(process: subprocess.Popen[bytes], message: dict[str, object]) -> None:
    payload = json.dumps(message, separators=(",", ":")).encode()
    process.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
    process.stdin.flush()


def read_message(process: subprocess.Popen[bytes], timeout: float) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    headers = bytearray()
    while b"\r\n\r\n" not in headers:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([process.stdout], [], [], remaining)[0]:
            raise TimeoutError("timed out reading LSP headers")
        headers.extend(os.read(process.stdout.fileno(), 1))

    length = int(
        next(
            line.split(b":", 1)[1]
            for line in headers.split(b"\r\n")
            if line.lower().startswith(b"content-length:")
        )
    )
    body = bytearray()
    while len(body) < length:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([process.stdout], [], [], remaining)[0]:
            raise TimeoutError("timed out reading LSP response body")
        body.extend(os.read(process.stdout.fileno(), length - len(body)))
    return json.loads(body)


def response_with_id(
    process: subprocess.Popen[bytes], request_id: int, timeout: float
) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    while True:
        message = read_message(process, max(0.001, deadline - time.monotonic()))
        if message.get("id") == request_id:
            return message


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary", type=pathlib.Path, default=pathlib.Path("target/debug/tua")
    )
    parser.add_argument("--timeout", type=float, default=15.0)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"Tua binary not found: {binary}; build it with `cargo build -p tua-cli`")

    with tempfile.TemporaryDirectory(prefix="tua-wide-completion-") as directory:
        root = pathlib.Path(directory)
        source_dir = root / "src"
        source_dir.mkdir()
        fields = ", ".join(f"f{index}: T" for index in range(4_200))
        (source_dir / "aliases.tua").write_text(
            f"export type Wide<T> = {{ {fields} }}\n", encoding="utf-8"
        )
        main_source = (
            'import type { Wide } from "./aliases"\n'
            "local wide: Wide<number>\n"
            "local result = wide.f419"
        )
        main_path = source_dir / "main.tua"
        main_path.write_text(main_source, encoding="utf-8")

        process = subprocess.Popen(
            [str(binary), "lsp", "--stdio"],
            cwd=root,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        try:
            send(
                process,
                {
                    "jsonrpc": "2.0",
                    "id": 0,
                    "method": "initialize",
                    "params": {
                        "processId": None,
                        "rootUri": root.as_uri(),
                        "capabilities": {},
                    },
                },
            )
            response_with_id(process, 0, args.timeout)
            send(process, {"jsonrpc": "2.0", "method": "initialized", "params": {}})
            for path in (source_dir / "aliases.tua", main_path):
                send(
                    process,
                    {
                        "jsonrpc": "2.0",
                        "method": "textDocument/didOpen",
                        "params": {
                            "textDocument": {
                                "uri": path.as_uri(),
                                "languageId": "tua",
                                "version": 1,
                                "text": path.read_text(encoding="utf-8"),
                            }
                        },
                    },
                )

            line = main_source.splitlines()[2]
            send(
                process,
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "textDocument/completion",
                    "params": {
                        "textDocument": {"uri": main_path.as_uri()},
                        "position": {"line": 2, "character": len(line)},
                    },
                },
            )
            started = time.perf_counter()
            response = response_with_id(process, 1, args.timeout)
            elapsed = time.perf_counter() - started
            result = response.get("result")
            items = result if isinstance(result, list) else (result or {}).get("items", [])
            labels = [item.get("label") for item in items]
            if response.get("error") or "f419" not in labels:
                raise SystemExit(f"completion failed: {response}")
            print(
                json.dumps(
                    {"elapsed_seconds": round(elapsed, 3), "items": len(items), "sample": labels[:10]},
                    indent=2,
                )
            )
        finally:
            process.kill()
            process.wait()


if __name__ == "__main__":
    main()
