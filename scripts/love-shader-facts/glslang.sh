#!/bin/sh
# Validates every fixture with glslangValidator after wrapping it the way
# LOVE 11.5 does, for comparison with `love scripts/love-shader-facts`.
#
# Usage: scripts/love-shader-facts/glslang.sh [fixtures-dir] [lua]
set -eu
dir=${1:-crates/tua-core/src/asset/fixtures/shaders}
lua=${2:-lua}
here=$(dirname "$0")
glslangValidator --version | head -1
for file in "$dir"/*.glsl; do
  name=$(basename "$file")
  for target in desktop gles; do
    stages=0
    for stage in vertex pixel; do
      code=$("$lua" "$here/wrap.lua" "$target" "$stage" "$file" 2>&1) || {
        printf '%-24s %-7s %-6s rejected: %s\n' "$name" "$target" "$stage" "$(printf '%s' "$code" | head -1)"
        continue
      }
      [ -n "$code" ] || continue
      stages=$((stages + 1))
      glslang_stage=frag
      [ "$stage" = vertex ] && glslang_stage=vert
      if output=$(printf '%s\n' "$code" | glslangValidator --stdin -S "$glslang_stage" 2>&1); then
        result=ok
      else
        result=$(printf '%s\n' "$output" | grep -m1 '^ERROR' | sed 's/^ERROR: [0-9]*:\([0-9]*\): /error line \1: /')
      fi
      printf '%-24s %-7s %-6s %s\n' "$name" "$target" "$stage" "$result"
    done
    # LOVE raises "missing 'position' or 'effect' function?" for these.
    [ "$stages" -gt 0 ] || printf '%-24s %-7s %-6s %s\n' "$name" "$target" "-" "no entry point"
  done
done
