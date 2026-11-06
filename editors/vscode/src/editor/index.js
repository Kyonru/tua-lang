"use strict";

const { registerDeclarationActions } = require("./declaration-actions");
const { registerGeneratedLuaCommand } = require("./generated-lua");
const { registerHoverActions } = require("./hover-actions");
const { registerInlayHintPresentation } = require("./inlay-hints");
const { registerInlineCompletion } = require("./inline-completion");
const { registerSymbolCommands } = require("./symbols");
const { registerTypeExplanation } = require("./type-explanation");
const { registerUnitTestScaffold } = require("./unit-test-scaffold");

function registerEditorDevex(options) {
  const generatedLua = registerGeneratedLuaCommand(options);
  const declarationActions = registerDeclarationActions(options);
  const hoverActions = registerHoverActions(options);
  const inlayHints = registerInlayHintPresentation(options);
  const inlineCompletion = registerInlineCompletion(options);
  const symbols = registerSymbolCommands(options);
  const typeExplanation = registerTypeExplanation({ ...options, onInspect: inlayHints.inspect });
  const unitTestScaffold = registerUnitTestScaffold(options);

  return {
    middleware: inlayHints.middleware,
    inlineCompletion,
    flushStats() {
      return inlineCompletion.flushLocalStats();
    },
    dispose() {
      generatedLua.dispose();
      declarationActions.dispose();
      hoverActions.dispose();
      inlayHints.dispose();
      inlineCompletion.dispose();
      symbols.dispose();
      typeExplanation.dispose();
      unitTestScaffold.dispose();
    },
  };
}

module.exports = {
  registerEditorDevex,
};
