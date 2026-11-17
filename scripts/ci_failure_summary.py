"""Expose the tail of a failed CI test log in public check annotations."""

import pathlib
import re
import sys


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: ci_failure_summary.py <log-path> <title>")
    lines = pathlib.Path(sys.argv[1]).read_text(errors="replace").splitlines()
    noise = (
        "[AgentHost",
        "[ChatModelSelection]",
        "[RemoteAgentHost]",
        "[main ",
        "Unknown channel:",
    )
    useful = [line for line in lines if not line.startswith(noise)]
    salient = re.compile(r"assertionerror|testrunfailederror|timed out|error:|failed|exit code", re.I)
    selected = set(range(max(0, len(useful) - 25), len(useful)))
    for index, line in enumerate(useful):
        if salient.search(line):
            selected.update(range(max(0, index - 4), min(len(useful), index + 7)))
    excerpt = "\n".join(useful[index] for index in sorted(selected))[-10000:]
    excerpt = re.sub(r"\x1b\[[0-9;]*m", "", excerpt)
    escaped = excerpt.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    title = sys.argv[2].replace("%", "%25").replace("\r", "").replace("\n", " ")
    print(f"::error title={title}::{escaped}")


if __name__ == "__main__":
    main()
