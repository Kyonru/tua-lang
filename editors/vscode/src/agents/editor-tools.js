"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../constants");

const ACTIONS = {
  tua_build_project: { command: "tua.buildProject", title: "Build the active Tua project" },
  tua_run_love: { command: COMMANDS.runLoveProject, title: "Run the active Tua LÖVE game" },
  tua_start_love_debug: { command: COMMANDS.debugLoveProject, title: "Debug the active Tua LÖVE game" },
};

function result(value) {
  return new vscode.LanguageModelToolResult([
    new vscode.LanguageModelTextPart(JSON.stringify(value)),
  ]);
}

function requireTrustedProject() {
  if (!vscode.workspace.isTrusted || !vscode.workspace.workspaceFolders?.some(folder => folder.uri.scheme === "file")) {
    throw new Error("Open and trust a local Tua project before using this tool.");
  }
}

function registerEditorAgentTools(activity, consoleBuffer) {
  if (typeof vscode.lm?.registerTool !== "function"
      || typeof vscode.LanguageModelToolResult !== "function"
      || typeof vscode.LanguageModelTextPart !== "function") return { dispose() {} };

  const registrations = Object.entries(ACTIONS).map(([name, action]) => vscode.lm.registerTool(name, {
    prepareInvocation() {
      return {
        invocationMessage: action.title,
        confirmationMessages: { title: action.title, message: `${action.title}? This starts a Tua task in the editor.` },
      };
    },
    async invoke() {
      requireTrustedProject();
      const started = await vscode.commands.executeCommand(action.command, name === "tua_run_love" ? { console: true } : undefined);
      return result(started ? { started: true, action: name, details: started }
        : { started: false, action: name, reason: "The editor action did not start. Check the Tua output and active project." });
    },
  }));

  registrations.push(vscode.lm.registerTool("tua_get_debug_context", {
    prepareInvocation() {
      return {
        invocationMessage: "Reading Tua run and debug context",
        confirmationMessages: {
          title: "Read Tua debug context",
          message: "Share recent game output and, when paused, the active Tua debug stack and local variables with this agent?",
        },
      };
    },
    async invoke() {
      requireTrustedProject();
      return result(await debugContext(activity, consoleBuffer));
    },
  }));
  return vscode.Disposable.from(...registrations);
}

function isTuaDebugSession(session) {
  const configuration = session?.configuration;
  return session?.type === "lua-local" && (configuration?.tuaProject === true
    || configuration?.tuaDebugInstance !== undefined
    || configuration?.name === "Debug LOVE Project (Tua)");
}

async function debugContext(activity, consoleBuffer) {
  const sessions = (activity?.sessions || []).slice(0, 10).map(session => ({
    id: session.id, kind: session.kind, title: session.title, phase: session.phase,
    active: session.active, exitCode: session.exitCode, started: session.started,
  }));
  const output = (consoleBuffer?.entries || []).slice(-40).map(entry => ({
    sessionId: entry.session.id, severity: entry.severity, text: entry.text.slice(0, 1000),
    location: entry.location, time: entry.time,
  }));
  const debug = vscode.debug.activeDebugSession;
  if (!isTuaDebugSession(debug)) return { sessions, output, debug: { active: false } };
  const snapshot = { active: true, name: debug.name, id: debug.id };
  try {
    const threads = (await request(debug, "threads", {})).threads || [];
    snapshot.threads = threads.slice(0, 8).map(thread => ({ id: thread.id, name: thread.name }));
    if (threads.length) {
      const frames = (await request(debug, "stackTrace", { threadId: threads[0].id, startFrame: 0, levels: 8 })).stackFrames || [];
      snapshot.frames = frames.slice(0, 8).map(frame => ({
        id: frame.id, name: frame.name, line: frame.line, column: frame.column,
        source: frame.source?.path,
      }));
      if (frames.length) {
        const scopes = (await request(debug, "scopes", { frameId: frames[0].id })).scopes || [];
        snapshot.scopes = [];
        for (const scope of scopes.slice(0, 3)) {
          const variables = (await request(debug, "variables", { variablesReference: scope.variablesReference, start: 0, count: 12 })).variables || [];
          snapshot.scopes.push({ name: scope.name, variables: variables.slice(0, 12).map(variable => ({
            name: variable.name, type: variable.type, value: String(variable.value).slice(0, 512),
          })) });
        }
      }
    }
  } catch (error) {
    snapshot.detail = `Live stack and variables are unavailable: ${error.message || error}`;
  }
  return { sessions, output, debug: snapshot };
}

function request(session, method, params) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${method} timed out`)), 2000);
    Promise.resolve().then(() => session.customRequest(method, params)).then(value => {
      clearTimeout(timer);
      resolve(value || {});
    }, error => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

module.exports = { registerEditorAgentTools, debugContext, isTuaDebugSession };
