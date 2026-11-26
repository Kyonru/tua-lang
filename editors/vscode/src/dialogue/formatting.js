"use strict";

const vscode = require("vscode");

const COMMAND = "tua.dialogue.formatText";
const APPLY = "tua.dialogue.applyTextFormat";
const KIND = "refactor.rewrite.dialogueText";

function registerDialogueFormatting() {
  let timer;
  let generation = 0;
  let pending;
  let disposed = false;

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

  async function apply(target, action) {
    if (!current(target)) return false;
    // Only compiler edits for this document are accepted, with one undo boundary.
    const entries = action.edit.entries();
    if (entries.length !== 1 || entries[0][0].toString() !== target.document.uri.toString()) return false;
    return target.editor.edit(builder => {
      for (const edit of entries[0][1]) builder.replace(edit.range, edit.newText);
    }, { undoStopBefore: true, undoStopAfter: true });
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
    if (picked) await apply(target, picked.action);
  }

  const registrations = [
    vscode.commands.registerCommand(COMMAND, choose),
    vscode.commands.registerCommand(APPLY, async (id, index) => {
      const item = pending;
      if (!item || id !== item.id || !Number.isInteger(index) || !item.actions[index]) return false;
      pending = undefined;
      return apply(item.target, item.actions[index]);
    }),
    vscode.languages.registerHoverProvider({ language: "tdialogue" }, {
      async provideHover(document, position, token) {
        const target = capture();
        if (!target || document !== target.document || !target.selection.contains(position)) return undefined;
        const available = await actions(target);
        if (token.isCancellationRequested || !current(target) || !available.length) return undefined;
        const id = ++generation;
        pending = { id, target, actions: available };
        const markdown = new vscode.MarkdownString();
        markdown.isTrusted = { enabledCommands: [APPLY] };
        const links = available.map((action, index) => {
          const args = encodeURIComponent(JSON.stringify([id, index]));
          // Titles are compiler-owned, but still escape Markdown delimiters.
          const title = action.title.replace(/[\\[\]()*_`]/g, "\\$&");
          return `[${title}](command:${APPLY}?${args})`;
        });
        markdown.appendMarkdown(`**Format dialogue**\n\n${links.join(" · ")}`);
        return new vscode.Hover(markdown, target.selection);
      },
    }),
    vscode.window.onDidChangeTextEditorSelection(event => {
      clearTimeout(timer);
      pending = undefined;
      const target = capture(event.textEditor);
      if (!target || event.kind !== vscode.TextEditorSelectionChangeKind.Mouse ||
          !vscode.workspace.getConfiguration("tua.dialogue", target.document.uri).get("formattingToolbar", true)) return;
      timer = setTimeout(async () => {
        if (!current(target)) return;
        try {
          const available = await actions(target);
          if (available.length && current(target)) {
            await vscode.commands.executeCommand("editor.action.showHover", { focus: "noAutoFocus" });
          }
        } catch {
          // Server restart/cancellation should never interrupt text selection.
        }
      }, 250);
    }),
  ];
  return { dispose() {
    disposed = true;
    clearTimeout(timer);
    pending = undefined;
    for (const registration of registrations) registration.dispose();
  } };
}

module.exports = { registerDialogueFormatting };
