"use strict";
const fs = require("node:fs");
const path = require("node:path");
const { createProfileLauncher } = require("./launcher");
const { parseReport, buildProfileModel, MAX_REPORT_BYTES } = require("./model");
const { loadSourceMapsFromDirectory } = require("../editor/source-maps");

class ProfileCapture {
  constructor({ durationSeconds, budgetMs, traceCategories = [], assetsDirectory, onReport, onError }) {
    Object.assign(this, { durationSeconds, budgetMs, traceCategories, assetsDirectory, onReport, onError });
    this.completed = false;
  }

  launch(outputDirectory) {
    this.outputDirectory = outputDirectory;
    this.launcher = createProfileLauncher(outputDirectory,
      path.join(this.assetsDirectory, "shim.lua"), this.durationSeconds, this.traceCategories);
    this.poller = setInterval(() => this.readReport(), 200);
    this.poller.unref?.();
    return this.launcher.directory;
  }

  readReport() {
    if (this.completed || !this.launcher) return;
    let raw;
    try {
      const stat = fs.statSync(this.launcher.reportPath);
      if (stat.size > MAX_REPORT_BYTES) throw new Error("profile report exceeds 2 MiB");
      raw = fs.readFileSync(this.launcher.reportPath, "utf8");
    } catch (error) {
      if (error.code === "ENOENT") return;
      this.completed = true;
      this.onError?.(error);
      return;
    }
    try {
      const report = parseReport(raw);
      const maps = loadSourceMapsFromDirectory(this.outputDirectory);
      this.completed = true;
      clearInterval(this.poller);
      this.onReport?.(buildProfileModel(report, maps, this.outputDirectory, this.budgetMs));
    } catch (error) {
      this.completed = true;
      clearInterval(this.poller);
      this.onError?.(error);
    }
  }

  stop() {
    if (!this.completed && this.launcher) this.launcher.stop();
  }

  finish() {
    const wasLaunched = !!this.launcher;
    this.readReport();
    clearInterval(this.poller);
    this.launcher?.dispose();
    this.launcher = undefined;
    if (wasLaunched && !this.completed) this.onError?.(new Error("LÖVE ended before the capture could be saved"));
    this.completed = true;
  }
}

module.exports = { ProfileCapture };
