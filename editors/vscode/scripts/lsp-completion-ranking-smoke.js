"use strict";

const path = require("path");
const {
  LspClient,
  completionItems,
  delay,
  pathToFileURL,
  positionAfter,
  resolveCommand,
  sourceFiles,
} = require("./lsp-profile-common");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const root = path.resolve(process.argv[3] || "benchmarks/lsp/medium");
const src = path.join(root, "src");

(async () => {
  const client = new LspClient(command, root);
  const files = sourceFiles(src);
  const main = files.find((file) => file.path.endsWith(`${path.sep}main.tua`));

  if (!main) {
    throw new Error(`missing benchmark main.tua under ${src}`);
  }

  await client.start();
  await client.request("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(root).href,
    capabilities: {},
    workspaceFolders: [{ uri: pathToFileURL(root).href, name: path.basename(root) }],
  });
  client.notify("initialized", {});

  for (const file of files) {
    client.notify("textDocument/didOpen", {
      textDocument: {
        uri: file.uri,
        languageId: file.path.endsWith(".lua") ? "lua" : "tua",
        version: 1,
        text: file.text,
      },
    });
  }

  await delay(20);

  await assertCompletionOrder(client, main, "Player.move", "Player.", ["makePlayer", "move"]);
  await assertCompletionOrder(
    client,
    main,
    "rankedPlayer.stats",
    "rankedPlayer.",
    ["stats", "pos", "name"],
  );

  await client.request("shutdown", null);
  client.notify("exit", null);
  await client.done();

  console.log("completion ranking smoke passed");
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});

async function assertCompletionOrder(client, file, fullNeedle, receiverPrefix, expectedLabels) {
  const response = await client.request("textDocument/completion", {
    textDocument: { uri: file.uri },
    position: positionAfterReceiverPrefix(file.text, fullNeedle, receiverPrefix),
  });
  const labels = completionItems(response).map((item) => item.label);
  const actual = labels.slice(0, expectedLabels.length);

  for (let index = 0; index < expectedLabels.length; index += 1) {
    if (actual[index] !== expectedLabels[index]) {
      throw new Error(
        `${receiverPrefix} expected first labels ${expectedLabels.join(", ")}, ` +
          `got ${actual.join(", ")} from ${labels.slice(0, 12).join(", ")}`,
      );
    }
  }
}

function positionAfterReceiverPrefix(source, fullNeedle, receiverPrefix) {
  const position = positionAfter(source, fullNeedle);

  position.character -= fullNeedle.length - receiverPrefix.length;

  return position;
}
