'use strict';
const fs=require('fs'),path=require('path'),cp=require('child_process');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const dir=__dirname,bin=path.resolve(dir,'../../../../target/release/tua');
const legacy=`---@alias Kind "friendly" | "hostile"
---@class Enemy
---@field name string Display name
---@field hp number
---@field kind Kind
---@param name string
---@return Enemy
local function makeEnemy(name)
 return {name=name,hp=20,kind="hostile"}
end
---@param head string
---@vararg number
---@return number
local function sum(head, ...) local n=0 for _,v in ipairs({...}) do n=n+v end return n end
---@overload fun(value: string): string
---@overload fun(value: number): number
local function identity(value) return value end
---@return number, string
local function pair() return 3,"three" end
return {makeEnemy=makeEnemy,sum=sum,identity=identity,pair=pair}
`;
const player=`local Player = {}\nPlayer.__index = Player\nfunction Player.new(name) return setmetatable({name=name,x=0,active=true},Player) end\nfunction Player:update(dt) self.x=self.x+100*dt end\nreturn Player\n`;
const cases=[
{name:'metadata-native',rules:true,source:`---@generic T
---@param value any
---@return any
local function id(value) return value end
---@deprecated Use id
---@nodiscard
---@param value string
---@return string
local function old(value) return value end
old("hi")
---@diagnostic disable: all
---@type number
local wrong = "bad"
`,probes:[]},
{name:'conflict-malformed',source:`---@param value string
local function f(value: number): number return value end
---@param
local function g() end
`,probes:[]},
{name:'overload-local',source:`---@overload fun(value: string): string
---@overload fun(value: number): number
local function identity(value) return value end
identity(true)
`,probes:[['signatureHelp','identity',3,9]]},
{name:'overload-import',files:{'legacy.lua':legacy},source:`local Legacy = require("legacy")
local value = Legacy.identity(true)
print(value)
`,probes:[['signatureHelp','identity',1,30],['hover','value',2,7]]},
{name:'mixed-success',files:{'legacy.lua':legacy},source:`local Legacy = require("legacy")
local enemy = Legacy.makeEnemy("slime")
enemy.hp -= 5
local n,s = Legacy.pair()
print(enemy.hp,n,s,Legacy.sum("total",1,2,3))
`,probes:[]},
{name:'metatable',files:{'player.lua':player},source:`local Player = require("player")
local p = Player.new("Ada")
p:update(0.02)
print(p.x)
`,probes:[['signatureHelp','update',2,9]]},
{name:'positive',files:{'legacy.lua':legacy,'player.lua':player},source:`local Legacy = require("legacy")
local enemy = Legacy.makeEnemy("slime")
enemy.hp -= 5
local Player = require("player")
local player = Player.new("Ada")
player:update(0.02)
local n, s = Legacy.pair()
local result = Legacy.sum("total",1,2,3)
print(enemy.hp, player.x, n, s, result, Legacy.identity(4))
`,probes:[['hover','enemy.hp',2,2],['completion','enemy.',2,6],['signatureHelp','Legacy.sum(',7,26],['hover','n, s',6,6],['completion','player:',5,7]]},
{name:'negative',files:{'legacy.lua':legacy},source:`local Legacy = require("legacy")
Legacy.makeEnemy(42)
Legacy.sum("x", "bad")
Legacy.identity(true)
local enemy = Legacy.makeEnemy("slime")
enemy.hp = "bad"
print(enemy.missing)
`,probes:[]},
{name:'annotations',ext:'lua',source:`---@alias Label string
---@generic T
---@param value any
---@return any
local function id(value) return value end
---@deprecated Use id
---@nodiscard
---@param value string
---@return string
local function old(value) return value end
old("hi")
---@diagnostic disable: all
---@type number
local wrong = "bad"
---@param value Label
local function label(value) return value end
`,probes:[['hover','deprecated',5,6],['completion','---@',14,4],['completion','Label',14,18],['signatureHelp','old("',10,5]]},
{name:'emission',source:`type Point = {x: number, y: number}
--- Make a point.
local function point(x: number, y: number): Point return {x=x,y=y} end
local p: Point = point(2,3)
print(p.x+p.y)
return {point=point}
`,probes:[]}
];
(async()=>{const all=[];for(const c of cases){const root=path.join(dir,c.name);fs.mkdirSync(path.join(root,'src'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nrules='+Boolean(c.rules)+'\ndoc_annotations=true\n[emit]\nannotation_mode="luacats"\n');const file=path.join(root,'src/main.'+(c.ext||'tua'));fs.writeFileSync(file,c.source);for(const [n,s]of Object.entries(c.files||{}))fs.writeFileSync(path.join(root,'src',n),s);const r={name:c.name};for(const command of ['check','build']){const p=cp.spawnSync(bin,[command],{cwd:root,encoding:'utf8'});r[command]={status:p.status,stdout:p.stdout,stderr:p.stderr};}if(r.build.status===0){const p=cp.spawnSync('/opt/homebrew/bin/lua',['main.lua'],{cwd:path.join(root,'build'),encoding:'utf8'});r.runtime={status:p.status,stdout:p.stdout,stderr:p.stderr};r.luaCopiesEqual=Object.keys(c.files||{}).filter(n=>n.endsWith('.lua')).every(n=>fs.readFileSync(path.join(root,'build',n),'utf8')===c.files[n]);}const client=new LspClient(bin,root,{requestTimeoutMs:30000});await client.start();try{await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{}});client.notify('initialized',{});const uri=pathToFileURL(file).href;client.notify('textDocument/didOpen',{textDocument:{uri,languageId:c.ext||'tua',version:1,text:c.source}});r.lsp=[];for(const [method,label,line,character]of c.probes)r.lsp.push({method,label,line,character,result:await client.request('textDocument/'+method,{textDocument:{uri},position:{line,character}})});await delay(700);r.diagnostics=client.diagnostics;await client.request('shutdown',null);client.notify('exit',null);}finally{client.child.kill();}all.push(r);console.log(c.name,JSON.stringify(r));}fs.writeFileSync(path.join(dir,'results.json'),JSON.stringify(all,null,2));})().catch(e=>{console.error(e);process.exitCode=1});
