'use strict';
const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay,completionItems}=require(path.join(process.cwd(),'editors/vscode/scripts/lsp-profile-common'));
const repo=process.cwd(),root=path.join(__dirname,'project');
fs.mkdirSync(path.join(root,'src'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nrules=false\n[shaders]\nvalidate="off"\n');
const sources={method:'type Receiver = {kind: "unrelated" | "leaked", call: (self: Receiver, mode: "yes" | "no") -> nil}\nlocal r: Receiver = {kind="unrelated", call=function(self: Receiver, mode: "yes" | "no") end}\nr:call("")\n',basic:'local function add(a: number, b: number): number return a+b end\nlocal value = add(1, 2)\nlocal emoji = "😀"; print(value)\nprint(value)\n',audio:'local a=audioSystem({cues={ping={source=sound("sample.wav")}},buses={master={},sfx={parent="master"}}})\na:play("")\na:stopBus("")\na:on("", function(e) end)\n',input:'local c = input({controls={left={"key:a"},right={"key:d"},up={"key:w"},down={"key:s"}},pairs={move={"left","right","up","down"}}})\nlocal x,y=c:get("move")\n',shader:'local fx = shader "effect.glsl"\nfx:send("alpha", 1)\n',literal:'type Choice = "yes" | "no"\nlocal function choose(c: Choice): boolean return c=="yes" end\nchoose("")\n',incomplete:'local player = {health=10, name="A"}\nplayer.\n'};
fs.writeFileSync(path.join(root,'effect.glsl'),'extern number alpha;\nvec4 effect(vec4 c, Image t, vec2 uv, vec2 px) {return c*alpha;}\n');
for(const [name,text] of Object.entries(sources))fs.writeFileSync(path.join(root,'src',name+'.tua'),text);
const c=new LspClient(path.join(repo,'target/debug/tua'),root,{requestTimeoutMs:120000}),out=[];
function pos(text,needle,delta=0){const i=text.indexOf(needle);if(i<0)throw Error(needle);const p=text.slice(0,i+delta).split('\n');return {line:p.length-1,character:p.at(-1).length};}
const uri=n=>pathToFileURL(path.join(root,'src',n+'.tua')).href;
async function req(label,method,n,needle,delta=0,extra={}){const params={textDocument:{uri:uri(n)},...(needle?{position:pos(sources[n],needle,delta)}:{}),...extra};const result=await c.request(method,params);out.push({label,method,params,result});return result;}
(async()=>{await c.start();try{out.push({label:'initialize',result:await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{general:{positionEncodings:['utf-16']},textDocument:{hover:{contentFormat:['markdown']}}}})});c.notify('initialized',{});for(const [name,text] of Object.entries(sources))c.notify('textDocument/didOpen',{textDocument:{uri:uri(name),languageId:'tua',version:1,text}});
await req('hover-number','textDocument/hover','basic','print(value)',7);
await req('definition-utf16','textDocument/definition','basic','print(value)',7);
await req('references-utf16','textDocument/references','basic','print(value)',7,{context:{includeDeclaration:true}});
await req('rename-utf16','textDocument/rename','basic','print(value)',7,{newName:'score'});
await req('signature','textDocument/signatureHelp','basic','add(1, 2)',7);
await req('hints','textDocument/inlayHint','basic',null,0,{range:{start:{line:0,character:0},end:{line:4,character:0}}});
await req('symbols','textDocument/documentSymbol','basic');
await req('tokens','textDocument/semanticTokens/full','basic');
out.push({label:'workspace-symbols',result:await c.request('workspace/symbol',{query:'add'})});
for(const [label,n,needle,delta] of [['method','method','call("',6],['audio-cue','audio','play("',6],['audio-bus','audio','stopBus("',9],['audio-event','audio','on("',4],['literal','literal','choose("',8],['incomplete','incomplete','player.',7]])await req(label,'textDocument/completion',n,needle,delta);
const inputHelp = await req('input-overload','textDocument/signatureHelp','input','get("move"',10);
require('assert').strictEqual(inputHelp.signatures[inputHelp.activeSignature].label,
  'function c:get(name: "move"): number, number', 'completed input pair literal must select the pair overload');
await req('shader-hover','textDocument/hover','shader','"alpha"',3);
const change={textDocument:{uri:uri('basic'),version:2},contentChanges:[{range:{start:pos(sources.basic,'add(1, 2)'),end:pos(sources.basic,'add(1, 2)',9)},text:'"unsaved"'}]};out.push({label:'incremental-change',params:change});c.notify('textDocument/didChange',change);sources.basic=sources.basic.replace('add(1, 2)','"unsaved"');
await req('unsaved-hover','textDocument/hover','basic','print(value)',7);
await req('unsaved-rename-utf16','textDocument/rename','basic','print(value)',7,{newName:'score'});
await delay(500);out.push({label:'diagnostics',result:c.diagnostics});await c.request('shutdown',null);c.notify('exit',null);
}finally{fs.writeFileSync(path.join(__dirname,'responses.json'),JSON.stringify(out,null,2));c.child.kill();}
for(const r of out)console.log(r.label,JSON.stringify(r.result??r.params).slice(0,230));})().catch(e=>{console.error(e);process.exitCode=1});
