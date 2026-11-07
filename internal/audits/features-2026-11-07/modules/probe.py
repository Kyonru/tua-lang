import pathlib,subprocess,json
base=pathlib.Path(__file__).resolve().parent
binary=base.parents[3]/'target/release/tua'
fixtures={
'happy':{'types.d.tua':'export type Player = {name: string}\n','facade.d.tua':'export type { Player as Person } from "./types"\n','player.tua':'local function new(name: string): {name: string}\n return {name=name}\nend\nreturn {new=new}\n','legacy.lua':'local M={}\nfunction M.double(x) return x*2 end\nreturn M\n','main.tua':'import type { Person } from "./facade"\nlocal P = import("./player")\nlocal L = require("legacy")\nlocal p: Person = P.new("Mina")\nassert(p.name=="Mina")\nassert(L.double(3)==6)\nprint("modules happy passed")\n'},
'cycle':{'a.d.tua':'export type { B as A } from "./b"\n','b.d.tua':'export type { A as B } from "./a"\n','main.tua':'import type { A } from "./a"\n'},
'missing':{'main.tua':'local M = import("./absent")\n'},
'domains':{'game.tua':'local game = store({score=0,paused=false})\nreturn {game=game}\n','components.tua':'local Position=ecs.component("Position",{x="number"})\nreturn {Position=Position}\n','normal.tua':'local function inc(x: number): number\n return x+1\nend\nreturn {inc=inc}\n','main.tua':'local G = require("game")\nlocal game = G.game\nlocal state = game:get()\nlocal direct = require("game").game\nlocal ds = direct:get()\nlocal N = require("normal")\nlocal n = N.inc(3)\nlocal C = require("components")\nlocal world = ecs.world()\nlocal e = world:create()\nworld:add(e,C.Position,{x=4})\nlocal position = world:get(e,C.Position)\nlocal x = position.x\nprint(state.score,ds.score,n,x)\n'}
}
results={}
for name,files in fixtures.items():
 p=base/name;(p/'src').mkdir(parents=True,exist_ok=True);(p/'tua.toml').write_text('source="src"\nout="build"\n')
 for f,s in files.items():(p/'src'/f).write_text(s)
 results[name]={}
 for command in ['check','build']:
  r=subprocess.run([str(binary),command],cwd=p,text=True,capture_output=True);results[name][command]={'code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
 if results[name]['build']['code']==0:
  r=subprocess.run(['/opt/homebrew/bin/lua','main.lua'],cwd=p/'build',text=True,capture_output=True);results[name]['runtime']={'code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
(base/'cli.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
