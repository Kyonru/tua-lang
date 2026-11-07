from pathlib import Path
import subprocess,json
root=Path(__file__).resolve().parent
results=[]
for mode in ['off','on']:
 for name in ['positive','chained_constructor','underflow_div','normalization','comment_math','edge_math']:
  game=root/('love-'+mode+'-'+name);game.mkdir(exist_ok=True)
  (game/'conf.lua').write_text('function love.conf(t) t.window=false; t.modules.graphics=false; t.modules.audio=false end')
  build=root/name/'build'
  (game/'main.lua').write_text('jit.'+mode+'(); print("jit",jit.status()); package.path='+json.dumps(str(build/'?.lua')+';')+'..package.path; local ok,err=pcall(dofile,'+json.dumps(str(build/'main.lua'))+'); print("result",ok,err); love.event.quit(ok and 0 or 1)')
  try:
   run=subprocess.run(['/Applications/LOVE.app/Contents/MacOS/love',str(game)],text=True,capture_output=True,timeout=20)
   results.append(dict(mode=mode,name=name,code=run.returncode,output=run.stdout+run.stderr))
  except subprocess.TimeoutExpired as e:results.append(dict(mode=mode,name=name,error='timeout'))
(root/'love-results.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
