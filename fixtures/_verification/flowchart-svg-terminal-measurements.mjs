import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

// Run with --write to collect; without it, verify the committed browser facts.
const root = fileURLToPath(new URL("../../", import.meta.url));
const require = createRequire(join(root, "tools/mermaid-cli/package.json"));
const puppeteer = require("puppeteer");
const matrix = JSON.parse(readFileSync(join(root,
  "fixtures/_verification/flowchart-elk-browser-measurements.json"), "utf8"));
const target = new URL("./flowchart-svg-terminal-measurements.json", import.meta.url);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const configured = await puppeteer.executablePath();
const executablePath = process.env.PUPPETEER_EXECUTABLE_PATH
  ?? (existsSync(configured) ? configured
    : "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge");
const browser = await puppeteer.launch({ executablePath, headless: true });
try {
  const page = await browser.newPage();
  const entries = [];
  for (const entry of matrix.entries) {
    const source = readFileSync(join(root, "fixtures/flowchart", `${entry.fixture}.mmd`));
    const svg = readFileSync(join(root, "fixtures/upstream-svgs/flowchart", `${entry.fixture}.svg`));
    assert.equal(sha256(source), entry.input_sha256);
    assert.equal(sha256(svg), entry.upstream_svg_sha256);
    if (!svg.includes(Buffer.from("text-inner-tspan"))) continue;
    await page.setContent(svg.toString());
    await page.addScriptTag({ path: require.resolve("dompurify/dist/purify.min.js") });
    const faces = await page.evaluate(async (nodes) => {
      await document.fonts.ready;
      const svg = document.querySelector("svg");
      // Keep one SVG user unit per CSS pixel; responsive viewBox scaling changes hinting.
      svg.removeAttribute("viewBox");
      svg.setAttribute("width", "2000");
      svg.setAttribute("height", "2000");
      svg.style.cssText = "width:2000px;height:2000px;max-width:none";
      const faces = [];
      for (const word of [...svg.querySelectorAll(".text-inner-tspan")]) {
        const css = getComputedStyle(word);
        const style = {
          font_family: css.fontFamily,
          font_size: parseFloat(css.fontSize),
          font_weight: css.fontWeight,
          font_style: css.fontStyle,
        };
        let face = faces.find((face) => JSON.stringify(face.style) === JSON.stringify(style));
        if (!face) faces.push(face = { style, texts: [] });
        const originalText = word.closest("text");
        const row = word.parentElement;
        // Both positions are real literal probes, never a height inferred from a label.
        const bare = word.textContent.replace(/^ /, "");
        const node = originalText.closest("g.node");
        const source = node ? nodes.find((entry) =>
          node.id.includes(`-flowchart-${entry.id}-`))?.label_box?.text : undefined;
        const rows = [...originalText.children].map((row) => row.textContent);
        const texts = [bare, ` ${bare}`];
        if (css.fontWeight === getComputedStyle(originalText).fontWeight) {
          // Label placement also asks operation 14 for complete source/plain payloads.
          texts.push(...rows, rows.join("\n"), rows.join(" "));
          if (source !== undefined) {
            // Pinned labelHelper passes sanitizeText's source to createText before placement.
            // Capture that exact spelling separately; terminal words still use textContent.
            texts.push(source, DOMPurify.sanitize(source, { FORBID_TAGS: ["style"] }));
          }
        }
        for (const text of texts) {
          if (face.texts.some((word) => word.text === text)) continue;
          const probe = document.createElementNS(svg.namespaceURI, "text");
          probe.style.fontFamily = style.font_family;
          probe.style.fontSize = `${style.font_size}px`;
          probe.style.fontWeight = style.font_weight;
          probe.style.fontStyle = style.font_style;
          const tspan = word.cloneNode(false);
          tspan.textContent = text;
          probe.append(tspan);
          originalText.parentElement.append(probe);
          const tspan_bbox_height = probe.getBBox().height;
          probe.remove();

          // Pinned createText.ts:createFormattedText/createTspan, at retained row zero.
          const formatted = originalText.cloneNode(false);
          const outer = row.cloneNode(false);
          outer.setAttribute("y", "-0.1em");
          outer.setAttribute("dy", "1.1em");
          outer.append(tspan);
          formatted.append(outer);
          originalText.parentElement.append(formatted);
          const create_text_bbox_y = formatted.getBBox().y;
          formatted.remove();
          face.texts.push({ text, tspan_bbox_height, create_text_bbox_y });
        }
      }
      return faces;
    }, entry.nodes);
    entries.push({
      fixture: entry.fixture,
      input_sha256: entry.input_sha256,
      upstream_svg_sha256: entry.upstream_svg_sha256,
      faces,
    });
  }
  const result = {
    schema_version: 1,
    mermaid_version: matrix.mermaid_version,
    mermaid_source_commit: matrix.mermaid_source_commit,
    browser: await browser.version(),
    measurement_source: "Literal single-tspan height and row-zero createFormattedText bbox y, "
      + "independently measured from pinned SVG word DOM with its computed typography. "
      + "Both row-start and leading-space forms are probed with textContent. "
      + "Complete raw and DOMPurify-sanitized source/plain payloads used by label placement "
      + "are also independently probed with textContent, never parsed as terminal markup. "
      + "Responsive viewBox scaling is removed; no fonts are injected.",
    entries,
  };
  if (process.argv.includes("--write")) {
    writeFileSync(target, `${JSON.stringify(result, null, 2)}\n`);
  } else {
    const committed = JSON.parse(readFileSync(target, "utf8"));
    assert.deepEqual(result, committed);
  }
  console.log(`${entries.length} fixtures; `
    + `${entries.flatMap((entry) => entry.faces.flatMap((face) => face.texts)).length} literal probes; `
    + `${result.browser}; ${process.argv.includes("--write") ? "captured" : "verified"}`);
} finally {
  await browser.close();
}
