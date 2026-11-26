"use strict";

const vscode = require("vscode");

const COMMAND = "tua.dialogue.formatText";
const KIND = "refactor.rewrite.dialogueText";
const FORMATS = [
  { id: "emphasis", title: "Emphasis" },
  { id: "thought", title: "Thought / italic" },
  { id: "underline", title: "Underline" },
  { id: "strike", title: "Strikethrough" },
  { id: "highlight", title: "Highlight" },
  { id: "whisper", title: "Whisper" },
];

function registerDialogueFormatting() {
  let timer;
  let generation = 0;
  let disposed = false;
  let cached;

  function capture(editor = vscode.window.activeTextEditor) {
    if (!editor || editor.document.languageId !== "tdialogue" ||
        editor.selections.length !== 1 || editor.selection.isEmpty) return undefined;
    return { editor, document: editor.document, version: editor.document.version, selection: editor.selection };
  }

  function current(target) {
    const editor = vscode.window.activeTextEditor;
    return !disposed && target && editor === target.editor && !target.document.isClosed &&
      target.document.version === target.version && editor.selections.length === 1 &&
      editor.selection.isEqual(target.selection);
  }

  async function actions(target) {
    if (!current(target)) return [];
    const result = await vscode.commands.executeCommand(
      "vscode.executeCodeActionProvider", target.document.uri, target.selection, KIND,
    );
    return current(target) ? (result || []).filter(action => action.kind?.value === KIND && action.edit) : [];
  }

  function matchingAction(available, format) {
    return available.find(action => action.title === format.title || action.title === `Remove ${format.title}`);
  }

  function setState(available = []) {
    return Promise.all(FORMATS.flatMap(format => {
      const action = matchingAction(available, format);
      return [
        vscode.commands.executeCommand("setContext", `tua.dialogue.format.${format.id}.enabled`, !!action),
        vscode.commands.executeCommand("setContext", `tua.dialogue.format.${format.id}.active`,
          action?.title === `Remove ${format.title}`),
      ];
    }));
  }

  function refresh() {
    clearTimeout(timer);
    const request = ++generation;
    if (!current(cached?.target)) {
      cached = undefined;
      void setState();
    }
    const target = capture();
    if (!target || disposed) return;
    timer = setTimeout(async () => {
      try {
        const available = await actions(target);
        if (request === generation && current(target)) {
          cached = { target, actions: available };
          await setState(available);
        }
      } catch {
        // A cancelled request or restarting server leaves the buttons disabled.
      }
    }, 100);
  }

  async function apply(target, action) {
    if (!current(target)) return false;
    const entries = action.edit.entries();
    if (entries.length !== 1 || entries[0][0].toString() !== target.document.uri.toString() ||
        entries[0][1].length !== 1) return false;
    const edit = entries[0][1][0];
    const offset = target.document.offsetAt(edit.range.start);
    const applied = await target.editor.edit(builder => builder.replace(edit.range, edit.newText),
      { undoStopBefore: true, undoStopAfter: true });
    if (applied && vscode.window.activeTextEditor === target.editor &&
        target.document.version === target.version + 1) {
      // Keep the formatted fragment selected so successive toggles compose.
      target.editor.selection = new vscode.Selection(
        target.document.positionAt(offset), target.document.positionAt(offset + edit.newText.length),
      );
    }
    refresh();
    return applied;
  }

  async function choose() {
    const target = capture();
    const available = await actions(target);
    if (!available.length) {
      await vscode.window.showInformationMessage(
        "Select text within one dialogue line, choice, or reason in a file with @text rich. Keep placeholders and markup intact.",
      );
      return;
    }
    const picked = await vscode.window.showQuickPick(
      available.map(action => ({ label: action.title, action })),
      { title: "Format Dialogue Text", placeHolder: "Named styles are interpreted by your renderer" },
    );
    if (picked) return apply(target, picked.action);
    return false;
  }

  const registrations = [
    vscode.commands.registerCommand(COMMAND, choose),
    ...FORMATS.flatMap(format => ["", ".active"].map(suffix =>
      vscode.commands.registerCommand(`tua.dialogue.format.${format.id}${suffix}`, async uri => {
        const target = capture();
        if (!target || (uri && uri.toString() !== target.document.uri.toString())) return false;
        const available = current(cached?.target) ? cached.actions : await actions(target);
        const action = matchingAction(available, format);
        return action ? apply(target, action) : false;
      }))),
    vscode.window.onDidChangeTextEditorSelection(refresh),
    vscode.window.onDidChangeActiveTextEditor(refresh),
    vscode.workspace.onDidChangeTextDocument(event => {
      if (event.document === vscode.window.activeTextEditor?.document) refresh();
    }),
    vscode.languages.onDidChangeDiagnostics(event => {
      const uri = vscode.window.activeTextEditor?.document.uri.toString();
      if (event.uris.some(item => item.toString() === uri)) refresh();
    }),
  ];
  refresh();
  return { dispose() {
    disposed = true;
    cached = undefined;
    generation++;
    clearTimeout(timer);
    for (const registration of registrations) registration.dispose();
    void setState();
  } };
}

module.exports = { registerDialogueFormatting };
