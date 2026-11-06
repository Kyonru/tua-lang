"use strict";
const path = require('node:path'), fs = require('node:fs');
const {makeProject, connect, save, position, binary, delay} = require('../harness');
(async () => {
  const report = {binary, cases: []};
  for (const prefix of ['abc', '漢字', '😀', '😀😀😀😀😀😀😀😀']) {
    const text = `local prefix = "${prefix}"; local value: number = 42; print(value)\n`;
    const root = makeProject({'src/main.tua': text}); const session = await connect(root);
    try {
      const doc = session.open('src/main.tua');
      const expectedUse = position(text, 'value)', 0, true);
      const expectedDeclaration = position(text, 'value:');
      await session.query('hover-use','textDocument/hover',doc,'value)',1);
      await session.query('definition-use','textDocument/definition',doc,'value)',1);
      await session.query('references-use','textDocument/references',doc,'value)',1,{context:{includeDeclaration:true}});
      await session.query('rename-use','textDocument/rename',doc,'value)',1,{newName:'renamed'});
      await session.query('symbols','textDocument/documentSymbol',doc);
      session.change(doc, `local prefix = "${prefix}"; local value: number = "bad"; print(value)\n`);
      await session.query('diagnostics','textDocument/diagnostic',doc);
      session.change(doc, `local prefix = "${prefix}"; local obj = {field=1}; print(obj.field)\n`);
      await session.query('member-completion','textDocument/completion',doc,'obj.field',4);
      report.cases.push({prefix, text, expectedUse, expectedDeclaration, evidence:session.inbound});
    } finally {await session.close();fs.rmSync(root,{recursive:true,force:true});}
  }
  save(path.join(__dirname,'unicode-evidence.json'),report);
  console.log(report.cases.map(c=>({prefix:c.prefix, hover:c.evidence.find(e=>e.label==='hover-use').result, definition:c.evidence.find(e=>e.label==='definition-use').result})));
})().catch(error=>{console.error(error);process.exitCode=1;});
