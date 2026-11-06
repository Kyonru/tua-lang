'use strict';
const fs=require('fs'),path=require('path'),cp=require('child_process');
const repo=path.resolve(__dirname,'../../../..');
const binary=process.env.TUA_AUDIT_BINARY||path.join(repo,'target/release/tua');
const {LspClient,pathToFileURL,completionItems}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const base=path.join(repo,'target/devex-audit-2026-11-06/typing/class-whitespace');
const head='final class Tank {\n fuel: number\n init(capacity: number) { self.fuel = capacity }\n getFuel(cost: number): number { return self.fuel - cost }\n}\n';
(async()=>{const evidence=[];for(const [layout,declaration] of [['spaced','local tank = Tank.new(12)'],['compact','local tank=Tank.new(12)']]){
 const root=path.join(base,layout);fs.mkdirSync(path.join(root,'src'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n');
 const source=head+declaration+'\nlocal remaining = tank:getFuel(1)\nlocal wrong: string = tank.fuel\nprint(remaining, wrong)\n';
 const file=path.join(root,'src/main.tua');fs.writeFileSync(file,source);const uri=pathToFileURL(file).href;
 const check=cp.spawnSync(binary,['check'],{cwd:root,encoding:'utf8',timeout:10000});
 const row={layout,source,check:{status:check.status,stdout:check.stdout,stderr:check.stderr},queries:[]};
 const client=new LspClient(binary,root);await client.start();
 try{await client.request('initialize',{rootUri:pathToFileURL(root).href,capabilities:{textDocument:{completion:{completionItem:{snippetSupport:true}},diagnostic:{}}}});client.notify('initialized',{});client.notify('textDocument/didOpen',{textDocument:{uri,version:1,languageId:'tua',text:source}});
 for(const [label,method,needle,delta] of [['completion','textDocument/completion','tank:getFuel',5],['receiver','textDocument/hover','tank:getFuel',1],['signature','textDocument/signatureHelp','getFuel(1)',8],['field','textDocument/hover','tank.fuel',6],['result','textDocument/hover','remaining,',2]]){
 const found=source.lastIndexOf(needle);if(found<0)throw Error(needle);const before=source.slice(0,found+delta);const position={line:before.split('\n').length-1,character:before.split('\n').at(-1).length};const params={textDocument:{uri},position,...(method==='textDocument/completion'?{context:{triggerKind:1}}:{})};const result=await client.request(method,params);row.queries.push({label,method,position,result});}
 row.diagnostics=await client.request('textDocument/diagnostic',{textDocument:{uri}});await client.request('shutdown',null);client.notify('exit',null);
 }finally{client.child.kill();evidence.push(row);}
 }fs.writeFileSync(path.join(__dirname,'class-whitespace-crosscheck.json'),JSON.stringify(evidence,null,2)+'\n');console.log(evidence.map(r=>({layout:r.layout,check:r.check,queries:r.queries})));})().catch(e=>{console.error(e);process.exitCode=1});
