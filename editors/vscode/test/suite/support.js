"use strict";

const fs = require("fs");
const vscode = require("vscode");

const fixtures = JSON.parse(process.env.TUA_TEST_FIXTURES);

function readEvents() {
  try {
    const text = fs.readFileSync(process.env.TUA_TEST_LSP_LOG, "utf8").trim();

    return text
      ? text
          .split("\n")
          .filter(Boolean)
          .map((line) => JSON.parse(line))
      : [];
  } catch (error) {
    if (error.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}

function eventSequence() {
  return readEvents().at(-1)?.sequence || 0;
}

async function waitForEvent(predicate, description, afterSequence = 0) {
  let matched;
  await waitUntil(() => {
    matched = readEvents().find(
      (event) => event.sequence > afterSequence && predicate(event),
    );
    return Boolean(matched);
  }, description);
  return matched;
}

async function waitUntil(predicate, description, timeoutMs = 7000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) {
      return;
    }
    await delay(25);
  }
  throw new Error(`Timed out waiting for ${description}. Events: ${JSON.stringify(readEvents())}`);
}

async function openFixture(name, options = {}) {
  const document = await vscode.workspace.openTextDocument(fixtures[name]);
  const editor = await vscode.window.showTextDocument(document, {
    preview: false,
    ...options,
  });

  return { document, editor };
}

async function closeActiveEditor() {
  await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
}

async function discardActiveEditor() {
  await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
}

function hoverText(hovers) {
  return (hovers || [])
    .flatMap((hover) => hover.contents || [])
    .map((content) => (typeof content === "string" ? content : content.value || ""))
    .join("\n");
}

function previewTabs() {
  return vscode.window.tabGroups.all
    .flatMap((group) => group.tabs)
    .filter(
      (tab) =>
        tab.input instanceof vscode.TabInputWebview &&
        String(tab.input.viewType || "").endsWith("tuaSpritePreview"),
    );
}

function moduleGraphTabs() {
  return vscode.window.tabGroups.all
    .flatMap((group) => group.tabs)
    .filter(
      (tab) =>
        tab.input instanceof vscode.TabInputWebview &&
        String(tab.input.viewType || "").endsWith("tuaModuleGraph"),
    );
}

function positionAfter(document, needle, occurrence = 1) {
  const text = document.getText();
  let offset = -1;

  for (let index = 0; index < occurrence; index += 1) {
    offset = text.indexOf(needle, offset + 1);
  }

  if (offset < 0) {
    throw new Error(`Could not find ${JSON.stringify(needle)} in ${document.uri}`);
  }

  return document.positionAt(offset + needle.length);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

module.exports = {
  closeActiveEditor,
  delay,
  discardActiveEditor,
  eventSequence,
  fixtures,
  hoverText,
  moduleGraphTabs,
  openFixture,
  positionAfter,
  previewTabs,
  readEvents,
  waitForEvent,
  waitUntil,
};
