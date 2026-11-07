import json,pathlib,subprocess,threading,queue,time
root=pathlib.Path(__file__).resolve().parent;repo=root.parents[3];workspace=root/'missing_get';binary=repo/'target/release/tua'
p=subprocess.Popen([str(binary),'lsp','--stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
q=queue.Queue()
def reader():
 while True:
  headers={}
  while True:
   line=p.stdout.readline()
   if not line:return
   if line==b'\r\n':break
   k,v=line.decode().split(':',1);headers[k.lower()]=v.strip()
  q.put(json.loads(p.stdout.read(int(headers['content-length']))))
threading.Thread(target=reader,daemon=True).start()
def send(method,params,id=None):
 msg={'jsonrpc':'2.0','method':method,'params':params}
 if id is not None:msg['id']=id
 data=json.dumps(msg).encode();p.stdin.write(f'Content-Length: {len(data)}\r\n\r\n'.encode()+data);p.stdin.flush()
def response(id):
 deadline=time.time()+15
 while time.time()<deadline:
  m=q.get(timeout=max(.1,deadline-time.time()))
  if m.get('id')==id:return m
 raise TimeoutError(id)
results={}
try:
 for wi,name in enumerate(['missing_get','bad_callback','imported']):
  workspace=root/name
  if wi==0:
   send('initialize',{'processId':None,'rootUri':root.as_uri(),'capabilities':{},'workspaceFolders':[{'uri':(root/n).as_uri(),'name':n} for n in ['missing_get','bad_callback','imported']]},1);response(1);send('initialized',{})
  doc=workspace/'src/main.tua';source=doc.read_text();uri=doc.as_uri()
  send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'tua','version':1,'text':source}})
  if name=='missing_get':probes=[('position hover','textDocument/hover',3,8),('get signature','textDocument/signatureHelp',3,28)]
  elif name=='bad_callback':probes=[('callback position hover','textDocument/hover',3,26),('callback completion','textDocument/completion',3,27)]
  else:probes=[('imported position hover','textDocument/hover',4,8)]
  for j,(label,method,line,char) in enumerate(probes):
   id=10+wi*10+j;send(method,{'textDocument':{'uri':uri},'position':{'line':line,'character':char}},id);results[name+' '+label]=response(id)
 send('tua/ecsGraph',{},50);results['ecs graph']=response(50)
 (root/'lsp-evidence.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results,indent=2))
finally:
 p.terminate();p.wait(timeout=5)
