"use strict";

const { registerGeneratedLuaCommand } = require("./generated-lua");
const { registerInlayHintPresentation } = require("./inlay-hints");
const { registerTypeExplanation } = require("./type-explanation");

function registerEditorDevex(options) {
  const generatedLua = registerGeneratedLuaCommand(options);
  const inlayHints = registerInlayHintPresentation(options);
  const typeExplanation = registerTypeExplanation(options);

  return {
    middleware: inlayHints.middleware,
    dispose() {
      generatedLua.dispose();
      inlayHints.dispose();
      typeExplanation.dispose();
    },
  };
}

module.exports = {
  registerEditorDevex,
};
