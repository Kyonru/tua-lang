from pathlib import Path
import subprocess,json,base64,shutil
ROOT=Path(__file__).resolve().parent
BIN=ROOT.parents[3]/'target/release/tua'
records=[]
def write(p,s):
 p.parent.mkdir(parents=True,exist_ok=True);p.write_text(s)
def run(p,label,cmd='check'):
 r=subprocess.run([str(BIN),cmd],cwd=p,text=True,capture_output=True)
 records.append(dict(case=label,command=[str(BIN),cmd],cwd=str(p),status=r.returncode,stdout=r.stdout,stderr=r.stderr))
def config(out='src/generated/a'):
 return f'''source="src"
out="build"
[[asset_pipelines]]
name="world"
inputs=["input.txt", "import.py"]
command=["python3", "import.py"]
output="{out}"
'''
p=ROOT/'fixtures/pipeline';write(p/'tua.toml',config());write(p/'input.txt','first');write(p/'src/main.tua','local x: number = 1\n')
write(p/'import.py','''import os,json,pathlib
p=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_PROJECT_ROOT'])
r=p/'runs.txt';r.write_text(str(int(r.read_text() if r.exists() else '0')+1))
out=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_OUTPUT_DIR']);out.mkdir(parents=True,exist_ok=True)
(out/'types.d.tua').write_text('export type Room = { name: string }\\n')
(out/'data.txt').write_text((p/'input.txt').read_text())
pathlib.Path(os.environ['TUA_ASSET_PIPELINE_MANIFEST']).write_text(json.dumps({'version':1,'outputs':['types.d.tua','data.txt'],'diagnostics':[]}))
''')
run(p,'pipeline-first');run(p,'pipeline-hit');records.append({'runsAfterHit':(p/'runs.txt').read_text()})
write(p/'input.txt','second');run(p,'pipeline-input-change','build');records.append({'runsAfterChange':(p/'runs.txt').read_text(),'declarationEmitted':(p/'build/generated/a/types.d.lua').exists()})
# Same-name output in new root is explicitly user-owned, never generated.
write(p/'src/generated/b/data.txt','USER AUTHORED CONTENT');write(p/'tua.toml',config('src/generated/b'))
run(p,'pipeline-output-root-change');records.append({'unownedFileAfterRootChange':(p/'src/generated/b/data.txt').read_text(),'oldRootStillExists':(p/'src/generated/a/data.txt').exists()})
# Corrupt regular cache output, preserving manifest. Last key matches b output.
cache=next(c for c in (p/'.tua/cache/asset-pipelines/world').iterdir() if c.is_dir() and (c/'files/data.txt').exists() and (c/'files/data.txt').read_text()=='second' and c.stat().st_mtime==max(x.stat().st_mtime for x in (p/'.tua/cache/asset-pipelines/world').iterdir() if x.is_dir()))
write(cache/'files/data.txt','CORRUPT');run(p,'pipeline-corrupt-cache');records.append({'materializedAfterCorruption':(p/'src/generated/b/data.txt').read_text(),'runsAfterCorruption':(p/'runs.txt').read_text(),'corruptCache':str(cache)})
a=ROOT/'fixtures/assets';write(a/'tua.toml','source="src"\nout="build"\n');write(a/'src/assets/pixel.png','')
(a/'src/assets/pixel.png').write_bytes(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6V9sAAAAASUVORK5CYII='))
write(a/'src/main.tua','local player = image "assets/pixel.png"\nlocal native = love.graphics.newImage("assets/pixel.png")\nlocal function image(path: string): string return path end\nlocal fake = image "assets/absent.png"\n')
run(a,'valid-shadowing','build')
write(a/'src/invalid.tua','local missing = image "assets/missing.png"\nlocal wrong = image "assets/sound.wav"\nlocal format = image "assets/file.xyz"\n');write(a/'src/assets/sound.wav','');write(a/'src/assets/file.xyz','');run(a,'missing-and-mismatch')
(ROOT/'cli-evidence.json').write_text(json.dumps(records,indent=2))
print(json.dumps(records,indent=2))
