# Alchemy Lab (Tua 0.7.0 mini-game)

Click two elements to discover a combination. The four starting elements are air, earth, fire, and water. New discoveries become clickable tiles. Press Escape to clear the first selection. The recipe chain includes water + fire → steam, earth + water → mud, fire + mud → brick, air + fire → energy, and energy + steam → life.

The game uses a cross-file `Combinable` protocol and `RecipeBook` conformance, a generic `Slot<T>` class with imported `SlotInstance<T>` alias, a cross-file generic `remember<T>` function, and a generic `string[]` history. This makes a small but realistic source graph for type checking and editor assistance.

From this directory:

```sh
/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua check
/Users/kyonru/Documents/projects/Experiments/tlua/target/debug/tua build
/Applications/love.app/Contents/MacOS/love build
```

For a headless gameplay smoke test after building:

```sh
lua -e 'package.path = "build/?.lua;" .. package.path; dofile("smoke.lua")'
```

`python3 lsp_probe.py` exercises protocol-level hover and completion against the local debug Tua server. It is not a VS Code UI test.
