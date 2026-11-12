use super::*;

#[test]
fn audio_graph_exposes_source_routing_and_timing_without_evaluating_dynamic_values() {
    let temp = TempProject::new("audio-workbench-graph");
    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        "local audio = require('audio')\nreturn audio\n",
    );
    temp.write(
        "src/audio.tua",
        r#"local generated = love.audio.newSource('assets/computed.wav', 'static')
return audioSystem({
  buses = { master = { volume = 0.8 }, sfx = { parent = 'master', effect = 'room' } },
  effects = { room = { type = 'reverb' } },
  cues = {
    hit = { source = sound('assets/hit.wav'), bus = 'sfx', maxVoices = 4 },
    dynamic = { source = generated, volume = computeGain() },
  },
  music = { combat = { bpm = 120, signature = { 4, 4 }, loopRegion = { start = 1, finish = 2 }, layers = { drums = { source = music('assets/drums.ogg') } } } },
})
"#,
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        json!({"jsonrpc":"2.0","id":2,"method":"tua/audioGraph","params":null}),
    ]);
    let systems = response_with_id(&output, 2)["result"]["graph"]["systems"]
        .as_array()
        .expect("systems");
    let system = systems
        .iter()
        .find(|system| system["uri"] == temp.uri_for("src/audio.tua"))
        .expect("imported declaration");
    let cues = system["cueItems"].as_array().expect("cues");
    let hit = cues
        .iter()
        .find(|cue| cue["name"] == "hit")
        .expect("hit cue");
    assert_eq!(hit["source"]["kind"], "asset");
    assert_eq!(hit["source"]["assetPath"], "assets/hit.wav");
    assert_eq!(hit["properties"]["bus"]["value"], "sfx");
    assert_eq!(hit["properties"]["maxVoices"]["value"], "4");
    assert_eq!(hit["range"]["start"]["line"], 5);
    let dynamic = cues
        .iter()
        .find(|cue| cue["name"] == "dynamic")
        .expect("dynamic cue");
    assert_eq!(dynamic["source"]["kind"], "unknown");
    assert_eq!(dynamic["properties"]["volume"]["kind"], "dynamic");
    assert!(dynamic["properties"]["volume"]["value"].is_null());
    assert_eq!(system["sceneItems"][0]["properties"]["bpm"]["value"], "120");
    assert_eq!(
        system["sceneItems"][0]["properties"]["signature"]["kind"],
        "authoredExpression"
    );
    assert_eq!(
        system["sceneItems"][0]["properties"]["loopRegion"]["kind"],
        "authoredExpression"
    );
    assert_eq!(
        system["sceneItems"][0]["layers"][0]["source"]["assetPath"],
        "assets/drums.ogg"
    );
}
