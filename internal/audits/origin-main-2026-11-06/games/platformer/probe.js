'use strict';
const path=require('node:path');
const assert=require('node:assert/strict');
const {connect,save}=require('../../../devex-2026-11-06/harness');
(async()=>{
 const root=path.resolve(__dirname); const s=await connect(root,{capabilities:{textDocument:{completion:{completionItem:{snippetSupport:true}}}}});
 try {
 const d=s.open('src/main.tua');
 await s.query('imported-player-hover','textDocument/hover',d,'world.player.x',13);
 await s.query('love-method-hover','textDocument/hover',d,'love.graphics.rectangle',16);
 await s.query('rectangle-signature','textDocument/signatureHelp',d,'"fill",world.player.x,',22);
 await s.query('player-definition','textDocument/definition',d,'world.player.x',7);
 await s.query('diagnostics','textDocument/diagnostic',d);
 await s.query('format','textDocument/formatting',d,undefined,0,{options:{tabSize:2,insertSpaces:true}});
 s.change(d,d.text+'\nlocal px = world.player.\n');
 await s.query('incomplete-imported-player-completion','textDocument/completion',d,'world.player.',13,{context:{triggerKind:1}});
 const result=label=>s.inbound.find(r=>r.label===label).result;
 assert.match(result('imported-player-hover').contents.value,/world\.player\.x: number/);
 assert.match(result('love-method-hover').contents.value,/Draws a rectangle/);
 assert.equal(result('rectangle-signature').activeParameter,2);
 assert.equal(result('rectangle-signature').signatures.length,2);
 assert.ok(result('player-definition').uri.endsWith('/src/world.tua'));
 assert.deepEqual(result('diagnostics').items,[]);
 assert.ok(result('format').length>0); // Response only; does not establish formatted-code validity.
 assert.deepEqual(result('incomplete-imported-player-completion').map(i=>i.label).sort(),['grounded','h','vx','vy','w','x','y'].sort());
 save(path.join(__dirname,'lsp-evidence.json'),s.inbound);
 }finally{await s.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
