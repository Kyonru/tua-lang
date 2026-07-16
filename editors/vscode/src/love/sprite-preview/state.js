"use strict";

function createSpritePreviewState() {
  let panel;
  let document;
  let renderSequence = 0;

  return {
    attach(nextPanel) {
      panel = nextPanel;
    },
    beginRender(nextDocument) {
      document = nextDocument;
      renderSequence += 1;
      return renderSequence;
    },
    clearPanel(disposedPanel) {
      if (panel === disposedPanel) {
        panel = undefined;
        document = undefined;
        renderSequence += 1;
      }
    },
    dispose() {
      const current = panel;

      panel = undefined;
      document = undefined;
      renderSequence += 1;
      current?.dispose?.();
    },
    document() {
      return document;
    },
    isCurrentRender(sequence) {
      return sequence === renderSequence;
    },
    panel() {
      return panel;
    },
  };
}

module.exports = {
  createSpritePreviewState,
};
