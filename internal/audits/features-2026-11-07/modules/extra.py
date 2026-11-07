import pathlib,subprocess,json
b=pathlib.Path(__file__).resolve().parent;binary=b.parents[3]/'target/release/tua'
p=b/'declarations';(p/'src/lib').mkdir(parents=True,exist_ok=True);(p/'src/vendor-types').mkdir(exist_ok=True)
(p/'tua.toml').write_text('source="src"\nout="build"\n[[type_libraries]]\npath="src/vendor-types"\ndeclaration_prefix="feel"\nruntime_prefix="lib.feel"\n')
(p/'src/vendor-types/feel.lua').write_text('---@meta feel\nlocal M={}\n---@param value string\n---@return string\nfunction M.echo(value) end\nreturn M\n')
(p/'src/lib/feel.lua').write_text('return {echo=function(value) return value end}\n')
(p/'src/main.tua').write_text('local F = require("lib.feel")\nlocal result = F.echo("hello")\nassert(result=="hello")\nprint(result)\n')
r={}
for cmd in ['check','build']:
 q=subprocess.run([str(binary),cmd],cwd=p,text=True,capture_output=True);r[cmd]=dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr)
q=subprocess.run(['/opt/homebrew/bin/lua','main.lua'],cwd=p/'build',text=True,capture_output=True);r['runtime']=dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr)
r['output_files']=[str(f.relative_to(p/'build')) for f in (p/'build').rglob('*') if f.is_file()]
(b/'declarations-cli.json').write_text(json.dumps(r,indent=2))
p=b/'domains';f=p/'src/main.tua';f.write_text(f.read_text()+'local inline = require("normal").inc\nlocal Position = ecs.component("Position",{x="number"})\nworld:add(e,Position,{x=5})\nlocal localpos = world:get(e,Position)\nlocal localx = localpos.x\nprint(localx,inline(2))\n')
# Preexisting stale output module must not defeat source-root provider resolution.
(p/'build/normal.lua').write_text('return {inc=function(x) return "stale output" end}\n')
