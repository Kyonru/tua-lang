"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const fs = require("node:fs");
const path = require("node:path");
const oniguruma = require("vscode-oniguruma");
const textmate = require("vscode-textmate");
const manifest = require("../../package.json");
const { DOCUMENT_SELECTOR } = require("../../src/constants");

test("dialogue scripts register their own language and reach the language server", () => {
  assert(manifest.activationEvents.includes("onLanguage:tdialogue"));
  const language = manifest.contributes.languages.find((entry) => entry.id === "tdialogue");
  assert.deepEqual(language.extensions, [".tdialogue"]);
  assert(DOCUMENT_SELECTOR.some((entry) => entry.language === "tdialogue"));
  assert(DOCUMENT_SELECTOR.some((entry) => entry.pattern === "**/*.tdialogue"));
});

test("dialogue grammar highlights imports and speakers without highlighting prose as code", async () => {
  const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm"));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const grammarPath = path.join(__dirname, "../../syntaxes/tdialogue.tmLanguage.json");
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
      createOnigString: (text) => new oniguruma.OnigString(text),
    }),
    loadGrammar: async () => textmate.parseRawGrammar(fs.readFileSync(grammarPath, "utf8"), grammarPath),
  });
  const grammar = await registry.loadGrammar("text.tdialogue");
  const directive = grammar.tokenizeLine('@characters Cast "characters"', textmate.INITIAL);
  assert(directive.tokens.some((token) => token.scopes.includes("keyword.control.tdialogue")));
  assert(directive.tokens.some((token) => token.scopes.includes("string.quoted.double.tdialogue")));
  const line = "Cast.Guide: local function Hello (end)";
  const tokens = grammar.tokenizeLine(line, directive.ruleStack).tokens;
  assert(tokens[0].scopes.includes("variable.other.tdialogue"));
  const prose = tokens.find((token) => token.startIndex <= line.indexOf("local") && token.endIndex > line.indexOf("local"));
  assert.deepEqual(prose.scopes, ["text.tdialogue"]);
  for (const line of ['@event shown 0.2 portrait.pulse', '@timing hold 3 fade 0.35', '@context \"story\"', '@bind name context.player.name', '@branch context.ready [target:yes] [else:no]', '@visible context.ready', '@enabled context.coins >= 20', '@reason Requires coins', '@fallback closed']) {
    const tokens = grammar.tokenizeLine(line, textmate.INITIAL).tokens;
    assert(tokens.some((token) => token.scopes.includes("keyword.control.tdialogue")));
  }
  const condition = grammar.tokenizeLine('@visible not context.hidden and context.coins >= 2 or false', textmate.INITIAL).tokens;
  assert(condition.some((token) => token.scopes.includes("keyword.operator.logical.tdialogue")));
  assert(condition.some((token) => token.scopes.includes("constant.language.tdialogue")));
  const identified = grammar.tokenizeLine('@event shown 0 pulse [id:arrival.pulse] "soft"', textmate.INITIAL).tokens;
  assert(identified.some((token) => token.scopes.includes("entity.name.tag.tdialogue")));
  for (const line of ['@events "story.events"', '@emit shown 0 reward [id:reward.one] {"coins":10}']) {
    const tokens = grammar.tokenizeLine(line, textmate.INITIAL).tokens;
    assert(tokens.some((token) => token.scopes.includes("keyword.control.tdialogue")));
  }
  registry.dispose();
});
