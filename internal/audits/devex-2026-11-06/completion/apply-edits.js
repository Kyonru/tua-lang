'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process');
const repo=path.resolve(__dirname,'../../../..');
const root=path.join(repo,'target/devex-audit-2026-11-06/completion/applied');
fs.mkdirSync(path.join(root,'src/assets'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n');
for(const n of ['player.png',"quote'image.png"])fs.copyFileSync(path.join(repo,'examples/features/src/assets/player.png'),path.join(root,'src/assets',n));
const offset=(source,p)=>source.split('\n').slice(0,p.line).reduce((n,l)=>n+l.length+1,0)+p.character;
function expandSnippet(s){return s.replace(/\$\{\d+\|([^|]*)\|\}/g,(_,s)=>s.split(',')[0]).replace(/\$\{\d+:([^}]*)\}/g,'$1').replace(/\$\{\d+\}/g,'').replace(/\$\d+/g,'');}
function apply(source,item,position){const edits=[...(item.additionalTextEdits||[]),item.textEdit||{range:{start:position,end:position},newText:item.insertText||item.label}];return edits.sort((a,b)=>offset(source,b.range.start)-offset(source,a.range.start)).reduce((s,e)=>s.slice(0,offset(source,e.range.start))+expandSnippet(e.newText)+s.slice(offset(source,e.range.end)),source);}
fs.writeFileSync(path.join(root,'src/library.tua'),'return {}\n');
const report=[];
const evidence=JSON.parse(fs.readFileSync(path.join(__dirname,'evidence.json')));
for(const id of ['module-middle','asset-middle','asset-apostrophe','love-existing-call','love-existing-open','quoted-member']){const row=evidence.cases.find(c=>c.id===id),item=row.resolved[0],source=apply(row.source,item,row.position);report.push({id,source,request:row.position,item});}
report.push({id:'trailing-comment',source:'local function f(x: number) -- hello\n return x\nend\nprint(f(1))\n'});
report.push({id:'suppression-comment',source:'function love.udpate(dt: number) -- tua-ignore-line TL5006\nend\n'});
for(const row of report){fs.writeFileSync(path.join(root,'src/main.tua'),row.source);const r=cp.spawnSync(path.resolve(process.env.TUA_BIN||path.join(repo,'target/release/tua')),['check'],{cwd:root,encoding:'utf8'});row.check={status:r.status,signal:r.signal,stdout:r.stdout,stderr:r.stderr};console.log(row.id,r.status,r.signal);}
fs.writeFileSync(path.join(__dirname,'applied-evidence.json'),JSON.stringify(report,null,2)+'\n');
