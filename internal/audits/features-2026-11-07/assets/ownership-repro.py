"""Usage: python3 ownership-repro.py /absolute/disposable/new/root /absolute/tua"""
from pathlib import Path
import sys,subprocess,json
p=Path(sys.argv[1]);binary=sys.argv[2]
p.mkdir(parents=True,exist_ok=False)
(p/'src').mkdir()
(p/'src/main.tua').write_text('return {}\n')
(p/'import.py').write_text('''import os,json,pathlib
out=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_OUTPUT_DIR'])
out.mkdir(parents=True,exist_ok=True)
(out/'data.txt').write_text('GENERATED CONTENT')
pathlib.Path(os.environ['TUA_ASSET_PIPELINE_MANIFEST']).write_text(json.dumps({'version':1,'outputs':['data.txt']}))
''')
def config(folder):
 (p/'tua.toml').write_text(f'''source="src"
out="build"
[[asset_pipelines]]
name="world"
inputs=["import.py"]
command=["python3","import.py"]
output="src/generated/{folder}"
''')
def check():
 r=subprocess.run([binary,'check'],cwd=p,capture_output=True,text=True)
 return dict(status=r.returncode,stdout=r.stdout,stderr=r.stderr)
config('a');first=check()
state=(p/'.tua/cache/asset-pipelines/world/materialized.json').read_text()
b=p/'src/generated/b/data.txt';b.parent.mkdir(parents=True);b.write_text('USER AUTHORED CONTENT');before=b.read_text()
config('b');second=check()
print(json.dumps(dict(root=str(p),first=first,stateBeforeMove=state,before=before,second=second,after=b.read_text()),indent=2))
