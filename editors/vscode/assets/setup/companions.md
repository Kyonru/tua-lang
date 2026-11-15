# Add tools only when you need them

| To do this | Editor tool | What Tua already provides |
| --- | --- | --- |
| Step through a running game and inspect variables | Second Local Lua Debugger | Builds the project, prepares source maps, and offers a Tua launch in Run and Debug |
| Explore a recorded LÖVE CPU capture as a flame chart | Flame Chart Visualizer for JavaScript Profiles | Captures LuaJIT samples and exports a compatible `.cpuprofile` |
| Read GLSL syntax more easily | Shader languages support | LÖVE shader validation and LÖVE syntax support |
| Recognize files quickly in Explorer | Material Icon Theme | Tua's own product icons |

The hot-reload flame button is built into Tua. **Profile LOVE Game** captures
timed LuaJIT samples; its results offer **Open flame chart** when the optional
Flame Chart Visualizer is installed, and **Export .cpuprofile** to save a copy.
The Testing view is built in; running Busted tests needs the Busted runtime.

Installing a companion is optional. The links in this step install only when
clicked. The Material Icons action also selects its file icons and offers a
Restore action in the icon step. Tua's language server, completion, hover,
previews, and formatter do not need a second language extension.
