const fs=require('fs'),path=require('path'),cp=require('child_process');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const repo=path.resolve(__dirname,'../../../..'),root=path.join(__dirname,'module');fs.mkdirSync(path.join(root,'src'),{recursive:true});
fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nrules=false\n');
fs.writeFileSync(path.join(root,'src/actor.tua'),`open class Actor {
 value: number = 0
 --- Increase the value.
 update(dt: number): number { self.value += dt; return self.value }
}
final class Fast extends Actor {
 override update(dt: number): number { self.value += dt * 2; return self.value }
}
export type ActorInstance = Actor
return { Fast = Fast }
`);
const source=`import type { ActorInstance } from "./actor"
local Actors = require("actor")
local actor: ActorInstance = Actors.Fast.new()
local value = actor:update(3)
assert(value == 6)
print(actor.value)
`;
const file=path.join(root,'src/main.tua');fs.writeFileSync(file,source);
(async()=>{const evidence={};for(const cmd of ['check','build']){const r=cp.spawnSync(path.join(repo,'target/release/tua'),[cmd],{cwd:root,encoding:'utf8'});evidence[cmd]={status:r.status,stdout:r.stdout,stderr:r.stderr};}const run=cp.spawnSync('/opt/homebrew/bin/lua',['-e','package.path="build/?.lua;"..package.path; dofile("build/main.lua")'],{cwd:root,encoding:'utf8'});evidence.lua={status:run.status,stdout:run.stdout,stderr:run.stderr};const client=new LspClient(path.join(repo,'target/release/tua'),root);await client.start();try{await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});client.notify('initialized',{});const uri=pathToFileURL(file).href;client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});await delay(300);for(const [name,method,line,character] of [['hover','hover',3,22],['signature','signatureHelp',3,27],['completion','completion',5,12],['definition','definition',3,22]]) evidence[name]=await client.request('textDocument/'+method,{textDocument:{uri},position:{line,character}});evidence.diagnostics=client.diagnostics;await client.request('shutdown',null);client.notify('exit',null);}finally{client.child.kill();}fs.writeFileSync(path.join(__dirname,'editor-results.json'),JSON.stringify(evidence,null,2));console.log(JSON.stringify(evidence,null,2));})().catch(e=>{console.error(e);process.exitCode=1});
