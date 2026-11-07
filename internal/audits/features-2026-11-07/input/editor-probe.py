import pathlib,subprocess,json,threading,queue,time
base=pathlib.Path(__file__).resolve().parent;repo=base.parents[3];work=base/'positive';doc=work/'src/main.tua';source=doc.read_text();p=subprocess.Popen([str(repo/'target/release/tua'),'lsp','--stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL);q=queue.Queue()
def read():
 while True:
  h={}
  while True:
   line=p.stdout.readline()
   if not line:return
   if line==b'\r\n':break
   k,v=line.decode().split(':',1);h[k.lower()]=v.strip()
  q.put(json.loads(p.stdout.read(int(h['content-length']))))
threading.Thread(target=read,daemon=True).start()
def send(method,params,id=None):
 m=dict(jsonrpc='2.0',method=method,params=params)
 if id is not None:m['id']=id
 b=json.dumps(m).encode();p.stdin.write(f'Content-Length: {len(b)}\r\n\r\n'.encode()+b);p.stdin.flush()
def response(id):
 deadline=time.time()+15
 while time.time()<deadline:
  m=q.get(timeout=15)
  if m.get('id')==id:return m
 raise TimeoutError(id)
def pos(text,offset=0):
 index=source.index(text)+offset;prefix=source[:index];return dict(line=prefix.count('\n'),character=len(prefix.rsplit('\n',1)[-1]))
results={}
try:
 send('initialize',dict(processId=None,rootUri=work.as_uri(),capabilities={}),1);response(1);send('initialized',{})
 send('textDocument/didOpen',dict(textDocument=dict(uri=doc.as_uri(),languageId='tua',version=1,text=source)))
 probes=[('x-hover','textDocument/hover','local x,y',6),('y-hover','textDocument/hover','local x,y',8),('names','textDocument/completion','get("move")',7),('pair-signature','textDocument/signatureHelp','get("move")',9),('pointer-hover','textDocument/hover','c:pointer()',3),('source-completion','textDocument/completion','key:a',4)]
 for id,(name,method,text,offset) in enumerate(probes,2):
  params=dict(textDocument=dict(uri=doc.as_uri()),position=pos(text,offset));send(method,params,id);results[name]=dict(params=params,response=response(id))
 (base/'editor-results.json').write_text(json.dumps(results,indent=2)+'\n')
 for key,value in results.items():
  r=value['response'].get('result');print(key,json.dumps(r)[:700])
finally:p.terminate();p.wait(timeout=5)
