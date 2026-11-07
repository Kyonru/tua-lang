from pathlib import Path
import ctypes,json
root=Path(__file__).resolve().parent
lib=ctypes.CDLL('/Applications/LOVE.app/Contents/Frameworks/Lua.framework/Lua')
lib.luaL_newstate.restype=ctypes.c_void_p
lib.luaL_openlibs.argtypes=[ctypes.c_void_p]
lib.luaL_loadstring.argtypes=[ctypes.c_void_p,ctypes.c_char_p]
lib.lua_pcall.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int]
lib.lua_tolstring.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_void_p];lib.lua_tolstring.restype=ctypes.c_char_p
lib.lua_close.argtypes=[ctypes.c_void_p]
results=[]
for mode in ['off','on']:
 for name in ['positive','chained_constructor','underflow_div','normalization','comment_math','bracket_math','edge_math']:
  state=lib.luaL_newstate();lib.luaL_openlibs(state)
  src='jit.'+mode+'(); local lines={}; print=function(...) local s={} for i=1,select("#",...) do s[i]=tostring(select(i,...)) end lines[#lines+1]=table.concat(s,"\\t") end; print(jit.version,"jit",jit.status()); love={math={}}; package.path='+json.dumps(str(root/name/'build/?.lua'))+'..";"..package.path; local ok,err=pcall(dofile,'+json.dumps(str(root/name/'build/main.lua'))+'); print("result",ok,err); return table.concat(lines,"\\n")'
  code=lib.luaL_loadstring(state,src.encode()) or lib.lua_pcall(state,0,1,0)
  results.append(dict(mode=mode,name=name,code=code,output=lib.lua_tolstring(state,-1,None).decode()))
  lib.lua_close(state)
(root/'luajit-results.json').write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2))
