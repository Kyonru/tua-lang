'use strict';
const fs=require('node:fs'), path=require('node:path');
const {LspClient,pathToFileURL,delay,completionItems}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const repo=path.resolve(__dirname,'../../../..');
const bin=path.resolve(process.env.TUA_AUDIT_BINARY||path.join(repo,'target/release/tua'));
const scratch=path.join(repo,'target/devex-audit-2026-11-06/hover');
const cases=[
{name:'basics',source:`type Player = { health: number, name: string }
--- Applies damage.
---@param player Player target player
---@param amount number hit points
---@return number remaining health
local function damage(player: Player, amount: number = 1): number
  return player.health - amount
end
local player: Player = {health = 10, name = "A"}
local health = damage(player, 2)
print(pla«h:local»yer, hea«h:inferred»lth, player.hea«h:field»lth)
damage(player, «s:second»2)
dam«h:function»age(player, 2)
`,probes:[]},
{name:'multi-return',source:`local function pair(): (number, string)
 return 42, "hi"
end
local n, s = pair()
print(«h:number»n, «h:string»s)
pai«h:pair»r(«s:pair»)
`},
{name:'nested',source:`local function inner(x: number, y: number): number return x+y end
local function outer(a: number, b: string, c: boolean): number return a end
outer(inner(1, «s:inner»2), «s:outer-b»"a,b", «s:outer-c»true)
outer(inner(1,2)«s:after-close», "x", true)
outer(1, -- a comma, and bracket (
 «s:after-comment»"hello", true)
`},
{name:'overload',source:`--- Load one of two value kinds.
---@overload fun(value: string): string
---@overload fun(value: number): number
local function choose(value) return value end
choose(«s:string»"x")
choose(«s:number»42)
cho«h:choose»ose(42)
`},
{name:'methods',source:`type Counter = {value: number, add: (self: Counter, amount: number) -> number}
local counter: Counter = {value=1, add=function(self: Counter, amount: number): number return self.value+amount end}
counter:add(«s:colon»1)
counter.add(counter, «s:dot»1)
counter:a«h:method»dd(1)
`},
{name:'classes',source:`class Counter {
 value: number = 0
 init(value: number) { self.value = value }
 --- Adds an amount.
 ---@param amount number the increment
 ---@return Self the same counter
 add(amount: number): Self { self.value += amount; return self }
 read(): number { return self.value }
}
local counter = Counter.new(1)
counter:add(«s:class-colon»1)
counter.add(«s:class-dot»1)
counter:add(1):re«h:chain»ad(«s:chain»)
print(coun«h:class»ter)
`},
{name:'love',source:`local font = love.graphics.newFont(12)
local text = love.graphics.newText(font, "hello")
local canvas = love.graphics.newCanvas(20, 20)
local w, h = text:getDimensions()
print(«h:width»w, «h:height»h)
text:getDime«h:dimensions»nsions(«s:dimensions»)
text:set(«s:text-set»"hello")
text:set("x"):getWidth(«s:void-chain»)
love.graphics.newCanvas(20,20):setFil«h:chain-filter»ter("nearest", «s:chain-filter»"linear")
love.graphics.draw(text, «s:draw-text»1, 2)
love.graphics.draw(canvas, «s:draw-canvas»1, 2)
love.graphics.print("hi", «s:print»1, 2)
love.graphics.newImage(«s:image-filedata»love.filesystem.newFileData("a", "a.png"))
`},
{name:'stdlib',source:`local text = string.upper("hi")
local parts = {"a", "b"}
print(te«h:text»xt)
string.up«h:upper»per(«s:upper»"hi")
string.find("hi", «s:find»"h")
table.insert(parts, «s:insert»"c")
local a,b=string.find("hi", "h")
print(«h:find-a»a, «h:find-b»b)
`},
{name:'narrowing',source:`type Item = { value: number, child?: Item }
local function consume(item: Item?)
 if item then
   print(it«h:narrow»em.value)
   if item.child then print(item.chi«h:child»ld.value) end
 end
 print(it«h:wide»em)
end
`},
{name:'annotation',source:`--- Useful documentation.
---@param amount number amount to add
---@return number total
local function add(amount) return amount+1 end
add(«s:documented»1)
a«h:documented»dd(1)
---@par«h:partial»
---@param«h:complete-tag»
---@unknown«h:unknown-tag»
`},
{name:'imported',files:{'module.lua':`--- Module description.
local M = {}
--- Multiplies a number.
---@param value number the operand
---@param factor number multiplier
---@return number product
function M.multiply(value, factor) return value*factor end
return M
`},source:`local mod = require("module")
local result = mod.multiply(2, 3)
print(res«h:result»ult)
mod.mul«h:import-function»tiply(2, «s:import-function»3)
`},
{name:'callback',source:`type Entry = {name: string, score: number}
local values: Entry[] = {{name="A", score=1}}
table.sort(values, function(a,b)
 print(«h:callback-a»a.name, «h:callback-b»b.score)
 return a.score < b.score
end)
`},
{name:'varargs',source:`local function many(head: string, ...: number): number return 1 end
many("x", 1, «s:third»2, «s:fourth»3)
`},
{name:'table-arg',source:`local function use(config: {x:number, y:number}, value: string): string return value end
use({x=1, y=2}, «s:table-following»"x")
use({x=1, y=«s:inside-table»2}, "x")
`},
{name:'callback-arg',source:`local function use(fn: (a:number,b:number)->number, name:string): string return name end
use(function(a,b) return a+b end, «s:after-function»"x")
use(function(a,b) return math.max(a,«s:inside-callback-call»b) end, "x")
`},
];
function decode(source){const probes=[]; let text='';let last=0; const re=/«([hsc]):([^»]+)»/g;for(const match of source.matchAll(re)){text+=source.slice(last,match.index);const before=text.split('\n');probes.push({kind:match[1],id:match[2],position:{line:before.length-1,character:before.at(-1).length}});last=match.index+match[0].length;}text+=source.slice(last);return {text,probes};}
async function executeCase(c){const root=path.join(scratch,c.name);fs.mkdirSync(path.join(root,'src'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nrules=false\n');const {text,probes}=decode(c.source); const filename=path.join(root,'src/main.tua');fs.writeFileSync(filename,text);for(const [name,source]of Object.entries(c.files||{}))fs.writeFileSync(path.join(root,'src',name),source);const uri=pathToFileURL(filename).href;const client=new LspClient(bin,root,{requestTimeoutMs:30000});const evidence={name:c.name,source:text,probes,results:[]};await client.start();try{await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{hover:{contentFormat:['markdown']},signatureHelp:{signatureInformation:{documentationFormat:['markdown'],parameterInformation:{labelOffsetSupport:true}}}}}});client.notify('initialized',{});client.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});for(const p of probes){const method=p.kind==='h'?'textDocument/hover':p.kind==='s'?'textDocument/signatureHelp':'textDocument/completion';const value=await client.request(method,{textDocument:{uri},position:p.position});evidence.results.push({id:p.id,kind:p.kind,position:p.position,value});}evidence.hints=await client.request('textDocument/inlayHint',{textDocument:{uri},range:{start:{line:0,character:0},end:{line:text.split('\n').length,character:0}}});await delay(100);evidence.diagnostics=client.diagnostics.map(x=>x.params||x);await client.request('shutdown',null);client.notify('exit',null);}finally{client.child.kill();}return evidence;}
(async()=>{const selected=process.argv.slice(2);const results=[];for(const c of cases.filter(c=>!selected.length||selected.includes(c.name))){const r=await executeCase(c);results.push(r);console.log(c.name, r.results.map(p=>`${p.id}=${p.value===null?'NULL':p.kind==='h'?p.value.contents.value.replace(/\n/g,' ').slice(0,140):`sig${p.value.activeSignature}/param${p.value.activeParameter}:`+p.value.signatures[p.value.activeSignature].label}`).join('\n  '));}fs.writeFileSync(path.join(__dirname,selected.length?'selected-evidence.json':'evidence.json'),JSON.stringify({bin,results},null,2)+'\n');})().catch(e=>{console.error(e);process.exitCode=1});
