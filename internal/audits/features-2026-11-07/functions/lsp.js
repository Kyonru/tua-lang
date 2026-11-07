const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
(async()=>{
 const rows=[];
 for(const name of ['bad_return','bad_local','bad_default','default_nil']){
  const root=path.join(__dirname,name), file=path.join(root,'src/main.tua');
  const client=new LspClient(path.resolve(__dirname,'../../../../target/release/tua'),root);
  await client.start();
  await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});
  client.notify('initialized',{});
  const uri=pathToFileURL(file).href,source=fs.readFileSync(file,'utf8');
  client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});
  await delay(600);
  const diagnostic=await client.request('textDocument/diagnostic',{textDocument:{uri}});
  rows.push({name,diagnostic,push:client.diagnostics});
  await client.request('shutdown',null);client.notify('exit',null);
 }
 fs.writeFileSync(path.join(__dirname,'lsp.json'),JSON.stringify(rows,null,2));
 console.log(JSON.stringify(rows,null,2));
})().catch(e=>{console.error(e);process.exit(1)});
