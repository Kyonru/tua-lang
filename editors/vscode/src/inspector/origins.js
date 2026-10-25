"use strict";
const fs = require("node:fs");
const path = require("node:path");
const HELPERS = { stores: ["store", "__tua_store"], pools: ["objectPool", "__tua_object_pool"], signals: ["signal", "__tua_signal", "new"], states: ["stateMachine", "__tua_state_machine", "create"], ecs: ["world"] };

function inside(root, file) {
  if (!root) return false;
  const relative = path.relative(path.resolve(root), path.resolve(file));
  return relative === "" || (relative !== ".." && !relative.startsWith(".." + path.sep) && !path.isAbsolute(relative));
}

// Keep line/column offsets while excluding comments and strings from binding
// detection. This is intentionally conservative, not a Lua expression evaluator.
function maskSource(source) {
  const blank = text => text.replace(/[^\n]/g, " ");
  return source.replace(/--\[(=*)\[[\s\S]*?\]\1\]|\[(=*)\[[\s\S]*?\]\2\]|--[^\n]*|"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'/g, blank);
}

function bindingAtLine(source, line, kind, constructor) {
  const masked = maskSource(source);
  const starts = [0];
  for (let i = 0; i < masked.length; i++) if (masked[i] === "\n") starts.push(i + 1);
  if (line < 1 || line > starts.length) return undefined;
  const targetEnd = starts[line] ?? masked.length;
  const helpers = new Set([...(HELPERS[kind] || []), ...(typeof constructor === "string" ? [constructor] : [])]);
  const assignment = /(?:^|\n)[ \t]*(?:local[ \t]+)?([A-Za-z_]\w*(?:[ \t]*\.[ \t]*[A-Za-z_]\w*)*)[ \t]*(?::[^=\n]+)?=[ \t\r\n]*([A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)[ \t\r\n]*([({])/g;
  let match, result;
  while ((match = assignment.exec(masked))) {
    if (match.index >= targetEnd) break;
    const callee = match[2].split(".").at(-1);
    if (!helpers.has(callee) && !helpers.has(match[2])) continue;
    const opening = assignment.lastIndex - 1;
    const stack = [masked[opening]], pairs = { ")": "(", "}": "{", "]": "[" };
    let end = opening + 1;
    for (; end < masked.length && stack.length && end - opening < 65536; end++) {
      const char = masked[end];
      if ("({[".includes(char)) stack.push(char);
      else if (pairs[char]) { if (stack.pop() !== pairs[char]) break; }
    }
    if (stack.length) continue;
    const startLine = masked.slice(0, opening).split("\n").length;
    const endLine = masked.slice(0, end).split("\n").length;
    if (line < startLine || line > endLine) continue;
    // Nested calls to the same constructor or multiple calls on one line do
    // not establish which result owns a binding. Keep only the source link.
    const nested = masked.slice(opening + 1, end - 1).match(/[A-Za-z_]\w*(?=\s*\()/g) || [];
    if (nested.some(name => helpers.has(name))) continue;
    const name = match[1].replace(/\s/g, "");
    const nameOffset = match.index + match[0].indexOf(match[1]);
    const nameLine = masked.slice(0, nameOffset).split("\n").length;
    const candidate = { name, line: nameLine, character: nameOffset - starts[nameLine - 1] };
    if (result) return undefined;
    result = candidate;
  }
  return result;
}

class RuntimeOrigins {
  constructor(readMap = () => undefined) { this.readMap = readMap; this.cache = new WeakMap(); }
  describe(entry, object) {
    if (!entry.outputDirectory || typeof object.source !== "string" || !object.source.startsWith("@") || !Number.isSafeInteger(object.line) || object.line < 1) return undefined;
    let cache = this.cache.get(entry);
    if (!cache || cache.generation !== entry.connectionGeneration) {
      cache = { generation: entry.connectionGeneration, objects: new Map() }; this.cache.set(entry, cache);
    }
    const key = `${object.id}:${object.kind}:${object.source}:${object.line}`;
    if (cache.objects.has(key)) return cache.objects.get(key);
    if (cache.objects.size >= 512) cache.objects.clear();
    let result;
    try {
      const source = object.source.slice(1);
      let generated = path.resolve(entry.outputDirectory, source);
      if (!inside(entry.outputDirectory, generated) && inside(entry.launcher?.directory, generated)) {
        generated = path.join(entry.outputDirectory, path.relative(entry.launcher.directory, generated));
      }
      if (!inside(entry.outputDirectory, generated) || !generated.endsWith(".lua")) return undefined;
      let file = generated, line = object.line, mapped = false;
      const map = this.readMap(generated);
      const mapping = map?.lineMappings?.find(item => item.generatedLine === object.line);
      if (mapping && map.sourcePath && inside(entry.session.cwd, map.sourcePath) && fs.existsSync(map.sourcePath)) {
        file = map.sourcePath; line = mapping.sourceLine; mapped = true;
      }
      if (!Number.isSafeInteger(line) || line < 1 || fs.statSync(file).size > 2 * 1024 * 1024) return undefined;
      const text = fs.readFileSync(file, "utf8");
      if (line > text.split("\n").length) return undefined;
      const binding = bindingAtLine(text, line, object.kind, object.constructor);
      result = { path: file, line: binding?.line || line, character: binding?.character || 0,
        name: binding?.name, generated: !mapped, file: path.relative(entry.session.cwd || entry.outputDirectory, file) };
    } catch { /* Missing maps or sources must not interrupt live inspection. */ }
    cache.objects.set(key, result);
    return result;
  }
}
module.exports = { RuntimeOrigins, bindingAtLine, maskSource };
