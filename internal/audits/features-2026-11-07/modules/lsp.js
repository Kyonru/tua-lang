const fs=require('fs'),path=require('path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL,delay}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common.js'));
(async()=>{
 const root=path.join(__dirname,process.argv[2]||'domains'),c=new LspClient(path.join(repo,'target/release/tua'),root);await c.start();
 const uri=f=>pathToFileURL(path.join(root,'src',f)).href;
 await c.request('initialize',{processId:null,rootUri:pathToFileURL(root).href,workspaceFolders:[{uri:pathToFileURL(root).href,name:'isolated'}],capabilities:{}});c.notify('initialized',{});
 const source=fs.readFileSync(path.join(root,'src/main.tua'),'utf8');
 c.notify('textDocument/didOpen',{textDocument:{uri:uri('main.tua'),languageId:'tua',version:1,text:source}});await delay(1200);
 const out={root,source,initial:[],diagnostics:c.diagnostics};
 for(const [i,line] of source.split('\n').entries()){
  const match=line.match(/^local (\w+) =/);if(!match)continue;
  let position={line:i,character:line.indexOf(match[1])+1};
  out.initial.push({name:match[1],hover:await c.request('textDocument/hover',{textDocument:{uri:uri('main.tua')},position})});
 }
 if(root.endsWith('declarations')){out.member=await c.request('textDocument/hover',{textDocument:{uri:uri('main.tua')},position:{line:1,character:18}});out.sig=await c.request('textDocument/signatureHelp',{textDocument:{uri:uri('main.tua')},position:{line:1,character:23}});}
 if(root.endsWith('domains')){
  out.signature=await c.request('textDocument/signatureHelp',{textDocument:{uri:uri('main.tua')},position:{line:4,character:22}});
  out.definition=await c.request('textDocument/definition',{textDocument:{uri:uri('main.tua')},position:{line:6,character:13}});
  const changed='local function inc(x: string): string\n return x\nend\nreturn {inc=inc}\n';
  c.notify('textDocument/didOpen',{textDocument:{uri:uri('normal.tua'),languageId:'tua',version:1,text:fs.readFileSync(path.join(root,'src/normal.tua'),'utf8')}});
  c.notify('textDocument/didChange',{textDocument:{uri:uri('normal.tua'),version:2},contentChanges:[{text:changed}]});await delay(800);
  out.unsavedHover=await c.request('textDocument/hover',{textDocument:{uri:uri('main.tua')},position:{line:6,character:13}});out.unsavedDiagnostics=c.diagnostics;
 }
 fs.writeFileSync(path.join(__dirname,`${path.basename(root)}-lsp.json`),JSON.stringify(out,null,2));await c.request('shutdown',null);c.notify('exit',{});
})().catch(e=>{console.error(e);process.exit(1)});
