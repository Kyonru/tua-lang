import pathlib,subprocess,json,shutil
root=pathlib.Path(__file__).resolve().parent;cli=root.parents[3]/'target/release/tua'
p=root/'edges-fixture';shutil.rmtree(p,ignore_errors=True);shutil.copytree(root/'fixture',p,dirs_exist_ok=True)
(p/'tests/invalid_spec.tua').unlink();(p/'tests/failing_spec.tua').unlink()
def run(name,args):
 r=subprocess.run(args,cwd=p,text=True,capture_output=True,timeout=30);(root/(name+'.json')).write_text(json.dumps({'argv':list(map(str,args)),'cwd':str(p),'returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr},indent=2));print(name,r.returncode,r.stdout[-120:],r.stderr[-700:])
run('coverage-only',[str(cli),'test','--coverage',str(root/'coverage-only-results.json')])
run('missing-lua-no-report',[str(cli),'test','--lua','/no/such/lua'])
run('luacov-check',['/opt/homebrew/bin/lua','-e',"require('luacov.runner')"])
run('all-no-coverage',[str(cli),'test','--report',str(root/'all-no-coverage-results.json')])
run('build-excludes-tests',[str(cli),'build'])
(p/'tests/syntax_spec.tua').write_text('local broken =\n')
run('parse-discovery',[str(cli),'test','--list'])
