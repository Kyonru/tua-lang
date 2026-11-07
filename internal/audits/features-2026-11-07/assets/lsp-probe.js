const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay,completionItems}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const repo=path.resolve(__dirname,'../../../..'),root=path.join(__dirname,'fixtures/assets'),bin=path.join(repo,'target/release/tua');
(async()=>{const c=new LspClient(bin,root),file=path.join(root,'src/main.tua'),uri=pathToFileURL(file).href,text=fs.readFileSync(file,'utf8'),out={};await c.start();try{
out.initialize=await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{hover:{contentFormat:['markdown']}}}});c.notify('initialized',{});c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});
for(const [name,line,char] of [['helper',0,30],['native',1,50],['shadow',3,30]]){
out[name]={hover:await c.request('textDocument/hover',{textDocument:{uri},position:{line,character:char}}),completion:completionItems(await c.request('textDocument/completion',{textDocument:{uri},position:{line,character:line===1?45:28}}))};}
out.rename=await c.request('workspace/willRenameFiles',{files:[{oldUri:pathToFileURL(path.join(root,'src/assets/pixel.png')).href,newUri:pathToFileURL(path.join(root,'src/assets/renamed.png')).href}]});out.inventory=await c.request('tua/assetInventory',{});
// Unfinished literal should still suggest only the image.
const unfinished='local p = image "assets/';c.notify('textDocument/didChange',{textDocument:{uri,version:2},contentChanges:[{text:unfinished}]});out.unfinished=completionItems(await c.request('textDocument/completion',{textDocument:{uri},position:{line:0,character:unfinished.length}}));await delay(150);out.diagnostics=c.diagnostics;await c.request('shutdown',null);c.notify('exit',null);
}finally{c.child.kill();fs.writeFileSync(path.join(__dirname,'lsp-evidence.json'),JSON.stringify(out,null,2));} console.log(JSON.stringify({helper:out.helper,native:out.native,shadow:out.shadow,rename:out.rename,unfinished:out.unfinished},null,2));})().catch(e=>{console.error(e);process.exit(1)});
