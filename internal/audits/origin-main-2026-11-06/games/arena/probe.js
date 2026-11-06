'use strict';
const fs = require('node:fs');
const assert = require('node:assert/strict');
const path = require('node:path');
const { connect, save, completionItems, binary } = require('../../../devex-2026-11-06/harness');
(async () => {
  const s = await connect(__dirname, {capabilities:{textDocument:{completion:{completionItem:{snippetSupport:true}}}}});
  try {
    const doc = s.open('src/main.tua');
    await s.query('imported-player-field', 'textDocument/hover', doc, 'player.speed', 8);
    await s.query('indexed-enemy-field', 'textDocument/hover', doc, 'enemy.radius', 7);
    await s.query('imported-function', 'textDocument/hover', doc, 'entities.moveToward', 12);
    await s.query('cross-file-signature', 'textDocument/signatureHelp', doc, 'entities.moveToward(enemy, player, dt)', 27);
    await s.query('member-completion', 'textDocument/completion', doc, 'player.speed', 7);
    await s.query('module-completion', 'textDocument/completion', doc, 'entities.moveToward', 9);
    await s.query('definition', 'textDocument/definition', doc, 'entities.moveToward', 12);
    await s.query('diagnostics', 'textDocument/diagnostic', doc);
    await s.query('formatting', 'textDocument/formatting', doc, undefined, 0, {options:{tabSize:2,insertSpaces:true}});
    const result = label => s.inbound.find(r => r.label === label).result;
    assert.match(result('imported-player-field').contents.value, /speed: number/);
    assert.match(result('indexed-enemy-field').contents.value, /radius: number/);
    assert.match(result('imported-function').contents.value, /body: Body, target: Body, dt: number/);
    assert.equal(result('cross-file-signature').activeParameter, 1);
    assert.equal(result('cross-file-signature').signatures[0].parameters.length, 3);
    assert.deepEqual(completionItems(result('member-completion')).map(i => i.label).sort(), ['radius','speed','x','y']);
    assert.deepEqual(completionItems(result('module-completion')).map(i => i.label).sort(), ['create','moveToward','touches']);
    assert.ok(result('definition').uri.endsWith('/src/entities.tua'));
    assert.deepEqual(result('diagnostics').items, []);
    assert.ok(result('formatting').length > 0); // Response only; not formatted-code validity.
    save(path.join(__dirname,'lsp-evidence.json'),{binary,verified:true,requests:s.inbound});
    for(const entry of s.inbound) {
      let result=entry.result;
      if(entry.method.endsWith('/completion')) result=completionItems(result).map(x=>x.label);
      console.log(entry.label, entry.ms.toFixed(2), JSON.stringify(result).slice(0,1500));
    }
  } finally { await s.close(); }
})().catch(e=>{console.error(e);process.exitCode=1;});
