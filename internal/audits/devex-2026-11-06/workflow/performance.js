"use strict";
const fs=require('node:fs'),path=require('node:path');
const {repo,binary,connect,save,completionItems}=require('../harness');
const summarize=values=>{const sorted=[...values].sort((a,b)=>a-b);return {n:values.length,min:sorted[0],median:sorted[Math.floor(sorted.length/2)],p95:sorted[Math.ceil(sorted.length*.95)-1],max:sorted.at(-1)}};
(async()=>{
 const report={binary,methodology:'Three fresh server processes per bundled project; first import-member completion follows initialize+didOpen. 10 warm and 10 full-buffer post-edit queries per process. End-to-end stdio request latency, not key-to-menu latency; no runtime source files changed.',projects:[]};
 for(const [project,module,field] of [['features','game_math','health'],['starfall','player','x'],['love-full','player','x']]){
  const result={project,groups:{},runs:[]};
  for(let run=0;run<3;run++){
   const root=path.join(repo,'examples',project),s=await connect(root);
   try{
    const text=`local imported = require("${module}")\nlocal canvas = love.graphics.newCanvas(32,32)\nprint(imported.${field})\ncanvas:getWidth()\n`;
    const doc=s.open('src/__audit_overlay__.tua',text);
    const query=async(label,method,needle,offset)=>{
     const response=await s.query(label,method,doc,needle,offset,{context:{triggerKind:1}});
     const entry=s.inbound.at(-1);(result.groups[label]||=[]).push(entry.ms);
     result.samples ||= {}; if (!result.samples[label]) result.samples[label]={params:entry.params,result:response};
     if (response === null) result.failures=(result.failures||[]).concat(`${label} null response`);
     if(method==='textDocument/completion'&&!completionItems(response).some(i=>i.label===field))result.failures=(result.failures||[]).concat(`${label} missing ${field}`);
     return response;
    };
    await query('cold-import-completion','textDocument/completion','imported.',9);
    for(let n=0;n<10;n++){
     await query('warm-import-completion','textDocument/completion','imported.',9);
     await query('warm-hover','textDocument/hover',`imported.${field}`,10);
     await query('warm-signature','textDocument/signatureHelp','getWidth(',9);
    }
    for(let n=0;n<10;n++){
     s.change(doc,text+`-- edit ${n}\n`);
     await query('post-edit-import-completion','textDocument/completion','imported.',9);
    }
    s.change(doc,text.replace(`imported.${field})`,'imported.)'));
    await query('incomplete-completion','textDocument/completion','imported.',9);
    result.runs.push({initializeMs:s.inbound[0].ms,requestCount:s.inbound.length,stderr:s.client.stderr});
   }finally{await s.close();}
  }
  result.summary=Object.fromEntries(Object.entries(result.groups).map(([k,v])=>[k,summarize(v)]));report.projects.push(result);
 }
 save(path.join(__dirname,'performance-evidence.json'),report);console.log(JSON.stringify(report.projects.map(({project,summary,failures})=>({project,summary,failures})),null,2));
})().catch(error=>{console.error(error);process.exitCode=1});
