# Real-server editor checks

These tests launch an isolated VS Code development host with the production extension
bundle and a real Tua server. They do not install an extension into your normal profile.
Temporary projects and editor profiles are removed on completion unless
`TUA_TEST_KEEP_TEMP=1` is set.

From the repository root:

```sh
cargo build -p tua-cli
npm --prefix editors/vscode run build
TUA_REAL_SERVER_PATH="$PWD/target/debug/tua" node editors/vscode/test/real-server/run.js
```

For the dialogue preview lifecycle check alone:

```sh
TUA_REAL_SERVER_PATH="$PWD/target/debug/tua" \
  TUA_REAL_SERVER_SUITE=dialogue-preview \
  node editors/vscode/test/real-server/run.js
```

The dialogue check reads the rendered webview through the harness's local debugging
connection. It covers source navigation, unsaved text, invalid-edit recovery, external
writes, deletion/recreation, dirty-buffer discard, and hidden-preview refresh. No
project handlers or audio run. Protocol regressions separately verify document versions
and overlay ownership. The synthetic webview suite covers detailed layout, zoom,
keyboard and error-state interactions.

Use `make extension-real-server-test` for the existing staged-server workflow, including
sidebar filesystem checks. `TUA_TEST_VSIX=/path/to/tua.vsix` selects the separate temporary
VSIX installation workflow; development-host success is not release-VSIX qualification.
