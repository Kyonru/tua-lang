'use strict';
const fs=require('node:fs'),path=require('node:path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const root=path.join(repo,'target/devex-audit-2026-11-06/completion/project');
function offset(source,p){return source.split('\n').slice(0,p.line).reduce((n,s)=>n+s.length+1,0)+p.character;}
function apply(source,edits){return [...edits].sort((a,b)=>offset(source,b.range.start)-offset(source,a.range.start)).reduce((s,e)=>s.slice(0,offset(source,e.range.start))+e.newText+s.slice(offset(source,e.range.end)),source);}
async function run(){const c=new LspClient(path.resolve(process.env.TUA_BIN||path.join(repo,'target/release/tua')),root,{requestTimeoutMs:30000});await c.start();await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{diagnostic:{}}}});c.notify('initialized',{});const results=[];
try{for(const [id,source,line] of [
 ['colon','local img=love.graphics.newImage("assets/player.png")\nimg.setFilter("nearest")\n',1],
 ['callback-typo','function love.udpate(dt: number)\nend\n',0],
 ['unused','local unused = 1\nprint("hello")\n',0],
 ['type-mismatch','local value: number = "wrong"\nprint(value)\n',0],
 ['unknown-type','local value: ExportedPlayer = {health=1}\nprint(value)\n',0],
 ['missing-module','local m=require("absent")\nprint(m)\n',0],
 ['duplicate-key','local m={x=1,x=2}\nprint(m)\n',0],
 ['annotation','local value=1\nprint(value)\n',0],
]){const uri=pathToFileURL(path.join(root,'src',`action-${id}.tua`)).href;c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});const diag=await c.request('textDocument/diagnostic',{textDocument:{uri}});const actions=await c.request('textDocument/codeAction',{textDocument:{uri},range:{start:{line,character:0},end:{line,character:source.split('\n')[line].length}},context:{diagnostics:diag.items||[]}});const outcomes=[];let version=1;
for(const action of actions||[]){const edits=action.edit?.changes?.[uri];if(!edits)continue;const applied=apply(source,edits);c.notify('textDocument/didChange',{textDocument:{uri,version:++version},contentChanges:[{text:applied}]});console.log('applying',id,action.title,JSON.stringify(applied));const after=await c.request('textDocument/diagnostic',{textDocument:{uri}}).catch(error=>({error:error.message}));outcomes.push({title:action.title,applied,diagnostics:after});c.notify('textDocument/didChange',{textDocument:{uri,version:++version},contentChanges:[{text:source}]});}
results.push({id,source,diagnostics:diag,actions,outcomes});c.notify('textDocument/didClose',{textDocument:{uri}});console.log(id,(actions||[]).map(x=>x.title).join('; '));}
}finally{fs.writeFileSync(path.join(__dirname,'actions-stderr.log'),c.stderr);fs.writeFileSync(path.join(__dirname,'actions-evidence.json'),JSON.stringify(results,null,2)+'\n');await c.request('shutdown',null).catch(()=>{});c.notify('exit',null);c.child.kill();}}
run().catch(e=>{console.error(e);process.exitCode=1});
