const fs=require('fs'),path=require('path'),cp=require('child_process');
const {LspClient,pathToFileURL,delay}=require('../../../../editors/vscode/scripts/lsp-profile-common');
const repo=path.resolve(__dirname,'../../../..'),bin=path.join(repo,'target/release/tua');
const cases={
objects:`local canvas = love.graphics.newCanvas(32, 24)
local font = love.graphics.newFont(12)
local text = love.graphics.newText(font, "hello")
local source = love.audio.newSource(love.sound.newSoundData(100, 44100, 16, 1))
local w, h = canvas:getDimensions()
print(ca«h:canvas»nvas, «h:width»w, «h:height»h, so«h:source»urce)
canvas:setFil«h:inherited»ter("nearest", «s:filter»"linear")
canvas:«c:canvas-members»getWidth()
canvas.«c:dot-rewrite»setFilter("nearest")
text:set("x"):«c:void-members»getWidth(«s:void-signature»)
love.graphics.newCanvas(32,24):getWi«d:chain-definition»dth()
love.graphics.dr«h:draw-doc»aw(canvas, «s:draw-canvas»0, 0)
love.graphics.draw(text, «s:draw-text»0, 0)
love.graphics.setColor(«s:color-table»{1,0,0})
source:«c:source-members»play()
`,
callbacks:`function love.load() end
function love.update(dt)
 print(«h:callback-dt»dt)
end
function love.keypressed(key, scancode, isrepeat)
 print(«h:callback-key»key, «h:callback-repeat»isrepeat)
end
function love.mousepressed(x: string) end
function love.udpate(dt: number) end
function love.draw(unused: number) end
`,
colors:`local background: LoveColor = {0.1,0.2,0.3}
local bad: LoveColor = {"red",0,0}
love.graphics.setColor(background)
local permissive: LoveColor = {3}
`,
callbackcompletion:`function love.«c:callback-snippets»\n`,
};
function decode(input){let text='',last=0;const probes=[];for(const m of input.matchAll(/«([hscd]):([^»]+)»/g)){text+=input.slice(last,m.index);const lines=text.split('\n');probes.push({kind:m[1],id:m[2],position:{line:lines.length-1,character:lines.at(-1).length}});last=m.index+m[0].length;}return{text:text+input.slice(last),probes};}
(async()=>{const results=[];for(const [name,input]of Object.entries(cases)){const root=path.join(__dirname,name);fs.mkdirSync(path.join(root,'src'),{recursive:true});fs.writeFileSync(path.join(root,'tua.toml'),'source="src"\nout="build"\n[diagnostics]\nrules=false\n');const {text,probes}=decode(input),file=path.join(root,'src/main.tua'),uri=pathToFileURL(file).href;fs.writeFileSync(file,text);const cli=cp.spawnSync(bin,['check'],{cwd:root,encoding:'utf8'});const result={name,text,cli:{status:cli.status,stdout:cli.stdout,stderr:cli.stderr},responses:[]};const c=new LspClient(bin,root);await c.start();try{await c.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,capabilities:{textDocument:{completion:{completionItem:{snippetSupport:true}}}}});c.notify('initialized',{});c.notify('textDocument/didOpen',{textDocument:{uri,languageId:'tua',version:1,text}});for(const p of probes){const method={h:'hover',s:'signatureHelp',c:'completion',d:'definition'}[p.kind];const value=await c.request('textDocument/'+method,{textDocument:{uri},position:p.position});result.responses.push({...p,value});if(p.kind==='c'){const item=(Array.isArray(value)?value:value?.items||[]).find(x=>x.label===(name==='callbackcompletion'?'update':'setFilter'));if(item)result.responses.push({id:p.id+'-resolved',value:await c.request('completionItem/resolve',item)});}if(p.kind==='d'&&value?.uri){try{result.builtin=await c.request('tua/builtinDocument',{uri:value.uri});}catch(e){result.builtinError=e.message;}}}result.diagnostics=await c.request('textDocument/diagnostic',{textDocument:{uri}});await c.request('shutdown',null);c.notify('exit',null);}finally{c.child.kill();}results.push(result);console.log(name,JSON.stringify(result.cli),JSON.stringify(result.diagnostics));for(const r of result.responses)console.log(r.id,JSON.stringify(r.value).slice(0,350));}fs.writeFileSync(path.join(__dirname,'evidence.json'),JSON.stringify(results,null,2));})().catch(e=>{console.error(e);process.exitCode=1});
