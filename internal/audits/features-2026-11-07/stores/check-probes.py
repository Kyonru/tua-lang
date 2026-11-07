from pathlib import Path
import subprocess,json
root=Path(__file__).resolve().parents[4]
d=Path(__file__).resolve().parent
p=d/'typing';(p/'src').mkdir(parents=True,exist_ok=True)
(p/'tua.toml').write_text('source="src"\nout="build"\n[diagnostics]\nrules=false\n')
probes={
'bad_update': 'local game = store({ score = 0 })\ngame:update(function(state) state.score = "wrong" end)',
'bad_subscribe': 'local game = store({ score = 0 })\ngame:subscribe(function(state, previous) local wrong: string = previous.score end)',
'bad_watch': 'local game = store({ score = 0 })\ngame:watch("score", function(value) local wrong: string = value end)',
'bad_watch_explicit': 'local game = store({ score = 0 })\ngame:watch("score", function(value: string) print(value) end)',
'unknown_key': 'local game = store({ score = 0 })\ngame:watch("missing", function(value) print(value) end)',
'missing_set': 'local game = store({ score = 0, paused = false })\ngame:set({ score = 0 })',
'watch_variable_key': 'local game = store({ score = 0, paused = false })\nlocal key: "score" | "paused" = "score"\ngame:watch(key, function(value) print(value) end)',
'watch_optional_nil': 'type State = { score?: number }\nlocal initial: State = {}\nlocal game = store(initial)\ngame:watch("score", function(value: number) print(value + 1) end)',
'state_optional_update':'type State = { score?: number }\nlocal initial: State = {}\nlocal game = store(initial)\ngame:update(function(state) state.score = nil end)',
'direct_get_mutation': 'local game = store({ score = 0 })\ngame:get().score = "wrong"',
}
records=[]
for name,source in probes.items():
 (p/'src/main.tua').write_text(source+'\n')
 (d/(name+'.tua')).write_text(source+'\n')
 r=subprocess.run([str(root/'target/release/tua'),'check'],cwd=p,text=True,capture_output=True)
 records.append(dict(probe=name,code=r.returncode,stdout=r.stdout,stderr=r.stderr))
(d/'typing-results.json').write_text(json.dumps(records,indent=2));print(json.dumps(records,indent=2))
