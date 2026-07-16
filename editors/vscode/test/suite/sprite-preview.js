"use strict";

const assert = require("assert/strict");
const vscode = require("vscode");
const {
  closeActiveEditor,
  eventSequence,
  hoverText,
  openFixture,
  positionAfter,
  previewTabs,
  waitForEvent,
  waitUntil,
} = require("./support");

async function runSpritePreviewTests() {
  const { document } = await openFixture("sprite");
  const beforeHover = eventSequence();
  const spritePosition = positionAfter(document, "local Play");
  const hovers = await vscode.commands.executeCommand(
    "vscode.executeHoverProvider",
    document.uri,
    spritePosition,
  );
  const text = hoverText(hovers);
  const previewLink = hovers
    .flatMap((hover) => hover.contents || [])
    .find(
      (content) =>
        typeof content !== "string" &&
        String(content.value || "").includes("command:tua.previewSprites"),
    );

  assert.match(text, /command:tua\.previewSprites/);
  assert.match(text, /Preview Player/);
  assert.deepEqual(previewLink.isTrusted, {
    enabledCommands: ["tua.previewSprites"],
  });
  const hoverInspection = await waitForEvent(
    (event) =>
      event.type === "inspect.request" &&
      event.uri === document.uri.toString() &&
      event.version === document.version,
    "sprite hover inspection at the current document version",
    beforeHover,
  );
  const beforePreview = hoverInspection.sequence;

  const opened = await vscode.commands.executeCommand("tua.previewSprites");
  assert.deepEqual(opened, {
    animationCount: 1,
    missingAssetCount: 0,
    spriteCount: 1,
  });
  await waitForEvent(
    (event) =>
      event.type === "inspect.request" &&
      event.uri === document.uri.toString() &&
      event.version === document.version,
    "sprite preview inspection at the current document version",
    beforePreview,
  );
  await waitUntil(
    () => previewTabs().length === 1 && previewTabs()[0].isActive,
    "one active sprite preview tab",
  );

  const edit = new vscode.WorkspaceEdit();
  const idleLine = document.lineAt(4);
  edit.insert(
    document.uri,
    idleLine.range.end,
    "\n    run = { from = 4, to = 9, fps = 12 },",
  );
  assert.equal(await vscode.workspace.applyEdit(edit), true);
  assert.equal(await document.save(), true);

  const reloaded = await vscode.commands.executeCommand("tua.previewSprites.reload");
  assert.deepEqual(reloaded, {
    animationCount: 2,
    missingAssetCount: 0,
    spriteCount: 1,
  });
  assert.equal(previewTabs().length, 1, "reload should keep one preview tab");
  assert(previewTabs()[0].isActive, "reload should reveal the preview as the active tab");

  const reopened = await vscode.commands.executeCommand("tua.previewSprites");
  assert.deepEqual(reopened, reloaded);
  assert.equal(previewTabs().length, 1, "reopening should reuse the existing preview tab");
  assert(previewTabs()[0].isActive, "reopening should keep the preview on top");

  const missing = await openFixture("missingSprite");
  const missingResult = await vscode.commands.executeCommand("tua.previewSprites");
  assert.deepEqual(missingResult, {
    animationCount: 1,
    missingAssetCount: 1,
    spriteCount: 1,
  });
  await waitUntil(
    () => previewTabs().length === 1 && previewTabs()[0].isActive,
    "asset errors to render in the visible existing preview",
  );

  await closeActiveEditor();
  await vscode.window.showTextDocument(missing.document);
  await closeActiveEditor();
}

module.exports = {
  runSpritePreviewTests,
};
