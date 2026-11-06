'use strict';
// Run from any directory; only generated projects below target/ are rewritten.
const fs=require('fs'),path=require('path'),cp=require('child_process');
const repo=path.resolve(__dirname,'../../../../');
const {LspClient,pathToFileURL,completionItems}=require(path.join(repo,'editors/vscode/scripts/lsp-profile-common'));
const bin=path.resolve(process.env.TUA_AUDIT_BINARY || path.join(repo,'target/release/tua'));
const cases=JSON.parse(fs.readFileSync(path.join(__dirname,'cases.json'),'utf8'));
const out=[];
async function main(){
for(const c of cases){
 if(process.argv[2]&&!c.id.includes(process.argv[2]))continue;
 const root=path.join(repo,'target/devex-audit-2026-11-06/typing',c.id);
 fs.rmSync(root,{recursive:true,force:true});fs.mkdirSync(path.join(root,'src'),{recursive:true});
 fs.writeFileSync(path.join(root,'tua.toml'),"source = 'src'\nout = 'build'\n"+(c.config||''));
 for(const [name,text] of Object.entries(c.files)){const p=path.join(root,'src',name);fs.mkdirSync(path.dirname(p),{recursive:true});fs.writeFileSync(p,text);}
 const checked=cp.spawnSync(bin,['check'],{cwd:root,encoding:'utf8',timeout:30000});
 const r={id:c.id,expectation:c.expectation,check:{status:checked.status,stdout:checked.stdout,stderr:checked.stderr},probes:[]};
 const client=new LspClient(bin,root,{requestTimeoutMs:20000});await client.start();
 try{
 await client.request('initialize',{processId:process.pid,rootUri:pathToFileURL(root).href,workspaceFolders:[{uri:pathToFileURL(root).href,name:c.id}],capabilities:{textDocument:{diagnostic:{dynamicRegistration:false}}}});client.notify('initialized',{});
 for(const [name,text] of Object.entries(c.files)){const uri=pathToFileURL(path.join(root,'src',name)).href;client.notify('textDocument/didOpen',{textDocument:{uri,languageId:name.endsWith('.lua')?'lua':'tua',version:1,text}});}
 for(const q of c.probes||[]){const file=q.file||'main.tua',source=c.files[file];const found=q.first?source.indexOf(q.needle):source.lastIndexOf(q.needle);if(found<0)throw Error('missing needle '+q.needle);const offset=found+(q.offset??1);const before=source.slice(0,offset),position={line:before.split('\n').length-1,character:before.split('\n').at(-1).length};const method=q.method||'textDocument/hover';const params={textDocument:{uri:pathToFileURL(path.join(root,'src',file)).href},position,...q.params};const result=await client.request(method,params);r.probes.push({query:q,position,result});}
 const file=c.main||'main.tua';r.diagnostics=await client.request('textDocument/diagnostic',{textDocument:{uri:pathToFileURL(path.join(root,'src',file)).href}});
 await client.request('shutdown',null);client.notify('exit',null);
 }catch(e){r.error=String(e)}finally{client.child.kill();}
 out.push(r);console.log(c.id+' check='+r.check.status+' probes='+r.probes.length+(r.error?' '+r.error:''));
 fs.writeFileSync(path.join(__dirname,process.argv[2]?'results-'+process.argv[2]+'.json':'results.json'),JSON.stringify({binary:bin,results:out},null,2)+'\n');
}
}
main().catch(e=>{console.error(e);process.exitCode=1});
