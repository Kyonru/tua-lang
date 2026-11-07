import pathlib,subprocess,json
base=pathlib.Path(__file__).resolve().parent;repo=base.parents[3];binary=repo/'target/release/tua'
cases={
'happy':'''local P = ecs.component("Position", { x = "number" })
local V = ecs.component("Velocity", { x = "number" })
local world = ecs.world()
local a = world:create()
local b = world:create()
world:add(a, P, {x=1})
world:add(b, P, {x=2})
world:add(a, V, {x=3})
local movement = ecs.system(world, {P, V}, function(entity, position, velocity, dt)
 position.x += velocity.x * dt
end)
movement:update(2)
assert(world:get(a,P).x == 7)
assert(world:query({P,V}):count() == 1)
movement.enabled = false
movement:update(3)
assert(world:get(a,P).x == 7)
world:query({P}):each(function(entity) world:destroy(entity) end)
assert(world:query({P}):count() == 0)
local c = world:create()
world:add(c,P,{x=5})
local calls = 0
world:query({P}):each(function(entity) calls += 1; world:clear() end)
assert(calls == 1)
assert(world:create() > c)
print("happy lifecycle passed")
''',
'false_value':'''local schema: any = nil
local Flag = ecs.component("Flag", schema)
local world = ecs.world()
local e = world:create()
world:add(e, Flag, false)
print("has", world:has(e,Flag), "get", world:get(e,Flag), "count", world:query({Flag}):count())
assert(world:get(e,Flag) == false, "false is a valid stored component value")
''',
'optional_schema':'''local Flag = ecs.component("Flag")
print(Flag)
''',
'missing_get':'''local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local e = world:create()
local position = world:get(e,P)
local x: number = position.x
print(x)
''',
'missing_remove':'''local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local e = world:create()
local position = world:remove(e,P)
local x: number = position.x
print(x)
''',
'bad_schema':'''local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local e = world:create()
world:add(e,P,{x="bad"})
''',
'bad_callback':'''local P = ecs.component("Position", {x="number"})
local world = ecs.world()
local system = ecs.system(world,{P},function(entity,position,dt)
 local n: number = position.x
 local bad: string = position.x
end)
''',
'override':'''local ecs = require("custom")
assert(ecs.world() == "custom")
print("override passed")
''',
'imported':'''local domain = require("components")
local world = ecs.world()
local e = world:create()
world:add(e,domain.Position,{x=4})
local position = world:get(e,domain.Position)
local n: number = position.x
assert(n == 4)
print("imported passed")
'''
}
results={}
for name,source in cases.items():
 root=base/name;(root/'src').mkdir(parents=True,exist_ok=True)
 (root/'tua.toml').write_text('source="src"\nout="build"\n')
 (root/'src/main.tua').write_text(source)
 if name=='override':(root/'src/custom.lua').write_text('return {world=function() return "custom" end}\n')
 if name=='imported':(root/'src/components.tua').write_text('local Position=ecs.component("Position",{x="number"})\nreturn {Position=Position}\n')
 entries=[]
 for args in [['check'],['build']]:
  p=subprocess.run([str(binary),*args],cwd=root,text=True,capture_output=True);entries.append(dict(command=args,code=p.returncode,stdout=p.stdout,stderr=p.stderr))
 if entries[-1]['code']==0:
  p=subprocess.run(['lua','main.lua'],cwd=root/'build',text=True,capture_output=True);entries.append(dict(command=['lua','main.lua'],code=p.returncode,stdout=p.stdout,stderr=p.stderr))
 results[name]=entries
(base/'results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results,indent=2))
