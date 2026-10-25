"use strict";
const fs = require("node:fs"), os = require("node:os"), path = require("node:path"), assert = require("node:assert/strict");
const { spawn, execFileSync } = require("node:child_process");
const { ActivityStore } = require("../src/workspace/activity-store");
const { InspectorSessions } = require("../src/inspector/sessions");
const game = process.argv[2];
if (!game) throw new Error("Pass the zombie project directory; all work happens in a temporary copy.");
const root = fs.mkdtempSync(path.join(os.tmpdir(), "tua-game-inspector-benchmark-"));
const outputFile = process.env.TUA_GAME_OUTPUT || path.join(os.tmpdir(), "tua-game-inspector-benchmark.json");
const report = { game: path.basename(game), scenario: "960x540 visible rendering, fixed seed and 60 Hz simulation, deterministic aim, five shots/second, paced at 60 FPS, 180 warmup + 600 measured frames", runs: [], complete: false };
const percentile = (values, q) => [...values].sort((a,b) => a-b)[Math.min(values.length-1,Math.ceil(values.length*q)-1)];
(async()=>{
 try {
  fs.cpSync(path.join(game, "src"), path.join(root, "src"), { recursive: true });
  fs.copyFileSync(path.join(game, "tua.toml"), path.join(root, "tua.toml"));
  report.buildOutput = execFileSync(process.env.TUA_BIN || path.resolve(__dirname, "../bin/tua"), ["build"], { cwd: root, encoding: "utf8", timeout: 120000 });
  const output = path.join(root, "build"), main = path.join(output, "main.lua");
  fs.writeFileSync(main, 'love.math.setRandomSeed(42); math.randomseed(42)\n' + fs.readFileSync(main, "utf8") + '\nassert(love.filesystem.load("inspector-benchmark-driver.lua"))()\n');
  fs.copyFileSync(path.join(__dirname, "fixtures/inspector-game-benchmark.lua"), path.join(output, "inspector-benchmark-driver.lua"));
  fs.writeFileSync(path.join(output, "conf.lua"), 'io.stdout:setvbuf("no")\nfunction love.conf(t) t.window.width=960; t.window.height=540; t.window.vsync=0; t.window.title="Tua inspector validation — temporary game copy"; t.modules.audio=false end\n');
  for(let repeat=1;repeat<=3;repeat++) for(const mode of (repeat%2?["no-shim","active","stores-only"]:["stores-only","active","no-shim"])) {
   const activity = new ActivityStore();
   const inspector = new InspectorSessions({ activity, enabled:()=>mode!=="no-shim", options:()=>({categories:mode==="stores-only"?["stores"]:undefined}), assetsDirectory:path.resolve(__dirname,"../assets/inspector") });
   const session=activity.begin({title:mode,kind:"run",cwd:root});let child,timer,snapshots=0,lastSequence;
   const sub=inspector.subscribe(()=>{const sequence=inspector.entries.get(session.id)?.snapshot?.sequence;if(sequence&&sequence!==lastSequence){snapshots++;lastSequence=sequence;}});
   try {
    const launch=await inspector.launch(session,output);
    const result=await new Promise((resolve,reject)=>{
     child=spawn(process.env.TUA_LOVE||"/Applications/love.app/Contents/MacOS/love",[launch||output],{cwd:root,env:process.env,stdio:["ignore","pipe","pipe"]});
     let text="",result,errors="";
     timer=setTimeout(()=>{child.kill("SIGKILL");reject(Error("Game benchmark timed out: "+errors+text));},90000);
     child.stdout.on("data",data=>{text+=data;let index;while((index=text.indexOf("\n"))>=0){const line=text.slice(0,index).trim();text=text.slice(index+1);if(line.startsWith("GAME_RESULT "))result=JSON.parse(line.slice(12));}});
     child.stderr.on("data",data=>{errors+=data;});child.on("error",reject);
     child.on("exit",code=>code===0&&result?resolve(result):reject(Error(`Game exited ${code}: ${errors}${text}`)));
    });
    if(mode!=="no-shim")assert(snapshots>=15,"Inspection must remain active throughout measured gameplay");
    const row={mode,repeat,...result,snapshots,workMeanMs:result.workMs.reduce((a,b)=>a+b,0)/result.workMs.length,workP99Ms:percentile(result.workMs,.99),overBudgetFrames:result.workMs.filter(value=>value>1000/60).length};
    report.runs.push(row);console.log(`${mode} ${repeat}: ${row.fps.toFixed(2)} FPS, work ${row.workMeanMs.toFixed(3)} ms, p99 ${row.workP99Ms.toFixed(3)} ms, over-budget ${row.overBudgetFrames}`);
   }finally{clearTimeout(timer);child?.kill();sub.dispose();activity.finish(session,0);inspector.dispose();activity.dispose();fs.writeFileSync(outputFile,JSON.stringify(report,null,2));}
  }
  report.complete=true;
 }catch(error){report.error=error.message;throw error;}finally{fs.writeFileSync(outputFile,JSON.stringify(report,null,2));fs.rmSync(root,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exitCode=1});
