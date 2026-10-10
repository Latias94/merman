// Checks real renderer output in Chromium; Rust supplies SVG fixtures through stdin.
// Run with:
// cargo nextest run -p merman-render --test eventmodeling_svg_test \
//   --run-ignored ignored-only -E 'test(eventmodeling_browser_text_cascade)'
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const requireFromTools = createRequire(new URL("../mermaid-cli/package.json", import.meta.url));
const { default: puppeteer } = await import(pathToFileURL(requireFromTools.resolve("puppeteer")));
let input = "";
for await (const chunk of process.stdin) input += chunk;
const fixtures = JSON.parse(input);
assert.ok(fixtures.length > 0, "at least one rendered SVG fixture is required");
const browser = await puppeteer.launch({ headless: "shell" });
try {
  const page = await browser.newPage();
  for (const fixture of fixtures) {
    await page.setContent(fixture.svg);
    const actual = await page.evaluate(() => {
      const lanes = [...document.querySelectorAll(".em-swimlane text")];
      const boxes = [...document.querySelectorAll(".em-box span")];
      const children = [...document.querySelectorAll(".em-box span b, .em-box span code")];
      return {
        fills: lanes.map((element) => getComputedStyle(element).fill),
        colors: boxes.map((element) => getComputedStyle(element).color),
        childColors: children.map((element) => getComputedStyle(element).color),
        localPaint: [...lanes, ...boxes].some((element) =>
          element.hasAttribute("fill") || element.hasAttribute("color") ||
          element.style.fill || element.style.color),
      };
    });
    assert.equal(actual.localPaint, false, `${fixture.name}: no local text paint`);
    assert.deepEqual(actual.fills, Array(3).fill(fixture.fill), `${fixture.name}: swimlane fill`);
    assert.deepEqual(actual.colors, Array(3).fill(fixture.color), `${fixture.name}: XHTML color`);
    assert.ok(actual.childColors.length >= 3, `${fixture.name}: visible box titles`);
    assert.ok(actual.childColors.every((color) => color === fixture.color), `${fixture.name}: nested XHTML inheritance`);
    console.log(JSON.stringify({ name: fixture.name, ...actual }));
  }
} finally {
  await browser.close();
}
