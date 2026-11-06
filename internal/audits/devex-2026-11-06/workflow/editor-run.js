"use strict";
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),net=require('node:net');
const {repo,binary}=require('../harness');
(async()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'tua-devex-editor-'));
 const project=path.join(root,'project');fs.mkdirSync(path.join(project,'src'),{recursive:true});
 fs.writeFileSync(path.join(project,'tua.toml'),'source="src"\nout="build"\n');
 fs.writeFileSync(path.join(project,'src/entities.tua'),'export type Entity = {health:number}\nlocal function createEntity(): Entity return {health=1} end\nreturn {createEntity=createEntity}\n');
 fs.writeFileSync(path.join(project,'src/main.tua'),'local result = createEnt\n');
 fs.writeFileSync(path.join(project,'src/unicode.tua'),'local prefix="😀"; local score: number = 7; print(score)\n');
 fs.writeFileSync(path.join(project,'src/control.tua'),'local obj={health=1}\nprint(obj.health)\n');
 const user=path.join(root,'user');fs.mkdirSync(path.join(user,'User'),{recursive:true});
 fs.writeFileSync(path.join(user,'User/settings.json'),JSON.stringify({'telemetry.telemetryLevel':'off','workbench.startupEditor':'none','tua.server.path':binary,'editor.inlineSuggest.enabled':false}));
 const server=net.createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));
 try{
  await require(path.join(repo,'editors/vscode/node_modules/@vscode/test-electron')).runTests({
   extensionDevelopmentPath:path.join(repo,'editors/vscode'),extensionTestsPath:path.join(__dirname,'editor-suite.js'),
   vscodeExecutablePath:process.env.VSCODE_EXECUTABLE_PATH||'/Applications/Visual Studio Code.app/Contents/MacOS/Code',
   extensionTestsEnv:{TUA_SERVER_PATH:binary,TUA_AUDIT_PROJECT:project,TUA_AUDIT_PORT:String(port),TUA_AUDIT_OUTPUT:__dirname},
   launchArgs:[project,'--disable-extensions','--disable-workspace-trust','--skip-welcome','--skip-release-notes',`--user-data-dir=${user}`,`--extensions-dir=${root}/extensions`,`--remote-debugging-port=${port}`],reuseMachineInstall:true,
  });
 }finally{fs.rmSync(root,{recursive:true,force:true});}
})().catch(e=>{console.error(e);process.exitCode=1});
