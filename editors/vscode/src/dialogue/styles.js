"use strict";

const STYLES = `
  :root {
    --dialogue-space: 8px;
    --dialogue-rule: var(--vscode-contrastBorder, var(--vscode-panel-border));
    --dialogue-paper: var(--vscode-editorWidget-background, var(--vscode-editor-background));
  }
  * { box-sizing: border-box; }
  [hidden] { display: none !important; }
  body { margin: 0; padding: 20px; color: var(--vscode-foreground); background: var(--vscode-editor-background); font: var(--vscode-font-size) var(--vscode-font-family); }
  header { display: flex; justify-content: space-between; align-items: start; gap: 16px; }
  h1 { font-size: 1.3em; margin: 0; } h2 { font-size: 1.15em; margin: 0; overflow-wrap: anywhere; }
  p { line-height: 1.6; max-width: 75ch; }
  .file-path { color: var(--vscode-descriptionForeground); overflow-wrap: anywhere; margin: 6px 0; }
  .meta, small { color: var(--vscode-descriptionForeground); }
  button, select, textarea { font: inherit; }
  button { cursor: pointer; color: var(--vscode-button-secondaryForeground, var(--vscode-foreground)); background: var(--vscode-button-secondaryBackground, var(--dialogue-paper)); border: 1px solid var(--vscode-contrastBorder, transparent); padding: 6px 10px; }
  button:hover { background: var(--vscode-button-secondaryHoverBackground, var(--vscode-list-hoverBackground)); }
  button:disabled { cursor: default; color: var(--vscode-disabledForeground); }
  button[aria-pressed=true], button[aria-current=true] { border-color: var(--vscode-focusBorder); }
  button:focus-visible, select:focus-visible, textarea:focus-visible, summary:focus-visible, h2:focus { outline: 2px solid var(--vscode-focusBorder); outline-offset: 2px; }
  .source { background: none; color: var(--vscode-textLink-foreground); padding: 2px 0; border: 0; text-align: left; }
  .source:hover { background: none; color: var(--vscode-textLink-activeForeground); text-decoration: underline; }
  .preview-note { margin: 12px 0; line-height: 1.6; }
  .preview-help { display: inline; } summary { cursor: pointer; }
  .preview-help summary { display: inline; text-decoration: underline; }
  .mock-controls { margin: 16px 0; border-block: 1px solid var(--dialogue-rule); padding: 12px 0; }
  .mock-controls > summary { font-weight: 600; }
  #mock-status { font-weight: normal; margin-left: 12px; }
  .mock-controls label { display: block; margin-top: 16px; }
  textarea { display: block; width: 100%; max-width: 75ch; min-height: 110px; margin: 8px 0; padding: 8px; resize: vertical; color: var(--vscode-input-foreground); background: var(--vscode-input-background); border: 1px solid var(--vscode-input-border, var(--dialogue-rule)); font-family: var(--vscode-editor-font-family); }
  .mock-actions, .reader-toolbar, .reply-actions, .section-heading, .line-source { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .reader-toolbar { padding: 8px 0; }
  .reader-toolbar > [role=group] { display: flex; gap: 4px; }
  .section-picker { display: none; }
  select { max-width: 100%; color: var(--vscode-dropdown-foreground); background: var(--vscode-dropdown-background); border: 1px solid var(--vscode-dropdown-border, var(--dialogue-rule)); padding: 6px; }
  #reader-history { display: flex; gap: 4px; overflow-x: auto; padding: 8px 0 16px; }
  #reader-history button { white-space: nowrap; background: none; font-size: .9em; }
  #reader-history button + button::before { content: '›'; margin-right: 8px; color: var(--vscode-descriptionForeground); }
  .reader-layout { display: grid; grid-template-columns: minmax(120px, 170px) minmax(0, 1fr); gap: 24px; }
  .section-list { display: flex; flex-direction: column; gap: 4px; align-self: start; max-height: 70vh; overflow: auto; }
  .section-list button { text-align: left; overflow-wrap: anywhere; background: none; }
  main { min-width: 0; max-width: 85ch; width: 100%; margin-inline: auto; }
  .section-heading { margin: 4px 0 24px; }
  .section-heading .source { margin-left: auto; }
  .utterance { margin: 0 0 24px; }
  .bubble { position: relative; max-width: 88%; padding: 16px 20px 12px; background: var(--dialogue-paper); border: 1px solid var(--dialogue-rule); border-left: 3px solid var(--speaker-accent); }
  .bubble::after { content: ''; position: absolute; bottom: -7px; left: 18px; width: 12px; height: 12px; background: var(--dialogue-paper); border-right: 1px solid var(--dialogue-rule); border-bottom: 1px solid var(--dialogue-rule); transform: rotate(45deg); }
  .side-1 { margin-left: auto; border-left: 1px solid var(--dialogue-rule); border-right: 3px solid var(--speaker-accent); }
  .side-1::after { left: auto; right: 18px; }
  .speaker-0 { --speaker-accent: var(--vscode-charts-green, var(--vscode-focusBorder)); }
  .speaker-1 { --speaker-accent: var(--vscode-charts-purple, var(--vscode-focusBorder)); }
  .speaker-2 { --speaker-accent: var(--vscode-charts-orange, var(--vscode-focusBorder)); }
  .speaker-3 { --speaker-accent: var(--vscode-charts-blue, var(--vscode-focusBorder)); }
  .speaker-heading { display: flex; align-items: center; gap: 12px; }
  .side-1 .speaker-heading { flex-direction: row-reverse; }
  .portrait { object-fit: contain; flex: 0 0 56px; background: var(--vscode-editor-background); border: 1px solid var(--dialogue-rule); }
  .portrait-fallback { font-size: .85em; color: var(--vscode-descriptionForeground); overflow-wrap: anywhere; }
  .rich-highlight { background: var(--vscode-editor-findMatchHighlightBackground); }
  .rich-underline { text-decoration: underline; }
  .rich-strike { text-decoration: line-through; }
  .rich-thought { font-style: italic; }
  .rich-whisper { color: var(--vscode-descriptionForeground); }
  .rich-warning { text-decoration: underline wavy var(--vscode-editorWarning-foreground); }
  .rich-success { text-decoration: underline var(--vscode-charts-green); }
  .rich-radio { font-family: var(--vscode-editor-font-family); }
  .rich-metadata, .rich-style:not([class*="rich-style rich-"]) { text-decoration: underline dotted; text-underline-offset: 3px; }
  .rich-annotation { display: inline-block; margin-inline: 4px; padding: 0 4px; font: .8em var(--vscode-font-family); color: var(--vscode-descriptionForeground); border: 1px solid var(--dialogue-rule); }
  .speaker { display: block; font-size: .9em; }
  .speech { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.75; margin: 6px 0 10px; }
  .line-source { font-size: .85em; justify-content: space-between; }
  .narration, .stage-direction { text-align: center; margin: 16px auto; max-width: 65ch; color: var(--vscode-descriptionForeground); }
  .narration .line-source { justify-content: center; }
  .replies { list-style: none; padding: 0; margin: 20px 0 0 24px; border-left: 1px solid var(--dialogue-rule); }
  .reply { padding: 8px 12px; margin: 0 0 8px; overflow-wrap: anywhere; }
  .reply-text { line-height: 1.6; margin-bottom: 6px; white-space: pre-wrap; }
  small { display: block; margin-top: 6px; }
  .error { color: var(--vscode-errorForeground); }
  .authored, .mock-value { white-space: pre-wrap; overflow-wrap: anywhere; }
  .speech details { white-space: normal; font-size: .9em; }
  .speech summary { color: var(--vscode-descriptionForeground); }
  code { font-family: var(--vscode-editor-font-family); }
  .reason { font-size: .9em; margin-top: 6px; }
  .flow-view { grid-template-columns: minmax(0, 1fr) minmax(260px, 1fr); }
  .flow-view .section-list { display: none; }
  .dialogue-map { margin: 0; }
  .dialogue-map > summary { display: none; }
  .dialogue-node { border-color: var(--dialogue-rule); }
  .dialogue-node [aria-current=true] { color: var(--vscode-textLink-foreground); }
  .dialogue-node button[data-select-section] { display: block; width: 100%; text-align: left; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; padding: 0; background: none; }
  .dialogue-node small { margin-top: 4px; }
  .dialogue-node small .source { display: inline; width: auto; }
  .dialogue-route-list { padding-left: 20px; }
  .dialogue-route-list li { padding: 6px 0; overflow-wrap: anywhere; }
  @media (max-width: 1000px) { .flow-view { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 650px) {
    body { padding: 12px; } header { flex-wrap: wrap; } .reader-layout { grid-template-columns: minmax(0, 1fr); }
    .section-list { display: none; } .section-picker { display: flex; gap: 8px; align-items: center; min-width: 0; }
    .section-picker select { min-width: 0; } .bubble { max-width: 95%; padding: 12px; }
    .replies { margin-left: 12px; } .section-heading { gap: 8px; }
  }
`;

module.exports = { STYLES };
