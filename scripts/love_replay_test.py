"""Runner filesystem/process regressions; real rendering is checked separately."""
import argparse
import json
from pathlib import Path
import sys
import tempfile
import unittest

import love_replay


class ReplayTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="replay-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.project = self.root / "project"
        self.project.mkdir()
        (self.project / "tua.toml").write_text('out = "build"\n')
        self.scenario = self.root / "scenario.lua"
        self.scenario.write_text('return { verify = function() end }\n')
        self.tua = self.script("tua", '''
import json, pathlib, sys
if '--version' in sys.argv:
    print(json.dumps({'version': 'fixture'}))
else:
    build = pathlib.Path('build')
    build.mkdir(exist_ok=True)
    (build / 'main.lua').write_text('original game')
    (build / 'conf.lua').write_text('original config')
''')
        self.love = self.script("love", '''
import os, pathlib, sys
root = pathlib.Path(sys.argv[1])
assert 'require("__tua_replay_runtime")' in (root / 'main.lua').read_text()
assert (root / '__tua_replay_game.lua').read_text() == 'original game'
conf = (root / 'conf.lua').read_text()
assert conf.index('setIdentity') < conf.index('require')
assert os.environ['TUA_REPLAY_IDENTITY'] in conf
save = pathlib.Path(os.environ['TUA_REPLAY_OUTPUT']) / os.environ['TUA_REPLAY_IDENTITY']
save.mkdir()
(save / 'test-save').write_text('disposable')
print('TUA_REPLAY_SAVE\\t' + str(save), flush=True)
print('TUA_REPLAY_RUNTIME\\t11.5.0\\tLuaJIT fixture\\tfalse')
print('TUA_REPLAY_OK\\t' + os.environ['TUA_REPLAY_FRAMES'])
''')
        self.args = argparse.Namespace(project=self.project, tua=str(self.tua), love=str(self.love),
            output=self.root / "evidence", scenario=self.scenario, scenario_module=None,
            frames=3, fps=30, seed=1, timeout=5, capture=False, video=False, ffmpeg="unused")

    def script(self, name, body):
        path = self.root / name
        path.write_text(f"#!{sys.executable}\n" + body)
        path.chmod(0o755)
        return path

    def test_disposable_build_save_cleanup_and_provenance(self):
        result = love_replay.run(self.args)
        metadata = json.loads((result / "provenance.json").read_text())
        self.assertEqual(metadata["status"], "passed")
        self.assertFalse(metadata["jit_enabled"])
        self.assertEqual(metadata["love_version"], "11.5.0")
        self.assertFalse((result / metadata["identity"]).exists())
        self.assertEqual((self.project / 'build/main.lua').read_text(), 'original game')
        self.assertEqual((self.project / 'build/conf.lua').read_text(), 'original config')
        self.assertEqual(metadata["scenario_sha256"], love_replay.digest(self.scenario))
        with self.assertRaisesRegex(ValueError, "new directory"):
            love_replay.run(self.args)

    def test_missing_verification_fails_and_preserves_logs(self):
        self.love.write_text(f"#!{sys.executable}\nprint('stopped early')\n")
        with self.assertRaisesRegex(ValueError, "without completing"):
            love_replay.run(self.args)
        self.assertEqual(json.loads((self.args.output / 'provenance.json').read_text())["status"], "failed")
        self.assertIn('stopped early', (self.args.output / 'runtime.log').read_text())

    def test_timeout_records_failure(self):
        self.love.write_text(f"#!{sys.executable}\nimport time\ntime.sleep(10)\n")
        self.args.timeout = 0.1
        with self.assertRaises(love_replay.subprocess.TimeoutExpired):
            love_replay.run(self.args)
        self.assertEqual(json.loads((self.args.output / 'provenance.json').read_text())["status"], "failed")

    def test_capture_requires_every_frame(self):
        self.args.capture = True
        with self.assertRaisesRegex(ValueError, "Incomplete frame"):
            love_replay.run(self.args)

    def test_rejects_unsafe_output_and_module_paths(self):
        for output in ('.', '..', '../elsewhere'):
            with self.subTest(output=output), self.assertRaises(ValueError):
                love_replay.built_root(self.project, {'out': output})
        self.args.scenario = None
        self.args.scenario_module = '../outside'
        with self.assertRaisesRegex(ValueError, "dotted Lua"):
            love_replay.run(self.args)

    def test_save_cleanup_ignores_unrelated_directory(self):
        unrelated = self.root / 'player-save'
        unrelated.mkdir()
        love_replay.cleanup_save('TUA_REPLAY_SAVE\t' + str(unrelated), 'tua-replay-test')
        self.assertTrue(unrelated.is_dir())


if __name__ == '__main__':
    unittest.main()
