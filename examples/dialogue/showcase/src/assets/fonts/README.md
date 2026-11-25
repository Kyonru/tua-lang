# Bundled fonts

Downloaded from the Google Fonts source repository on 2026-11-25:

- Lora: https://github.com/google/fonts/tree/main/ofl/lora — `Lora[wght].ttf`,
  distributed unmodified as `lora.ttf`. Its default is regular (400).
  Copyright and reserved font name are retained in `lora-OFL.txt`.
- Manrope: https://github.com/google/fonts/tree/main/ofl/manrope — `Manrope[wght].ttf`,
  instantiated at weight 400 as `manrope.ttf` with fontTools `instantiateVariableFont`.
  LÖVE otherwise selects the original font's extra-light default. The SIL OFL license
  is included as `manrope-OFL.txt`; it does not declare a reserved font name.

Both fonts are under SIL Open Font License 1.1. No system font installation or network
request is needed at runtime. The example's build copies these resources with the other assets.
