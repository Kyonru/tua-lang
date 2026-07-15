"use strict";

const vscode = require("vscode");
const {
  contentChangeDebugLabel,
  documentDebugLabel,
} = require("../shared/documents");

const SUGGEST_TRIGGER_DELAY_MS = 25;
const SUGGEST_TRIGGER_PATTERN = /^[A-Za-z0-9_.:'"@-]$/;

function registerSuggestTrigger({ debugLog, getServerState, isActiveDocument, isTuaDocument }) {
  let pendingTrigger;

  const changeSubscription = vscode.workspace.onDidChangeTextDocument((event) => {
    if (!isTuaDocument(event.document)) {
      return;
    }

    if (!isActiveDocument(event.document)) {
      debugLog(`suggest skip inactive Tua document: ${documentDebugLabel(event.document)}`, {
        repeatKey: "suggest.skipInactive",
        repeatLabel: "suggest skip inactive Tua document",
      });
      return;
    }

    const changes = event.contentChanges.map(contentChangeDebugLabel).join(", ");

    if (!event.contentChanges.some((change) => SUGGEST_TRIGGER_PATTERN.test(change.text))) {
      debugLog(
        `suggest skip non-trigger change: ${documentDebugLabel(event.document)} changes=[${changes}]`,
        {
          repeatKey: "suggest.skipNonTrigger",
          repeatLabel: "suggest skip non-trigger change",
        },
      );
      return;
    }

    if (pendingTrigger) {
      debugLog("suggest debounce: canceled older pending trigger.", {
        repeatKey: "suggest.debounceCanceled",
        repeatLabel: "suggest debounce canceled",
      });
      clearTimeout(pendingTrigger);
    }

    const document = event.document;
    const serverState = getServerState();

    debugLog(
      `suggest scheduled in ${SUGGEST_TRIGGER_DELAY_MS}ms: ${documentDebugLabel(document)} changes=[${changes}] server=${serverState}`,
    );

    pendingTrigger = setTimeout(() => {
      pendingTrigger = undefined;

      const latestServerState = getServerState();
      if (latestServerState !== "running" || !isActiveDocument(document)) {
        debugLog(
          `suggest canceled before firing: ${documentDebugLabel(document)} server=${latestServerState}`,
          {
            repeatKey: "suggest.canceledBeforeFiring",
            repeatLabel: "suggest canceled before firing",
          },
        );
        return;
      }

      debugLog(`suggest trigger command: ${documentDebugLabel(document)}`);
      vscode.commands.executeCommand("editor.action.triggerSuggest");
    }, SUGGEST_TRIGGER_DELAY_MS);
  });

  return vscode.Disposable.from(changeSubscription, {
    dispose() {
      if (pendingTrigger) {
        clearTimeout(pendingTrigger);
        pendingTrigger = undefined;
      }
    },
  });
}

module.exports = {
  registerSuggestTrigger,
};
