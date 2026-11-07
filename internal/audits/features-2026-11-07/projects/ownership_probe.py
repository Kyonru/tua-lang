from pathlib import Path
import subprocess,json
R=Path(__file__).resolve().parent; B=R.parents[3]/'target/release/tua'; rows=[]
for name,config,files in [
 ('inplace-declaration','source="src"\nout="src"\n',{'src/types.d.tua':'export type Point = {x: number}\n','src/types.d.lua':'return { important = "handwritten source" }\n'}),
 ('unowned-declaration','',{'src/types.d.tua':'export type Point = {x: number}\n','build/types.d.lua':'return { important = "unrecorded output" }\n'}),
 ('modified-maps','emit_maps=false\n',{'src/main.tua':'print("ok")\n','build/main.lua.map':'manually maintained map\n','build/main.lua.map.json':'manually maintained metadata\n'})]:
 p=R/'fixtures'/name;p.mkdir(parents=True,exist_ok=True);files['tua.toml']=config
 for path,t in files.items():
  f=p/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(t)
 r=subprocess.run([str(B),'build'],cwd=p,text=True,capture_output=True)
 row=dict(name=name,command=[str(B),'build'],cwd=str(p),before=files,exit=r.returncode,stdout=r.stdout,stderr=r.stderr,after={str(f.relative_to(p)):f.read_text() for f in p.rglob('*') if f.is_file()})
 rows.append(row);print(json.dumps(row))
(R/'ownership.json').write_text(json.dumps(rows,indent=2))
