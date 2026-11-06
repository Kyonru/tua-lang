"use strict";
const fs=require('node:fs'),path=require('node:path');
const {makeProject,connect,save,delay,position}=require('../harness');
(async()=>{
 const root=makeProject({'src/main.tua':'local amount: number = 1\nprint(amount)\n'});const s=await connect(root);
 try {
  const doc=s.open('src/main.tua');
  for(let n=0;n<24;n++)s.change(doc,`local amount: number = ${n%2?'1':'"bad"'}\nprint(amount)\n`);
  await delay(650);await s.query('final-hover','textDocument/hover',doc,'amount)',1);await s.query('final-pull','textDocument/diagnostic',doc);
  const burst={version:doc.version,text:doc.text,evidence:[...s.inbound],publishes:s.notifications.filter(n=>n.method==='textDocument/publishDiagnostics')};
  // Incremental edit through a UTF-16/non-BMP prefix and CRLF. Correct type change
  // isolates document synchronization from WF-01's query/range conversion defect.
  const source='local prefix="😀"; local amount: number = 1\r\nprint(amount)\r\n';s.change(doc,source);
  const start=position(source,'= 1',2),end={...start,character:start.character+1};
  s.client.notify('textDocument/didChange',{textDocument:{uri:doc.uri,version:++doc.version},contentChanges:[{range:{start,end},rangeLength:1,text:'"bad"'}]});
  doc.text=source.replace('= 1','= "bad"');
  await s.query('incremental-utf16-crlf-diagnostics','textDocument/diagnostic',doc);
  save(path.join(__dirname,'freshness-evidence.json'),{burst,incremental:{source,change:{range:{start,end},text:'"bad"'},expectedText:doc.text,evidence:s.inbound.slice(burst.evidence.length)}});
  console.log('version',doc.version,'publishes',burst.publishes.map(n=>n.params.version),'last diagnostics',burst.publishes.at(-1)?.params.diagnostics,'incremental',s.inbound.at(-1).result);
 }finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}
})().catch(e=>{console.error(e);process.exitCode=1});
