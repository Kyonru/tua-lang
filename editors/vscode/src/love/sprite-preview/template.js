"use strict";

const { spritePreviewRuntime } = require("./runtime");

function spritePreviewHtml(webview, viewModel) {
  const nonce = webviewNonce();

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
  <title>Tua Sprite Preview</title>
  <style>
    :root {
      color-scheme: light dark;
      --canvas: var(--vscode-editor-background);
      --panel: var(--vscode-sideBar-background, var(--vscode-editor-background));
      --panel-raised: var(--vscode-editorWidget-background, var(--vscode-sideBar-background));
      --panel-border: color-mix(in srgb, var(--vscode-panel-border) 82%, transparent);
      --text: var(--vscode-foreground);
      --muted: var(--vscode-descriptionForeground);
      --quiet: color-mix(in srgb, var(--vscode-descriptionForeground) 72%, transparent);
      --accent: var(--vscode-button-background);
      --accent-text: var(--vscode-button-foreground);
      --accent-hover: var(--vscode-button-hoverBackground, var(--vscode-button-background));
      --input: var(--vscode-input-background);
      --input-border: var(--vscode-input-border, var(--vscode-panel-border));
      --focus: var(--vscode-focusBorder, var(--vscode-button-background));
      --danger: var(--vscode-errorForeground);
      --radius: 6px;
    }

    * {
      box-sizing: border-box;
    }

    body {
      margin: 0;
      color: var(--text);
      background: var(--canvas);
      font: 13px/1.45 var(--vscode-font-family);
      -webkit-font-smoothing: antialiased;
    }

    .shell {
      min-height: 100vh;
      display: grid;
      grid-template-rows: auto 1fr;
    }

    header {
      position: sticky;
      top: 0;
      z-index: 2;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      align-items: center;
      padding: 12px 16px;
      background: color-mix(in srgb, var(--canvas) 94%, transparent);
      border-bottom: 1px solid var(--panel-border);
    }

    h1 {
      margin: 0;
      font-size: 16px;
      line-height: 1.2;
      font-weight: 600;
    }

    .source {
      max-width: 70ch;
      margin-top: 2px;
      color: var(--muted);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 12px;
    }

    .summary {
      display: flex;
      gap: 6px;
      flex-wrap: wrap;
      justify-content: end;
    }

    .header-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      justify-content: end;
    }

    .pill {
      min-height: 24px;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 2px 8px;
      border: 1px solid var(--panel-border);
      border-radius: 999px;
      color: var(--muted);
      background: var(--panel);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    .sprites {
      display: grid;
      gap: 14px;
      padding: 16px;
    }

    .sprite {
      display: grid;
      gap: 12px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel);
      overflow: visible;
    }

    .sprite-head {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 12px;
      align-items: center;
      padding: 12px 14px 0;
    }

    .name {
      font-size: 15px;
      font-weight: 600;
      line-height: 1.25;
    }

    .path {
      margin-top: 2px;
      color: var(--muted);
      font-size: 12px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .meta,
    .error {
      color: var(--muted);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    .error {
      color: var(--danger);
    }

    .workbench {
      display: grid;
      grid-template-columns: 1fr;
      min-height: 360px;
      border-top: 1px solid var(--panel-border);
    }

    .stage {
      position: sticky;
      top: 64px;
      z-index: 1;
      display: grid;
      grid-template-rows: minmax(220px, 42vh) auto;
      gap: 12px;
      align-self: start;
      max-height: calc(100vh - 80px);
      padding: 14px;
      border-bottom: 1px solid var(--panel-border);
      background: var(--panel-raised);
    }

    .stage-frame {
      min-height: 220px;
      max-height: 420px;
      display: grid;
      place-items: center;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background:
        linear-gradient(45deg, rgba(127, 127, 127, .14) 25%, transparent 25%),
        linear-gradient(-45deg, rgba(127, 127, 127, .14) 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, rgba(127, 127, 127, .14) 75%),
        linear-gradient(-45deg, transparent 75%, rgba(127, 127, 127, .14) 75%);
      background-size: 20px 20px;
      background-position: 0 0, 0 10px, 10px -10px, -10px 0;
      overflow: auto;
      padding: 12px;
    }

    canvas {
      image-rendering: pixelated;
      image-rendering: crisp-edges;
    }

    .sprite-canvas {
      display: block;
      box-shadow: 0 0 0 1px rgba(127, 127, 127, .22);
      background: transparent;
    }

    .stage-readout {
      display: grid;
      gap: 6px;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .readout {
      min-width: 0;
      display: grid;
      gap: 1px;
      padding: 8px 10px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel);
    }

    .readout span:first-child {
      color: var(--muted);
      font-size: 11px;
    }

    .readout span:last-child {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 13px;
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }

    .inspector {
      display: grid;
      gap: 14px;
      align-content: start;
      padding: 14px;
    }

    .toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      justify-content: space-between;
    }

    .transport,
    .adjustments {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      align-items: center;
    }

    label {
      color: var(--muted);
      font-size: 12px;
    }

    button,
    input[type="range"] {
      font: inherit;
    }

    button {
      min-height: 30px;
      padding: 4px 10px;
      color: var(--text);
      background: var(--input);
      border: 1px solid var(--input-border);
      border-radius: 4px;
      cursor: pointer;
      transition: background-color 140ms ease-out, border-color 140ms ease-out, color 140ms ease-out, transform 100ms ease-out;
    }

    button:hover {
      border-color: var(--focus);
      background: color-mix(in srgb, var(--input) 72%, var(--accent) 28%);
    }

    button:focus-visible,
    input[type="range"]:focus-visible {
      outline: 1px solid var(--focus);
      outline-offset: 2px;
    }

    button:active {
      transform: scale(.98);
    }

    .primary {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .primary:hover {
      background: var(--accent-hover);
    }

    .animation-tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }

    .animation-tabs button {
      color: var(--muted);
    }

    .animation-tabs button.active {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .slider {
      min-width: 138px;
      display: grid;
      grid-template-columns: auto minmax(80px, 1fr) 34px;
      gap: 7px;
      align-items: center;
      color: var(--muted);
      font-size: 12px;
      font-variant-numeric: tabular-nums;
    }

    input[type="range"] {
      width: 100%;
      accent-color: var(--accent);
    }

    .metrics {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 6px;
    }

    .metric {
      min-width: 0;
      display: grid;
      gap: 2px;
      padding: 8px 10px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel-raised);
    }

    .metric span:first-child {
      color: var(--muted);
      font-size: 11px;
    }

    .metric span:last-child {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }

    .strip {
      display: grid;
      gap: 8px;
    }

    .strip-head,
    .sheet-head {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: center;
      color: var(--muted);
      font-size: 12px;
    }

    .timeline {
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
    }

    .frame-chip {
      min-width: 32px;
      min-height: 28px;
      padding: 3px 7px;
      border: 1px solid var(--panel-border);
      border-radius: 4px;
      text-align: center;
      color: var(--muted);
      background: var(--panel-raised);
      font-variant-numeric: tabular-nums;
    }

    .frame-chip.active {
      color: var(--accent-text);
      background: var(--accent);
      border-color: var(--accent);
    }

    .sheet {
      display: grid;
      gap: 8px;
    }

    .sheet-viewport {
      min-height: 96px;
      max-height: 260px;
      overflow: auto;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--panel-raised);
      padding: 10px;
    }

    .sheet-canvas {
      display: block;
    }

    .empty {
      margin: 0 14px 14px;
      padding: 12px;
      border: 1px solid color-mix(in srgb, var(--danger) 42%, var(--panel-border));
      border-radius: var(--radius);
      background: color-mix(in srgb, var(--danger) 8%, var(--panel));
    }

    .problems {
      display: grid;
      gap: 6px;
      margin: 0 14px;
    }

    .problem {
      padding: 8px 10px;
      border-left: 2px solid var(--danger);
      background: color-mix(in srgb, var(--danger) 7%, transparent);
      overflow-wrap: anywhere;
    }

    @media (max-width: 780px) {
      header,
      .sprite-head,
      .workbench {
        grid-template-columns: 1fr;
      }

      .summary {
        justify-content: start;
      }

      .stage {
        position: sticky;
        top: 70px;
      }

      .metrics {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }
    }

    @media (prefers-reduced-motion: reduce) {
      button {
        transition: none;
      }

      button:active {
        transform: none;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <header>
      <div>
        <h1>Sprite Animation Preview</h1>
        <div class="source">${escapeHtml(viewModel.source)}</div>
      </div>
      <div class="header-actions">
        <div class="summary" id="summary"></div>
        <button type="button" class="reload-preview" id="reload-preview">Reload</button>
      </div>
    </header>
    <main class="sprites" id="sprites"></main>
  </div>
  <script nonce="${nonce}">
${spritePreviewRuntime(viewModel.sprites)}
  </script>
</body>
</html>`;
}

function spritePreviewLoadingHtml(webview, source) {
  return spritePreviewStateHtml(
    webview,
    source,
    "Loading sprite metadata...",
    "The preview is waiting for the current language-server snapshot.",
  );
}

function spritePreviewErrorHtml(webview, source, message) {
  return spritePreviewStateHtml(
    webview,
    source,
    "Sprite preview unavailable",
    message,
  );
}

function spritePreviewStateHtml(webview, source, title, message) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline';">
  <title>Tua Sprite Preview</title>
  <style>
    body { margin: 0; padding: 24px; color: var(--vscode-foreground); background: var(--vscode-editor-background); font: 13px/1.5 var(--vscode-font-family); }
    main { max-width: 640px; }
    h1 { margin: 0 0 6px; font-size: 16px; }
    p { margin: 0; color: var(--vscode-descriptionForeground); }
    code { display: block; margin-top: 16px; overflow-wrap: anywhere; color: var(--vscode-textPreformat-foreground); }
  </style>
</head>
<body>
  <main>
    <h1>${escapeHtml(title)}</h1>
    <p>${escapeHtml(message)}</p>
    <code>${escapeHtml(source)}</code>
  </main>
</body>
</html>`;
}

function webviewNonce() {
  return Array.from({ length: 16 }, () => Math.floor(Math.random() * 36).toString(36)).join("");
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

module.exports = {
  spritePreviewErrorHtml,
  spritePreviewHtml,
  spritePreviewLoadingHtml,
};
