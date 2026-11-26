"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const { richTextHtml, portraitHtml, loadPortraits } = require("../../src/dialogue/media");

function model(...ids) {
  return { sections: [{ lines: ids.map(portrait => ({ character: { name: "Mara", portrait } })) }] };
}

test("rich presentation escapes text and annotations without interpreting substituted markup", () => {
  const html = richTextHtml([
    { text: '<img src=x> [em]literal[/em]', marks: ["em", "style:underline"] },
    { text: "Hello", marks: ['style:evil\" onclick=\"x'] },
    { text: "", marks: [], annotation: '<pause>' },
  ]);
  assert.match(html, /<strong><span class="rich-style rich-underline"/);
  assert.match(html, /&lt;img src=x&gt; \[em\]literal\[\/em\]/);
  assert.doesNotMatch(html, /<img|onclick="x/);
  assert.match(html, /&lt;pause&gt;/);
});

test("portrait IDs map to bounded local images; missing, remote and escaping paths stay unavailable", async t => {
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), "dialogue-portraits-"));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));
  const root = path.join(temp, "src");
  await fs.mkdir(root);
  await fs.copyFile(path.join(__dirname, "../fixtures/player.png"), path.join(root, "mara.png"));
  await fs.copyFile(path.join(root, "mara.png"), path.join(temp, "outside.png"));
  await fs.symlink(path.join(temp, "outside.png"), path.join(root, "escape.png"));
  await fs.writeFile(path.join(root, "large.png"), Buffer.alloc(2 * 1024 * 1024 + 1));
  const preview = model("mara", "mara.png", "missing", "https://example.com/a.png", "../outside.png", "escape.png", "large.png");
  await loadPortraits(preview, root, { mara: "mara.png" });
  const portraits = preview.sections[0].lines.map(line => line.character);
  assert.match(portraits[0].portraitUri, /^data:image\/png;base64,/);
  assert.equal(portraits[0].portraitUri, portraits[1].portraitUri);
  for (const portrait of portraits.slice(2)) {
    assert.equal(portrait.portraitUri, undefined);
    assert.ok(portrait.portraitIssue);
  }
  assert.match(portraits[2].portraitIssue, /map game IDs/);
  assert.match(portraits[5].portraitIssue, /symlink/);
  assert.match(portraitHtml(portraits[0]), /alt="Mara"/);
  assert.match(portraitHtml(portraits[2]), /portrait unavailable/);
  assert.equal(portraitHtml({ name: "Narrator" }), "");
  await fs.rm(path.join(root, "mara.png"));
  await loadPortraits(preview, root, { mara: "mara.png" });
  assert.equal(portraits[0].portraitUri, undefined, "deleted images cannot retain an earlier decoded payload");
  assert.match(portraits[0].portraitIssue, /Image not found/);
});
