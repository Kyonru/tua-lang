"use strict";
const fs=require('node:fs'), path=require('node:path'), Module=require('node:module');
const repo=path.resolve(__dirname,'../../../..'), root=path.join(__dirname,'fixture');
const old=Module._load; Module._load=function(id,...args){if(id==='vscode')return {};return old.call(this,id,...args)};
const maps=require(path.join(repo,'editors/vscode/src/editor/source-maps'));
const {LspClient,pathToFileURL}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
(async()=>{
 const client=new LspClient(path.join(repo,'target/release/tua'),root);await client.start();
 const out={};out.initialize=await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});client.notify('initialized',{});
 out.layouts=[];
 for(const file of ['src/main.tua','src/nested dir/crash.tua']) {const uri=pathToFileURL(path.join(root,file)).href;client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:fs.readFileSync(path.join(root,file),'utf8')}});out.layouts.push(await client.request('tua/projectLayout',{uri}));}
 const all=maps.loadSourceMapsFromDirectory(path.join(root,'build'));
 out.navigation=all.map(m=>({source:m.sourcePath,generated:m.generatedPath,forward:maps.nearestMapping(m.lineMappings,'sourceLine',7),backward:maps.nearestMapping(m.lineMappings,'generatedLine',17)}));
 out.traceback=fs.readFileSync(path.join(__dirname,'raw/class-trace.log'),'utf8').split('\n').map(line=>({line,mapped:maps.remapTracebackLine(line,all)||null}));
 fs.writeFileSync(path.join(__dirname,'raw/lsp-navigation.json'),JSON.stringify(out,null,2));
 await client.request('shutdown',null);client.notify('exit');
})().catch(e=>{console.error(e);process.exitCode=1});
