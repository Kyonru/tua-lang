"use strict";
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { normalizeOptions } = require("./options");
const { createHash } = require("node:crypto");

// Decimal byte escapes are portable to Lua 5.1 and never interpret path contents.
function luaString(value) { return '"' + [...Buffer.from(value)].map(byte => "\\" + String(byte).padStart(3, "0")).join("") + '"'; }

function createLauncher(outputDirectory, assetsDirectory, connection, settings) {
  const options = normalizeOptions(settings);
  const luaOptions = `{ updateInterval = ${options.updateInterval}, categories = { ${options.categories.map(kind => `[${luaString(kind)}] = true`).join(", ")} } }`;
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-inspector-"));
  const directory = path.join(root, path.basename(outputDirectory));
  let watcher, timer, reconciliation, disposed = false;
  const copied = new Map();
  let confVersion;
  try {
    fs.chmodSync(root, 0o700);
    fs.mkdirSync(directory);
    const originalConf = path.join(outputDirectory, "conf.lua");
    const bootstrap = [
      "-- Tua editor-only inspector launcher. Original build remains untouched.",
      `assert(not love.filesystem.isFused(), "Inspector cannot run in a fused game")`,
      `local ok, reason = pcall(function() assert(loadfile(${luaString(path.join(assetsDirectory, "shim.lua"))}))(${luaString(assetsDirectory)}, ${connection.port}, ${luaString(connection.token)}, ${luaOptions}) end)`,
      `if not ok then print("[tua inspector] " .. tostring(reason)) end`,
      `local file = io.open(${luaString(originalConf)}, "rb")`,
      `if file then file:close(); assert(loadfile(${luaString(originalConf)}))() end`,
      "",
    ].join("\n");
    const synchronize = () => {
      if (disposed || !fs.existsSync(outputDirectory)) return;
      const names = new Set(fs.readdirSync(outputDirectory).filter(name => name !== "conf.lua" && name !== ".tuahotreload.tmp"));
      for (const name of fs.readdirSync(directory)) {
        if (name !== "conf.lua" && !names.has(name)) { fs.rmSync(path.join(directory, name), { force: true, recursive: true }); copied.delete(name); }
      }
      const confSource = fs.existsSync(originalConf) ? fs.readFileSync(originalConf) : Buffer.alloc(0);
      const version = createHash("sha256").update(confSource).digest("hex");
      if (version !== confVersion) {
        fs.writeFileSync(path.join(directory, "conf.lua"), bootstrap + `-- Original config: ${version}\n`, { mode: 0o600 });
        confVersion = version;
      }
      // Publish the reload signal last, after every root Lua module is copied.
      // PhysFS reports file symlinks as symlinks, so getInfo(path, "file") used
      // by hot reload needs real root files. Directory links preserve assets.
      const ordered = [...names].sort((a, b) => (a === ".tuahotreload") - (b === ".tuahotreload"));
      for (const name of ordered) {
        const target = path.join(directory, name), source = path.join(outputDirectory, name);
        const stat = fs.statSync(source);
        if (stat.isDirectory()) {
          try { fs.lstatSync(target); } catch (error) {
            if (error.code !== "ENOENT") throw error;
            fs.symlinkSync(source, target, process.platform === "win32" ? "junction" : "dir");
          }
        } else {
          const stamp = `${stat.ino}:${stat.size}:${stat.mtimeMs}`;
          if (copied.get(name) !== stamp) { fs.copyFileSync(source, target); copied.set(name, stamp); }
        }
      }
    };
    synchronize();
    // Mirror completed rebuilds without changing source or compiler output.
    watcher = fs.watch(outputDirectory, () => {
      clearTimeout(timer);
      timer = setTimeout(() => { try { synchronize(); } catch { /* retry on next change */ } }, 30);
    });
    watcher.on("error", () => {});
    watcher.unref();
    // Filesystem notifications can be lost under heavy rebuild load. Reconcile
    // root metadata periodically; unchanged files are never recopied.
    reconciliation = setInterval(() => { try { synchronize(); } catch { /* retry */ } }, 1000);
    reconciliation.unref();
    return { directory, dispose() { disposed = true; clearTimeout(timer); clearInterval(reconciliation); watcher.close(); fs.rmSync(root, { recursive: true, force: true }); } };
  } catch (error) {
    watcher?.close(); clearTimeout(timer); clearInterval(reconciliation); fs.rmSync(root, { recursive: true, force: true }); throw error;
  }
}
module.exports = { createLauncher, luaString };
