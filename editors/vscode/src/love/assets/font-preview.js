"use strict";

const path = require("node:path");
const vscode = require("vscode");
const { escapeHtml, webviewNonce } = require("../../shared/webview");

function isFontFile(uri) {
  return uri?.scheme === "file" && /\.(?:otf|ttf)$/i.test(uri.fsPath);
}

function showFontPreview(uri) {
  const name = path.basename(uri.fsPath);
  const panel = vscode.window.createWebviewPanel(
    "tuaFontPreview",
    `Font: ${name}`,
    vscode.ViewColumn.Active,
    {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.file(path.dirname(uri.fsPath))],
    },
  );
  panel.webview.html = fontPreviewHtml(panel.webview, uri);
}

function fontPreviewHtml(webview, uri) {
  const name = path.basename(uri.fsPath);
  const format = path.extname(name).slice(1).toUpperCase();
  const fontUrl = cssString(webview.asWebviewUri(uri).toString());
  const nonce = webviewNonce();
  const samples = [18, 32, 48, 72]
    .map((size) => `<section class="sample"><h2>${size} px</h2><p class="size-${size}">The quick brown fox jumps over the lazy dog.</p></section>`)
    .join("\n");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}'; font-src ${webview.cspSource};">
  <title>${escapeHtml(name)} font preview</title>
  <style nonce="${nonce}">
    @font-face { font-family: 'TuaFontPreview'; src: url("${fontUrl}"); }
    body { margin: 0; padding: 24px; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); }
    main { max-width: 960px; margin: auto; }
    h1 { font-size: 20px; overflow-wrap: anywhere; }
    h2 { font-size: 12px; font-weight: 600; color: var(--vscode-descriptionForeground); }
    .meta, .note { color: var(--vscode-descriptionForeground); }
    .sample { border-top: 1px solid var(--vscode-panel-border); padding: 12px 0; }
    .sample p, .glyphs { font-family: 'TuaFontPreview'; margin: 8px 0 16px; line-height: 1.3; overflow-wrap: anywhere; }
    .size-18 { font-size: 18px; }
    .size-32 { font-size: 32px; }
    .size-48 { font-size: 48px; }
    .size-72 { font-size: 72px; }
    .glyphs { font-size: 28px; letter-spacing: 0.03em; }
    #status[data-error] { color: var(--vscode-errorForeground); }
    @media (max-width: 480px) { body { padding: 16px; } }
  </style>
</head>
<body>
  <main>
    <h1>${escapeHtml(name)}</h1>
    <p class="meta">${escapeHtml(format)} · Game text at common sizes</p>
    <p id="status" role="status">Loading font…</p>
    ${samples}
    <section class="sample"><h2>Letters, numbers, punctuation</h2><p class="glyphs">ABCDEFGHIJKLMNOPQRSTUVWXYZ<br>abcdefghijklmnopqrstuvwxyz<br>0123456789 !?.,:;+-*/()[]</p></section>
    <p class="note">This uses the editor's font renderer. Check final text in LÖVE at the game's scale.</p>
  </main>
  <script nonce="${nonce}">
    document.fonts.load('24px TuaFontPreview').then((faces) => {
      const status = document.getElementById('status');
      status.textContent = faces.length ? 'Font loaded' : 'Font could not be rendered';
      if (!faces.length) status.dataset.error = 'true';
    }, () => {
      const status = document.getElementById('status');
      status.textContent = 'Font could not be rendered';
      status.dataset.error = 'true';
    });
  </script>
</body>
</html>`;
}

function cssString(value) {
  return value.replace(/[\\"'<>\r\n]/g, (character) =>
    `\\${character.codePointAt(0).toString(16)} `);
}

module.exports = { fontPreviewHtml, isFontFile, showFontPreview };
