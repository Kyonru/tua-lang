# Developer-experience journey log

These are agent-simulated tasks, not human usability observations.

| Journey | Attempt and outcome | Limit |
| --- | --- | --- |
| First project | `tua init` in a fresh temporary directory created config, source, and `.gitignore`; replacing the entry with a pure typed function then running check, lint, format check, build, and Lua execution succeeded. The CLI's top-level help exposed the required commands. | Runtime used system Lua 5.4, not LuaJIT/LOVE; editor installation was not part of this journey yet. |
| Run unit tests | In `examples/unit-testing`, `tua test` discovered and ran three Busted cases successfully. | No gutter click or VS Code Test Explorer interaction was observed. |
| Package a minimal game | `tua package` produced a `.love` ZIP containing `main.lua`, and `unzip -t` passed. | No graphical launch or platform fusion tested. |

Other journeys in the plan remain pending or assigned to investigation lanes. Record wrong turns, correction quality, and time-to-feedback when those journeys run; do not infer a user-experience defect from task duration alone.
