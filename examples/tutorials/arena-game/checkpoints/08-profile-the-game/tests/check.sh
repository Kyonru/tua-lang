#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
TUA_BIN="${TUA_BIN:-../../../../../target/release/tua}"
"$TUA_BIN" fmt --check
"$TUA_BIN" fmt --check tests/reference-combat.tua
"$TUA_BIN" fmt --check src/locales/en.lua src/locales/fr.lua tests/*.lua
"$TUA_BIN" check --deny-warnings
"$TUA_BIN" build
cd build
for suite in movement session combat decisions soundscape conversation; do
  lua "../tests/$suite.lua"
done
