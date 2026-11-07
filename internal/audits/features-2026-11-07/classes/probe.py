from pathlib import Path
import subprocess,json
root=Path(__file__).resolve().parent
repo=root.parents[3]
cases={
'self-control':'''final class Node {
 next: Self
 init() {
  local owner = self
  self.next = owner
 }
}
local node = Node.new()
assert(node.next == node, "self reference lost")
print("control ok")
''',
'uninitialized':'''class Item {
 value: number
 init(flag: boolean) {
  if flag then self.value = 1 end
 }
}
''',
'override-drift':'''open class Base {
 read(): number { return 1 }
}
class Child extends Base {
 override read(): string { return "bad" }
}
''',
'self-reference':'''final class Node {
 next: Self
 init() {
  self.next = self
 }
}
local node = Node.new()
assert(node.next == node, "self reference lost")
print("ok")
''',
'positive':'''open class Entity {
 x: number
 init(x: number) { self.x = x }
 update(dt: number) { self.x += dt }
 final read(): number { return self.x }
 same(): Self { return self }
}
final class Particle extends Entity {
 speed: number = 2
 trail: number[]
 init(x: number) {
  super(x)
  self.trail = {}
 }
 override update(dt: number) {
  super.update(dt)
  self.x += self.speed * dt
  table.insert(self.trail, self.x)
 }
}
local p = Particle.new(10)
local entities: Entity[] = {p}
entities[1]:update(2)
assert(p:read() == 16)
assert(p:same() == p)
assert(p.trail[1] == 16)
print("positive ok")
''',
'default-order':'''open class Base {
 value: number = 3
 init(value: number) { self.value = value }
}
final class Child extends Base {
 value: number = 5
 init(value: number) { super(value) }
}
assert(Base.new(1).value == 3)
assert(Child.new(1).value == 5)
print("defaults ok")
''',
'negative':'''class Base {
 value: number
 init(flag: boolean) {
  if flag then self.value = 1 end
 }
 final update(): number { return 1 }
}
class Child extends Base {
 override update(): string { return "bad" }
}
''',
'forward':'''final class Child extends Base {
 init(value: number) { super(value) }
 override read(): number { return super.read() + 1 }
}
open class Base {
 value: number
 init(value: number) { self.value = value }
 read(): number { return self.value }
}
assert(Child.new(2):read() == 3)
print("forward ok")
''',
'implicit-parent':'''open class Base {
 x: number
 init(x: number) { self.x = x }
}
class Child extends Base {
}
local c = Child.new()
assert(type(c.x) == "number", "uninitialized inherited field")
'''
}
results=[]
for name,source in cases.items():
 d=root/name;(d/'src').mkdir(parents=True,exist_ok=True)
 (d/'tua.toml').write_text('source="src"\nout="build"\n[diagnostics]\nrules=false\n')
 (d/'src/main.tua').write_text(source)
 r={'name':name}
 for cmd in [[str(repo/'target/release/tua'),'check'],[str(repo/'target/release/tua'),'build']]:
  p=subprocess.run(cmd,cwd=d,capture_output=True,text=True);r[cmd[-1]]={'code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 if r['build']['code']==0:
  p=subprocess.run(['/opt/homebrew/bin/lua','build/main.lua'],cwd=d,capture_output=True,text=True);r['lua']={'code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 results.append(r)
(root/'results.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
