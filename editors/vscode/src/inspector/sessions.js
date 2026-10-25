"use strict";
const { EventEmitter } = require("node:events");
const { listen } = require("./connection");
const { createLauncher } = require("./launcher");

class InspectorSessions {
  constructor({ activity, enabled, assetsDirectory, describeObject, reportError = () => {} }) {
    Object.assign(this, { activity, enabled, assetsDirectory, describeObject, reportError });
    this.entries = new Map(); this.events = new EventEmitter(); this.disposed = false;
    this.subscription = activity.subscribe(() => {
      for (const [id, entry] of this.entries) {
        if (!entry.session.active) this.close(entry);
        if (!activity.sessions.some(session => session.id === id)) this.entries.delete(id);
      }
      this.changed();
    });
  }
  subscribe(fn) { this.events.on("change", fn); return { dispose: () => this.events.off("change", fn) }; }
  changed() { if (!this.disposed) this.events.emit("change"); }
  async launch(session, outputDirectory) {
    if (!session?.active || !this.enabled() || this.disposed) return undefined;
    const entry = { session, outputDirectory, status: "waiting", paused: false };
    this.entries.set(session.id, entry); this.changed();
    try {
      entry.connection = await listen({
        onSnapshot: snapshot => { for (const object of snapshot.objects) object.origin = this.describeObject?.(entry, object); entry.previousSnapshot = entry.snapshot; entry.snapshot = snapshot; entry.received = Date.now(); this.changed(); },
        onStatus: status => { if (status === "connected") { entry.connectionGeneration = (entry.connectionGeneration || 0) + 1; entry.previousSnapshot = undefined; entry.snapshot = undefined; } entry.status = status; this.changed(); },
      });
      if (this.disposed || !session.active || !this.enabled()) { this.close(entry); return undefined; }
      entry.launcher = createLauncher(outputDirectory, this.assetsDirectory, entry.connection);
      return entry.launcher.directory;
    } catch (error) {
      this.close(entry); entry.status = "unavailable"; this.changed();
      this.reportError(`Runtime inspector unavailable: ${error.message}. The game will run without inspection.`);
      return undefined;
    }
  }
  close(entry) {
    entry.connection?.dispose(); entry.connection = undefined;
    entry.launcher?.dispose(); entry.launcher = undefined;
    entry.status = "ended";
  }
  pause(id, paused) { const entry = this.entries.get(id); if (!entry?.connection) return; entry.paused = paused ?? !entry.paused; entry.connection.pause(entry.paused); this.changed(); }
  refresh(id) { this.entries.get(id)?.connection?.refresh(); }
  disconnectAll() {
    for (const entry of this.entries.values()) {
      entry.connection?.stop(); entry.connection?.dispose(); entry.connection = undefined;
      entry.status = "disabled";
      // Keep the filesystem mirror until the game exits: it is still its source.
    }
    this.changed();
  }
  dispose() { this.disposed = true; this.subscription.dispose(); for (const entry of this.entries.values()) this.close(entry); this.events.removeAllListeners(); }
}
module.exports = { InspectorSessions };
