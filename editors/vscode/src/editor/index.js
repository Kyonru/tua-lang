"use strict";

const { registerGeneratedLuaCommand } = require("./generated-lua");
const { registerHoverActions } = require("./hover-actions");
const { registerInlayHintPresentation } = require("./inlay-hints");
const { registerOllamaCompletion } = require("./ollama-completion");
const { registerSymbolCommands } = require("./symbols");
const { registerTypeExplanation } = require("./type-explanation");

function registerEditorDevex(options) {
  const generatedLua = registerGeneratedLuaCommand(options);
  const hoverActions = registerHoverActions(options);
  const inlayHints = registerInlayHintPresentation(options);
  const ollamaCompletion = registerOllamaCompletion(options);
  const symbols = registerSymbolCommands(options);
  const typeExplanation = registerTypeExplanation(options);

  return {
    middleware: inlayHints.middleware,
    ollamaCompletion,
    dispose() {
      generatedLua.dispose();
      hoverActions.dispose();
      inlayHints.dispose();
      ollamaCompletion.dispose();
      symbols.dispose();
      typeExplanation.dispose();
    },
  };
}

module.exports = {
  registerEditorDevex,
};
