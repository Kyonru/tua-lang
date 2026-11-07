const fs=require('fs'),path=require('path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL,delay}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
(async()=>{const root=path.join(__dirname,'positive'),file=path.join(root,'src/main.tua'),text=fs.readFileSync(file,'utf8'),uri=pathToFileURL(file).href;
const c=new LspClient(path.join(repo,'target/release/tua'),root,{requestTimeoutMs:30000}); await c.start();let out={};try{
await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});c.notify('initialized',{});c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});
for(const [name,method,line,character] of [['hover_to','hover',1,17],['completion_setter','completion',2,2],['signature_after','signatureHelp',6,8],['signature_callback','signatureHelp',5,13]]) out[name]=await c.request('textDocument/'+method,{textDocument:{uri},position:{line,character}});
await delay(150);out.diagnostics=c.diagnostics;await c.request('shutdown',null);c.notify('exit',null);
}finally{c.child.kill();}fs.writeFileSync(path.join(__dirname,'lsp.json'),JSON.stringify(out,null,2));})().catch(e=>{console.error(e);process.exitCode=1});
