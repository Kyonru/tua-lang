"use strict";

const assert = require("assert/strict");
const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const test = require("node:test");

const {
  InlineReplayCapture,
  MAX_TRACKED_EDITS,
  createInlineReplayCapture,
  replaySourceHash,
} = require("../../src/editor/inline-replay-capture");

function collectingWriter(lines) {
  return (line) => {
    lines.push(line);
    return Buffer.byteLength(line, "utf8");
  };
}

test("capture is absent unless both developer environment variables are set", () => {
  assert.equal(createInlineReplayCapture({ env: {} }), undefined);
  assert.equal(
    createInlineReplayCapture({
      env: { TUA_INLINE_REPLAY_ROOT: "/dataset" },
    }),
    undefined,
  );
});

test("capture writes versioned local menu replay events without document bodies", () => {
  const root = path.resolve("/dataset");
  const source = "local secret = player.velocity\n";
  const lines = [];
  const times = [100, 167];
  const capture = new InlineReplayCapture({
    datasetRoot: root,
    now: () => times.shift(),
    resolveRealPath: (value) => path.resolve(value),
    writeLine: collectingWriter(lines),
  });
  const document = replayDocument(path.join(root, "project", "main.tua"), source);
  capture.noteEdit(document);
  capture.record(
    document,
    { line: 0, character: 12 },
    {
      selectedCompletionInfo: {
        text: "player",
        range: {
          start: { line: 0, character: 7 },
          end: { line: 0, character: 13 },
        },
      },
    },
  );

  assert.equal(lines.length, 1);
  assert(!lines[0].includes(source));
  assert.deepEqual(JSON.parse(lines[0]), {
    schemaVersion: 1,
    datasetPath: "project/main.tua",
    sourceHash: replaySourceHash(source),
    position: { line: 0, character: 12 },
    documentVersion: 4,
    elapsedSinceEditMs: 67,
    menuOpen: true,
    selectedCompletion: {
      text: "player",
      range: {
        start: { line: 0, character: 7 },
        end: { line: 0, character: 13 },
      },
    },
  });
});

test("capture skips files outside the declared dataset root", () => {
  const lines = [];
  const capture = new InlineReplayCapture({
    datasetRoot: path.resolve("/dataset"),
    resolveRealPath: (value) => path.resolve(value),
    writeLine: collectingWriter(lines),
  });

  capture.record(
    replayDocument(path.resolve("/private/main.tua"), "print('private')"),
    { line: 0, character: 3 },
    {},
  );

  assert.deepEqual(lines, []);
});

test("capture without an observed edit records an unknown delay", () => {
  const lines = [];
  const capture = new InlineReplayCapture({
    datasetRoot: path.resolve("/dataset"),
    resolveRealPath: (value) => path.resolve(value),
    writeLine: collectingWriter(lines),
  });

  capture.record(
    replayDocument(path.resolve("/dataset/main.tua"), "print(value)"),
    { line: 0, character: 5 },
    {},
  );

  const event = JSON.parse(lines[0]);
  assert.equal(event.elapsedSinceEditMs, null);
  assert.equal(event.menuOpen, false);
  assert.equal(event.selectedCompletion, undefined);
});

test("factory capture hashes an unsaved edit instead of the dataset file", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inline-replay-"));
  const sourcePath = path.join(root, "main.tua");
  const outputPath = path.join(root, "capture", "menu.jsonl");
  const datasetSource = "local value = player.velocity\n";
  const editedSource = "local value = player.vel";
  fs.writeFileSync(sourcePath, datasetSource);

  try {
    const capture = createInlineReplayCapture({
      env: {
        TUA_INLINE_REPLAY_ROOT: root,
        TUA_INLINE_REPLAY_CAPTURE: outputPath,
      },
    });
    assert(capture);
    capture.noteEdit(replayDocument(sourcePath, editedSource));
    capture.record(
      replayDocument(sourcePath, editedSource),
      { line: 0, character: editedSource.length },
      {},
    );
    capture.dispose();

    const event = JSON.parse(fs.readFileSync(outputPath, "utf8"));
    assert.equal(event.datasetPath, "main.tua");
    assert.equal(
      event.sourceHash,
      crypto.createHash("sha256").update(editedSource, "utf8").digest("hex"),
    );
    assert.notEqual(
      event.sourceHash,
      crypto.createHash("sha256").update(datasetSource, "utf8").digest("hex"),
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test(
  "factory capture restricts an existing output file to its owner",
  { skip: process.platform === "win32" },
  () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inline-replay-"));
    const outputPath = path.join(root, "menu.jsonl");
    fs.writeFileSync(outputPath, "", { mode: 0o644 });
    fs.chmodSync(outputPath, 0o644);

    try {
      const capture = createInlineReplayCapture({
        env: {
          TUA_INLINE_REPLAY_ROOT: root,
          TUA_INLINE_REPLAY_CAPTURE: outputPath,
        },
      });
      assert(capture);
      capture.dispose();
      assert.equal(fs.statSync(outputPath).mode & 0o777, 0o600);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  },
);

test("capture hashes CRLF exactly like the evaluator's normalized scenario", () => {
  const crlf = "local value = player.vel\r\nreturn value\r\n";
  const lf = "local value = player.vel\nreturn value\n";
  assert.equal(replaySourceHash(crlf), replaySourceHash(lf));
  assert.equal(
    replaySourceHash(crlf),
    crypto.createHash("sha256").update(lf, "utf8").digest("hex"),
  );
});

test("a partial write closes capture immediately and only once", () => {
  let closes = 0;
  const errors = [];
  const capture = new InlineReplayCapture({
    datasetRoot: path.resolve("/dataset"),
    dispose: () => closes++,
    onError: (message) => errors.push(message),
    resolveRealPath: (value) => path.resolve(value),
    writeLine: () => 1,
  });

  capture.record(
    replayDocument(path.resolve("/dataset/main.tua"), "print(value)"),
    { line: 0, character: 5 },
    {},
  );
  capture.dispose();

  assert.equal(capture.failed, true);
  assert.equal(closes, 1);
  assert.equal(errors.length, 1);
});

test("edit tracking is root-scoped and bounded", () => {
  const capture = new InlineReplayCapture({
    datasetRoot: path.resolve("/dataset"),
    resolveRealPath: (value) => path.resolve(value),
    writeLine: () => 0,
  });
  capture.noteEdit(replayDocument(path.resolve("/private/outside.tua"), ""));
  for (let index = 0; index <= MAX_TRACKED_EDITS; index++) {
    capture.noteEdit(replayDocument(path.resolve(`/dataset/${index}.tua`), ""));
  }

  assert.equal(capture.lastEdit.size, MAX_TRACKED_EDITS);
  assert.equal(capture.lastEdit.has("file:///dataset/0.tua"), false);
  assert.equal(capture.lastEdit.has("file:///private/outside.tua"), false);
});

function replayDocument(filePath, source) {
  return {
    getText: () => source,
    uri: {
      fsPath: filePath,
      scheme: "file",
      toString: () => `file://${filePath}`,
    },
    version: 4,
  };
}
