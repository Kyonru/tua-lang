from pathlib import Path
import subprocess,json
ROOT=Path(__file__).resolve().parent
BIN=ROOT.parents[3]/'target/release/tua'
results=[]
def fixture(name, files):
 p=ROOT/'fixtures'/name;p.mkdir(parents=True,exist_ok=True)
 for n,t in files.items():
  f=p/n;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(t)
 return p
def run(p,*args,exe=BIN):
 r=subprocess.run([str(exe),*args],cwd=p,text=True,capture_output=True)
 row=dict(cwd=str(p.relative_to(ROOT)),command=[str(exe),*args],exit=r.returncode,stdout=r.stdout,stderr=r.stderr)
 results.append(row);print(json.dumps(row));return r
def note(p):
 results.append(dict(cwd=str(p.relative_to(ROOT)),files={str(f.relative_to(p)):f.read_text(errors='replace') for f in sorted(p.rglob('*')) if f.is_file() and not f.is_symlink()}))
p=fixture('init', {'.gitignore':'custom/\n'});run(p,'init');run(p,'init');run(p,'check');run(p,'build');note(p)
p=fixture('symlink-init',{'outside/keep':'preserve'});(p/'src').symlink_to('outside',target_is_directory=True);run(p,'init');note(p)
p=fixture('mixed',{'tua.toml':'source="code"\nout="output"\nmain="boot/start.tua"\n','code/boot/start.tua':'local m = require("helper")\nassert(m.value == 42)\nprint("mixed-ok")\n','code/helper.lua':'return {value=42}\n','code/data.txt':'asset exact bytes\n','code/broken_spec.tua':'local =\n','code/.git/bad.tua':'local =\n'});run(p,'check');run(p,'build');run(p/'output','main.lua',exe='/opt/homebrew/bin/lua');note(p)
for name,config in [('unknown','soruce="src"'),('enum','[emit]\nannotation_mode="wat"'),('missing-main','main="missing.tua"'),('missing-source','source="missing"'),('unsupported-target','target="potato"'),('invalid-extension','extension=".tua"')]:
 p=fixture(name,{'tua.toml':config+'\n','src/main.tua':'print("hello")\n'});run(p,'check');run(p,'build');note(p)
p=fixture('cleanup',{'tua.toml':'','src/main.tua':'print("ok")\n','src/stale.tua':'return 1\n','src/modified.tua':'return 2\n'});run(p,'build');(p/'build/modified.lua').write_text('-- user modified\n');(p/'build/untracked.lua').write_text('-- untracked\n');(p/'src/stale.tua').unlink();(p/'src/modified.tua').unlink();run(p,'build');note(p)
p=fixture('emit-switch',{'tua.toml':'','src/main.tua':'local x: number = 1\nprint(x)\n'});run(p,'build');(p/'tua.toml').write_text('emit_maps=false\n[emit]\nannotation_mode="luacats"\n');run(p,'build');note(p)
p=fixture('error',{'tua.toml':'','src/main.tua':'local x: number = "bad"\nprint(x)\n','src/syntax.tua':'local =\n','src/good.tua':'print("good")\n'});run(p,'build');note(p);run(p,'build','--emit-on-error');note(p)
p=fixture('nested-output',{'tua.toml':'source="."\nout="build"\n','main.tua':'print("ok")\n'});run(p,'build');run(p,'build');note(p)
p=fixture('in-place',{'tua.toml':'source="src"\nout="src"\n','src/main.tua':'print("ok")\n','src/data.txt':'safe\n'});run(p,'build');run(p,'build');note(p)
p=fixture('precedence',{'tua.toml':'','src/main.tua':'local m = require("mod")\nlocal n: number = m.value\nprint(n)\n','src/mod.tua':'return {value=42}\n','src/mod.lua':'return {value="wrong"}\n'});run(p,'check');run(p,'build');run(p/'build','main.lua',exe='/opt/homebrew/bin/lua');note(p)
(ROOT/'raw.json').write_text(json.dumps(results,indent=2))
