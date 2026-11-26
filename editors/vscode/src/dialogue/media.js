"use strict";

const fs = require("node:fs/promises");
const { Buffer } = require("node:buffer");
const path = require("node:path");
const { escapeHtml } = require("../shared/webview");

const STYLE_CLASSES = new Set(["highlight", "underline", "strike", "thought", "whisper", "warning", "success", "radio"]);

function richTextHtml(runs) {
  return runs.map(run => {
    if (run.annotation) return `<span class="rich-annotation">${escapeHtml(run.annotation)}</span>`;
    let text = escapeHtml(run.text);
    for (const mark of [...(run.marks || [])].reverse()) {
      if (mark === "em") text = `<strong>${text}</strong>`;
      else if (mark.startsWith("style:")) {
        const name = mark.slice(6);
        const css = STYLE_CLASSES.has(name) ? " rich-" + name : "";
        text = `<span class="rich-style${css}" title="${escapeHtml("Style: " + name)}">${text}</span>`;
      } else {
        text = `<span class="rich-metadata" title="${escapeHtml(mark)}">${text}</span>`;
      }
    }
    return text;
  }).join("");
}

function portraitHtml(character, registerImage = () => 0) {
  if (!character?.portrait) return "";
  const name = character.name || "Character";
  if (character.portraitUri) {
    return `<img class="portrait" data-portrait="${registerImage(character.portraitUri)}" alt="${escapeHtml(name + (character.expression ? " · " + character.expression : ""))}" width="56" height="56"><span class="portrait-fallback" hidden>Portrait unavailable</span>`;
  }
  return `<span class="portrait-fallback" title="${escapeHtml(character.portraitIssue || "Unresolved portrait")}">${escapeHtml(character.portrait)} · portrait unavailable</span>`;
}

function within(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== ".." && !relative.startsWith(".." + path.sep) && !path.isAbsolute(relative);
}

// Only local raster files under the owning project's source directory. Opaque
// game IDs need an explicit editor mapping; no game loader or network request runs.
async function loadPortraits(preview, sourceDirectory, mapping = {}) {
  const cache = new Map();
  let bytes = 0;
  let root;
  try { root = await fs.realpath(sourceDirectory); } catch { /* explain per portrait */ }
  for (const section of preview.sections.slice(0, 200)) {
    for (const line of section.lines.slice(0, 100)) {
      const character = line.character;
      if (!character?.portrait) continue;
      const id = character.portrait;
      if (!cache.has(id)) {
        let result;
        try {
          if (!root) throw new Error("Project source directory is unavailable");
          if (cache.size >= 32) throw new Error("Preview limit: 32 distinct portraits");
          const asset = Object.hasOwn(mapping, id) ? mapping[id] : id;
          if (typeof asset !== "string" || path.isAbsolute(asset) || /^[a-z]+:/i.test(asset)) {
            throw new Error("Use a source-relative local image path");
          }
          const candidate = path.resolve(root, asset);
          if (!within(root, candidate)) throw new Error("Portrait is outside the project source directory");
          const resolved = await fs.realpath(candidate);
          if (!within(root, resolved)) throw new Error("Portrait symlink leaves the project source directory");
          const mime = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp" }[path.extname(resolved).toLowerCase()];
          if (!mime) throw new Error("Preview supports PNG, JPEG and WebP portraits");
          const file = await fs.open(resolved, "r");
          try {
            const stat = await file.stat();
            if (!stat.isFile() || stat.size > 2 * 1024 * 1024 || bytes + stat.size > 8 * 1024 * 1024) {
              throw new Error("Portrait limit: 2 MiB per file, 8 MiB per preview");
            }
            const buffer = Buffer.alloc(stat.size + 1);
            const read = await file.read(buffer, 0, buffer.length, 0);
            if (read.bytesRead > stat.size) throw new Error("Portrait changed while loading; reload");
            bytes += read.bytesRead;
            result = { portraitUri: `data:${mime};base64,${buffer.subarray(0, read.bytesRead).toString("base64")}` };
          } finally { await file.close(); }
        } catch (error) {
          result = { portraitIssue: error.code === "ENOENT"
            ? "Image not found; map game IDs with tua.dialogue.preview.portraits" : error.message };
        }
        cache.set(id, result);
      }
      delete character.portraitUri;
      delete character.portraitIssue;
      Object.assign(character, cache.get(id));
    }
  }
  return preview;
}

module.exports = { richTextHtml, portraitHtml, loadPortraits };
