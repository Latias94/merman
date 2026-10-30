import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import test from "node:test";

// Run after build:ts with a locally built full WASM artifact in pkg/full.
test("Browser text consumers retain legacy ER wrapped metrics, literal SVG text and subpixel Pie legends", {
  skip: !process.env.PUPPETEER_EXECUTABLE_PATH && "Set PUPPETEER_EXECUTABLE_PATH to run Chromium",
  timeout: 30_000,
}, async () => {
  const require = createRequire(new URL("../../../tools/mermaid-cli/package.json", import.meta.url));
  const puppeteer = require(process.env.MERMAN_PUPPETEER_MODULE ?? "puppeteer");
  const server = createServer(async (request, response) => {
    try {
      if (request.url === "/") {
        response.setHeader("Content-Type", "text/html");
        response.end("<!doctype html><title>Pie text measurement</title><main></main>");
      } else if (request.url.startsWith("/dist/") || request.url.startsWith("/pkg/full/")) {
        const file = new URL(`..${request.url}`, import.meta.url);
        response.setHeader("Content-Type", file.pathname.endsWith(".wasm") ? "application/wasm" : "text/javascript");
        response.end(await readFile(file));
      } else response.writeHead(404).end();
    } catch { response.writeHead(404).end(); }
  });
  let browser;
  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    browser = await puppeteer.launch({ executablePath: process.env.PUPPETEER_EXECUTABLE_PATH, headless: true });
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const evidence = await page.evaluate(async () => {
      const { initMerman } = await import("/dist/runtime-core.js");
      const { createBrowserTextMeasurementSession, renderSvgWithTextMeasurer } = await import("/dist/runtime-render.js");
      const { BINDING_OPTIONS_SCHEMA_VERSION: version } = await import("/dist/generated/resource-contract.js");
      await initMerman({
        loader: () => import("/pkg/full/merman_wasm.js"),
        wasm: new URL("/pkg/full/merman_wasm_bg.wasm", location.href),
      });
      await document.fonts.ready;
      const session = createBrowserTextMeasurementSession();
      const ns = "http://www.w3.org/2000/svg";
      const controlSvg = document.createElementNS(ns, "svg");
      controlSvg.style.cssText = "position:fixed;left:-10000px;top:-10000px;visibility:hidden";
      document.body.append(controlSvg);
      // This callback independently implements Pie's direct-text client-rect primitive.
      const nativeWidth = request => {
        if (request.operation !== "bounding-client-rect-width") return undefined;
        const text = document.createElementNS(ns, "text");
        text.style.fontFamily = request.font_family;
        text.style.fontSize = `${request.font_size}px`;
        text.style.fontWeight = request.font_weight;
        text.style.fontStyle = request.font_style;
        text.textContent = request.text;
        controlSvg.append(text);
        try { return { kind: "length", length: text.getBoundingClientRect().width }; }
        finally { text.remove(); }
      };
      // Protocol 1 Wrapped treats explicit breaks as rows, including decoded ER generic text.
      // This is the legacy metrics contract; the final ER terminal still emits literal text.
      const nativeWrappedMetrics = request => {
        const control = document.createElement("div");
        control.style.cssText = "position:fixed;left:-10000px;top:-10000px;visibility:hidden;display:inline-block;padding:0;margin:0;border:0;white-space:nowrap";
        Object.assign(control.style, {
          fontFamily: request.font_family,
          fontSize: `${request.font_size}px`,
          fontWeight: request.font_weight,
          fontStyle: request.font_style,
          lineHeight: `${request.line_height}px`,
          letterSpacing: `${request.letter_spacing}px`,
          wordSpacing: `${request.word_spacing}px`,
        });
        control.dir = request.direction;
        for (const [index, line] of request.text.split(/(?:<br\s*\/?>|\r?\n)/gi).entries()) {
          if (index > 0) control.append(document.createElement("br"));
          control.append(document.createTextNode(line));
        }
        document.body.append(control);
        try {
          const rect = control.getBoundingClientRect();
          return { kind: "metrics", width: rect.width, height: rect.height,
            line_count: Math.max(1, Math.round(rect.height / request.line_height)) };
        } finally { control.remove(); }
      };
      try {
        const er = [];
        for (const attributes of [false, true]) {
          const source = `erDiagram\n"A~br~B"${attributes ? " {\n string id\n}" : ""}`;
          const requests = [];
          const svgText = renderSvgWithTextMeasurer(source, request => {
            const result = session.measure(request);
            if (request.operation === "wrapped" && request.wrap_mode === "html-like" && !request.has_max_width) {
              requests.push({ text: request.text, phase: request.phase, result,
                expected: nativeWrappedMetrics(request) });
            }
            return result;
          }, { version, site_config: { htmlLabels: true, look: "classic", theme: "default",
            fontFamily: "Arial", themeVariables: { fontFamily: "Arial", fontSize: "16px" } },
            svg: { diagram_id: `er-literal-${attributes}` } });
          const mount = document.createElement("section");
          mount.innerHTML = svgText;
          document.querySelector("main").append(mount);
          const expectedText = attributes ? "A<br>B" : "A~br~B";
          const terminal = [...mount.querySelectorAll("foreignObject")]
            .find(node => node.textContent === expectedText);
          if (!terminal) throw new Error(`Missing literal ER name terminal: ${JSON.stringify({ attributes, labels: [...mount.querySelectorAll("foreignObject")].map(node => node.textContent), expectedText })}`);
          const control = document.createElement("div");
          control.style.cssText = "display:inline-block;padding:0;margin:0;border:0;white-space:nowrap;font:16px Arial;line-height:24px";
          control.textContent = expectedText;
          document.body.append(control);
          er.push({ attributes, requests, text: terminal.textContent, expectedText,
            height: Number(terminal.getAttribute("height")),
            literalHeight: control.getBoundingClientRect().height,
            breaks: terminal.querySelectorAll("br").length });
          control.remove(); mount.remove();
        }
        const label = "M".repeat(128);
        const source = `pie\n"${label}": 1`;
        const results = [];
        for (const size of [0.5, 1, 0]) {
          const options = {
            version,
            site_config: { theme: "default", look: "classic", fontFamily: "monospace", themeVariables: {
              fontFamily: "monospace", pieLegendTextSize: `${size}px`,
            } },
            svg: { diagram_id: "pie-host-operation" },
          };
          const requests = [];
          const actual = renderSvgWithTextMeasurer(source, request => {
            const result = session.measure(request);
            if (request.operation === "bounding-client-rect-width") {
              requests.push({ text: request.text, font_size: request.font_size, result, expected: nativeWidth(request) });
            }
            return result;
          }, options);
          const expected = renderSvgWithTextMeasurer(source, nativeWidth, options);
          const mount = document.createElement("section");
          document.querySelector("main").append(mount);
          mount.innerHTML = actual;
          const svg = mount.querySelector("svg");
          if (!svg) throw new Error("Missing Pie SVG");
          const viewBoxWidth = svg.viewBox.baseVal.width;
          const viewBoxHeight = svg.viewBox.baseVal.height;
          // Keep user units at exactly one CSS pixel; fractional responsive scaling changes font hinting.
          svg.removeAttribute("viewBox");
          svg.style.width = `${Math.ceil(viewBoxWidth)}px`;
          svg.style.height = `${Math.ceil(viewBoxHeight)}px`;
          svg.style.maxWidth = "none";
          const legend = [...svg.querySelectorAll("text")].find(text => text.textContent === label);
          if (!legend) throw new Error("Missing Pie legend");
          results.push({ size, requests, identicalSvg: actual === expected,
            viewBoxWidth,
            legendFontSize: getComputedStyle(legend).fontSize,
            legendWidth: legend.getBoundingClientRect().width,
          });
          mount.remove();
        }
        return { pie: results, er };
      } finally {
        session.dispose();
        controlSvg.remove();
      }
    });
    console.log(JSON.stringify({ browser: await browser.version(), ...evidence }));
    for (const entry of evidence.er) {
      assert.equal(entry.breaks, 0);
      assert.equal(entry.text, entry.expectedText, "ER SVG terminals preserve literal text");
      const request = entry.requests.find(request => request.text === entry.expectedText);
      assert.ok(request, "ER name must reach the legacy HTML Wrapped operation");
      assert.equal(request.phase, "wrap");
      assert.deepEqual(request.result, request.expected, "Wrapped returns complete legacy metrics");
      assert.equal(request.result.line_count, entry.attributes ? 2 : 1);
      assert.equal(entry.height, request.expected.height, "ER retains the host's Wrapped height");
      assert.equal(entry.height, entry.literalHeight * request.result.line_count,
        "Legacy Wrapped geometry can reserve multiple rows for one literal ER terminal");
    }
    for (const entry of evidence.pie) {
      assert.ok(entry.identicalSvg, `Pie ${entry.size}px must match independently measured SVG`);
      assert.equal(entry.legendFontSize, `${entry.size}px`);
      const legendRequest = entry.requests.find(request => request.text === "M".repeat(128));
      assert.ok(legendRequest, `Pie ${entry.size}px must reach the direct-text host operation`);
      assert.equal(legendRequest.font_size, entry.size);
      assert.deepEqual(legendRequest.result, legendRequest.expected);
      assert.ok(Math.abs(entry.legendWidth - legendRequest.expected.length) < 0.001,
        `Pie ${entry.size}px emitted legend must match the measured display font`);
    }
    assert.ok(evidence.pie[0].viewBoxWidth < evidence.pie[1].viewBoxWidth, "Subpixel font size changes the reachable Pie viewport");
    assert.equal(evidence.pie[2].legendWidth, 0, "Pie retains its explicit zero-size behavior");
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
});
