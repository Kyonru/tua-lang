# GitHub Linguist Support For Tua

This directory contains the pieces needed to propose Tua support upstream to
`github/linguist`.

## Files

- `languages.yml` is the proposed language entry.
- `grammar-source.yml` records the existing TextMate grammar in
  `editors/vscode/syntaxes/tua.tmLanguage.json`.
- `samples/Tua/features.tua` is a representative sample for Linguist's language
  classifier.

## Upstreaming Notes

GitHub only recognizes new language names after they land in Linguist itself.
The repository-level `.gitattributes` entry documents the intended
classification, but it cannot teach github.com a brand-new language on its own.

The current Linguist workflow for a new language is documented in
https://github.com/github-linguist/linguist/blob/master/CONTRIBUTING.md.

When preparing an upstream PR:

1. Copy the `Tua` entry from `languages.yml` into Linguist's
   `lib/linguist/languages.yml`. Keep `language_id` omitted at first.
2. Publish the TextMate grammar from this repository, then add it with
   Linguist's `script/add-grammar`.
3. Copy `samples/Tua/features.tua` into Linguist's `samples/Tua/` directory.
4. Run Linguist's `script/update-ids` to assign the final `language_id`.
5. State the sample license in the PR. The sample here is original Tua example
   code written for the language fixture.

Linguist normally expects evidence that a new extension is used in the wild.
For Tua, link to non-fork repositories containing `.tua` files when opening the
upstream PR.
