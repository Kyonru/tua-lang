from pathlib import Path
import subprocess,json
ROOT=Path(__file__).resolve().parent
BIN=ROOT.parents[3]/'target/release/tua'
cases={
'branches':('local n=0\nif true then\nif false then n=1 else n=2 end\nelseif false then n=3 else n=4 end\nprint(n)\n',''),
'strings':('local s=[=[first  \n\n  x=2  \n]=]\nlocal q="hi\\\n  😀 world"\n--[=[ if true then\n   x= 2\n]=]\nprint(s,q,"it\\\'s",\'a"b\')\n','quote_style="single"\n'),
'wrap':('local t={1,2,{3,4},5}\nprint("abcdefghijk",t[1],t[2],t[3][1],t[4])\n','line_width=20\n'),
'crlf':('if true then\r\nprint("😀",1+2)\r\nend\r\n','line_ending="preserve"\nindent_style="tabs"\nfinal_newline=false\n'),
'classes':('final class Box {\nvalue:number=4\nget():number {\nreturn self.value\n}\n}\nlocal function add(x:number,y:number=3):number\nreturn x+y\nend\nlocal b=Box.new()\nprint(b:get(),add(2))\n',''),
'incomplete':('if true then\nprint(1)\n',''),
'bom':('\ufeffprint(1+2)\n',''),
'blank_literal':('local s=[[\n \t \nlast  \n]]\nprint(#s,s)\n',''),
'escaped_space':('local s="hello\\\n  there"\nprint(#s,s)\n',''),
'comment_suffix':('local s=[[\nabc\n]]; if true then\nprint(s)\nend\n',''),
'invalid_config':('print(1)\n','indent_width=0\n'),
}
results=[]
for name,(src,opts) in cases.items():
 p=ROOT/name;(p/'src').mkdir(parents=True,exist_ok=True)
 (p/'tua.toml').write_text('source="src"\nout="build"\n[diagnostics]\nrules=false\n[format]\n'+opts)
 f=p/'src/main.tua';f.write_bytes(src.encode());(p/'before.tua').write_bytes(f.read_bytes())
 def run(args):
  r=subprocess.run(args,cwd=p,capture_output=True,text=True);return {'command':list(map(str,args)),'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
 r={'name':name,'before':src,'precheck':run([BIN,'fmt','--check']),'prebuild':run([BIN,'build','--emit-on-error'])}
 if (p/'build/main.lua').exists():
  (p/'before.lua').write_bytes((p/'build/main.lua').read_bytes());r['prerun']=run(['/opt/homebrew/bin/lua','build/main.lua'])
 r['fmt']=run([BIN,'fmt']);r['after']=f.read_bytes().decode();(p/'after.tua').write_bytes(f.read_bytes())
 r['postcheck']=run([BIN,'fmt','--check']);r['postbuild']=run([BIN,'build','--emit-on-error'])
 if (p/'build/main.lua').exists():r['postrun']=run(['/opt/homebrew/bin/lua','build/main.lua'])
 r['fmt2']=run([BIN,'fmt']);r['idempotent']=f.read_bytes().decode()==r['after'];r['runtime_equal']=r.get('prerun')==r.get('postrun')
 results.append(r)
(ROOT/'cli-evidence.json').write_text(json.dumps(results,indent=2,default=str))
for r in results:print(r['name'], 'fmt',r['fmt']['exit'],'idempotent',r['idempotent'],'runtime',r['runtime_equal'], 'build',r['prebuild']['exit'],r['postbuild']['exit'])
