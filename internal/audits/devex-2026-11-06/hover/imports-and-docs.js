'use strict';
const fs=require('node:fs'),path=require('node:path');
const {makeProject,connect,save}=require('../harness');
const files={
'src/counter.tua':`final class Counter {
 value: number = 0
 --- Increase the counter.
 ---@param amount number Units to add.
 ---@return Self The same counter.
 add(amount: number): Self {
  self.value += amount
  return self
 }
}
export type CounterInstance = Counter
return { Counter = Counter }
`,
'src/main.tua':`import type { CounterInstance } from "./counter"
local mod = require("counter")
local counter: CounterInstance = mod.Counter.new()
counter:add(2)
print(counter.value)
`};
(async()=>{const root=makeProject(files,'source="src"\nout="build"\n[diagnostics]\nrules=false\n');const s=await connect(root);try{const doc=s.open('src/main.tua');for(const [label,method,needle,offset]of [['type-import','textDocument/hover','CounterInstance }',2],['type-use','textDocument/hover','CounterInstance =',2],['method-hover','textDocument/hover','counter:add',9],['method-signature','textDocument/signatureHelp','counter:add(',12],['field-hover','textDocument/hover','counter.value',9],['method-definition','textDocument/definition','counter:add',9],['diagnostics','textDocument/diagnostic',undefined,0]])await s.query(label,method,doc,needle,offset);save(path.join(__dirname,'imports-and-docs-evidence.json'),{files,evidence:s.inbound});console.log(s.inbound.filter(e=>e.label!=='initialize'));}finally{await s.close();fs.rmSync(root,{recursive:true,force:true});}})().catch(e=>{console.error(e);process.exitCode=1});
