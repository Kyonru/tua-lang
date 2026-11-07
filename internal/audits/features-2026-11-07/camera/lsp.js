const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const root=path.join(__dirname,'happy'),repo=path.resolve(__dirname,'../../../..');
(async()=>{
 const client=new LspClient(path.join(repo,'target/release/tua'),root);
 await client.start();
 const uri=pathToFileURL(path.join(root,'src/main.tua')).href;
 const text=fs.readFileSync(path.join(root,'src/main.tua'),'utf8');
 const result={};
 try {
 await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});
 client.notify('initialized',{});
 client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});
 for(const [name,method,line,character] of [
 ['constructor','hover',0,21],['camera','hover',1,2],['follow','signatureHelp',1,14],
 ['methods','completion',1,4],['worldX','hover',5,22],['worldY','hover',5,26]])
 result[name]=await client.request('textDocument/'+method,{textDocument:{uri},position:{line,character}});
 await delay(200);result.diagnostics=client.diagnostics;
 await client.request('shutdown',null); client.notify('exit',null);
 }finally {client.child.kill();}
 fs.writeFileSync(path.join(__dirname,'lsp.json'),JSON.stringify(result,null,2));
 console.log('LSP probes saved');
})().catch(e=>{console.error(e);process.exitCode=1});
