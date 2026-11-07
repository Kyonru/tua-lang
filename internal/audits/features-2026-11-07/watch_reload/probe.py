from pathlib import Path
import subprocess, time, os, json, shutil
ROOT=Path(__file__).resolve().parent
BIN=ROOT.parents[3]/'target/release/tua'
results=[]
def setup(name):
 p=ROOT/name
 if p.exists(): shutil.rmtree(p)
 (p/'src').mkdir(parents=True)
 (p/'tua.toml').write_text('source = "src"\nout = "src/build"\n')
 (p/'src/main.tua').write_text('print(1)\n')
 return p
def run(name,mode,poll=False):
 p=setup(name); log=open(ROOT/(name+'.log'),'w'); env=os.environ.copy()
 if poll: env['TUA_INTERNAL_WATCH_FORCE_POLL']='1'
 proc=subprocess.Popen([str(BIN),*mode],cwd=p,stdout=log,stderr=log,env=env)
 def wait():
  time.sleep(1.2)
  if proc.poll() is not None: raise RuntimeError('process exited')
 def check(label,ok): results.append({'probe':name,'check':label,'passed':bool(ok)})
 try:
  wait(); out=p/'src/build'; old=(out/'main.lua').read_text(); check('initial output', 'print(1)' in old)
  (p/'src/main.tua').write_text('print(2)\n'); wait(); good=(out/'main.lua').read_text(); check('saved edit', 'print(2)' in good)
  (p/'src/main.tua').write_text('local x: number = "bad"\n'); wait(); check('error preserves output', (out/'main.lua').read_text()==good)
  (p/'src/main.tua').write_text('print(3)\n'); wait(); check('error recovery','print(3)' in (out/'main.lua').read_text())
  (p/'src/temp.tua').write_text('print(4)\n'); wait(); check('create',(out/'temp.lua').exists())
  (p/'src/temp.tua').unlink(); wait(); check('delete',not (out/'temp.lua').exists())
  (out/'ignored.tua').write_text('broken ?\n'); wait(); check('output excluded',not (out/'build').exists())
  (p/'tua.toml').write_text('invalid = [\n'); wait(); (p/'src/main.tua').write_text('print(5)\n'); wait(); check('invalid config retains old layout','print(5)' in (out/'main.lua').read_text())
  (out/'ignored.tua').unlink(); (p/'tua.toml').write_text('source = "src"\nout = "new-build"\n'); wait(); new=p/'new-build'; check('new output after config',(new/'main.lua').exists())
  if mode[0]=='dev':
   check('new output instrumented', 'tuahotreload' in (new/'main.lua').read_text()); check('new output signalled',(new/'.tuahotreload').exists())
   results.append({'probe':name,'old_signal':(out/'.tuahotreload').read_text(),'new_signal_exists':(new/'.tuahotreload').exists()})
 finally:
  proc.terminate()
  try: proc.wait(timeout=3)
  except subprocess.TimeoutExpired: proc.kill(); proc.wait()
  log.close()
for name,mode,poll in [('native',['watch'],False),('poll',['watch'],True),('dev',['dev','--no-launch'],True)]: run(name,mode,poll)
(ROOT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results,indent=2))
