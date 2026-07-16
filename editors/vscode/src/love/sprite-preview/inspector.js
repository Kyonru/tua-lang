"use strict";

async function inspectSpriteSheets({ client, document, position }) {
  if (!client) {
    throw new Error("The Tua language server is not running.");
  }

  const params = {
    textDocument: {
      uri: document.uri.toString(),
    },
  };

  if (position) {
    params.position = {
      line: position.line,
      character: position.character,
    };
  }

  let response = await client.sendRequest("tua/inspect", params);

  if (isStale(response, document)) {
    response = await client.sendRequest("tua/inspect", params);
  }

  if (isStale(response, document)) {
    throw new Error("Sprite inspection is stale. Reload after document synchronization.");
  }

  return {
    analysisRevision: numberOrUndefined(response?.analysisRevision),
    documentVersion: numberOrUndefined(response?.documentVersion),
    items: (Array.isArray(response?.items) ? response.items : []).filter(
      (item) => item?.kind === "spriteSheet" && typeof item.name === "string",
    ),
  };
}

function isStale(response, document) {
  return (
    Number.isInteger(document.version) &&
    response?.documentVersion !== document.version
  );
}

function numberOrUndefined(value) {
  return Number.isFinite(value) ? Number(value) : undefined;
}

module.exports = {
  inspectSpriteSheets,
};
