from pathlib import Path
import subprocess,time,json,sys
root=Path(__file__).resolve().parent; p=root/'dev-restart-project';(p/'src').mkdir(parents=True,exist_ok=True)
binary=Path.cwd()/'target/debug/tua'
(p/'src/main.tua').write_text('print("ready")\n')
(p/'tua.toml').write_text('source="src"\nout="build"\n')
fake=p/'fake-love';fake.write_text(f'#!{sys.executable}\nfrom pathlib import Path\nimport sys,time\np=Path(sys.argv[1]); (p/"launched").write_text(str(__import__("os").getpid()))\nwhile not (p/"stop").exists(): time.sleep(.05)\n');fake.chmod(0o755)
env=dict(__import__('os').environ,TUA_INTERNAL_WATCH_POLL_MS='20',TUA_INTERNAL_WATCH_DEBOUNCE_MS='40',TUA_INTERNAL_WATCH_RECONCILE_MS='100',TUA_INTERNAL_WATCH_FORCE_POLL='1')
child=subprocess.Popen([binary,'dev','--love',fake],cwd=p,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
def wait(path):
 end=time.monotonic()+10
 while not path.exists() and time.monotonic()<end and child.poll() is None:time.sleep(.025)
 assert path.exists(),str(path)
try:
 wait(p/'build/launched');old=(p/'build/launched').read_text()
 (p/'tua.toml').write_text('source="src"\nout="moved"\n')
 wait(p/'moved/launched');new=(p/'moved/launched').read_text()
 (p/'moved/stop').write_text('stop');out,err=child.communicate(timeout=10)
 result={'status':child.returncode,'stdout':out,'stderr':err,'oldPid':old,'newPid':new,'hook': 'require("tuahotreload").install()' in (p/'moved/main.lua').read_text(),'signal':(p/'moved/.tuahotreload').read_text()};assert result['status']==0 and result['oldPid']!=result['newPid'] and result['hook'];(root/'dev-restart.json').write_text(json.dumps(result,indent=2));print(result)
finally:
 if child.poll() is None:child.terminate();child.wait(timeout=5)
