"use strict";
const path=require('node:path'),fs=require('node:fs'),{spawnSync}=require('node:child_process');
const {makeProject,connect,save,binary,completionItems}=require('../harness');
(async()=>{
 const cases=[
 {id:'contextual-callback-independent',files:{'src/main.tua':'local function render(cb: (n: number) -> string): string return cb(3) end\nlocal output = render(function(n) return n + 2 end)\nprint(type(output))\n'},needle:'output)'},
 {id:'imported-chain-independent',files:{'src/main.tua':'local module = require("factory")\nlocal actual = module.make().stats.hp\nlocal label: string = actual\nprint(type(label))\n','src/factory.tua':'local function make() return {stats={hp=99}} end\nreturn {make=make}\n'},needle:'actual\n'},
 {id:'annotated-return-control',files:{'src/main.tua':'local function render(n: number): string return n + 2 end\nprint(render(3))\n'},needle:'render(3)'}
 ];const out=[];
 for(const c of cases){ const root=makeProject(c.files);const s=await connect(root);try{
  const cli=spawnSync(binary,['check'],{cwd:root,encoding:'utf8'});
  const doc=s.open('src/main.tua');await s.query('hover','textDocument/hover',doc,c.needle,1);await s.query('diagnostics','textDocument/diagnostic',doc);
  const build=spawnSync(binary,['build'],{cwd:root,encoding:'utf8'});
  const runtime=build.status===0?spawnSync('lua',['main.lua'],{cwd:path.join(root,'build'),encoding:'utf8'}):null;
  out.push({...c,check:{status:cli.status,stdout:cli.stdout,stderr:cli.stderr},build:{status:build.status,stderr:build.stderr},runtime:runtime&&{status:runtime.status,stdout:runtime.stdout,stderr:runtime.stderr},evidence:s.inbound});
 }finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}}
 const root=makeProject({'src/main.tua':'local result = createEnt\n','src/entities.tua':'export type Entity = {health:number}\nlocal function createEntity(): Entity return {health=1} end\nreturn {createEntity=createEntity}\n'});
 const s=await connect(root);try{const d=s.open('src/main.tua');for(const [label,context] of [['invoked',{triggerKind:1}],['omitted',undefined],['incomplete-retrigger',{triggerKind:3}]]){
  await s.query(label,'textDocument/completion',d,'createEnt',9,context?{context}:{});
 }out.push({id:'autoimport-context-independent',text:d.text,evidence:s.inbound});}finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}
 save(path.join(__dirname,'crosscheck-evidence.json'),out);
 for(const c of out)console.log(c.id,c.check||'',c.runtime||'',c.evidence.slice(1).map(e=>({label:e.label,result:e.method==='textDocument/completion'?completionItems(e.result).filter(i=>i.label==='createEntity'):e.result})));
})().catch(e=>{console.error(e);process.exitCode=1});
