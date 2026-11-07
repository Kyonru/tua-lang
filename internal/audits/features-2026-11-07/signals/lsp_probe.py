import json,pathlib,subprocess,threading,queue,time
root=pathlib.Path(__file__).resolve().parent;repo=root.parents[3];workspace=root/'editor';binary=repo/'target/release/tua'
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
try:
 send('initialize',{'processId':None,'rootUri':workspace.as_uri(),'capabilities':{},'workspaceFolders':[{'uri':workspace.as_uri(),'name':'signals'}]},1);response(1)
 send('initialized',{})
 doc=workspace/'src/main.tua';source=doc.read_text();uri=doc.as_uri()
 send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'tua','version':1,'text':source}})
 probes=[('player hover','textDocument/hover',2,9),('amount hover','textDocument/hover',2,20),('handler return hover','textDocument/hover',7,8),('listeners hover','textDocument/hover',6,9),('payload fields','textDocument/completion',2,15),('event names','textDocument/completion',5,16),('emit signature','textDocument/signatureHelp',5,21),('event definition','textDocument/definition',5,15)]
 results={}
 for id,(name,method,line,char) in enumerate(probes,2):
  send(method,{'textDocument':{'uri':uri},'position':{'line':line,'character':char}},id);results[name]=response(id)
 time.sleep(1)
 send('textDocument/definition',{'textDocument':{'uri':uri},'position':{'line':5,'character':15}},20);results['event definition after idle']=response(20)
 other=workspace/'src/events.tua'
 send('textDocument/didOpen',{'textDocument':{'uri':other.as_uri(),'languageId':'tua','version':1,'text':other.read_text()}})
 send('textDocument/definition',{'textDocument':{'uri':uri},'position':{'line':5,'character':15}},21);results['event definition after opening dependency']=response(21)
 send('textDocument/references',{'textDocument':{'uri':uri},'position':{'line':5,'character':15},'context':{'includeDeclaration':True}},22);results['event references']=response(22)
 (root/'lsp-evidence.json').write_text(json.dumps(results,indent=2)+'\n')
 print(json.dumps(results,indent=2))
finally:
 p.terminate();p.wait(timeout=5)
