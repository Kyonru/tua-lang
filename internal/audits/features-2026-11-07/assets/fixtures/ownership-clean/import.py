import os,json,pathlib
out=pathlib.Path(os.environ['TUA_ASSET_PIPELINE_OUTPUT_DIR'])
out.mkdir(parents=True,exist_ok=True)
(out/'data.txt').write_text('GENERATED CONTENT')
pathlib.Path(os.environ['TUA_ASSET_PIPELINE_MANIFEST']).write_text(json.dumps({'version':1,'outputs':['data.txt']}))
