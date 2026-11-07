package.path = 'vendor/runtime/?.lua;' .. package.path
dofile('vendor/tests/hotreload_test.lua')
print('runtime=' .. _VERSION .. ' ' .. (jit and jit.version or ''))
