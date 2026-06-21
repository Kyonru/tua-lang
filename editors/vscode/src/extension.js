"use strict";

const childProcess = require("child_process");
const vscode = require("vscode");

let client;

async function activate(context) {
  const outputChannel = vscode.window.createOutputChannel("tua");
  context.subscriptions.push(outputChannel);

  const serverCommand = configuredServerPath();
  const cwd = workspaceCwd();
  const preflight = await checkServerCommand(serverCommand, cwd);

  if (!preflight.ok) {
    showServerStartupWarning(serverCommand, preflight.error);
    return;
  }

  let languageClient;

  try {
    languageClient = require("vscode-languageclient/node");
  } catch (error) {
    outputChannel.appendLine(`Failed to load vscode-languageclient: ${formatError(error)}`);
    vscode.window.showErrorMessage(
      "The tua extension is missing its language client dependency. Reinstall the extension or run npm install in editors/vscode.",
    );
    return;
  }

  const { LanguageClient, TransportKind } = languageClient;
  const serverOptions = {
    run: serverExecutable(serverCommand, cwd, TransportKind),
    debug: serverExecutable(serverCommand, cwd, TransportKind),
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "tua" }],
    outputChannel,
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.tua"),
    },
  };

  client = new LanguageClient("tua", "tua language server", serverOptions, clientOptions);
  context.subscriptions.push(client);

  try {
    await client.start();
  } catch (error) {
    outputChannel.appendLine(`Failed to start tua language server: ${formatError(error)}`);
    showServerStartupWarning(serverCommand, error);
    client = undefined;
  }
}

async function deactivate() {
  if (!client) {
    return undefined;
  }

  const activeClient = client;
  client = undefined;

  return activeClient.stop();
}

function configuredServerPath() {
  const envOverride = process.env.TUA_SERVER_PATH;
  if (typeof envOverride === "string" && envOverride.trim()) {
    return envOverride.trim();
  }

  const configured = vscode.workspace.getConfiguration("tua").get("server.path");

  return typeof configured === "string" && configured.trim() ? configured.trim() : "tua";
}

function workspaceCwd() {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];

  return workspaceFolder?.uri.scheme === "file" ? workspaceFolder.uri.fsPath : undefined;
}

function serverExecutable(command, cwd, TransportKind) {
  return {
    command,
    args: ["lsp"],
    transport: TransportKind.stdio,
    options: {
      cwd,
    },
  };
}

function checkServerCommand(command, cwd) {
  return new Promise((resolve) => {
    const child = childProcess.spawn(command, ["--version"], {
      cwd,
      stdio: "ignore",
      windowsHide: true,
    });

    child.once("error", (error) => {
      resolve({ ok: false, error });
    });

    child.once("exit", (code) => {
      if (code === 0) {
        resolve({ ok: true });
      } else {
        resolve({
          ok: false,
          error: new Error(`\`${command} --version\` exited with status ${code}`),
        });
      }
    });
  });
}

function showServerStartupWarning(command, error) {
  const detail = formatError(error);
  const message =
    `Could not start the tua language server with \`${command}\`. ` +
    "Install the tua CLI, set `tua.server.path`, or launch with `TUA_SERVER_PATH`.";

  vscode.window
    .showWarningMessage(`${message}${detail ? ` ${detail}` : ""}`, "Open Settings")
    .then((choice) => {
      if (choice === "Open Settings") {
        vscode.commands.executeCommand("workbench.action.openSettings", "tua.server.path");
      }
    });
}

function formatError(error) {
  if (!error) {
    return "";
  }

  if (typeof error.message === "string" && error.message) {
    return error.message;
  }

  return String(error);
}

module.exports = {
  activate,
  deactivate,
};
