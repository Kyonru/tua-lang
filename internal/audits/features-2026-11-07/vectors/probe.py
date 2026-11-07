from pathlib import Path
import subprocess,json
root=Path(__file__).resolve().parent
repo=root.parents[3]
bin=repo/'target/release/tua'
cases={
'positive':'''local p = vec2(3,4)
local v = vec2(2,-1)
local dt = 0.5
local moved = p + v * dt
local scaled = 2 * p
local divided = p / 2
local neg = -p
local product = p * v
local x,y = p:split()
assert(moved.x == 4 and moved.y == 3.5)
assert(scaled.x == 6 and divided.y == 2 and neg.x == -3)
assert(product.x == 6 and product.y == -4)
assert(x == 3 and y == 4 and p:dot(v) == 2)
assert(p.length == 5 and p.length2 == 25)
local zero = vec2(0,0)
assert(zero.normalized == zero)
assert(p:trim(2).length == 2)
assert(p:hadamard(v) == product)
assert(p:clamp(vec2(0,0),vec2(2,3)) == vec2(2,3))
assert(p.floor == p and p.ceil == p and p.copy == p)
p.length = 10
assert(p.length == 10)
p.angle = 0
assert(p.x == 10 and p.y == 0)
assert(p:angled(0) == p and p:rotated(0) == p)
assert(math.round(-1.5) == -2 and math.round(150,-2) == 200)
assert(math.round(10.946,2) == 10.95)
assert(math.clamp(20,0,10) == 10 and math.lerp(0,10,1.5) == 15)
assert(math.inverseLerp(0,10,5) == 0.5 and math.remap(5,0,10,0,100) == 50)
assert(math.sign(-10) == -1 and math.sign(0) == 0)
assert(math.smoothstep(0,1,0.5) == 0.5 and math.tau == 2*math.pi)
assert(math.rgb(255,0,128)[1] == 1 and love.math.rgba(255,0,0,128)[4] == 128/255)
print("positive passed",type(p))
''',
'underflow_div':'''local p = vec2(1e-200,1e-200)
local q = vec2(1e-200,1e-200)
local result = p / q
print(result.x,result.y)
''',
'chained_constructor':'''local n = vec2(3,4).normalized
print(n.x,n.y)
''',
'normalization':'''local b = vec2(1e200,0)
local t = vec2(1e-200,0)
local big = b.normalized
local tiny = t.normalized
print("big",big.x,big.y,big.length)
print("tiny",tiny.x,tiny.y,tiny.length)
''',
'comment_math':'''print(math. -- helper comment
  clamp(20,0,10))
''',
'bracket_math':'''print(math["clamp"](20,0,10))
''',
'zero_div':'''local p=vec2(1,2)
local result=p/0
print(result)
''',
'known_nominal':'''local p: Vec2 = {x=1,y=2}
print(p)
''',
'edge_math':'''print("equal endpoints",math.inverseLerp(1,1,1),math.smoothstep(1,1,1))
print("reverse clamp",math.clamp(5,10,0))
print("round extreme",math.round(1,400))
'''}
results=[]
for name,src in cases.items():
 p=root/name;(p/'src').mkdir(parents=True,exist_ok=True)
 (p/'tua.toml').write_text('source="src"\nout="build"\n[diagnostics]\nrules=false\n')
 (p/'src/main.tua').write_text(src)
 build=subprocess.run([str(bin),'build'],cwd=p,text=True,capture_output=True)
 out={'name':name,'build_code':build.returncode,'build':build.stdout+build.stderr}
 if build.returncode==0:
  run=subprocess.run(['/opt/homebrew/bin/lua','-e','love={math={}}; dofile("main.lua")'],cwd=p/'build',text=True,capture_output=True)
  out.update(lua_code=run.returncode,lua=run.stdout+run.stderr)
 results.append(out)
(root/'cli-results.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
