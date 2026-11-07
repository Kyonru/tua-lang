import json,pathlib,subprocess,threading,queue
root=pathlib.Path(__file__).resolve().parents[4]
folder=pathlib.Path(__file__).resolve().parent
responses=[]
for case in ['happy','dynamic','reserved']:
 project=folder/case;file=project/'src/main.tua';source=file.read_text();uri=file.as_uri()
 p=subprocess.Popen([str(root/'target/release/tua'),'lsp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 q=queue.Queue()
 def reader():
  while True:
   headers={}
   while True:
    line=p.stdout.readline()
    if not line:return
    if line in (b'\r\n',b'\n'):break
    k,v=line.decode().split(':',1);headers[k.lower()]=v.strip()
   q.put(json.loads(p.stdout.read(int(headers['content-length']))))
 threading.Thread(target=reader,daemon=True).start()
 def send(method,params,id=None):
  msg={'jsonrpc':'2.0','method':method,'params':params}
  if id is not None:msg['id']=id
  body=json.dumps(msg).encode();p.stdin.write(f'Content-Length: {len(body)}\r\n\r\n'.encode()+body);p.stdin.flush()
  if id is None:return
  while True:
   result=q.get(timeout=20)
   if result.get('id')==id:return result
 try:
  send('initialize',{'processId':None,'rootUri':project.as_uri(),'capabilities':{}},1)
  send('initialized',{})
  send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'tua','version':1,'text':source}})
  inspection=send('tua/inspect',{'textDocument':{'uri':uri}},2)
  needle='machine.current' if case!='reserved' else 'machine:current'
  offset=source.index(needle)+len('machine.')
  pos={'line':source[:offset].count('\n'),'character':len(source[:offset].rsplit('\n',1)[-1])}
  hover=send('textDocument/hover',{'textDocument':{'uri':uri},'position':pos},3)
  completion=send('textDocument/completion',{'textDocument':{'uri':uri},'position':pos},4)
  responses.append({'case':case,'position':pos,'hover':hover,'inspection':inspection,'completion':completion})
 finally:p.terminate();p.wait(timeout=5)
(folder/'lsp-results.json').write_text(json.dumps(responses,indent=2)+'\n')
print('Saved actual hover/completion/inspector responses for',len(responses),'fixtures')
