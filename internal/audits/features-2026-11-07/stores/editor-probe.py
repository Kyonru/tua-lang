import json,subprocess,threading,queue,time
from pathlib import Path
D=Path(__file__).resolve().parent;ROOT=D.parents[3];P=D/'project';uri=(P/'src/main.tua').as_uri()
p=subprocess.Popen([str(ROOT/'target/release/tua'),'lsp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=P)
q=queue.Queue()
def read():
 while True:
  header={}
  while True:
   line=p.stdout.readline()
   if not line:return
   if line==b'\r\n':break
   k,v=line.decode().split(':',1);header[k.lower()]=v.strip()
  q.put(json.loads(p.stdout.read(int(header['content-length']))))
threading.Thread(target=read,daemon=True).start();results=[];counter=0
def send(method,params,request=True):
 global counter
 counter+=1;v=dict(jsonrpc='2.0',method=method,params=params)
 if request:v['id']=counter
 b=json.dumps(v).encode();p.stdin.write(f'Content-Length: {len(b)}\r\n\r\n'.encode()+b);p.stdin.flush()
 if not request:return
 while True:
  r=q.get(timeout=20)
  if r.get('id')==counter:results.append(dict(request=v,response=r));return r
try:
 send('initialize',{'processId':None,'rootUri':P.as_uri(),'capabilities':{},'workspaceFolders':[{'uri':P.as_uri(),'name':'stores'}]})
 send('initialized',{},False)
 send('textDocument/didOpen',{'textDocument':{'uri':(P/'src/game.tua').as_uri(),'languageId':'tua','version':1,'text':(P/'src/game.tua').read_text()}},False)
 source='local game = store({ score = 0, paused = false })\ngame:watch("score", function(value)\n  print(value)\nend)\ngame:update(function(state)\n  print(state.score)\nend)\n'
 send('textDocument/didOpen',{'textDocument':{'uri':uri,'languageId':'tua','version':1,'text':source}},False)
 time.sleep(1)
 for method,line,char in [('hover',1,29),('hover',4,21),('signatureHelp',1,19),('completion',5,14),('definition',1,13)]:
  send('textDocument/'+method,{'textDocument':{'uri':uri},'position':{'line':line,'character':char}})
 send('textDocument/diagnostic',{'textDocument':{'uri':uri}})
 send('shutdown',{});send('exit',{},False)
finally:
 p.terminate();p.wait(timeout=5)
 (D/'editor-results.json').write_text(json.dumps(results,indent=2))
 print('Saved actual LSP hover/signature/completion/definition/diagnostic responses')
