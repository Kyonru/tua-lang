"use strict";

const assert = require("assert");
const fs = require("fs");
const path = require("path");
const oniguruma = require("vscode-oniguruma");
const textmate = require("vscode-textmate");

const extensionRoot = path.resolve(__dirname, "..");
const grammarPath = path.join(extensionRoot, "syntaxes", "tua.tmLanguage.json");
const fixturePath = path.join(extensionRoot, "test", "fixtures", "highlighting.tua");

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm"));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));

  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner(patterns) {
        return new oniguruma.OnigScanner(patterns);
      },
      createOnigString(text) {
        return new oniguruma.OnigString(text);
      },
    }),
    loadGrammar(scopeName) {
      if (scopeName !== "source.tua") {
        return null;
      }

      return textmate.parseRawGrammar(fs.readFileSync(grammarPath, "utf8"), grammarPath);
    },
  });
  const grammar = await registry.loadGrammar("source.tua");
  assert(grammar, "expected source.tua grammar to load");

  const source = fs.readFileSync(fixturePath, "utf8");
  const tokenizedLines = [];
  let ruleStack = textmate.INITIAL;

  for (const line of source.split("\n")) {
    const result = grammar.tokenizeLine(line, ruleStack);
    tokenizedLines.push({ line, tokens: result.tokens });
    ruleStack = result.ruleStack;
  }

  assertScope(tokenizedLines, "---@class LegacyPlayer", "@class", "storage.type.annotation.tua");
  assertScope(tokenizedLines, "---@class LegacyPlayer", "LegacyPlayer", "entity.name.type.annotation.tua");
  assertScope(tokenizedLines, "---@field hp number", "hp", "variable.other.property.annotation.tua");
  assertScope(tokenizedLines, "type Callback", "Callback", "entity.name.type.alias.tua");
  assertScope(tokenizedLines, "enum PlayerState", "PlayerState", "entity.name.type.enum.tua");
  assertScope(tokenizedLines, '  "idle"', "idle", "variable.other.enummember.tua");
  assertScope(tokenizedLines, "open class Entity", "Entity", "entity.name.type.class.tua");
  assertScope(tokenizedLines, "open class Entity", "BaseEntity", "entity.other.inherited-class.tua");
  assertScope(tokenizedLines, "  x: number", "x", "variable.other.property.declaration.tua");
  assertScope(tokenizedLines, "  init(name: string)", "init", "entity.name.function.member.tua");
  assertScope(tokenizedLines, "  init(name: string)", "name", "variable.parameter.tua");
  assertScope(tokenizedLines, "local events = signal()", "signal", "support.function.tua");
  assertScope(tokenizedLines, "local game = store({", "store", "support.function.tua");
  assertScope(tokenizedLines, "local controls = input({", "input", "support.function.tua");
  assertScope(tokenizedLines, 'events:on("player.hit"', "player.hit", "entity.name.event.tua");
  assertScope(tokenizedLines, 'local Player = import("game.player")', "game.player", "entity.name.namespace.module.tua");
  assertScope(tokenizedLines, 'local sprite = image "assets/player.png"', "assets/player.png", "string.quoted.double.filename.asset.tua");
  assertScope(tokenizedLines, "local bullets = objectPool({", "objectPool", "support.function.tua");
  assertScope(tokenizedLines, "local world = ecs.world()", "ecs", "support.variable.namespace.tua");
  assertScope(tokenizedLines, "world.entities.player.position.x", "entities", "variable.other.property.tua");
  assertScope(tokenizedLines, "world.entities.player.position.x", "player", "variable.other.property.tua");
  assertScope(tokenizedLines, "world.entities.player.position.x", "position", "variable.other.property.tua");
  assertScope(tokenizedLines, "world.entities.player.position.x", "x", "variable.other.property.tua");
  assertScope(tokenizedLines, "function love.load()", "load", "entity.name.function.tua");
  assertScope(tokenizedLines, "if value is Entity then", "is", "keyword.operator.word.tua");
  assertScope(tokenizedLines, "::retry::", "retry", "entity.name.label.tua");
  assertScope(tokenizedLines, "local text = [=[long text]=]", "long text", "string.quoted.other.multiline.tua");
  assertScope(tokenizedLines, "--[=[block comment]=]", "block comment", "comment.block.tua");

  console.log(`validated Tua TextMate scopes (${tokenizedLines.length} fixture lines)`);
}

function assertScope(tokenizedLines, lineFragment, tokenText, expectedScope) {
  const lineEntry = tokenizedLines.find(({ line }) => line.includes(lineFragment));
  assert(lineEntry, `missing fixture line containing ${JSON.stringify(lineFragment)}`);

  const start = lineEntry.line.indexOf(tokenText);
  assert(start >= 0, `missing ${JSON.stringify(tokenText)} in ${JSON.stringify(lineEntry.line)}`);

  const token = lineEntry.tokens.find(
    (candidate) => candidate.startIndex <= start && candidate.endIndex >= start + tokenText.length,
  );
  assert(token, `no TextMate token covers ${JSON.stringify(tokenText)} in ${JSON.stringify(lineEntry.line)}`);
  assert(
    token.scopes.includes(expectedScope),
    `expected ${JSON.stringify(tokenText)} to include ${expectedScope}, got ${token.scopes.join(", ")}`,
  );
}
