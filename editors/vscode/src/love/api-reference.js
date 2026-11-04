"use strict";

const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");

const COMMAND = "tua.openLoveApiReference";
const RESOURCE = path.join("resources", "love-api", "index.html");

function registerLoveApiReference(context) {
  return vscode.commands.registerCommand(COMMAND, () => {
    const referencePath = path.join(context.extensionPath, RESOURCE);
    const html = fs.readFileSync(referencePath, "utf8");
    const panel = vscode.window.createWebviewPanel(
      "tuaLoveApiReference",
      "LÖVE API Reference (Offline)",
      vscode.ViewColumn.One,
      { enableScripts: true },
    );

    panel.webview.html = html.replace(
      "<head>",
      '<head><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; script-src \'unsafe-inline\'; style-src \'unsafe-inline\'; img-src data:; font-src data:">',
    );
    return panel;
  });
}

module.exports = { COMMAND, registerLoveApiReference };
