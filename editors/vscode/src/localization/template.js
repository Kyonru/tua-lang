"use strict";

function localizationHtml(webview, nonce) {
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tua Localization</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'nonce-${nonce}'; script-src 'nonce-${nonce}';">
<style nonce="${nonce}">
:root { color-scheme: light dark; }
body { margin: 0; padding: 20px; color: var(--vscode-foreground); background: var(--vscode-editor-background); font: var(--vscode-font-size) var(--vscode-font-family); }
header { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
h1 { font-size: 1.4rem; margin: 0 0 12px; }
h2 { font-size: 1.1rem; margin: 0 0 10px; }
.tools { display: flex; gap: 8px; margin: 0 0 14px; }
input, button { font: inherit; color: inherit; }
input { flex: 1; min-width: 0; padding: 6px 8px; border: 1px solid var(--vscode-input-border, var(--vscode-panel-border)); background: var(--vscode-input-background); color: var(--vscode-input-foreground); }
.count-input { max-width: 75px; }
button { cursor: pointer; padding: 5px 9px; border: 1px solid var(--vscode-button-border, var(--vscode-panel-border)); background: var(--vscode-button-secondaryBackground); color: var(--vscode-button-secondaryForeground); }
button:hover { background: var(--vscode-button-secondaryHoverBackground); }
button:focus-visible, input:focus-visible { outline: 2px solid var(--vscode-focusBorder); outline-offset: 2px; }
.layout { display: grid; grid-template-columns: minmax(300px, 1fr) minmax(250px, .8fr); gap: 16px; align-items: start; }
.table-wrap { overflow: auto; max-height: calc(100vh - 180px); border: 1px solid var(--vscode-panel-border); }
table { border-collapse: collapse; width: 100%; }
th, td { padding: 6px 8px; text-align: left; border-bottom: 1px solid var(--vscode-panel-border); white-space: nowrap; }
th { position: sticky; top: 0; background: var(--vscode-editor-background); z-index: 1; }
td button { width: 100%; text-align: left; border: 0; background: transparent; padding: 3px; }
tr[aria-selected="true"] { background: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }
.status { font-size: .9em; } .status[data-state="translated"] { color: var(--vscode-testing-iconPassed); }
.status[data-state="missing"] { color: var(--vscode-errorForeground); }
.status[data-state="fallback"], .status[data-state="dynamic"] { color: var(--vscode-editorWarning-foreground); }
.detail { border-top: 2px solid var(--vscode-panel-border); padding-top: 12px; }
.locale { border-bottom: 1px solid var(--vscode-panel-border); padding: 10px 0; }
.locale h3 { margin: 0 0 6px; font-size: 1em; }
.sample { white-space: pre-wrap; overflow-wrap: anywhere; margin: 7px 0; }
.small { color: var(--vscode-descriptionForeground); font-size: .9em; }
.pager { display: flex; align-items: center; gap: 8px; margin-top: 10px; }
@media (max-width: 760px) { body { padding: 12px; } .layout { grid-template-columns: 1fr; } .table-wrap { max-height: 45vh; } }
</style></head><body>
<header><h1>Localization</h1><span id="summary" class="small" aria-live="polite">Loading catalogs…</span></header>
<div class="tools"><input id="search" type="search" aria-label="Find translation key" placeholder="Find translation key"><button id="refresh" type="button">Refresh</button></div>
<div class="layout"><section aria-label="Translation coverage"><div class="table-wrap"><table id="table"><thead></thead><tbody></tbody></table></div><div class="pager"><button id="previous" type="button">Previous</button><span id="range" class="small"></span><button id="next" type="button">Next</button></div></section><section class="detail" aria-label="Selected translation"><h2 id="selected">Choose a key</h2><div class="tools"><input id="sample" aria-label="Sample placeholder value" value="Ada"><input id="count" class="count-input" type="number" aria-label="Sample count" value="2"></div><p class="small">Source-text preview with sample arguments. The Lua runtime chooses plural categories and applies formatting.</p><div id="details" class="small">Select a row to compare its locale entries and open the source.</div></section></div>
<script nonce="${nonce}">
const vscode = acquireVsCodeApi();
let page, selected, timer;
const $ = id => document.getElementById(id);
function element(tag, text, className) { const node = document.createElement(tag); if (text != null) node.textContent = text; if (className) node.className = className; return node; }
function setStatus(text) { $('summary').textContent = text; }
function sample(text) { return String(text).replace(/%\{([^}]+)\}/g, (_, name) => name === 'count' ? $('count').value : $('sample').value); }
function select(key) {
  selected = key; $('selected').textContent = key;
  const row = page.rows.find(item => item.key === key);
  const detail = $('details'); detail.replaceChildren();
  if (!row) return;
  for (const cell of row.cells) {
    const block = element('div', null, 'locale');
    const heading = element('h3', cell.locale + ' · ' + cell.status); block.append(heading);
    if (cell.preview != null) block.append(element('p', sample(cell.preview), 'sample'));
    for (const [form, value] of cell.forms || []) block.append(element('p', form + ': ' + sample(value), 'sample'));
    if (cell.placeholders?.length) block.append(element('p', 'Arguments: ' + cell.placeholders.join(', '), 'small'));
    if (cell.status === 'dynamic') block.append(element('p', 'Calculated by Lua at game runtime.', 'small'));
    if (cell.uri) {
      const open = element('button', 'Open catalog entry'); open.type = 'button';
      open.addEventListener('click', () => vscode.postMessage({ command: 'openLocation', uri: cell.uri, range: cell.range }));
      block.append(open);
    }
    detail.append(block);
  }
  for (const tr of $('table').tBodies[0].rows) tr.setAttribute('aria-selected', tr.dataset.key === key);
}
function render(data) {
  page = data;
  if (!data.enabled) { setStatus('Enable [i18n] in tua.toml to inspect catalogs.'); $('table').replaceChildren(); $('details').textContent = 'Localization is disabled for this project.'; return; }
  setStatus(data.total + ' keys · ' + data.locales.length + ' locales');
  const head = $('table').tHead || $('table').createTHead(); head.replaceChildren();
  const headings = element('tr'); headings.append(element('th', 'Key'));
  for (const locale of data.locales) headings.append(element('th', locale)); head.append(headings);
  const body = $('table').tBodies[0] || $('table').createTBody(); body.replaceChildren();
  for (const row of data.rows) {
    const tr = element('tr'); tr.dataset.key = row.key; tr.setAttribute('aria-selected', row.key === selected);
    const keyCell = element('td'); const keyButton = element('button', row.key); keyButton.type = 'button'; keyButton.addEventListener('click', () => select(row.key)); keyCell.append(keyButton); tr.append(keyCell);
    for (const cell of row.cells) { const td = element('td'); const state = element('span', cell.status, 'status'); state.dataset.state = cell.status; td.append(state); tr.append(td); }
    body.append(tr);
  }
  $('range').textContent = data.total ? (data.offset + 1) + '–' + (data.offset + data.rows.length) + ' of ' + data.total : 'No keys';
  $('previous').disabled = data.offset <= 0; $('next').disabled = data.offset + data.rows.length >= data.total;
  if (selected && data.rows.some(row => row.key === selected)) select(selected);
  else if (data.rows.length) select(data.rows[0].key);
  else { $('selected').textContent = 'No matching key'; $('details').textContent = 'Change the search or refresh the catalogs.'; }
}
window.addEventListener('message', event => { if (event.data?.type === 'page') render(event.data.page); else if (event.data?.type === 'error') setStatus(event.data.message); });
$('refresh').addEventListener('click', () => vscode.postMessage({ type: 'refresh' }));
$('search').addEventListener('input', event => { clearTimeout(timer); const value = event.target.value; timer = setTimeout(() => vscode.postMessage({ type: 'search', value }), 180); });
$('previous').addEventListener('click', () => vscode.postMessage({ type: 'page', offset: Math.max(0, page.offset - 100) }));
$('next').addEventListener('click', () => vscode.postMessage({ type: 'page', offset: page.offset + 100 }));
for (const id of ['sample', 'count']) $(id).addEventListener('input', () => { if (selected && page) select(selected); });
vscode.postMessage({ type: 'ready' });
</script></body></html>`;
}

module.exports = { localizationHtml };
