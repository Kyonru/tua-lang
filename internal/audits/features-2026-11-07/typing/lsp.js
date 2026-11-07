'use strict';
const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const base=__dirname,repo=path.resolve(base,'../../../..');
(async()=>{
 const results=[];
 for(const name of ['positive','callback_short','callback_full','optional_dot','dictionary_runtime','dictionary_bracket','multi_assignment']) {
  const root=path.join(base,'cases',name),file=path.join(root,'src/main.tua'),text=fs.readFileSync(file,'utf8'),uri=pathToFileURL(file).href;
  const c=new LspClient(path.join(repo,'target/release/tua'),root);await c.start();
  try {
   await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{diagnostic:{}}}});
   c.notify('initialized',{});c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});
   const diagnostic=await c.request('textDocument/diagnostic',{textDocument:{uri}});
   const hover=[];
   for(const [line,s] of text.split('\n').entries()) {
    const needle=name.startsWith('callback')&&line===1?'x':name==='dictionary_runtime'&&line===3?'n +':name==='multi_assignment'&&line===3?'s':null;
    if(needle){const character=s.indexOf(needle);hover.push({line,character,result:await c.request('textDocument/hover',{textDocument:{uri},position:{line,character}})});}
   }
   results.push({name,diagnostic,hover});
   await c.request('shutdown',null);c.notify('exit',{});
  } finally {c.child.kill();}
 }
 fs.writeFileSync(path.join(base,'lsp-results.json'),JSON.stringify(results,null,2));
})().catch(e=>{console.error(e);process.exitCode=1});
