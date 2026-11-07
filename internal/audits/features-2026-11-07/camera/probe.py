from pathlib import Path
import subprocess
root=Path(__file__).resolve().parent
repo=root.parents[3]
cases={
'happy':'''local cam = Camera.new({style="platformer", smoothing=8, zoom=2})
cam:follow(250, 40)
cam:update(0.1)
local sx, sy = cam:toScreen(23, 45)
local wx, wy = cam:toWorld(sx, sy)
print(cam.x, cam.y, wx, wy)
''',
'alias':'''local C = Camera
local cam = C.new()
print(cam.x)
''',
'chained':'''Camera.new():follow(1, 2)
''',
'invalid':'''local cam = Camera.new({style="orbit"})
Camera.new({bounds={x=0,y=0}})
cam:shake("hard")
''',
'zero':'''local cam = Camera.new({zoom=0})
print(cam:toWorld(0, 0))
''',
}
with (root/'cli.log').open('w') as log:
 for name, source in cases.items():
  p=root/name
  (p/'src').mkdir(parents=True,exist_ok=True)
  (p/'tua.toml').write_text('source="src"\nout="build"\n')
  (p/'src/main.tua').write_text(source)
  for args,cwd in [([str(repo/'target/release/tua'),'check'],p),([str(repo/'target/release/tua'),'build'],p)]:
   r=subprocess.run(args,cwd=cwd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
   log.write(f'{name} {args[-1]} exit={r.returncode}\n{r.stdout}\n')
  if (p/'build/main.lua').exists():
   r=subprocess.run(['/opt/homebrew/bin/lua','-e','package.path="./?.lua;"..package.path; dofile("main.lua")'],cwd=p/'build',text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
   log.write(f'{name} runtime exit={r.returncode}\n{r.stdout}\n')
