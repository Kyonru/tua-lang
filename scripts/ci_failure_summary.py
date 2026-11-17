"""Expose the tail of a failed CI test log in public check annotations."""

import pathlib
import re
import sys


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: ci_failure_summary.py <log-path> <title>")
    lines = pathlib.Path(sys.argv[1]).read_text(errors="replace").splitlines()
    noise = ("[AgentHost]", "[ChatModelSelection]", "[RemoteAgentHost]")
    useful = [line for line in lines if not line.startswith(noise)]
    excerpt = "\n".join(useful[-80:])[-6000:]
    excerpt = re.sub(r"\x1b\[[0-9;]*m", "", excerpt)
    escaped = excerpt.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    title = sys.argv[2].replace("%", "%25").replace("\r", "").replace("\n", " ")
    print(f"::error title={title}::{escaped}")


if __name__ == "__main__":
    main()
