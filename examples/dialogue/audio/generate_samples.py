"""Generate original synthetic fixtures; no speech or proprietary recordings."""
import math
from pathlib import Path
import struct
import wave

root = Path(__file__).parent / "src" / "assets"
root.mkdir(exist_ok=True)
fixtures = [("guide", 440, 0.07), ("robot", 180, 0.10), ("recording", 270, 2.4)]
for name, frequency, duration in fixtures:
    rate = 22050
    count = int(rate * duration)
    samples = []
    for index in range(count):
        time = index / rate
        envelope = min(1, time / 0.01, (duration - time) / 0.025)
        pulse = 0.55 + 0.45 * math.sin(2 * math.pi * 3 * time) if name == "recording" else 1
        value = 0.22 * envelope * pulse * math.sin(2 * math.pi * frequency * time)
        samples.append(struct.pack("<h", round(value * 32767)))
    with wave.open(str(root / f"{name}.wav"), "wb") as output:
        output.setparams((1, 2, rate, count, "NONE", "not compressed"))
        output.writeframes(b"".join(samples))
