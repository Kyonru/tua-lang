'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process');
const {makeProject,connect,save,binary}=require('../harness');
const cases=[
 {name:'required-nilable',source:'local function scale(value: number | nil): number return value or 2 end\nscale()\nscale(1)\n',needle:'scale(1)',offset:6},
 {name:'optional',source:'local function scale(value?: number): number return value or 2 end\nscale()\nscale(1)\n',needle:'scale(1)',offset:6},
 {name:'default',source:'local function scale(value: number = 2): number return value end\nscale()\nscale(1)\n',needle:'scale(1)',offset:6},
 {name:'font-chain',source:'local height=love.graphics.newFont(14):getHeight()\nprint(height)\n',needle:'getHeight()',offset:3},
 {name:'class-arithmetic',source:'class Hero {\n health: number = 10\n read(): number { return self.health }\n}\nlocal result = 50 + 3\nprint(result)\n',needle:'50 + 3',offset:3}
];
(async()=>{const result=[];for(const c of cases){const root=makeProject({'src/main.tua':c.source},'source="src"\nout="build"\n[diagnostics]\nrules=false\n');const s=await connect(root);try{const doc=s.open('src/main.tua');await s.query('hover','textDocument/hover',doc,c.needle,['required-nilable','optional','default'].includes(c.name)?2:c.offset);if(c.name==='font-chain')await s.query('signature','textDocument/signatureHelp',doc,'getHeight(',10);else if(!c.name.startsWith('class'))await s.query('signature','textDocument/signatureHelp',doc,c.needle,c.offset);const check=cp.spawnSync(binary,['check'],{cwd:root,encoding:'utf8'});result.push({name:c.name,source:c.source,requests:s.inbound,check:{status:check.status,stdout:check.stdout,stderr:check.stderr}});}finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}}save(path.join(__dirname,'review-hover-evidence.json'),result);})().catch(e=>{console.error(e);process.exitCode=1});
