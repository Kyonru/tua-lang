from pathlib import Path
import subprocess,json,zipfile,hashlib,plistlib,os
R=Path.cwd(); A=R/'internal/audits/features-2026-11-07/packaging'; B=R/'target/release/tua'; results=[]
def run(args,cwd=R):
 try:
  p=subprocess.run(list(map(str,args)),cwd=cwd,capture_output=True,text=True,timeout=70)
  d=dict(command=list(map(str,args)),cwd=str(cwd),returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
 except subprocess.TimeoutExpired as e:d=dict(command=list(map(str,args)),cwd=str(cwd),timeout=True,stdout=str(e.stdout),stderr=str(e.stderr))
 results.append(d); (A/'commands.json').write_text(json.dumps(results,indent=2)); return d
P=A/'normal'; (P/'src').mkdir(parents=True,exist_ok=True)
(P/'tua.toml').write_text('source = "src"\nout = "custom output"\ntarget = "love11-luajit"\nemit_maps = true\n[test]\nsource = "tests"\n')
(P/'src/main.tua').write_text('print("PACKAGING_RUNTIME_OK")\nos.exit(0)\n')
(P/'src/conf.lua').write_text('function love.conf(t) t.window = false; t.modules.audio = false end\n')
(P/'src/main.spec.tua').write_text('print("SHOULD_NOT_SHIP")\n')
(P/'tests').mkdir(exist_ok=True); (P/'tests/test.tua').write_text('print("TEST_ONLY")\n')
(P/'src/.hidden').write_text('hidden')
(P/'src/level.txt').write_text('asset')
run([B,'package','--name','Star Dust'],P)
Z=P/'dist/Star Dust.love'; before=Z.read_bytes()
run([B,'package','--name','Star Dust'],P)
results.append(dict(normal_archive=zipfile.ZipFile(Z).namelist(),reproducible=before==Z.read_bytes()))
run(['/Applications/LOVE.app/Contents/MacOS/love',Z])
run([B,'package','--target','windows'],P)
run([B,'package','--target','macos','--love',A/'absent'],P)
run([B,'package','--name','../../escape'],P)
run([B,'package','--target','macos','--love','/Applications/LOVE.app','--name','Star Dust','--identifier','org.example.packagingaudit'],P)
APP=P/'dist/macos/Star Dust.app'
if APP.exists():
 info=plistlib.loads((APP/'Contents/Info.plist').read_bytes()); results.append(dict(mac_plist=info)); run(['codesign','--verify','--deep','--strict',APP]); run([APP/'Contents/MacOS/love'])
I=A/'in-place'; I.mkdir(exist_ok=True)
(I/'tua.toml').write_text('source = "."\nout = "."\ntarget = "love11-luajit"\n')
(I/'main.tua').write_text('print("INPLACE")\n'); (I/'main.lua.map').write_text('exclude'); (I/'main.lua.map.json').write_text('exclude'); (I/'.hidden').write_text('exclude'); (I/'secret.tua').write_text('-- source\n'); (I/'conf.lua').write_text('-- config\n')
(I/'linked.txt').symlink_to('conf.lua') if not (I/'linked.txt').is_symlink() else None
run([B,'package'],I)
results.append(dict(inplace_archive=zipfile.ZipFile(I/'dist/in-place.love').namelist()))
run(['python3','scripts/package_release.py','archive','--binary',B,'--version','0.6.0','--target','aarch64-apple-darwin','--output',A/'release'])
run(['python3','scripts/smoke_release.py','--archive',A/'release/tua-0.6.0-aarch64-apple-darwin.tar.gz','--vsix',R/'editors/vscode/tua-0.6.0.vsix','--version','0.6.0','--target','aarch64-apple-darwin'])
run(['python3','scripts/package_release_test.py'])
run(['node','scripts/sync-version.js','--check'])
(A/'commands.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
