from pathlib import Path
import subprocess,json
R=Path(__file__).resolve().parent;B=R.parents[3]/'target/release/tua';rows=[]
for name,files in [
 ('no-config',{'src/main.tua':'print("defaults-ok")\n'}),
 ('custom-extension',{'tua.toml':'extension="tl"\nmain="entry.tl"\n','src/entry.tl':'local x: number = 7\nprint(x)\n'}),
 ('main-collision',{'tua.toml':'main="boot.tua"\n','src/main.tua':'print("main")\n','src/boot.tua':'print("boot")\n'}),
 ('explicit-specs',{'tua.toml':'[test]\nexclude_specs=false\n','src/main.tua':'print("main")\n','src/bad_spec.tua':'local =\n'}),
 ('dedicated-test-root',{'tua.toml':'[test]\nsource="tests"\n','src/main.tua':'print("main")\n','tests/bad_spec.tua':'local =\n','tests/asset.txt':'test asset\n'})]:
 p=R/'fixtures'/name;p.mkdir(parents=True,exist_ok=True)
 for n,t in files.items():
  f=p/n;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(t)
 for args in [['check'],['build']]:
  r=subprocess.run([str(B),*args],cwd=p,text=True,capture_output=True)
  rows.append(dict(cwd=str(p),command=[str(B),*args],exit=r.returncode,stdout=r.stdout,stderr=r.stderr))
 rows.append(dict(name=name,files=[str(f.relative_to(p)) for f in p.rglob('*') if f.is_file()]))
(R/'extra.json').write_text(json.dumps(rows,indent=2))
