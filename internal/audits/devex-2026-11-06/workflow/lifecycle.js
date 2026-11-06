"use strict";
const fs = require('node:fs'), path = require('node:path');
const {makeProject, connect, save, binary, pathToFileURL, delay} = require('../harness');
(async()=>{
 const report={binary, sessions:[]};
 const main='local dep = require("dep")\nlocal total: number = dep.value\nprint(dep.value, total)\n';
 const root=makeProject({'src/main.tua':main,'src/dep.tua':'return {value=1}\n'});
 const s=await connect(root,{capabilities:{textDocument:{diagnostic:{dynamicRegistration:false}},workspace:{diagnostics:{refreshSupport:true}}}});
 try{
  const doc=s.open('src/main.tua');
  const sample=async label=>{await s.query(label+'-hover','textDocument/hover',doc,'dep.value',5);await s.query(label+'-diagnostics','textDocument/diagnostic',doc);};
  await sample('initial');
  const dep=s.open('src/dep.tua');s.change(dep,'return {value="updated"}\n');await sample('unsaved-dependency');
  s.client.notify('textDocument/didClose',{textDocument:{uri:dep.uri}});await sample('closed-dependency');
  fs.writeFileSync(path.join(root,'src/dep.tua'),'return {value=true}\n');
  s.client.notify('workspace/didChangeWatchedFiles',{changes:[{uri:dep.uri,type:2}]});await sample('watched-dependency');
  fs.renameSync(path.join(root,'src/dep.tua'),path.join(root,'src/renamed.tua'));
  const renamed=pathToFileURL(path.join(root,'src/renamed.tua')).href;
  s.client.notify('workspace/didChangeWatchedFiles',{changes:[{uri:dep.uri,type:3},{uri:renamed,type:1}]});await sample('renamed-missing-import');
  s.change(doc,main.replace('"dep"','"renamed"'));await sample('renamed-import-updated');await delay(350);await sample('renamed-import-updated-idle');
  fs.unlinkSync(path.join(root,'src/renamed.tua'));s.client.notify('workspace/didChangeWatchedFiles',{changes:[{uri:renamed,type:3}]});await sample('deleted-dependency');
  fs.writeFileSync(path.join(root,'src/renamed.tua'),'return {value=42}\n');s.client.notify('workspace/didChangeWatchedFiles',{changes:[{uri:renamed,type:1}]});await sample('recreated-dependency');
  s.change(doc,'local dep=require("renamed")\ndep.');await s.query('incomplete-completion','textDocument/completion',doc,'dep.',4);
  s.change(doc,main.replace('"dep"','"renamed"'));await sample('recovered-valid-edit');
  s.change(doc,'print(allowed_global)\n');await s.query('before-config','textDocument/diagnostic',doc);
  fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nglobals=["allowed_global"]\n');
  s.client.notify('workspace/didChangeWatchedFiles',{changes:[{uri:pathToFileURL(path.join(root,'tua.toml')).href,type:2}]});await s.query('after-config','textDocument/diagnostic',doc);
  await delay(300);await s.query('after-config-idle','textDocument/diagnostic',doc);
  report.sessions.push({name:'lifecycle-pull',files:{main,dep:'return {value=1}'},evidence:s.inbound,notifications:s.notifications});
 }finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}
 const a=makeProject({'src/main.tua':main,'src/dep.lua':'return {value=7}\n'});
 const b=makeProject({'src/main.tua':main.replace('total: number','total: string'),'src/dep.lua':'return {value="second"}\n'});
 for (const [label, root] of [['a',a],['b',b]]) {
  const solo=await connect(root);
  try { const doc=solo.open('src/main.tua'); await solo.query('single-root-'+label+'-hover','textDocument/hover',doc,'dep.value',5); await solo.query('single-root-'+label+'-diagnostics','textDocument/diagnostic',doc); report.sessions.push({name:'single-root-control-'+label,evidence:solo.inbound}); }
  finally {await solo.close();}
 }
 const m=await connect(a,{folders:[a,b]});
 try{
  const da=m.open('src/main.tua'),db=m.open(path.join(b,'src/main.tua'));
  await m.query('project-a-hover','textDocument/hover',da,'dep.value',5);
  await m.query('project-b-hover','textDocument/hover',db,'dep.value',5);
  await m.query('project-a-diagnostics','textDocument/diagnostic',da);
  await m.query('project-b-diagnostics','textDocument/diagnostic',db);
  const dep=m.open(path.join(a,'src/dep.lua'));m.change(dep,'return {value=false}');
  await m.query('project-a-changed-hover','textDocument/hover',da,'dep.value',5);
  await m.query('project-b-after-a-change-hover','textDocument/hover',db,'dep.value',5);
  report.sessions.push({name:'multi-root-isolation',evidence:m.inbound});
 }finally{await m.close();fs.rmSync(a,{recursive:true,force:true});fs.rmSync(b,{recursive:true,force:true});}
 save(path.join(__dirname,'lifecycle-evidence.json'),report);
 for(const session of report.sessions) for(const e of session.evidence) if(e.label!=='initialize') console.log(e.label,JSON.stringify(e.result).slice(0,600));
})().catch(error=>{console.error(error);process.exitCode=1});
