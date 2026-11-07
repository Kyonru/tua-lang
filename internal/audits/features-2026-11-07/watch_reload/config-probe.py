from pathlib import Path
import subprocess,time,os,shutil
r=Path(__file__).resolve().parent;p=r/'config-dev'
if p.exists():shutil.rmtree(p)
(p/'src').mkdir(parents=True)
(p/'src/main.tua').write_text('print(1)\n')
(p/'tua.toml').write_text('source="src"\nout="build"\n')
f=open(r/'config-dev.log','w');proc=subprocess.Popen([str(r.parents[3]/'target/release/tua'),'dev','--no-launch'],cwd=p,env={**os.environ,'TUA_INTERNAL_WATCH_FORCE_POLL':'1'},stdout=f,stderr=f)
try:
 time.sleep(2)
 (p/'tua.toml').write_text('source="src"\nout="new-build"\n')
 time.sleep(2)
 print('old signal:',(p/'build/.tuahotreload').read_text().strip())
 print('new main:',(p/'new-build/main.lua').read_text().strip())
 print('new signal exists:',(p/'new-build/.tuahotreload').exists())
 print('new runtime exists:',(p/'new-build/tuahotreload.lua').exists())
finally:
 proc.terminate();proc.wait(timeout=3);f.close()
