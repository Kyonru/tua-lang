import pathlib,os,json
out=pathlib.Path(os.environ["TUA_ASSET_PIPELINE_OUTPUT_DIR"])
out.mkdir(parents=True,exist_ok=True)
def manifest(v): pathlib.Path(os.environ["TUA_ASSET_PIPELINE_MANIFEST"]).write_text(json.dumps(v))
(out/"x.txt").write_text("ok")
manifest({"version":1,"outputs":["x.txt","x.txt"]})