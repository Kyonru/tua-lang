import pathlib,json,subprocess
base=pathlib.Path(__file__).resolve().parent
repo=base.parents[3]
binary=repo/'target/release/tua'
cases={
'positive': '''type Row = { name: string, score: number, note?: string }
local rows: Row[] = {}
table.insert(rows, {name="Mina",score=3})
local scores: {[string]: number} = {}
scores.Mina = 3
local n = scores.Mina
if n then print(n + 1) end
for _, row in ipairs(rows) do print(row.name, row.score) end
local function label(prefix: string = "ok", suffix?: string): string
 return prefix .. (suffix or "!")
end
print(label())
local state: number | nil = nil
state = 5
if state then print(state+1) end
local a: any = "dynamic"
local b: string = a
print(b)
do local state = "shadow" print(state) end
''',
'optional_dot': 'type Row = {value?: number}\nlocal row: Row = {}\nrow.value = nil\n',
'optional_bracket': 'type Row = {value?: number}\nlocal row: Row = {}\nrow["value"] = nil\n',
'required_nil': 'local row: {value: number} = {value=1}\nrow.value = nil\n',
'nullable_dot': 'local row: {value: number | nil} = {value=1}\nrow.value = nil\n',
'callback_short': 'local function use(cb: (x: number, y: string) -> nil) cb(1,"a") end\nuse(function(x) print(x+1) end)\n',
'callback_full': 'local function use(cb: (x: number, y: string) -> nil) cb(1,"a") end\nuse(function(x,y) print(x+1,y) end)\n',
'callback_typed_short': 'local function use(cb: (x: number, y: string) -> nil) cb(1,"a") end\nuse(function(x: number) print(x+1) end)\n',
'callback_extra': 'local function use(cb: (x: number) -> nil) cb(1) end\nuse(function(x: number,y: string) print(x,y) end)\n',
'callback_wrong': 'local function use(cb: (x: number) -> nil) cb(1) end\nuse(function(x: string) print(x) end)\n',
'stable_assignment': 'local score = 1\nscore="wrong"\n',
'dictionary_read': 'local scores: {[string]: number} = {}\nlocal n: number = scores.missing\n',
'dictionary_write': 'local scores: {[string]: number} = {}\nscores.missing="wrong"\n',
'dictionary_delete': 'local scores: {[string]: number} = {}\nscores.missing=nil\n',
'array_write': 'local values: number[] = {}\nvalues[1]="wrong"\n',
'array_insert': 'local values: number[] = {}\ntable.insert(values,"wrong")\n',
'array_remove': 'local values: number[] = {}\nlocal n: number = table.remove(values)\n',
'generics': 'type Box<T> = {value: T}\n',
'native_tuple': 'local value: [number,string] = {1,"a"}\n',
'union_invalid': 'local value: number | string = true\n',
'guard_invalidation': 'local row: {value?: {n: number}} = {}\nif row.value then\n row.value = nil\n print(row.value.n)\nend\n',
'guard_positive': 'type Row = {value?: {n: number}}\nlocal function use(row: Row): number\n if row.value == nil then\n  return 0\n end\n return row.value.n\nend\nprint(use({value={n=3}}))\n',
'shadow': 'local value: number = 1\ndo local value: string = "yes" value="ok" print(value) end\nvalue=2\nprint(value)\n',
'lua_tuple': '---@alias Pair [number, string]\n---@type Pair\nlocal pair = {1, "a"}\nlocal n: number = pair[1]\nlocal s: string = pair[2]\nprint(n,s)\n',
'lua_tuple_wrong': '---@alias Pair [number, string]\n---@type Pair\nlocal pair = {1, "a"}\nlocal n: number = pair[2]\n',
}
cases.update({
'dictionary_runtime': 'local scores: {[string]: number} = {}\nscores.missing="wrong"\nlocal n = scores.missing\nif n then print(n + 1) end\n',
'dictionary_bracket': 'local scores: {[string]: number} = {}\nscores["missing"]="wrong"\n',
'dictionary_dynamic': 'local scores: {[string]: number} = {}\nlocal key="missing"\nscores[key]="wrong"\n',
'callback_return': 'local function use(cb: (x: number) -> string): string return cb(1) end\nlocal result = use(function(x) return x+1 end)\nprint(result)\n',
'optional_init_nil': 'type Row = {value?: number}\nlocal row: Row = {value=nil}\n',
'multi_assignment': 'local n: number = 1\nlocal s: string = "s"\nn,s = 2\nprint(s .. "!")\n',
})
results=[]
for name,src in cases.items():
 p=base/'cases'/name;(p/'src').mkdir(parents=True,exist_ok=True)
 (p/'tua.toml').write_text('source="src"\nout="build"\n[diagnostics]\nrules=false\n')
 (p/'src/main.tua').write_text(src)
 r=subprocess.run([str(binary),'check'],cwd=p,capture_output=True,text=True)
 item={'name':name,'check':{'code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}}
 if name in ['positive','guard_positive','shadow','lua_tuple','dictionary_runtime','multi_assignment'] and r.returncode==0:
  b=subprocess.run([str(binary),'build'],cwd=p,capture_output=True,text=True)
  item['build']={'code':b.returncode,'stdout':b.stdout,'stderr':b.stderr}
  if b.returncode==0:
   l=subprocess.run(['/opt/homebrew/bin/lua','main.lua'],cwd=p/'build',capture_output=True,text=True)
   item['runtime']={'code':l.returncode,'stdout':l.stdout,'stderr':l.stderr}
 results.append(item)
(base/'results.json').write_text(json.dumps(results,indent=2))
for r in results:print(r['name'],r['check']['code'],r['check']['stdout'].strip(),r['check']['stderr'].strip())
