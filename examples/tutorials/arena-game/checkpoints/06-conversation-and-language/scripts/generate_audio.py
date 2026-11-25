"""Generate original, deterministic PCM examples; no external samples."""
import hashlib
import json
import math
from pathlib import Path
import struct
import wave

DEST = Path(__file__).resolve().parents[1] / "src/assets/audio"
DEST.mkdir(parents=True, exist_ok=True)
RATE = 22050

def write(name, duration, sample):
    path = DEST / (name + ".wav")
    data = bytearray()
    for i in range(round(duration * RATE)):
        t = i / RATE
        value = max(-1, min(1, sample(t)))
        data.extend(struct.pack("<h", round(value * 32767)))
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(RATE)
        output.writeframes(data)
    return {"file": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

def tone(hz, duration, gain):
    return lambda t: math.sin(2 * math.pi * hz * t) * gain * min(1, t / .006) * max(0, 1 - t / duration) ** 2

files = []
for name, hz, duration in [("shot", 440, .09), ("hit", 880, .12), ("hurt", 110, .2), ("defeat", 165, .5)]:
    files.append(write(name, duration, tone(hz, duration, .22)))
for name, notes in [("harbor", [220, 277.18, 329.63, 277.18]), ("combat", [220, 261.63, 293.66, 329.63])]:
    def phrase(t, notes=notes):
        phase = t % .5
        hz = notes[int(t / .5) % len(notes)]
        envelope = min(1, phase / .015) * max(0, 1 - phase / .5) ** 2
        return (.12 * math.sin(2 * math.pi * hz * phase) + .04 * math.sin(math.pi * hz * phase)) * envelope
    files.append(write(name, 4, phrase))
(DEST / "provenance.json").write_text(json.dumps({"origin": "Original procedural tones; no third-party samples", "license": "CC0-1.0", "generator": "scripts/generate_audio.py", "sample_rate": RATE, "channels": 1, "files": files}, indent=2) + "\n")
