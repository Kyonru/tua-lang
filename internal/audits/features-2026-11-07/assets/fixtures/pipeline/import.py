import os,json,pathlib
p=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_PROJECT_ROOT'])
r=p/'runs.txt';r.write_text(str(int(r.read_text() if r.exists() else '0')+1))
out=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_OUTPUT_DIR']);out.mkdir(parents=True,exist_ok=True)
(out/'types.d.tua').write_text('export type Room = { name: string }\n')
(out/'data.txt').write_text((p/'input.txt').read_text())
pathlib.Path(os.environ['TUA_ASSET_PIPELINE_MANIFEST']).write_text(json.dumps({'version':1,'outputs':['types.d.tua','data.txt'],'diagnostics':[]}))
