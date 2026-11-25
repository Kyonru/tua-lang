# Character artwork

The unchanged `adventurer.png` is `PNG/Adventurer/adventurer_tilesheet.png` from
[Kenney Platformer Characters](https://kenney.nl/assets/platformer-characters),
licensed CC0. See `License.txt` for the included license notice.

The atlas is 720×330: nine columns and three rows of 80×110 cells, without borders.
The tutorial uses idle at column 0/row 0 and walking at columns 0 and 6/row 1.
Unused poses stay in the original image so this example needs no asset-editing step.
The host scales frames to 60% and flips horizontally when moving left.

`provenance.json` records the download and file hashes. No network access or asset
generation is required to run the example.

The unchanged `zombie.png` comes from the same Kenney pack, original path
`PNG/Zombie/zombie_tilesheet.png`. It uses the same cell dimensions; this chapter
uses idle plus the shared two-frame walking animation. `zombie-provenance.json` records its original source and
file hash. Both atlases are CC0; no reference screenshot is a production asset.
