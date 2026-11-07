"use strict";
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),Module=require('node:module'),{spawn,execFileSync}=require('node:child_process');
const repo=path.resolve(__dirname,'../../../..'),root=path.join(__dirname,'runtime-fixture');
const old=Module._load;Module._load=function(id,...args){if(id==='vscode')return {};return old.call(this,id,...args)};
const {loadSourceMapForGeneratedPath}=require(path.join(repo,'editors/vscode/src/editor/source-maps'));
const {RuntimeOrigins}=require(path.join(repo,'editors/vscode/src/inspector/origins'));
const {ActivityStore}=require(path.join(repo,'editors/vscode/src/workspace/activity-store'));
const {InspectorSessions}=require(path.join(repo,'editors/vscode/src/inspector/sessions'));
const activity=new ActivityStore(),origins=new RuntimeOrigins(loadSourceMapForGeneratedPath),inspector=new InspectorSessions({activity,enabled:()=>true,assetsDirectory:path.join(repo,'editors/vscode/assets/inspector'),describeObject:(e,o)=>origins.describe(e,o),reportError:console.error});
let child; const sleep=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{const build=execFileSync(path.join(repo,'target/release/tua'),['build'],{cwd:root,encoding:'utf8'});const session=activity.begin({title:'mapped store',kind:'run',cwd:root});const launch=await inspector.launch(session,path.join(root,'build'));child=spawn('/Applications/LOVE.app/Contents/MacOS/love',[launch],{cwd:root,stdio:'ignore'});let object;
for(let i=0;i<100;i++){object=inspector.entries.get(session.id)?.snapshot?.objects.find(o=>o.kind==='stores');if(object)break;await sleep(100)}
assert(object,'store snapshot');assert.equal(object.origin.path,path.join(root,'src/main.tua'));assert.equal(object.origin.line,1);assert.equal(object.origin.name,'player');assert.equal(object.origin.generated,false);assert(object.data.state.score>=10);
fs.writeFileSync(path.join(__dirname,'raw/runtime-mapped.json'),JSON.stringify({build,object},null,2));console.log('Compiled Tua store runtime snapshot maps to player at src/main.tua:1');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(async()=>{child?.kill('SIGKILL');await sleep(150);inspector.dispose();activity.dispose()});
