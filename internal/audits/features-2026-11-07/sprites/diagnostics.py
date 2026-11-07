from pathlib import Path
import subprocess,json,shutil
base=Path(__file__).resolve().parent
binary=base.parents[3]/'target/release/tua'
cases={'conflict':'from = 0, to = 1, fps = 4, durations = 0.1','unknown_local':'from = 0, to = 1, fps = 4','unknown_name':'from = 0, to = 1, fps = 4','marker_bounds':'from = 0, to = 1, fps = 4, markers = { { frame = 2, name = "x" } }','zero_list':'from = 0, to = 1, durations = { 0, 0 }','missing_timing':'from = 0, to = 1'}
results=[]
for name,fields in cases.items():
 p=base/name; (p/'src/assets').mkdir(parents=True,exist_ok=True)
 shutil.copy(base/'project/src/assets/player.png',p/'src/assets/player.png')
 shutil.copy(base/'project/tua.toml',p/'tua.toml')
 source='local S = spriteSheet({ image = "assets/player.png", frame = { width = 32, height = 32 }, animations = { idle = { '+fields+' } } })\n'
 source+= 'return S:player("absent")\n' if name=='unknown_name' else ('local p = S:player("absent")\nreturn p\n' if name=='unknown_local' else 'return S\n')
 (p/'src/sheets.tua').write_text(source)
 r=subprocess.run([str(binary),'build'],cwd=p,text=True,capture_output=True)
 results.append(dict(name=name,status=r.returncode,stdout=r.stdout,stderr=r.stderr))
(base/'diagnostics.json').write_text(json.dumps(results,indent=2))
