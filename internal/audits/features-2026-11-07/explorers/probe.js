const fs=require('fs'),path=require('path');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const out=__dirname,root=path.join(out,'fixture'),src=path.join(root,'src');
fs.writeFileSync(path.join(root,'tua.toml'),'source = "src"\nout = "build"\nmain = "main.tua"\n');
const source=`local other = require("other")
local events = signal()
events:declare("player.hit", function(player: string) end)
events:on("player.hit", function(player: string) end)
events:emit("player.hit", "Ada")
local Position = ecs.component("Position", { x = "number" })
local world = ecs.world()
local movement = ecs.system(world, { Position }, function(_, position, dt) position.x += dt end)
local found = world:query({ Position })
local bullets = objectPool({ create = function() return { active = false } end })
local bullet = bullets:acquire()
bullets:release(bullet)
local machine = stateMachine({ initial = "idle", events = { { name = "go", from = "idle", to = "run" }, { name = "stop", from = "*", to = "idle" } } })
local audio = audioSystem({ buses = { master = { volume = 1 }, sfx = { parent = "master" } }, cues = {} })
return { Position = Position }
`;
fs.writeFileSync(path.join(src,'main.tua'),source);fs.writeFileSync(path.join(src,'other.tua'),'local main = require("main")\nreturn { value = 42 }\n');fs.writeFileSync(path.join(src,'interop.lua'),'return { plain = true }\n');
(async()=>{const c=new LspClient(path.resolve('target/release/tua'),root);await c.start();const uri=pathToFileURL(path.join(src,'main.tua')).href;
const results={};results.initialize=await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{},workspaceFolders:[{uri:pathToFileURL(root).href,name:'explorers'}]});c.notify('initialized',{});c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text:source}});
const methods=['moduleGraph','signalGraph','ecsGraph','objectPoolGraph','audioGraph','assetInventory'];async function capture(label){results[label]={};for(const method of methods)results[label][method]=await c.request('tua/'+method,null);results[label].inspect=await c.request('tua/inspect',{textDocument:{uri}});}
await delay(1000);await capture('initial');c.notify('textDocument/didChange',{textDocument:{uri,version:2},contentChanges:[{text:source.replaceAll('player.hit','player.changed').replaceAll('Position','Location').replaceAll('"idle"','"rest"').replaceAll('sfx','effects')}]});await capture('unsaved');
c.notify('textDocument/didChange',{textDocument:{uri,version:3},contentChanges:[{text:'local config = require("other")\nlocal machine = stateMachine(config)\nlocal audio = audioSystem(config)\n'}]});await capture('dynamic');
c.notify('textDocument/didChange',{textDocument:{uri,version:4},contentChanges:[{text:''}]});await capture('empty');
fs.writeFileSync(path.join(src,'new.tua'),'local New = ecs.component("New", { value = "number" })\nreturn New\n');c.notify('workspace/didChangeWatchedFiles',{changes:[{uri:pathToFileURL(path.join(src,'new.tua')).href,type:1}]});await capture('created');
fs.unlinkSync(path.join(src,'new.tua'));c.notify('workspace/didChangeWatchedFiles',{changes:[{uri:pathToFileURL(path.join(src,'new.tua')).href,type:3}]});await capture('deleted');
fs.writeFileSync(path.join(out,'responses.json'),JSON.stringify(results,null,2));fs.writeFileSync(path.join(out,'diagnostics.json'),JSON.stringify(c.diagnostics,null,2));await c.request('shutdown',null);const done=c.done();c.notify('exit',null);await done;console.log(Object.fromEntries(Object.entries(results).filter(([k])=>k!=='initialize').map(([k,v])=>[k,Object.fromEntries(Object.entries(v).map(([m,r])=>[m,JSON.stringify(r).length]))])));})().catch(e=>{console.error(e);process.exit(1)});
