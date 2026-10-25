"use strict";
const { randomUUID } = require("node:crypto");
const vscode = require("vscode");
const { sourceLocation } = require("../workspace/console-buffer");
const { createDebugSnapshot } = require("./debug-snapshot");

function registerDebugInstanceGroups({ activity, appendOutputLine, build, prepare, configuration, scriptFiles }) {
  const groups = new Set();
  const players = new Map();
  const find = session => players.get(session?.configuration?.tuaDebugInstance);
  const listeners = [
    vscode.debug.onDidStartDebugSession?.(session => {
      const player = find(session);
      if (!player || player.done) return;
      player.debugSession = session;
      if (player.group.cancelled || player.stopped) void vscode.debug.stopDebugging(session);
      else activity?.update(player.activitySession, "running");
    }),
    vscode.debug.onDidTerminateDebugSession(session => {
      const player = find(session);
      if (player) finishPlayer(player);
    }),
    vscode.debug.registerDebugAdapterTrackerFactory?.("lua-local", {
      createDebugAdapterTracker(session) {
        const player = find(session);
        if (!player) return undefined;
        return { onDidSendMessage(message) {
          if (message.type !== "event") return;
          if (message.event === "exited") player.exitCode = message.body?.exitCode || 0;
          if (message.event !== "output" || typeof message.body?.output !== "string") return;
          for (const line of message.body.output.split(/\r?\n/)) {
            const source = message.body.source?.path;
            const location = source?.endsWith(".tua")
              ? { path: source, line: Math.max(0, (message.body.line || 1) - 1), character: 0 }
              : sourceLocation(line, player.group.layout.configDirectory);
            activity?.output(player.activitySession, line, { location, stream: message.body.category });
          }
        } };
      },
    }),
  ].filter(Boolean);

  function finishPlayer(player) {
    if (player.done) return;
    player.done = true;
    players.delete(player.id);
    activity?.finish(player.activitySession, player.exitCode || 0, player.stopped || player.group.cancelled);
    finishGroup(player.group);
  }

  function finishGroup(group) {
    if (group.launching || group.finished || group.players.some(player => !player.done)) return;
    group.finished = true;
    group.snapshot?.dispose();
    groups.delete(group);
    const code = group.failed ? 1 : group.players.find(player => player.exitCode)?.exitCode || 0;
    activity?.finish(group.activitySession, code, group.cancelled && !group.failed);
  }

  function stopPlayer(player) {
    player.stopped = true;
    if (player.debugSession) return vscode.debug.stopDebugging(player.debugSession);
    // A startDebugging call may still be pending. Its start event will stop it.
  }

  function stopGroup(group) {
    group.cancelled = true;
    group.abort.abort();
    return Promise.all(group.players.map(player => {
      player.stopped = true;
      return player.debugSession && !player.done ? vscode.debug.stopDebugging(player.debugSession) : undefined;
    }));
  }

  async function start({ layout, instances, title = "Debug LOVE Project", tuaCommand, loveCommand, configurationName }) {
    const group = { layout, players: [], launching: true, cancelled: false, abort: new AbortController() };
    group.activitySession = activity?.begin({ kind: "debug", title, cwd: layout.configDirectory, configurationName });
    if (group.activitySession) group.activitySession.stop = () => stopGroup(group);
    groups.add(group);
    const output = line => { appendOutputLine?.(line); activity?.output(group.activitySession, line, { module: "build" }); };
    try {
      const code = await build({ cwd: layout.configDirectory, tuaCommand, appendOutputLine: output, signal: group.abort.signal });
      if (group.cancelled) return undefined;
      if (code !== 0) throw new Error(`Tua debug build failed with exit code ${code}.`);
      group.snapshot = createDebugSnapshot(layout.outputDirectory);
      const debugLayout = { ...layout, outputDirectory: group.snapshot.outputDirectory };
      prepare(debugLayout.outputDirectory);
      const files = scriptFiles(debugLayout.outputDirectory);
      const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(layout.configDirectory));
      for (const instance of instances) {
        if (group.cancelled) break;
        const player = { id: randomUUID(), group, done: false };
        player.activitySession = activity?.begin({ kind: "debug", title: `${title}: ${instance.name}`,
          cwd: layout.configDirectory, groupId: group.activitySession?.id, configurationName });
        if (player.activitySession) player.activitySession.stop = () => stopPlayer(player);
        group.players.push(player);
        players.set(player.id, player);
        let started = false;
        try {
          started = await vscode.debug.startDebugging(folder, configuration({ layout: debugLayout, loveCommand, overrides: {
            name: `${title}: ${instance.name}`, args: instance.args, scriptFiles: files, tuaDebugInstance: player.id,
          } }));
        } finally {
          if (!started) { player.exitCode = 1; finishPlayer(player); }
        }
        if (!started) throw new Error(`Debugger did not start ${instance.name}.`);
        activity?.update(player.activitySession, "running");
      }
      if (!group.cancelled) activity?.update(group.activitySession, "running");
      return { outputDirectory: debugLayout.outputDirectory, debuggerType: "lua-local" };
    } catch (error) {
      group.failed = true;
      await stopGroup(group);
      output(`[tua] ${error.message}`);
      vscode.window.showErrorMessage(error.message);
      return undefined;
    } finally {
      group.launching = false;
      finishGroup(group);
    }
  }

  return { start, async dispose() {
    await Promise.all([...groups].map(stopGroup));
    // Shutdown must release snapshots even when the editor no longer delivers termination events.
    for (const group of groups) { group.launching = false; for (const player of group.players) finishPlayer(player); finishGroup(group); }
    for (const listener of listeners) listener.dispose();
  } };
}

module.exports = { registerDebugInstanceGroups };
