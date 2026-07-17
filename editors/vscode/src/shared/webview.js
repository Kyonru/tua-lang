"use strict";

const crypto = require("crypto");

function webviewNonce(length = 32) {
  return crypto
    .randomBytes(Math.ceil((Math.max(1, length) * 3) / 4))
    .toString("base64url")
    .slice(0, length);
}

function jsonForHtml(value) {
  return (JSON.stringify(value) ?? "null").replace(/[<>&\u2028\u2029]/g, (character) => {
    const code = character.charCodeAt(0).toString(16).padStart(4, "0");
    return `\\u${code}`;
  });
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function webviewDocument(webview, { title, body, styles = "", script = "" }) {
  const nonce = webviewNonce();

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
  <title>${escapeHtml(title)}</title>
  <style nonce="${nonce}">${styles}</style>
</head>
<body>
  ${body}
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    ${script}
  </script>
</body>
</html>`;
}

module.exports = {
  escapeHtml,
  jsonForHtml,
  webviewDocument,
  webviewNonce,
};
