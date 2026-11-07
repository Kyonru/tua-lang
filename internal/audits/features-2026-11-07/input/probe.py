import pathlib,subprocess,json
root=pathlib.Path(__file__).resolve().parent;repo=root.parents[3];binary=repo/'target/release/tua'
cases={
'positive':'''local c = input({controls={left={"key:a"},right={"key:d"},up={"key:w"},down={"key:s"},fire={"mouse:1"}},pairs={move={"left","right","up","down"}},pointer=true,deadzone=0.25})
local x,y=c:get("move")
local nx:number=x
local ny:number=y
local fire:number=c:get("fire")
local mx,my=c:pointer()
return c
''',
'negative_deadzone':'local c = input({controls={go={"key:a"}},deadzone=-0.25})\nreturn c\n',
'large_deadzone':'local c = input({controls={go={"key:a"}},deadzone=1.25})\nreturn c\n',
'empty_unknown':'local c = input({controls={}})\nprint(c:get("missing"))\n',
'known_unknown':'local c = input({controls={go={"key:a"}}})\nprint(c:get("missing"))\n',
'unknown_pair':'local c = input({controls={go={"key:a"}},pairs={move={"go","go","go","missing"}}})\n',
'invalid_source':'local c = input({controls={go={"key:not-a-key"}}})\n',
'bad_pointer':'local c = input({controls={},pointer=1})\n',
'hat10':'local c = input({controls={go={"hat:10u"}}})\n',
'negative_hex':'local c = input({controls={go={"key:a"}},deadzone=0x2})\nreturn c\n',
'dynamic':'local config={controls={go={"key:a"}}}\nlocal c=input(config)\nprint(c:get("go"))\n',
}
results={}
for name,source in cases.items():
 p=root/name;(p/'src').mkdir(parents=True,exist_ok=True);(p/'tua.toml').write_text('source="src"\nout="build"\n');(p/'src/main.tua').write_text(source)
 cmds=[]
 for action in ['check','build']:
  r=subprocess.run([str(binary),action],cwd=p,text=True,capture_output=True);cmds.append(dict(action=action,exit=r.returncode,stdout=r.stdout,stderr=r.stderr))
 results[name]=cmds
(root/'cli-results.json').write_text(json.dumps(results,indent=2)+'\n')
for name,v in results.items():print(name,v[0]['exit'],v[0]['stderr'].strip())
