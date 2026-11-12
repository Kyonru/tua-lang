"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");
const vscode = require("vscode");
const { isPathInside } = require("../editor/project-layout");
const { configuredLoveCommand } = require("../love/run-project");
const { auditionSource, canAudition, sceneSource, MAX_SECONDS, SCENE_SECONDS } = require("./audition-source");

function run(command, args, cwd, onChild, timeoutMs, onOutput) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    onChild(child);
    let stdout = "", stderr = "", settled = false;
    const timer = setTimeout(() => { child.kill(); finish(new Error(`${path.basename(command)} exceeded ${timeoutMs / 1000} seconds`)); }, timeoutMs);
    const finish = (error, result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error); else resolve(result);
    };
    child.stdout.on("data", data => { stdout += data; if (stdout.length > 8192) stdout = stdout.slice(-8192); onOutput?.(stdout); });
    child.stderr.on("data", data => { stderr += data; if (stderr.length > 8192) stderr = stderr.slice(-8192); });
    child.once("error", finish);
    child.once("exit", code => code === 0 ? finish(null, stdout) : finish(new Error(stderr.trim() || stdout.trim() || `${path.basename(command)} exited ${code}`)));
  });
}
function createAudition({ getServerCommand, onStatus } = {}) {
  let generation = 0, child, temp;
  const stop = () => {
    generation++;
    child?.kill(); child = undefined;
    if (temp) fs.rmSync(temp, { recursive: true, force: true });
    temp = undefined;
    onStatus?.("Audition stopped.");
  };
  const execute = async ({ label, source, assets, seconds }) => {
    if (!vscode.workspace.isTrusted) throw new Error("Trust this workspace before auditioning audio.");
    stop();
    const current = generation;
    temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-audio-audition-"));
    const project = temp;
    let finished = false, transition, unsupported;
    const began = Date.now();
    try {
      fs.mkdirSync(path.join(project, "src"), { recursive: true });
      for (const asset of assets) {
        const target = path.resolve(project, "src", asset.path);
        if (!isPathInside(path.join(project, "src"), target)) throw new Error("Audio asset path leaves the audition project.");
        fs.mkdirSync(path.dirname(target), { recursive: true });
        fs.copyFileSync(vscode.Uri.parse(asset.uri).fsPath, target);
      }
      fs.writeFileSync(path.join(project, "tua.toml"), 'source = "src"\nout = "build"\nmain = "main.tua"\ntarget = "love11-luajit"\n');
      fs.writeFileSync(path.join(project, "src/main.tua"), source);
      onStatus?.(`Preparing ${label}…`);
      await run(getServerCommand(), ["build"], project, process => { child = process; }, 20000);
      if (current !== generation) return;
      fs.writeFileSync(path.join(project, "build/conf.lua"), 'function love.conf(t) t.window.visible=false; t.window.width=64; t.window.height=64 end\n');
      onStatus?.(`Starting LÖVE for ${label}…`);
      let ready = false;
      const output = await run(configuredLoveCommand(), [path.join(project, "build")], project, process => { child = process; }, (seconds + 8) * 1000,
        text => { if (!ready && text.includes("TUA_AUDIO_AUDITION_READY")) { ready = true; onStatus?.(`Playing ${label} · ready in ${Date.now() - began} ms · stops after ${seconds} seconds`); } });
      if (!ready) throw new Error("LÖVE exited before the audio cue became ready.");
      transition = /TUA_AUDIO_TRANSITION:([^\r\n]+)/u.exec(output)?.[1];
      unsupported = /TUA_AUDIO_EFFECT_UNSUPPORTED:([^\r\n]+)/u.exec(output)?.[1];
      finished = true;
    } catch (error) {
      if (current === generation) { onStatus?.(`Audition failed: ${error.message}`); throw error; }
    } finally {
      if (current === generation) {
        child = undefined;
        fs.rmSync(temp, { recursive: true, force: true });
        temp = undefined;
        if (finished) onStatus?.(`Finished ${label}.${transition ? ` Transition: ${transition}.` : ""}${unsupported ? ` Effect unavailable in this LÖVE runtime: ${unsupported}.` : ""}`);
      }
    }
  };
  const play = async ({ system, cue }) => {
    if (!canAudition(system, cue)) throw new Error("This cue needs a present static asset and static settings for isolated audition.");
    return execute({ label: cue.name, source: auditionSource(system, cue, cue.source.assetPath),
      assets: [{ path: cue.source.assetPath, uri: cue.assetUri }], seconds: MAX_SECONDS });
  };
  const playScene = async ({ system, scene, target, quantize }) => {
    const model = sceneSource(system, scene, target, quantize);
    if (!model) throw new Error("This scene needs static streamed layers and authored timing for isolated audition.");
    return execute({ label: target ? `${scene.name} → ${target.name}` : scene.name,
      source: model.source, assets: model.assets, seconds: SCENE_SECONDS });
  };
  return { play, playScene, stop };
}

module.exports = { createAudition };
