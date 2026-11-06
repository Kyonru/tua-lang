'use strict';
const fs=require('node:fs'),path=require('node:path');
const repo=path.resolve(__dirname,'../../../..');
const {LspClient,pathToFileURL,completionItems}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const root=path.join(repo,'target/devex-audit-2026-11-06/completion/project');
const output=process.env.TUA_AUDIT_OUT || path.join(__dirname,'evidence.json');
const cases=[
 ['locals','local playerName = 1\nlocal playerScore = 2\nlocal copy = play<cursor>',['playerName','playerScore']],
 ['scope','do local hiddenValue = 1 end\nlocal visibleValue = 2\nlocal copy = <cursor>',['hiddenValue','visibleValue','print']],
 ['params','local function update(player: {health:number}, delta: number)\n  local x = pla<cursor>\nend',['player']],
 ['shadow-library','local math = {special = 1}\nlocal x = math.<cursor>',['special','sin']],
 ['snippet','<cursor>',['if','for','class','function']],
 ['middle-identifier','local playerName = 1\nlocal x = play<cursor>Wrong',['playerName']],
 ['function-prefix','fun<cursor>',['function']],
 ['quoted-member','local t={["foo-bar"]=1,["end"]=2,normal=3}\nlocal v=t.<cursor>',['foo-bar','end','normal']],
 ['colon-record','local t={health=1,run=function(self) end}\nt:<cursor>',['health','run']],
 ['callback-multiline','local function apply(callback: (player: {health:number}) -> nil) callback({health=1}) end\napply(function(player)\n player.<cursor>\nend)',['health']],
 ['member','local p = {health=3, nested={speed=4}}\nlocal v = p.<cursor>',['health','nested']],
 ['nested','local p = {health=3, nested={speed=4}}\nlocal v = p.nested.<cursor>',['speed']],
 ['indexed','local items: {health:number}[] = {{health=1}}\nlocal v = items[1].<cursor>',['health']],
 ['call-result','local function make(): {health:number} return {health=1} end\nlocal v = make().<cursor>',['health']],
 ['union','local p: {common:number,first:string} | {common:number,second:boolean}\nlocal v=p.<cursor>',['common','first','second']],
 ['unknown','local p: any\nlocal v=p.<cursor>',['print','health']],
 ['malformed-stdlib','local broken = (\nlocal v=string.<cursor>',['upper','sub']],
 ['callback','local f: (callback: (player: {health:number}) -> nil) -> nil\nf(function(player) player.<cursor> end)',['health']],
 ['callback-valid','local f: (callback: (player: {health:number}) -> nil) -> nil\nf(function(player) print(player.<cursor>health) end)',['health']],
 ['callback-function','local function apply(callback: (player: {health:number}) -> nil) callback({health=1}) end\napply(function(player) print(player.<cursor>health) end)',['health']],
 ['expected-table','type Options = {width:number,height:number}\nlocal options: Options = {wi<cursor>}',['width']],
 ['expected-table-fields','type Options = {width:number,height:number}\nlocal options: Options = {width=1, <cursor>}',['width','height']],
 ['nested-call-index','local function make(): {stats:{speed:number}}[] return {{stats={speed=1}}} end\nlocal n=make()[1].stats.<cursor>',['speed']],
 ['love-existing-open','local img=love.graphics.newImage("assets/player.png")\nimg.setFilter<cursor>("nearest", "nearest")',['setFilter']],
 ['module','local m=require("library")\nlocal v=m.<cursor>',['title','make','config']],
 ['lua-module','local m=require("lua_library")\nlocal v=m.<cursor>',['value','greet']],
 ['module-prefix','local m = require("lib<cursor>")',['library','library_more']],
 ['module-middle','local m = require("lib<cursor>ZZZ")',['library']],
 ['static-import','local m = import("./lib<cursor>")',['./library']],
 ['love-global','local v=love.graphics.<cursor>',['newImage','rectangle']],
 ['love-colon','local img=love.graphics.newImage("assets/player.png")\nimg:<cursor>',['getWidth','setFilter']],
 ['love-dot','local img=love.graphics.newImage("assets/player.png")\nimg.<cursor>',['getWidth','setFilter']],
 ['love-existing-call','local img=love.graphics.newImage("assets/player.png")\nimg.setF<cursor>ilter("nearest", "nearest")',['setFilter']],
 ['love-chain','love.graphics.newImage("assets/player.png"):<cursor>',['getWidth','setFilter']],
 ['void-chain','love.graphics.newText(love.graphics.newFont(12)):set("hello"):<cursor>',['getWidth','set']],
 ['enum','love.graphics.rectangle("fi<cursor>",0,0,1,1)',['fill']],
 ['asset','local img=love.graphics.newImage("assets/pl<cursor>")',['assets/player.png']],
 ['asset-middle','local img=love.graphics.newImage("assets/pl<cursor>ZZZ.png")',['assets/player.png']],
 ['asset-apostrophe','local img=love.graphics.newImage(\'assets/quo<cursor>\')',["assets/quote\'image.png"]],
 ['annotation','---@pa<cursor>',['param']],
 ['type-position','type Player = {health:number}\nlocal p: Pla<cursor>',['Player']],
 ['autoimport-type','local p: Expor<cursor> = nil',['ExportedPlayer']],
 ['autoimport-value','local p = makeExpor<cursor>',['makeExported']],
 ['return-prefix','local function test()\nret<cursor>\nend',['return']],
 ['incomplete-class','final class Player {\nhealth: number = 0\nupdate() {\n self.<cursor>\n',['health','update']],
];
function pos(source){const i=source.indexOf('<cursor>');if(i<0)throw Error('marker');const before=source.slice(0,i);return{source:source.replace('<cursor>',''),position:{line:before.split('\n').length-1,character:before.slice(before.lastIndexOf('\n')+1).length}};}
async function run(){
 fs.mkdirSync(path.join(root,'src/assets'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n');
 fs.writeFileSync(path.join(root,'src/library.tua'),'return {title="hello",config={enabled=true},make=function(): {speed:number} return {speed=2} end}\n');
 fs.writeFileSync(path.join(root,'src/library_more.tua'),'return {}\n');
 fs.writeFileSync(path.join(root,'src/lua_library.lua'),'---@param name string\nlocal function greet(name) return name end\nreturn {value=1,greet=greet}\n');
 fs.writeFileSync(path.join(root,'src/exports.tua'),'export type ExportedPlayer = {health:number}\nlocal function makeExported(): ExportedPlayer return {health=1} end\nreturn {makeExported=makeExported}\n');
 for(const n of ['player.png',"quote'image.png"]){fs.copyFileSync(path.join(repo,'examples/features/src/assets/player.png'),path.join(root,'src/assets',n));}
 const c=new LspClient(path.resolve(process.env.TUA_BIN||path.join(repo,'target/release/tua')),root,{requestTimeoutMs:30000});await c.start();
 const init=await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{completion:{completionItem:{snippetSupport:process.env.TUA_AUDIT_SNIPPETS !== "false",resolveSupport:{properties:['documentation','detail','additionalTextEdits']}}}}}});c.notify('initialized',{});
 const results={client:{snippetSupport:process.env.TUA_AUDIT_SNIPPETS !== "false", triggerKind:process.env.TUA_AUDIT_TRIGGER === "omit" ? "omitted" : Number(process.env.TUA_AUDIT_TRIGGER || 1)},initialize:init,cases:[]};
 try{for(const [id,input,wanted] of cases){const {source,position}=pos(input),uri=pathToFileURL(path.join(root,'src',`${id}.tua`)).href;c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});const start=performance.now();const response=await c.request('textDocument/completion',{textDocument:{uri},position,...(process.env.TUA_AUDIT_TRIGGER === 'omit' ? {} : {context:{triggerKind:Number(process.env.TUA_AUDIT_TRIGGER || 1)}})});const items=completionItems(response);const selected=items.filter(x=>wanted.includes(x.label));const resolved=[];for(const item of selected.slice(0,5)) resolved.push(await c.request('completionItem/resolve',item));
 results.cases.push({id,source,position,ms:performance.now()-start,labels:items.map(x=>x.label),selected,resolved});c.notify('textDocument/didClose',{textDocument:{uri}});console.log(id,items.length,selected.map(x=>x.label).join(','));}
 }finally{fs.writeFileSync(output,JSON.stringify(results,null,2)+'\n');await c.request('shutdown',null).catch(()=>{});c.notify('exit',null);c.child.kill();}
}
run().catch(e=>{console.error(e);process.exitCode=1});
