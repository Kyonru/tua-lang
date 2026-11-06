'use strict';
const fs=require('fs'),path=require('path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const base=path.join(repo,'target/devex-audit-2026-11-06/typing/multi-root');
const roots=['inventory','menu'].map(n=>path.join(base,n));
const sources=['local model = require("shared")\nlocal result: number = model.value\nprint(model.value)\n','local model = require("shared")\nlocal result: string = model.value\nprint(model.value)\n'];
(async()=>{
 for(let i=0;i<2;i++){fs.mkdirSync(path.join(roots[i],'src'),{recursive:true});fs.writeFileSync(path.join(roots[i],'tua.toml'),'source="src"\nout="build"\n');fs.writeFileSync(path.join(roots[i],'src',i?'view.tua':'inventory.tua'),sources[i]);fs.writeFileSync(path.join(roots[i],'src/shared.lua'),i?'return {value="menu"}\n':'return {value=7}\n');}
 const client=new LspClient(path.join(repo,'target/release/tua'),roots[0]);await client.start();
 const out={roots,probes:[]};
 try{out.initialize=await client.request('initialize',{rootUri:pathToFileURL(roots[0]).href,workspaceFolders:roots.map(root=>({uri:pathToFileURL(root).href,name:path.basename(root)})),capabilities:{textDocument:{diagnostic:{}}}});client.notify('initialized',{});
 for(let i=0;i<2;i++){const uri=pathToFileURL(path.join(roots[i],'src',i?'view.tua':'inventory.tua')).href;client.notify('textDocument/didOpen',{textDocument:{uri,version:1,languageId:'tua',text:sources[i]}});}
 for(let i=0;i<2;i++){const uri=pathToFileURL(path.join(roots[i],'src',i?'view.tua':'inventory.tua')).href;out.probes.push({root:roots[i],hover:await client.request('textDocument/hover',{textDocument:{uri},position:{line:2,character:13}}),diagnostics:await client.request('textDocument/diagnostic',{textDocument:{uri}}),definition:await client.request('textDocument/definition',{textDocument:{uri},position:{line:0,character:26}})});}
 await client.request('shutdown',null);client.notify('exit',null);
 }finally{client.child.kill();fs.writeFileSync(path.join(__dirname,'multi-root-crosscheck.json'),JSON.stringify(out,null,2)+'\n');}
})().catch(e=>{console.error(e);process.exitCode=1});
