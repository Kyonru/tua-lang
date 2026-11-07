import json, pathlib, subprocess, hashlib, select
BASE=pathlib.Path(__file__).resolve().parent
ROOT=BASE.parents[3]
BIN=ROOT/'target/release/tua'
W=BASE/'fixture'; (W/'src').mkdir(parents=True,exist_ok=True)
(W/'tua.toml').write_text('source = "src"\n')
(W/'src/main.tua').write_text('local value: number = 1\nlocal next = \n')
OUT=BASE/'outside.tua'; OUT.write_text('local outside_secret = "AUDIT_CANARY"\n')
link=W/'src/linked.tua'
if not link.exists(): link.symlink_to(OUT)
log=[]
def hashes():
 return {str(p.relative_to(BASE)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [W/'tua.toml', W/'src/main.tua',OUT]}
before=hashes()
class Client:
 def __init__(self,mode):
  self.mode=mode; self.n=0; self.p=subprocess.Popen([str(BIN),mode,'--stdio','--workspace',str(W)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 def call(self,method,params=None,raw=None):
  self.n+=1; req=raw if raw is not None else {'jsonrpc':'2.0','id':self.n,'method':method,'params':params or {}}
  line=req if isinstance(req,str) else json.dumps(req)
  self.p.stdin.write(line+'\n'); self.p.stdin.flush()
  if not select.select([self.p.stdout],[],[],5)[0]: raise RuntimeError('timeout '+line)
  res=self.p.stdout.readline(); val=json.loads(res) if res else {'eof':True}
  log.append({'mode':self.mode,'request':req,'response':val}); return val
 def stop(self):
  self.p.stdin.close(); self.p.wait(timeout=5); log.append({'mode':self.mode,'exit':self.p.returncode,'stderr':self.p.stderr.read()})
a=Client('api')
a.call('tua.api/projectSummary')
a.call('',raw='{bad')
a.call('tua.api/initialize')
for m in ['completionContext','formatPreview','codeActions']: a.call('tua.api/'+m,{'path':'src/main.tua','position':{'line':1,'character':13}})
a.call('tua.api/initialize',{'capabilities':{'sourceContext':True}})
for m in ['projectSummary','diagnostics','semanticFacts','moduleGraph','assetMetadata','references','codeActions','formatPreview','completionContext']:
 a.call('tua.api/'+m,{'path':'src/main.tua','position':{'line':1,'character':13}})
for path in ['../outside.tua',str(OUT),'src/linked.tua']: a.call('tua.api/formatPreview',{'path':path})
a.call('tua.api/setOverlay',{'path':'src/main.tua','source':'local overlay: string = "present"\n'})
a.call('tua.api/projectSummary')
a.call('tua.api/refresh')
a.call('tua.api/formatPreview',{'path':'src/main.tua'})
a.call('tua.api/clearOverlay',{'path':'src/main.tua'})
a.call('tua.api/projectSummary')
a.call('tua.api/semanticFacts',{'path':'src/main.tua'})
# Controlled fixture disk mutation, restored before readonly comparison.
(W/'src/main.tua').write_text('local disk_changed: string = "new"\n')
a.call('tua.api/semanticFacts',{'path':'src/main.tua'})
a.call('tua.api/refresh')
a.call('tua.api/semanticFacts',{'path':'src/main.tua'})
(W/'src/main.tua').write_text('local value: number = 1\nlocal next = \n')
a.call('tua.api/noSuchMethod')
a.call('',raw={'jsonrpc':'1.0','id':90,'method':'tua.api/projectSummary'})
a.call('',raw={'jsonrpc':'2.0','id':{},'method':'tua.api/projectSummary'})
a.call('tua.api/shutdown'); a.stop()
b=Client('mcp'); b.call('tools/list'); b.call('initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'audit','version':'1'}})
r=b.call('tools/list')
for t in r['result']['tools']:
 args={'path':'src/main.tua','position':{'line':1,'character':13}}
 if t['name']=='tua_validate_completion': args.update(contextId='bad',analysisRevision=0,contextBytes=0,sourceFingerprint='bad',candidate='1')
 b.call('tools/call',{'name':t['name'],'arguments':args})
b.call('tools/call',{'name':'unknown','arguments':{}})
b.call('unknown'); b.call('shutdown'); b.stop()
# Notification lifecycle replies and malformed envelope independent of regular requests.
for mode,method in [('api','tua.api/initialize'),('mcp','initialize')]:
 c=Client(mode); c.call('',raw={'jsonrpc':'2.0','method':method}); c.stop()
c=Client('api'); c.call('tua.api/initialize',{'capabilities':{'sourceContext':True}})
ctx=c.call('tua.api/completionContext',{'path':'src/main.tua','position':{'line':1,'character':13}})['result']
params={k:ctx[k] for k in ['contextId','analysisRevision','contextBytes','sourceFingerprint','position']}; params.update(path='src/main.tua',candidate='1')
c.call('tua.api/validateCompletion',params)
c.call('tua.api/setOverlay',{'path':'src/new.tua','source':'local broken: number = "wrong"\n'})
c.call('tua.api/diagnostics',{'path':'src/new.tua'})
c.call('tua.api/diagnostics')
c.call('tua.api/projectSummary')
c.call('tua.api/moduleGraph')
c.call('tua.api/validateCompletion',params)
c.call('tua.api/refresh')
c.call('tua.api/diagnostics',{'path':'src/new.tua'})
c.call('tua.api/diagnostics')
c.call('tua.api/shutdown'); c.stop()
# Bounded framing cases and a cancellation notification followed by ping.
for mode,payload in [('api',b' '+b' '*8388608+b'\n'),('api',b'\xff\n'),('mcp',b'{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":99}}\r\n{"jsonrpc":"2.0","id":1,"method":"ping"}\r\n')]:
 r=subprocess.run([str(BIN),mode,'--stdio','--workspace',str(W)],input=payload,capture_output=True,timeout=5)
 log.append({'mode':mode,'frameBytes':len(payload),'framePrefix':repr(payload[:100]),'stdout':r.stdout.decode(errors='replace'),'stderr':r.stderr.decode(errors='replace'),'exit':r.returncode})
after=hashes(); log.append({'readonly_hashes_equal':before==after,'before':before,'after':after})
(BASE/'evidence.json').write_text(json.dumps(log,indent=2)+'\n')
print('captured',len(log),'events; readonly',before==after)
