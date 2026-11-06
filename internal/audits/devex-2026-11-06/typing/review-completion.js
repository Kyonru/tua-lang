'use strict';
const fs=require('fs'),path=require('path'),cp=require('child_process');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL,completionItems}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const binary=process.env.TUA_AUDIT_BINARY||path.join(repo,'target/release/tua');
const root=path.join(repo,'target/devex-audit-2026-11-06/typing/completion-review');
const cases=[
 {id:'quote-double',source:'local picture = love.graphics.newImage("assets/her<cursor>")\n',label:'assets/hero"portrait.png'},
 {id:'keyword-key',source:'local record = {["repeat"]=7,["dash-key"]=8,ordinary=9}\nlocal answer = record.<cursor>\n',label:'repeat'},
 {id:'module-right-suffix',source:'local service = require("serv<cursor>XXX")\n',label:'service'},
 {id:'middle-local',source:'local customerName = "a"\nlocal name = cust<cursor>Typo\n',label:'customerName'},
];
const offset=(text,pos)=>text.split('\n').slice(0,pos.line).reduce((n,l)=>n+l.length+1,0)+pos.character;
(async()=>{
 fs.mkdirSync(path.join(root,'src/assets'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n');fs.writeFileSync(path.join(root,'src/service.tua'),'return {valid=true}\n');fs.copyFileSync(path.join(repo,'examples/features/src/assets/player.png'),path.join(root,'src/assets/hero"portrait.png'));
 const client=new LspClient(binary,root);await client.start();const report=[];
 try{await client.request('initialize',{rootUri:pathToFileURL(root).href,capabilities:{textDocument:{completion:{completionItem:{snippetSupport:true,resolveSupport:{properties:['documentation','detail','additionalTextEdits']}}}}}});client.notify('initialized',{});
 for(const c of cases){const marker=c.source.indexOf('<cursor>');const before=c.source.slice(0,marker);const position={line:before.split('\n').length-1,character:before.split('\n').at(-1).length};const source=c.source.replace('<cursor>','');const filename=path.join(root,'src/main.tua');const uri=pathToFileURL(filename).href;client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});const response=await client.request('textDocument/completion',{textDocument:{uri},position,context:{triggerKind:1}});const items=completionItems(response);const selected=items.find(item=>item.label===c.label);const row={id:c.id,source,position,wanted:c.label,labels:items.map(item=>item.label)};
 if(selected){const item=await client.request('completionItem/resolve',selected);row.item=item;if(item.insertTextFormat===2)throw Error('This independent replay deliberately does not expand snippets');const edits=[...(item.additionalTextEdits||[]),item.textEdit||{range:{start:position,end:position},newText:item.insertText||item.label}];const spans=edits.map(e=>({start:offset(source,e.range.start),end:offset(source,e.range.end),text:e.newText})).sort((a,b)=>b.start-a.start);row.applied=spans.reduce((text,e)=>text.slice(0,e.start)+e.text+text.slice(e.end),source);fs.writeFileSync(filename,row.applied);const result=cp.spawnSync(binary,['check'],{cwd:root,encoding:'utf8',timeout:10000});row.check={status:result.status,stdout:result.stdout,stderr:result.stderr};}
 client.notify('textDocument/didClose',{textDocument:{uri}});report.push(row);
 }
 await client.request('shutdown',null);client.notify('exit',null);
 }finally{client.child.kill();fs.writeFileSync(path.join(__dirname,'completion-review-evidence.json'),JSON.stringify(report,null,2)+'\n');}
 console.log(report.map(r=>({id:r.id,labels:r.labels,applied:r.applied,status:r.check?.status})));
})().catch(e=>{console.error(e);process.exitCode=1});
