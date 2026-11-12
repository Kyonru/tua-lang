"use strict";

const CUE_OPTIONS = ["volume", "pitch", "priority", "maxVoices", "overflow", "fadeIn", "looping", "effect", "bus"];
const BUS_OPTIONS = ["parent", "volume", "maxVoices", "overflow", "effect", "muted"];
const EFFECT_OPTIONS = ["type", "volume", "decaytime", "density", "diffusion", "gain", "highgain", "lowgain", "delay", "feedback"];
const SCENE_OPTIONS = ["bpm", "signature", "loopRegion", "bus", "volume"];
const LAYER_OPTIONS = ["volume", "bus", "looping", "priority"];
const MAX_SECONDS = 3;
const SCENE_SECONDS = 6;

function luaLiteral(property, name) {
  if (!property || property.kind === "dynamic" || property.value == null) return undefined;
  if (property.kind === "string") return JSON.stringify(property.value);
  if (property.kind === "boolean" && ["true", "false"].includes(property.value)) return property.value;
  if (property.kind === "number" && Number.isFinite(Number(property.value))) return String(Number(property.value));
  if (property.kind === "authoredExpression" && name === "signature" && /^\{\s*\d+\s*,\s*\d+\s*,?\s*\}$/u.test(property.value)) return property.value;
  if (property.kind === "authoredExpression" && name === "loopRegion") {
    const start = /\bstart\s*=\s*(\d+(?:\.\d+)?)/u.exec(property.value);
    const finish = /\bfinish\s*=\s*(\d+(?:\.\d+)?)/u.exec(property.value);
    if (start && finish && /^\{[\s\w=.,]+\}$/u.test(property.value)) return `{ start = ${Number(start[1])}, finish = ${Number(finish[1])} }`;
  }
  return undefined;
}
function staticOptions(item, names) {
  const properties = item?.properties || {};
  if (Object.keys(properties).some(name => !names.includes(name))) return undefined;
  const clauses = [];
  for (const name of names) {
    if (!(name in properties)) continue;
    const literal = luaLiteral(properties[name], name);
    if (literal === undefined) return undefined;
    clauses.push(`${name} = ${literal}`);
  }
  return clauses;
}
function auditionSource(system, cue, assetPath) {
  const cueOptions = staticOptions(cue, CUE_OPTIONS);
  if (!cueOptions) return undefined;
  const bus = cue.properties?.bus?.value;
  if (bus && bus !== "master" && !(system.busItems || []).some(item => item.name === bus)) return undefined;
  const effect = cue.properties?.effect?.value;
  if (effect && !(system.effectItems || []).some(item => item.name === effect)) return undefined;
  const buses = [];
  for (const bus of system.busItems || []) {
    const options = staticOptions(bus, BUS_OPTIONS);
    if (!options) return undefined;
    buses.push(`[${JSON.stringify(bus.name)}] = { ${options.join(", ")} }`);
  }
  const effects = [];
  for (const effect of system.effectItems || []) {
    const options = staticOptions(effect, EFFECT_OPTIONS);
    if (!options || !effect.properties?.type) return undefined;
    effects.push(`[${JSON.stringify(effect.name)}] = { ${options.join(", ")} }`);
  }
  const sourceKind = cue.source?.loader === "music" ? "music" : "sound";
  const source = `${sourceKind}(${JSON.stringify(assetPath)})`;
  return `-- Generated only inside a disposable audition project.\nlocal audio = audioSystem({\n  maxVoices = 8,\n  buses = { ${buses.join(", ")} },\n  effects = { ${effects.join(", ")} },\n  cues = { audition = { source = ${source}, ${cueOptions.join(", ")} } },\n})\naudio:on("effectUnsupported", function(event) print("TUA_AUDIO_EFFECT_UNSUPPORTED:" .. tostring(event.effect) .. ":" .. tostring(event.reason)) end)\nlocal elapsed = 0\nfunction love.load()\n  local _voice, result = audio:play("audition", { volume = 0.6 })\n  if result == "rejected" then error("Audio system rejected the audition cue") end\n  print("TUA_AUDIO_AUDITION_READY")\nend\nfunction love.update(dt: number)\n  audio:update(dt)\n  elapsed = elapsed + dt\n  if elapsed >= ${MAX_SECONDS} then love.event.quit() end\nend\n`;
}
function sceneSource(system, scene, target, quantize = "immediate") {
  if (!scene || !["immediate", "beat", "bar"].includes(quantize)) return undefined;
  const scenes = target && target.id !== scene.id ? [scene, target] : [scene];
  const assets = [];
  const definitions = [];
  for (const item of scenes) {
    const properties = { ...item.properties };
    // Runtime analysis cannot be reproduced without loading an arbitrary catalog.
    if (properties.analysis) {
      delete properties.analysis;
    }
    const options = staticOptions({ properties }, SCENE_OPTIONS);
    if (!options || !item.layers?.length) return undefined;
    const layers = [];
    for (const layer of item.layers) {
      if (layer.source?.kind !== "asset" || layer.source.loader !== "music" ||
          layer.assetState !== "present" || !layer.assetUri) return undefined;
      const layerOptions = staticOptions(layer, LAYER_OPTIONS);
      if (!layerOptions) return undefined;
      const assetPath = layer.source.assetPath;
      assets.push({ path: assetPath, uri: layer.assetUri });
      layers.push(`[${JSON.stringify(layer.name)}] = { source = music(${JSON.stringify(assetPath)}), ${layerOptions.join(", ")} }`);
    }
    definitions.push(`[${JSON.stringify(item.name)}] = { ${options.join(", ")}, layers = { ${layers.join(", ")} } }`);
  }
  const buses = [];
  for (const bus of system.busItems || []) {
    const options = staticOptions(bus, BUS_OPTIONS);
    if (!options) return undefined;
    buses.push(`[${JSON.stringify(bus.name)}] = { ${options.join(", ")} }`);
  }
  const effects = [];
  for (const effect of system.effectItems || []) {
    const options = staticOptions(effect, EFFECT_OPTIONS);
    if (!options || !effect.properties?.type) return undefined;
    effects.push(`[${JSON.stringify(effect.name)}] = { ${options.join(", ")} }`);
  }
  const transition = target && target.id !== scene.id
    ? `if elapsed >= 0.5 and not transitioned then\n    transitioned = true\n    local status = audio:setMusicState(${JSON.stringify(target.name)}, { quantize = ${JSON.stringify(quantize)} })\n    print("TUA_AUDIO_TRANSITION:" .. status)\n  end\n  `
    : "";
  const source = `-- Isolated scene audition; authored static layers only.\nlocal audio = audioSystem({\n  buses = { ${buses.join(", ")} },\n  effects = { ${effects.join(", ")} },\n  music = { ${definitions.join(", ")} },\n})\naudio:on("effectUnsupported", function(event) print("TUA_AUDIO_EFFECT_UNSUPPORTED:" .. tostring(event.effect) .. ":" .. tostring(event.reason)) end)\nlocal elapsed, transitioned = 0, false\nfunction love.load()\n  audio:setMusicState(${JSON.stringify(scene.name)})\n  print("TUA_AUDIO_AUDITION_READY")\nend\nfunction love.update(dt: number)\n  audio:update(dt)\n  elapsed = elapsed + dt\n  ${transition}if elapsed >= ${SCENE_SECONDS} then love.event.quit() end\nend\n`;
  return { source, assets };
}
function canAuditionScene(system, scene, target, quantize) {
  return sceneSource(system, scene, target, quantize) !== undefined;
}
function canAudition(system, cue) {
  return cue?.source?.kind === "asset" && cue.assetState === "present" && Boolean(cue.assetUri) &&
    ["sound", "music"].includes(cue.source.loader) &&
    auditionSource(system, cue, cue.source.assetPath) !== undefined;
}

module.exports = { auditionSource, canAudition, canAuditionScene, sceneSource, MAX_SECONDS, SCENE_SECONDS };
