"use strict";
const { LoveProjectTerminal } = require("./run-project");

// One watcher publishes completed builds; every game watches the same reload signal.
class HotReloadInstancesTerminal extends LoveProjectTerminal {
  async run() {
    const code = await this.runProcess(this.tuaCommand, ["dev", "--no-launch"]);
    if (!this.finished) {
      for (const child of this.children) child.close();
      // The watcher should stay alive as long as any player does.
      this.finish(code || 1);
    }
  }

  writeProcessLine(line, stream) {
    super.writeProcessLine(line, stream);
    if (this.finished || stream === "stderr") return;
    if (line.startsWith("dev: ready ") && !this.playersStarted) {
      this.playersStarted = true;
      void this.runInstances().finally(() => this.killActiveProcess());
    }
    if (line.startsWith("dev: sent reload ")) {
      for (const child of this.children) {
        if (child.finished) continue;
        child.sourceMaps = child.loadSourceMaps();
        this.activity?.line(child.session, line);
      }
    }
    if (line === "dev: keeping the running game on its last good build") {
      for (const child of this.children) this.activity?.line(child.session, line);
    }
  }
}

module.exports = { HotReloadInstancesTerminal };
