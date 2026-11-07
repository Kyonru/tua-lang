const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
(async()=>{
 const root=path.join(__dirname,'project'),bin=path.resolve(__dirname,'../../../../target/release/tua');
 const client=new LspClient(bin,root); await client.start();
 await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}}); client.notify('initialized',{});
 const source=fs.readFileSync(path.join(root,'src/sheets.tua'),'utf8').replace('return { Sheet = Sheet, Parent = Parent }','local player = Sheet:player("run")\nplayer:play("absent")\nplayer:play("run")\nplayer:on("step", function() end)');
 const uri=pathToFileURL(path.join(root,'src/sheets.tua')).href;
 client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}}); await delay(500);
 const pos=(s,offset)=>{const text=source.slice(0,source.lastIndexOf(s)+offset),lines=text.split('\n');return {line:lines.length-1,character:lines.at(-1).length}};
 const completion=await client.request('textDocument/completion',{textDocument:{uri},position:pos('player:play("run")',13)});
 const hover=await client.request('textDocument/hover',{textDocument:{uri},position:pos('player:play("run")',8)});
 fs.writeFileSync(path.join(__dirname,'lsp.json'),JSON.stringify({source,diagnostics:client.diagnostics,completion,hover},null,2));
 await client.request('shutdown',null);client.notify('exit',null);
})().catch(e=>{console.error(e);process.exit(1)});
