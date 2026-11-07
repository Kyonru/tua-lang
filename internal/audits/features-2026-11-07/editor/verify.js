const assert=require('assert'),fs=require('fs');const records=JSON.parse(fs.readFileSync(__dirname+'/responses.json'));const get=n=>records.find(r=>r.label===n).result;
assert(get('hover-number').contents.value.includes('value: number'));assert(get('unsaved-hover').contents.value.includes('value: string'));
assert.equal(get('signature').activeParameter,1);assert.equal(get('references-utf16').length,3);
for(const label of ['rename-utf16','unsaved-rename-utf16']) {const edits=Object.values(get(label).changes).flat();assert.equal(edits.length,3);assert.deepEqual(edits.find(e=>e.range.start.line===2).range,{start:{line:2,character:26},end:{line:2,character:31}});}
assert.deepEqual(get('literal').map(x=>x.label).sort(),['no','yes']);assert.deepEqual(get('incomplete').map(x=>x.label).sort(),['health','name']);assert(get('symbols').some(x=>x.name==='add'));assert(get('workspace-symbols').some(x=>x.name==='add'));assert(get('hints').some(x=>x.label===': number'));
let line=0,col=0;const tokens=[];const data=get('tokens').data;for(let i=0;i<data.length;i+=5){col=data[i]===0?col+data[i+1]:data[i+1];line+=data[i];tokens.push({line,col,length:data[i+2]});}assert(tokens.some(t=>t.line===2&&t.col===26&&t.length===5));
assert(get('method').some(x=>x.label.includes('-> nil')));assert(get('audio-event').some(x=>x.label.includes('-> nil')));assert(get('shader-hover').contents.value.startsWith('```tua\n```glsl'));
assert.equal(get('input-overload').activeSignature,0);assert(get('input-overload').signatures[1].label.includes('"move"'));
console.log('Passed: representative editor workflow, UTF-16 tokens/rename, unsaved invalidation; reproduced two defects and known overload gap.');
