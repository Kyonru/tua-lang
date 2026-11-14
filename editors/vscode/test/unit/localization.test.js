"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const { localizationHtml } = require("../../src/localization/template");

test("localization view keeps catalog data out of initial HTML and uses text nodes", () => {
  const html = localizationHtml({}, "fixed-nonce");
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /script-src 'nonce-fixed-nonce'/);
  assert.match(html, /node\.textContent = text/);
  assert.match(html, /aria-label="Translation coverage"/);
  assert.match(html, /type: 'search'/);
  assert.match(html, /openLocation/);
});
