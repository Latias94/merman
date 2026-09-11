// Chromium consumes the actual SVG fixtures emitted by requirement_browser_text_cascade.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const requireFromTools = createRequire(new URL("../mermaid-cli/package.json", import.meta.url));
const { default: puppeteer } = await import(pathToFileURL(requireFromTools.resolve("puppeteer")));
let input = "";
for await (const chunk of process.stdin) input += chunk;
const fixtures = JSON.parse(input);
assert.ok(fixtures.length > 0);
const browser = await puppeteer.launch({ headless: "shell" });
try {
  const page = await browser.newPage();
  for (const fixture of fixtures) {
    await page.setContent(fixture.svg);
    const colors = await page.evaluate((html) => {
      const selector = html ? "span.nodeLabel" : ".label text";
      const paint = (element) => getComputedStyle(element)[html ? "color" : "fill"];
      return {
        owned: [...document.querySelectorAll(`#requirement-theme-req1 ${selector}`)].map(paint),
        inherited: [...document.querySelectorAll(`#requirement-theme-elem1 ${selector}`)].map(paint),
        edges: [...document.querySelectorAll(html ? "g.edgeLabel span.edgeLabel" : "g.edgeLabel text")].map(paint),
      };
    }, fixture.html);
    for (const role of ["owned", "inherited", "edges"]) {
      assert.ok(colors[role].length > 0, `${fixture.name}: ${role} must exist`);
      assert.ok(colors[role].every((color) => color === fixture[role]),
        `${fixture.name}: ${role}: ${JSON.stringify(colors[role])}`);
    }
    console.log(JSON.stringify({ name: fixture.name, ...colors }));
  }
} finally {
  await browser.close();
}
