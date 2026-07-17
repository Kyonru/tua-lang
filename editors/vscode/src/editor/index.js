"use strict";

const { registerGeneratedLuaCommand } = require("./generated-lua");
const { registerInlayHintPresentation } = require("./inlay-hints");
const { registerSymbolCommands } = require("./symbols");
const { registerTypeExplanation } = require("./type-explanation");

function registerEditorDevex(options) {
  const generatedLua = registerGeneratedLuaCommand(options);
  const inlayHints = registerInlayHintPresentation(options);
  const symbols = registerSymbolCommands(options);
  const typeExplanation = registerTypeExplanation(options);

  return {
    middleware: inlayHints.middleware,
    dispose() {
      generatedLua.dispose();
      inlayHints.dispose();
      symbols.dispose();
      typeExplanation.dispose();
    },
  };
}

module.exports = {
  registerEditorDevex,
};
