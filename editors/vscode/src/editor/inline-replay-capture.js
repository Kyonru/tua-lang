"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const SCHEMA_VERSION = 1;
const MAX_TRACKED_EDITS = 256;

function createInlineReplayCapture(options = {}) {
  const env = options.env || process.env;
  const rootSetting = env.TUA_INLINE_REPLAY_ROOT;
  const outputSetting = env.TUA_INLINE_REPLAY_CAPTURE;
  if (!rootSetting || !outputSetting) {
    return undefined;
  }

  let descriptor;
  try {
    const datasetRoot = fs.realpathSync.native(path.resolve(rootSetting));
    const outputPath = path.resolve(outputSetting);
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    descriptor = fs.openSync(outputPath, "a", 0o600);
    return new InlineReplayCapture({
      datasetRoot,
      dispose: () => fs.closeSync(descriptor),
      onError: options.appendOutputLine,
      resolveRealPath: (value) => fs.realpathSync.native(value),
      writeLine: (line) => fs.writeSync(descriptor, line, undefined, "utf8"),
    });
  } catch (error) {
    if (descriptor !== undefined) {
      try {
        fs.closeSync(descriptor);
      } catch {}
    }
    options.appendOutputLine?.(
      `Tua inline replay capture is disabled: ${String(error?.message || error)}`,
    );
    return undefined;
  }
}

class InlineReplayCapture {
  constructor(options) {
    this.datasetRoot = options.datasetRoot;
    this.writeLine = options.writeLine;
    this.resolveRealPath = options.resolveRealPath || path.resolve;
    this.now = options.now || Date.now;
    this.close = options.dispose;
    this.onError = options.onError;
    this.lastEdit = new Map();
    this.failed = false;
  }

  noteEdit(document) {
    if (this.failed || document?.uri?.scheme !== "file") {
      return;
    }
    try {
      if (!this.datasetPath(document)) {
        return;
      }
      const key = document.uri.toString();
      this.lastEdit.delete(key);
      this.lastEdit.set(key, this.now());
      while (this.lastEdit.size > MAX_TRACKED_EDITS) {
        this.lastEdit.delete(this.lastEdit.keys().next().value);
      }
    } catch {
      // Unrelated, deleted, and not-yet-created files are not replay inputs.
    }
  }

  record(document, position, completionContext) {
    if (this.failed || document?.uri?.scheme !== "file") {
      return;
    }

    try {
      const datasetPath = this.datasetPath(document);
      if (!datasetPath) {
        return;
      }

      const selected = completionContext.selectedCompletionInfo;
      const editedAt = this.lastEdit.get(document.uri.toString());
      const event = {
        schemaVersion: SCHEMA_VERSION,
        datasetPath,
        sourceHash: replaySourceHash(document.getText()),
        position: protocolPosition(position),
        documentVersion: document.version,
        elapsedSinceEditMs:
          editedAt === undefined ? null : Math.max(0, Math.round(this.now() - editedAt)),
        menuOpen: Boolean(selected),
      };
      if (selected) {
        event.selectedCompletion = {
          text: selected.text,
          range: protocolRange(selected.range),
        };
      }
      const line = `${JSON.stringify(event)}\n`;
      const written = this.writeLine(line);
      if (written !== Buffer.byteLength(line, "utf8")) {
        throw new Error("could not write the complete replay event");
      }
    } catch (error) {
      this.fail(error);
    }
  }

  dispose() {
    this.lastEdit.clear();
    try {
      this.closeOnce();
    } catch (error) {
      this.fail(error);
    }
  }

  fail(error) {
    if (this.failed) {
      return;
    }
    this.failed = true;
    try {
      this.closeOnce();
    } catch {}
    this.onError?.(
      `Tua inline replay capture stopped: ${String(error?.message || error)}`,
    );
  }

  closeOnce() {
    const close = this.close;
    this.close = undefined;
    close?.();
  }

  datasetPath(document) {
    const filePath = this.resolveRealPath(document.uri.fsPath);
    const relative = path.relative(this.datasetRoot, filePath);
    if (!relative || path.isAbsolute(relative) || outsideRoot(relative)) {
      return undefined;
    }
    return relative.split(path.sep).join("/");
  }
}

function replaySourceHash(source) {
  const normalized = source.replace(/\r\n/g, "\n");
  return crypto.createHash("sha256").update(normalized, "utf8").digest("hex");
}

function outsideRoot(relative) {
  return relative === ".." || relative.startsWith(`..${path.sep}`);
}

function protocolPosition(position) {
  return { line: position.line, character: position.character };
}

function protocolRange(range) {
  return {
    start: protocolPosition(range.start),
    end: protocolPosition(range.end),
  };
}

module.exports = {
  InlineReplayCapture,
  MAX_TRACKED_EDITS,
  SCHEMA_VERSION,
  createInlineReplayCapture,
  replaySourceHash,
};
