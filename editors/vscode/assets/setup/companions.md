# Add tools only when you need them

| To do this | Editor tool | What Tua already provides |
| --- | --- | --- |
| Step through a running game and inspect variables | Second Local Lua Debugger | Builds the project, prepares source maps, and offers a Tua launch in Run and Debug |
| Read GLSL syntax more easily | Shader languages support | LÖVE shader validation and LÖVE syntax support |
| Recognize files quickly in Explorer | Material Icon Theme | Tua's own product icons |

The flame button is **Hot Reload**, built into Tua. It needs the LÖVE
executable, not another editor extension. **Profile LOVE Game** and the
Testing view are also built in; running Busted tests needs the Busted runtime.

Installing a companion is optional. The links in this step install only when
clicked. The Material Icons action also selects its file icons and offers a
Restore action in the icon step. Tua's language server, completion, hover,
previews, and formatter do not need a second language extension.
