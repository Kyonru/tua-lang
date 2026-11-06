'use strict';
const fs = require('node:fs'), path = require('node:path'), cp = require('node:child_process'), assert = require('node:assert/strict');
const { makeProject, connect, save, binary } = require('../devex-2026-11-06/harness');
const cases = [
  { id:'HOV-01-void', source:'love.graphics.newCanvas(20,20):setFilter("nearest", "linear")\n', needle:'setFilter', contains:'setFilter' },
  { id:'HOV-01-value-spaced', source:'local width = love.graphics.newCanvas(20,20): getWidth()\n', needle:'getWidth', contains:'getWidth' },
  { id:'HOV-01-value-newline', source:'local width = love.graphics.newCanvas(20,20):\n  getWidth()\n', needle:'getWidth', contains:'getWidth' },
  { id:'HOV-02', source:'local function use(value: number): number return value end\ndo\n local function use(value: string): string return value end\n use("inner")\nend\nuse(1)\n', needle:'use("inner")', contains:'value: string' },
  { id:'HOV-03', source:'class Counter {\n value: number = 0\n read(): number { return self.value }\n}\nlocal result = 40 + 2\nprint(result)\n', needle:'+', contains:'number', offset:0 },
  { id:'HOV-04', source:'local function damage(amount: number = 5): number return amount end\nlocal value = damage()\ndamage(1)\n', needle:'damage(1)', contains:'amount?: number' },
  { id:'CMP-02', source:'local function f(x: number) -- hello\n return x\nend\nprint(f(1))\n', needle:'f(1)', contains:'number' },
  { id:'WF-01', source:'local label = "😀😀😀😀😀😀😀😀"; local score: number = 7; print(score, label)\n', needle:'score, label', contains:'score: number', rename:true },
];
(async () => {
 const results=[];
 for (const c of cases) {
  const root=makeProject({'src/main.tua':c.source}, 'source="src"\nout="build"\n[diagnostics]\nrules=false\n');
  const s=await connect(root);
  try {
   const doc=s.open('src/main.tua');
   const hover=await s.query('hover','textDocument/hover',doc,c.needle,c.offset??1);
   const diagnostic=await s.query('diagnostic','textDocument/diagnostic',doc);
   const checked=cp.spawnSync(binary,['check'],{cwd:root,encoding:'utf8'});
   let applied;
   if (c.rename) {
    const rename=await s.query('rename','textDocument/rename',doc,c.needle,1,{newName:'points'});
    applied=c.source;
    const edits=Object.values(rename.changes||{}).flat().sort((a,b)=>b.range.start.character-a.range.start.character);
    for (const edit of edits) applied=applied.slice(0,edit.range.start.character)+edit.newText+applied.slice(edit.range.end.character);
   }
   const result={id:c.id,source:c.source,cli:{status:checked.status,stdout:checked.stdout,stderr:checked.stderr},evidence:s.inbound,applied};
   const checks=[];
   try { assert.ok(hover?.contents?.value?.includes(c.contains), `hover lacks ${c.contains}`); checks.push('hover'); assert.equal(checked.status,0,'CLI check'); checks.push('CLI'); assert.ok(!diagnostic.items?.some(item=>item.severity===1),'LSP error'); checks.push('diagnostics'); if(c.rename){assert.equal(applied,c.source.replaceAll('score','points'));checks.push('applied rename');} result.pass=true; }
   catch(error){result.pass=false;result.failure=String(error);}
   result.checks=checks;results.push(result); console.log(c.id,result.pass?'PASS':result.failure);
  } finally { await s.close();fs.rmSync(root,{recursive:true,force:true}); }
 }
 save(path.join(__dirname,'independent-hover-replay.json'),{binary,results});
 if(results.some(result=>!result.pass)) process.exitCode=1;
})().catch(error=>{console.error(error);process.exitCode=1;});
