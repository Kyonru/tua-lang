"use strict";
const { EventEmitter } = require("node:events");

const LABELS = {
  building: "Building", running: "Running", reloading: "Reload requested",
  reloaded: "Reloaded", failed: "Build failed", previous: "Build failed — running previous code",
  reloadFailed: "Reload failed — inspect console", runtimeFailed: "Exited with errors", finished: "Finished", stopped: "Stopped",
};

class ActivityStore {
  constructor() { this.sessions = []; this.events = new EventEmitter(); this.serial = 0; }
  subscribe(listener) { this.events.on("change", listener); return { dispose: () => this.events.off("change", listener) }; }
  begin(options) {
    const session = { ...options, id: String(++this.serial), phase: "building", label: LABELS.building, started: Date.now(), active: true, modules: [] };
    this.sessions.unshift(session);
    // Keep all active sessions, plus at most twenty completed ones.
    this.prune();
    this.events.emit("change", session);
    return session;
  }
  prune() {
    let completed = 0;
    this.sessions = this.sessions.filter(session => session.active || completed++ < 20);
  }
  update(session, phase) {
    if (!session?.active || !LABELS[phase]) return;
    session.phase = phase;
    session.label = LABELS[phase];
    session.updated = Date.now();
    this.events.emit("change", session);
  }
  line(session, line) {
    if (!session?.active || session.kind !== "dev") return;
    const phase = devPhase(line);
    const reloaded = line.match(/^\[tua dev\] reloaded (.+)$/);
    if (reloaded && !session.modules.includes(reloaded[1])) session.modules.push(reloaded[1]);
    if (phase) this.update(session, phase);
  }
  output(session, line, options = {}) {
    if (session?.active) this.events.emit("output", { session, line, options });
  }
  finish(session, code, stopped = false) {
    if (!session?.active) return;
    if (stopped) this.update(session, "stopped");
    else if (code !== 0) this.update(session, ["building", "failed"].includes(session.phase) ? "failed" : "runtimeFailed");
    else this.update(session, "finished");
    session.active = false;
    session.exitCode = code;
    session.ended = Date.now();
    session.stop = undefined;
    this.prune();
    this.events.emit("change", session);
  }
  dispose() { this.events.removeAllListeners(); }
}

function devPhase(line) {
  if (line.startsWith("dev: started ")) return "running";
  if (line.startsWith("dev: sent reload ")) return "reloading";
  if (line.startsWith("[tua dev] reloaded ")) return "reloaded";
  if (line === "dev: keeping the running game on its last good build") return "previous";
  if (line === "dev: fix the errors above to start the game") return "failed";
  if (/^\[tua dev\].*(?:failed|error)/i.test(line)) return "reloadFailed";
  return undefined;
}

module.exports = { ActivityStore, devPhase, LABELS };
