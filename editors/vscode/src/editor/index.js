"use strict";

const { registerInlayHintPresentation } = require("./inlay-hints");

function registerEditorDevex(options) {
  return registerInlayHintPresentation(options);
}

module.exports = {
  registerEditorDevex,
};
