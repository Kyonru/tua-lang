package.path = 'vendor/runtime/?.lua;' .. package.path
_G.loadstring = loadstring or load
local files = { ['main.lua']='-- main', ['.tuahotreload']='1\n' }
local restarts=0
love = { filesystem = { getInfo=function(p) return files[p] and {type='file'} end, read=function(p) return files[p] end }, event={quit=function(reason) if reason=='restart' then restarts=restarts+1 end end} }
require('tuahotreload').install()
files['conf.lua']='function love.conf(t) t.window.width=900 end'
files['.tuahotreload']='2\n'
love.update(0.25)
print('restart after conf creation: '..restarts)
files['conf.lua']='function love.conf(t) t.window.width=1000 end'
files['.tuahotreload']='3\n'
love.update(0.25)
print('restart after subsequent conf edit: '..restarts)
files['main.lua']='-- changed main'
files['.tuahotreload']='4\n'
love.update(0.25)
print('restart after main edit (positive control): '..restarts)
assert(restarts==1)
