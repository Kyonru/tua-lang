from pathlib import Path
import subprocess,json
root=Path(__file__).resolve().parent; binary=Path.cwd()/'target/debug/tua'
results=[]
for name,source,ending in [('minimal','local s = [[a  \nb]]\nprint(#s)\n','lf'),('minimal-crlf','local s = [[a  \r\nb]]\r\nprint(#s)\r\n','preserve'),('interior-control','local s = [[\na  \nb]]\nprint(#s)\n','lf')]:
 p=root/name;(p/'src').mkdir(parents=True,exist_ok=True);f=p/'src/main.tua'
 (p/'tua.toml').write_text('source="src"\nout="build"\n[format]\nline_ending="'+ending+'"\n')
 f.write_bytes(source.encode());(p/'before.tua').write_bytes(f.read_bytes());r={'name':name,'source':source,'commands':[]}
 for phase in ['before','after']:
  if phase=='after':
   q=subprocess.run([binary,'fmt'],cwd=p,capture_output=True,text=True);r['commands'].append({'args':[str(binary),'fmt'],'exit':q.returncode,'stdout':q.stdout,'stderr':q.stderr})
  for args in [[str(binary),'build'],['lua','build/main.lua']]:
   q=subprocess.run(args,cwd=p,capture_output=True,text=True);r['commands'].append({'phase':phase,'args':args,'exit':q.returncode,'stdout':q.stdout,'stderr':q.stderr})
  (p/(phase+'.lua')).write_bytes((p/'build/main.lua').read_bytes());(p/(phase+'.tua')).write_bytes(f.read_bytes())
 r['formatted']=f.read_bytes().decode();results.append(r)
(root/'minimal-evidence.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
