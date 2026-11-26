"use strict";

const { dialogueGraph } = require("./graph");
const { installReader } = require("./reader");
const { STYLES } = require("./styles");
const { displayUriPath } = require("../shared/locations");
const { escapeHtml, jsonForHtml, webviewDocument } = require("../shared/webview");
const DEFAULT_MOCK = '{\n  "context": {},\n  "locals": {},\n  "args": {}\n}';

function dialoguePreviewHtml(webview, uri, preview, message, mock = {}) {
  const ranges = [];
  const link = (label, range) => {
    const index = ranges.push(range) - 1;
    return `<button type="button" class="source" data-source="${index}">${escapeHtml(label)}</button>`;
  };
  const conditionResults = new Map();
  for (const condition of preview?.mock?.conditions || []) {
    const key = `${condition.section}:${condition.line ?? ""}:${condition.choice ?? ""}`;
    if (!conditionResults.has(key)) conditionResults.set(key, []);
    conditionResults.get(key).push(condition);
  }
  const resultsHtml = (section, line = "", choice = "") => {
    const results = conditionResults.get(`${section}:${line}:${choice}`) || [];
    const status = result => result.error ? "Evaluation error" : result.kind === "visibility"
      ? (result.value ? "Visible" : "Hidden by condition") : result.kind === "availability"
        ? (result.value ? "Available" : "Unavailable") : (result.value ? "Condition true" : "Condition false");
    return results.map(result => `<small class="${result.error ? "error" : "meta"}">${status(result)} · Mock ${escapeHtml(result.kind)}: ${result.error ? escapeHtml(result.error) : String(result.value)} · ${link("Open condition", result.range)}</small>`).join("");
  };
  const mockTexts = new Map();
  for (const text of preview?.mock?.text?.texts || []) {
    mockTexts.set(`${text.section}:${text.line}:${text.choice ?? ""}:${text.kind}`, text);
  }
  const textHtml = (authored, section, line, choice, kind) => {
    const text = mockTexts.get(`${section}:${line}:${choice ?? ""}:${kind}`);
    if (text?.error) {
      return `${escapeHtml(authored)}<small class="error">Mock text: ${escapeHtml(text.error)} · ${link("Open source", text.range)}</small>`;
    }
    if (typeof text?.value === "string") {
      const original = text.value !== authored
        ? `<details><summary>Authored text</summary><div class="authored">${escapeHtml(authored)}</div></details>`
        : "";
      return `<span class="mock-value">${escapeHtml(text.value)}</span>${original}`;
    }
    return escapeHtml(authored) + (preview?.mock?.text?.truncated
      ? '<small class="meta">Authored text (preview limit)</small>' : "");
  };
  const sections = preview?.sections || [];
  const shown = sections.slice(0, 200);
  const speakers = new Map();
  for (const section of shown) {
    for (const line of section.lines.slice(0, 100)) {
      if (line.speaker && !speakers.has(line.speaker)) speakers.set(line.speaker, speakers.size);
    }
  }
  const destinations = new Map();
  sections.forEach((section, index) => {
    const matches = destinations.get(section.label) || [];
    matches.push(index);
    destinations.set(section.label, matches);
  });
  const routes = new Map();
  for (const route of preview?.routes || []) {
    if (!routes.has(route.section)) routes.set(route.section, []);
    routes.get(route.section).push(route);
  }
  const labels = { fallback: "No available choices", wait: "After wait", branchTrue: "Condition true", branchFalse: "Condition false", actionSuccess: "Action succeeds", actionFailure: "Action fails" };
  const content = shown.map((section, index) => {
    const outgoing = routes.get(index) || [];
    const renderRoute = route => {
      const choice = section.lines[route.line]?.choices?.[route.choice];
      const label = choice ? textHtml(choice.text, index, route.line, route.choice, "choice") : escapeHtml(labels[route.kind] || route.kind);
      const matches = destinations.get(route.target) || [];
      const destination = matches.length === 1 && matches[0] < shown.length ? matches[0] : undefined;
      const issue = matches.length > 1 ? "Ambiguous destination" : matches.length === 0 ? "Missing destination" : "Destination outside preview limit";
      const conditions = choice ? [choice.visibleCondition && "Visibility condition", choice.enabledCondition && "Availability condition"].filter(Boolean).join(" · ") : "";
      // Mock text can contain expandable authored text. Keep it outside the navigation button.
      return `<li class="reply"><div class="reply-text">${label}</div>
        <div class="reply-actions"><button type="button" ${destination === undefined ? 'disabled' : `data-follow="${destination}"`}>→ ${escapeHtml(route.target)}</button>${link("Source", route.range)}</div>
        ${destination === undefined ? `<small class="error">${issue}</small>` : ""}
        ${conditions && !conditionResults.has(`${index}:${route.line}:${route.choice}`) ? `<small class="meta">${conditions} (not evaluated)</small>` : ""}
        ${choice ? resultsHtml(index, route.line, route.choice) : ""}
        ${choice?.reason ? `<div class="reason">${textHtml(choice.reason, index, route.line, route.choice, "reason")}</div>` : ""}</li>`;
    };
    const routeList = entries => entries.length ? `<ol class="replies" aria-label="Outgoing routes">${entries.map(renderRoute).join("")}</ol>` : "";
    const shownRoutes = outgoing.slice(0, 200);
    const lines = section.lines.slice(0, 100).map((line, lineIndex) => {
      const speaker = speakers.get(line.speaker);
      const classes = speaker === undefined ? "narration" : `bubble side-${speaker % 2} speaker-${speaker % 4}`;
      return `<div class="utterance"><div class="${classes}">
        ${line.speaker ? `<strong class="speaker">${escapeHtml(line.speaker)}</strong>` : ""}
        <div class="speech">${textHtml(line.text, index, lineIndex, null, "line")}</div>
        <div class="line-source">${link("Line " + (line.range.start.line + 1), line.range)}${line.end ? '<span class="meta">Ends conversation</span>' : ""}</div>
        </div>${routeList(shownRoutes.filter(route => route.line === lineIndex))}</div>`;
    }).join("");
    const remaining = shownRoutes.filter(route => !Number.isInteger(route.line) || route.line < 0 || route.line >= Math.min(section.lines.length, 100));
    const limited = section.lines.length > 100 || outgoing.length > 200;
    return `<section class="conversation-section" data-section="${index}" ${index ? "hidden" : ""}>
      <div class="section-heading"><h2 tabindex="-1">${escapeHtml(section.label)}</h2>${link("Open source", section.range)}${index === 0 ? '<span class="meta">Default entry</span>' : ""}</div>
      ${resultsHtml(index)}${section.action ? `<p class="stage-direction">Action · <code>${escapeHtml(section.action)}</code></p>` : ""}
      ${section.wait != null ? `<p class="stage-direction">Wait · ${escapeHtml(section.wait)} seconds</p>` : ""}
      ${lines}${routeList(remaining)}
      ${!section.lines.length && !outgoing.length ? '<p class="meta">No dialogue lines or outgoing routes in this section.</p>' : ""}
      ${limited ? '<p class="meta">Large section: showing the first 100 lines and 200 routes. Open source for the full section.</p>' : ""}</section>`;
  }).join("");
  const graph = message ? { html: "", styles: "", script: "" } : dialogueGraph(preview, link, uri);
  return webviewDocument(webview, {
    title: "Dialogue Preview",
    styles: graph.styles + STYLES,
    body: `<header><div><h1>Dialogue preview</h1><p class="file-path">${escapeHtml(displayUriPath(uri))}</p></div><button type="button" id="reload">Reload</button></header>
      <div class="meta preview-note">Explore authored branches. No game actions or audio run. <details class="preview-help"><summary>Preview limits</summary><p>Text previews are plain, without localization or rich styling. Numeric spelling may differ from Lua. Conditions are tested independently against applied JSON; following a branch does not update context.</p></details></div>
      <details class="mock-controls" ${mock.error ? "open" : ""}><summary>Test conditions <span id="mock-status" class="meta">${preview?.mock ? "Applied mock context" : "Authored view"}</span></summary>
      <label for="mock-context">Mock context (JSON)</label><textarea id="mock-context" maxlength="65536" spellcheck="false" aria-describedby="mock-help">${escapeHtml(mock.draft ?? DEFAULT_MOCK)}</textarea>
      <p id="mock-help" class="meta">Use context and locals objects, plus optional scalar args. Missing fields and null mean nil. Changes apply only when you choose Apply mock context. Input stays in memory and clears when this preview closes.</p>
      <div class="mock-actions"><button id="apply-mock" type="button">Apply mock context</button><button id="clear-mock" type="button">Use authored view</button></div>
      ${mock.error ? `<p class="error" role="alert">${escapeHtml(mock.error)}</p>` : ""}</details>
      ${preview?.mock?.text?.bindingError ? `<p class="error">Mock text unavailable: ${escapeHtml(preview.mock.text.bindingError.message)}. Authored text is shown. ${link("Open source", preview.mock.text.bindingError.range)}</p>` : ""}
      ${shown.length && !message ? `<div class="reader-toolbar"><div role="group" aria-label="Preview view"><button id="view-read" type="button" aria-pressed="true">Read</button><button id="view-flow" type="button" aria-pressed="false">Flow</button></div><button id="reader-back" type="button">Back</button><button id="reader-restart" type="button">Restart from section</button><label class="section-picker">Section <select id="section-picker">${shown.map((section, index) => `<option value="${index}">${escapeHtml(section.label)}</option>`).join("")}</select></label></div>
      <nav id="reader-history" aria-label="Visited sections"></nav>` : ""}
      <div class="reader-layout"><aside class="section-list" aria-label="Dialogue sections">${!message ? shown.map((section, index) => `<button type="button" data-select-section="${index}">${escapeHtml(section.label)}</button>`).join("") : ""}</aside>
      <div id="flow-pane" hidden>${graph.html}</div>
      <main>${message ? `<p role="status">${escapeHtml(message)}</p>` : content || '<p>No dialogue sections found.</p>'}${preview?.mock?.text?.truncated ? '<p>Text preview limit reached (4,096 fields or 1 MiB combined output). Remaining text stays authored.</p>' : ""}${preview?.mock?.truncated ? '<p>Only the first 4,096 conditions were tested.</p>' : ""}${sections.length > 200 ? '<p>Showing the first 200 sections. Open source for the full conversation.</p>' : ""}</main></div>`,
    script: `${graph.script}
      (${installReader.toString()})(${jsonForHtml(uri)}, ${jsonForHtml(shown.map(section => section.label))}, ${jsonForHtml(mock.applied ?? null)}, ${jsonForHtml(Boolean(preview?.mock))});
      const ranges = ${jsonForHtml(ranges)};
      const uri = ${jsonForHtml(uri)};
      const input = document.getElementById("mock-context");
      input.addEventListener("input", () => vscode.postMessage({command:"mockDraft", uri, text:input.value}));
      document.getElementById("apply-mock").addEventListener("click", () => vscode.postMessage({command:"applyMock", uri, text:input.value}));
      document.getElementById("clear-mock").addEventListener("click", () => vscode.postMessage({command:"clearMock", uri}));
      document.getElementById("reload").addEventListener("click", () => vscode.postMessage({command:"reload"}));
      document.querySelectorAll("[data-source]").forEach(button => button.addEventListener("click", () => {
        vscode.postMessage({command:"openLocation", uri:${jsonForHtml(uri)}, range:ranges[Number(button.dataset.source)]});
      }));`,
  });
}

module.exports = { dialoguePreviewHtml, DEFAULT_MOCK };
