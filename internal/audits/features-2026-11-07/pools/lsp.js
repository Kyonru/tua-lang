const fs=require('fs'),path=require('path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL,completionItems}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
(async()=>{
const root=path.join(__dirname,'editor');fs.mkdirSync(root+'/src',{recursive:true});fs.writeFileSync(root+'/tua.toml','source="src"\nout="build"\n');
fs.writeFileSync(root+'/src/library.tua','type Bullet = {x:number}\nlocal pool=objectPool({create=function(): Bullet return {x=0} end})\nreturn {pool=pool}\n');
const src='type Bullet = { x:number }\nlocal pool=objectPool({create=function(): Bullet return {x=0} end,reset=function(b) print(b.x) end})\nlocal b=pool:acquire()\nprint(b.x)\npool:release(b)\nlocal imported=require("library")\nlocal other=imported.pool:acquire()\nprint(other.x)\n';
fs.writeFileSync(root+'/src/main.tua',src);
const c=new LspClient(repo+'/target/release/tua',root,{requestTimeoutMs:30000});await c.start();await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});c.notify('initialized',{});
const uri=pathToFileURL(root+'/src/main.tua').href;c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:src}});
const out={};try{
for (const [name,line,character] of [['acquired',3,6],['resetParameter',1,90],['importedAcquired',7,6]]) out[name]=await c.request('textDocument/hover',{textDocument:{uri},position:{line,character}});
out.completion=completionItems(await c.request('textDocument/completion',{textDocument:{uri},position:{line:3,character:8}}));
out.signature=await c.request('textDocument/signatureHelp',{textDocument:{uri},position:{line:4,character:13}});
out.graph=await c.request('tua/objectPoolGraph',{textDocument:{uri}});
out.diagnostics=await c.request('textDocument/diagnostic',{textDocument:{uri}});
} finally {fs.writeFileSync(__dirname+'/lsp-results.json',JSON.stringify(out,null,2));await c.request('shutdown',null).catch(()=>{});c.notify('exit',null);c.child.kill();}
console.log(JSON.stringify(out,null,2));
})().catch(e=>{console.error(e);process.exitCode=1});
