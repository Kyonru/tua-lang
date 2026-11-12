"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { auditionSource, canAudition, canAuditionScene, sceneSource } = require("../../src/audio/audition-source");

const property = (kind, value) => ({ kind, value });
const system = {
  busItems: [
    { name: "master", properties: { volume: property("number", "0.9") } },
    { name: "sfx", properties: { parent: property("string", "master") } },
  ],
  effectItems: [],
};
const cue = {
  name: "enemy.hit",
  source: { kind: "asset", loader: "sound", assetPath: "assets/hit.wav" },
  assetState: "present", assetUri: "file:///game/src/assets/hit.wav",
  properties: { bus: property("string", "sfx"), maxVoices: property("number", "4"), looping: property("boolean", "false") },
};

test("audition uses the audioSystem runtime and static authored routing", () => {
  const source = auditionSource(system, cue, cue.source.assetPath);
  assert(canAudition(system, cue));
  assert.match(source, /audioSystem\(\{/);
  assert.match(source, /\["sfx"\] = \{ parent = "master" \}/);
  assert.match(source, /source = sound\("assets\/hit.wav"\)/);
  assert.match(source, /maxVoices = 4/);
  assert.match(source, /looping = false/);
  assert.match(source, /audio:play\("audition", \{ volume = 0\.6 \}\)/);
  assert.match(source, /TUA_AUDIO_EFFECT_UNSUPPORTED/);
  assert.match(source, /elapsed >= 3/);
});

test("audition refuses computed sources and settings it cannot reproduce", () => {
  assert.equal(canAudition(system, { ...cue, source: { kind: "computed" } }), false);
  assert.equal(canAudition(system, { ...cue, assetState: "missing" }), false);
  assert.equal(canAudition(system, { ...cue, source: { ...cue.source, loader: "love.audio.newSource" } }), false);
  assert.equal(auditionSource(system, { ...cue, properties: { volume: property("dynamic", null) } }, cue.source.assetPath), undefined);
  assert.equal(auditionSource({ ...system, busItems: [{ name: "sfx", properties: { settings: property("dynamic", null) } }] }, cue, cue.source.assetPath), undefined);
});

test("audition quotes asset names as data rather than Lua code", () => {
  const source = auditionSource(system, cue, 'assets/hit"; os.execute("bad")--.wav');
  assert(source.includes('sound("assets/hit\\\"; os.execute(\\\"bad\\\")--.wav")'));
  assert.equal((source.match(/os\.execute/g) || []).length, 1);
});

test("scene audition uses streamed layers and actual quantized runtime transitions", () => {
  const layer = (name, asset) => ({ name, source: { kind: "asset", loader: "music", assetPath: asset },
    assetState: "present", assetUri: `file:///game/src/${asset}`, properties: {} });
  const exploration = { id: "scene:exploration", name: "exploration", properties: {
    bpm: property("number", "120"), signature: property("authoredExpression", "{ 4, 4 }") },
    layers: [layer("drums", "assets/drums.ogg")] };
  const combat = { id: "scene:combat", name: "combat", properties: { bpm: property("number", "120") },
    layers: [layer("bass", "assets/bass.ogg")] };
  const model = sceneSource(system, exploration, combat, "bar");
  assert(canAuditionScene(system, exploration, combat, "bar"));
  assert.equal(model.assets.length, 2);
  assert.match(model.source, /source = music\("assets\/drums.ogg"\)/);
  assert.match(model.source, /signature = \{ 4, 4 \}/);
  assert.match(model.source, /audio:setMusicState\("combat", \{ quantize = "bar" \}\)/);
  assert.match(model.source, /elapsed >= 6/);
  assert.equal(sceneSource(system, exploration, combat, "random"), undefined);
  assert.equal(sceneSource(system, { ...exploration, layers: [layer("bad", "assets/no.ogg"), { source: { kind: "computed" } }] }), undefined);
});
