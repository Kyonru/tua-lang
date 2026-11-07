'use strict';
const fs=require('fs'),path=require('path'),assert=require('assert/strict');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const {loopbackGenerateUrl}=require(path.join(repo,'editors/vscode/src/editor/ollama-client'));
const root=path.join(__dirname,'fixture');fs.mkdirSync(root,{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="."\n');
const c=new LspClient(path.join(repo,'target/release/tua'),root);const results=[];
async function req(label,method,params){const result=await c.request(method,params);results.push({label,method,params,result});return result;}
let index=0;
function open(source,ext='tua'){const uri=pathToFileURL(path.join(root,`probe-${++index}.${ext}`)).href;fs.writeFileSync(new URL(uri),source);c.notify('textDocument/didOpen',{textDocument:{uri,languageId:ext,version:1,text:source}});return {textDocument:{uri},position:{line:source.split('\n').length-1,character:source.split('\n').at(-1).length},documentVersion:1};}
function validate(p,x,candidate,extra={}){return {...p,contextId:x.contextId,analysisRevision:x.analysisRevision,contextBytes:x.contextBytes,sourceFingerprint:x.sourceFingerprint,candidate,...extra};}
(async()=>{await c.start();try{
await req('initialize','initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});c.notify('initialized',{});
const p=open('draw(player, score)\ndraw(player, ');
assert.deepEqual((await req('disabled','tua/localInlineCompletion',p)).items,[]);
c.notify('workspace/didChangeConfiguration',{settings:{tua:{completion:{inline:{enabled:true}}}}});
let local=await req('enabled-recall','tua/localInlineCompletion',p);assert.equal(local.items[0].insertText,'score');assert.deepEqual(local.items[0].range.start,p.position);
assert.deepEqual((await req('stale-local','tua/localInlineCompletion',{...p,documentVersion:0})).items,[]);
assert.equal((await req('standard','textDocument/inlineCompletion',{...p,context:{triggerKind:1}}))[0].insertText,'score');
await req('semantic-snippet','textDocument/inlineCompletion',{...open('whi'),context:{triggerKind:1}});
await req('lua-recall','tua/localInlineCompletion',open('draw(player, score)\ndraw(player, ','lua'));
for(const source of ['-- whi','local x = "whi','', 'local x = '])await req('abstain-'+JSON.stringify(source),'tua/localInlineCompletion',open(source));
const utf=open('print("😀", player.name)\nprint("😀", player.name)\nprint("😀", pla');await req('selected-utf16','tua/localInlineCompletion',{...utf,selectedCompletion:{text:'player',range:{start:{line:2,character:12},end:utf.position}}});
const model=open('local value: number = ');const x=await req('model-context','tua/inlineCompletionContext',{...model,contextBytes:4096});assert.equal(x.completionScope,'local');
for(const candidate of ['1','"wrong"','```lua\n1\n```','','const value = 1;','<|fim_middle|>1<|endoftext|>'])await req('validate-'+JSON.stringify(candidate),'tua/validateInlineCompletion',validate(model,x,candidate));
assert.equal((await req('fingerprint-reject','tua/validateInlineCompletion',validate(model,x,'1',{sourceFingerprint:'wrong'}))).reason,'stale');
c.notify('textDocument/didChange',{textDocument:{uri:model.textDocument.uri,version:2},contentChanges:[{text:'local value: number = 2'}]});
assert.equal((await req('edit-stale','tua/validateInlineCompletion',validate(model,x,'1'))).reason,'stale');
await req('function-context','tua/inlineCompletionContext',open('function update(dt: number)\n  '));
for(const source of ['-- no completion','local x = "text'])assert.equal(await req('model-skip','tua/inlineCompletionContext',open(source)),null);
for(const endpoint of ['http://localhost:11434','http://127.5.6.7:11434','http://[::1]:11434','https://localhost:11434','http://example.com','http://user:pass@localhost','http://127.0.0.1.example.com']){let result;try{result=loopbackGenerateUrl(endpoint).href}catch(e){result=e.message}results.push({label:'endpoint',endpoint,result});}
c.notify('workspace/didChangeConfiguration',{settings:{tua:{completion:{inline:{enabled:true,sources:{semantic:false,currentFile:false,statistical:false}}}}}});
assert.deepEqual((await req('sources-off','tua/localInlineCompletion',p)).items,[]);
await req('unopened-document','tua/localInlineCompletion',{...p,textDocument:{uri:pathToFileURL(path.join(root,'missing.tua')).href}});
const bounded=await req('bounded-context','tua/inlineCompletionContext',{...open('local bounded = '),contextBytes:99999999});assert.ok(bounded.contextBytes<=32768);
await c.request('shutdown',null);c.notify('exit',null);
}finally{fs.writeFileSync(path.join(__dirname,'protocol-results.json'),JSON.stringify(results,null,2));c.child.kill();}console.log(`PASS: ${results.length} recorded probes`);})().catch(e=>{console.error(e);process.exitCode=1});
