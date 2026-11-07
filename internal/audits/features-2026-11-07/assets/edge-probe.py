from pathlib import Path
import subprocess,json,shutil,zlib,struct
r=Path(__file__).resolve().parent;binary=r.parents[3]/'target/release/tua';results=[]
def run(p,label,cmd='check'):
 x=subprocess.run([str(binary),cmd],cwd=p,capture_output=True,text=True);results.append(dict(label=label,status=x.returncode,stdout=x.stdout,stderr=x.stderr))
for label,body in [('failed','import sys\nsys.stderr.write("import failed deliberately")\nsys.exit(7)\n'),('duplicate','(out/"x.txt").write_text("ok")\nmanifest({"version":1,"outputs":["x.txt","x.txt"]})'),('missing','manifest({"version":1,"outputs":["missing.txt"]})')]:
 p=r/'fixtures'/label;(p/'src').mkdir(parents=True,exist_ok=True);(p/'src/main.tua').write_text('return {}\n')
 (p/'tua.toml').write_text('source="src"\nout="build"\n[[asset_pipelines]]\nname="edge"\ninputs=["import.py"]\ncommand=["python3","import.py"]\noutput="src/generated"\n')
 (p/'import.py').write_text('import pathlib,os,json\nout=pathlib.Path(os.environ["TUA_ASSET_PIPELINE_OUTPUT_DIR"])\nout.mkdir(parents=True,exist_ok=True)\ndef manifest(v): pathlib.Path(os.environ["TUA_ASSET_PIPELINE_MANIFEST"]).write_text(json.dumps(v))\n'+body)
 run(p,label)
# Exercise actual LOVE decoding and emitted helper/native calls, with graphics disabled.
p=r/'fixtures/assets';png=p/'src/assets/pixel.png'
def chunk(k,b):return struct.pack('>I',len(b))+k+b+struct.pack('>I',zlib.crc32(k+b)&0xffffffff)
png.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(bytes([0,255,0,0,255])))+chunk(b'IEND',b''))
# Keep edge-invalid document out of isolated runtime source.
q=r/'fixtures/runtime';(q/'src/assets').mkdir(parents=True,exist_ok=True);shutil.copy(png,q/'src/assets/pixel.png');shutil.copy(p/'src/main.tua',q/'src/compiled.tua');(q/'tua.toml').write_text('source="src"\nout="build"\n')
(q/'src/conf.lua').write_text('function love.conf(t) t.window=false; t.modules.graphics=false; t.modules.audio=false end\n')
(q/'src/main.lua').write_text('function love.load() love.graphics={newImage=function(p) local d=love.image.newImageData(p); assert(d:getWidth()==1); print("decoded "..p); return d end}; dofile("compiled.lua"); love.event.quit(0) end\n')
run(q,'runtime-build','build')
x=subprocess.run(['/Applications/LOVE.app/Contents/MacOS/love',str(q/'build')],capture_output=True,text=True,timeout=15);results.append(dict(label='actual-LOVE-decode-emitted-loader',status=x.returncode,stdout=x.stdout,stderr=x.stderr,limitation='headless graphics.newImage adapter to real love.image.newImageData; no GPU texture creation'))
(r/'edge-evidence.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
