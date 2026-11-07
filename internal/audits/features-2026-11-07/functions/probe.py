from pathlib import Path
import subprocess,json,shutil
root=Path(__file__).resolve().parent
repo=root.parents[3]
cases={
'bad_default':'''local function f(x: number = "bad"): number return x end
print(f() + 1)
''',
'bad_unannotated_return':'''local function takes(n: number): number return n end
local function f() return takes("bad") end
print(f())
''',
'bad_extra_return':'''local function takes(n: number): number return n end
local function f(): number return 1, takes("bad") end
print(f())
''',
'default_omitted':'''local function f(x: number = 7): number return x end
print(f(), f(0))
''',
'optional_nil':'''local function f(x?: number): number
 if x == nil then return 7 end
 return x
end
print(f(), f(nil), f(0))
''',

'positive':'''enum Mode
 "fast"
 "slow"
end
local function factor(n: number): number
 if n <= 1 then return 1 end
 return n * factor(n - 1)
end
local function makeCounter(start: number = 2)
 local count = start
 return function(step: number = 1): number
  count += step
  return count
 end
end
local function pair(): (number, string)
 return 3, "ok"
end
local function forward(): (number, string)
 return pair()
end
local function spread(...: number)
 return ...
end
local function label(v: number | string): string
 if v is string then return v end
 return tostring(v)
end
local mode: Mode = "fast"
local c = makeCounter()
local a, b = forward()
local x, y = spread(4, 5)
local z = 2
z *= 3 + 4
print(factor(5), c(), c(2), a, b, x, y, z, label("yes"), mode)
''',
'bad_local':'''local function takes(n: number): number return n end
local n = takes("bad")
return n
''',
'bad_return':'''local function takes(n: number): number return n end
return takes("bad")
''',
'bad_nested_return':'''local function takes(n: number): number return n end
local function wrapper(): number
 return takes("bad")
end
print(wrapper())
''',
'bad_varargs':'''local function f(...: number) return ... end
local n = f(1, "bad")
''',
'default_nil':'''local function f(x: number = 7): number return x end
print(f(), f(nil), f(0))
''',
'default_scope':'''local x = 9
local function f(x: number = x): number return x end
print(f())
''',
'callback_short':'''local function call(cb: (a: number, b: string) -> nil)
 cb(1, "x")
end
call(function(a: number) print(a) end)
''',
'compound_complex':'''local a = { 1 }
a[1] += 2
''',
'enum_invalid':'''enum Mode
 "fast"
 "slow"
end
local mode: Mode = "no"
''',
'stdlib':'''local a = { 1, 2 }
table.insert(a, 3)
local s = string.format("%s:%d", "ok", #a)
local n = tonumber("12")
print(s, n, math.floor(2.9), select("#", 1, 2, 3))
'''
}
for name in ['sprite_return','sprite_local']:
 original='unknown_name' if name=='sprite_return' else 'unknown_local'
 src=root.parent/'sprites'/original
 dest=root/name
 if dest.exists(): shutil.rmtree(dest)
 shutil.copytree(src,dest,ignore=shutil.ignore_patterns('build'))
for name,source in cases.items():
 d=root/name
 (d/'src').mkdir(parents=True,exist_ok=True)
 (d/'src/main.tua').write_text(source)
 (d/'tua.toml').write_text('source = "src"\nout = "build"\ntarget = "love11-luajit"\nextension = "tua"\n')
results=[]
for name in list(cases)+['sprite_return','sprite_local']:
 d=root/name
 p=subprocess.run([str(repo/'target/release/tua'),'build'],cwd=d,text=True,capture_output=True)
 row={'case':name,'build_exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 (d/'build.log').write_text(p.stdout+p.stderr)
 if p.returncode==0 and (d/'build/main.lua').exists():
  q=subprocess.run(['/opt/homebrew/bin/lua','main.lua'],cwd=d/'build',text=True,capture_output=True)
  row.update(runtime_exit=q.returncode,runtime_stdout=q.stdout,runtime_stderr=q.stderr)
 results.append(row)
(root/'results.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
