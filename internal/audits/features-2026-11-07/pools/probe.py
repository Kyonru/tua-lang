from pathlib import Path
import subprocess,json
repo=Path(__file__).resolve().parents[4];base=Path(__file__).resolve().parent
cases={
'positive':'''type Bullet = { x: number }
local pool = objectPool({create=function(): Bullet return {x=0} end,reset=function(b: Bullet) b.x=0 end,initial=2})
local b = pool:acquire()
b.x = 12
assert(pool:release(b))
assert(not pool:release(b))
assert(not pool:release({x=0}))
assert(pool:acquire()==b)
assert(pool:prewarm(4)==3)
assert(pool:capacity()==5)
pool:clear()
assert(pool:capacity()==0)
print("positive PASS")
''',
'bad_reset':'''type Bullet = { x: number }
local pool = objectPool({create=function(): Bullet return {x=0} end,reset=function(b: {name:string}) print(string.upper(b.name)) end})
local b = pool:acquire()
pool:release(b)
''',
'bad_dispose':'''local pool = objectPool({create=function(): {x:number} return {x=0} end,dispose=function(b: {name:string}) print(string.upper(b.name)) end,initial=1})
pool:clear()
''',
'bad_release':'''local pool = objectPool({create=function(): {x:number} return {x=0} end})
pool:release({name="wrong"})
''',
'zero_hook':'''local pool = objectPool({create=function(): {x:number} return {x=0} end,reset=function() print("reset") end})
local b=pool:acquire()
pool:release(b)
''',
}
results=[]
for name,src in cases.items():
 p=base/name;(p/'src').mkdir(parents=True,exist_ok=True);(p/'tua.toml').write_text('source="src"\nout="build"\n');(p/'src/main.tua').write_text(src)
 for cmd in [[str(repo/'target/release/tua'),'check'],[str(repo/'target/release/tua'),'build']]:
  x=subprocess.run(cmd,cwd=p,capture_output=True,text=True);results.append(dict(case=name,command=cmd,cwd=str(p),code=x.returncode,stdout=x.stdout,stderr=x.stderr))
  if cmd[-1]=='build' and x.returncode==0:
   x=subprocess.run(['lua','main.lua'],cwd=p/'build',capture_output=True,text=True);results.append(dict(case=name,command=['lua','main.lua'],cwd=str(p/'build'),code=x.returncode,stdout=x.stdout,stderr=x.stderr))
(base/'cli-results.json').write_text(json.dumps(results,indent=2)+'\n')
for x in results: print(x['case'],x['command'][-1],x['code'],x['stdout'],x['stderr'])
