import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import test from "node:test";

// Run after compiling TypeScript, using the existing Puppeteer/Chromium installation.
test("DOM probes match native typography, reject invalid CSS and recover after detachment in Chromium", {
  skip: !process.env.PUPPETEER_EXECUTABLE_PATH && "Set PUPPETEER_EXECUTABLE_PATH to run Chromium",
  timeout: 30_000,
}, async () => {
  const require = createRequire(new URL("../../../tools/mermaid-cli/package.json", import.meta.url));
  const puppeteer = require(process.env.MERMAN_PUPPETEER_MODULE ?? "puppeteer");
  const server = createServer(async (request, response) => {
    try {
      if (request.url === "/") {
        response.setHeader("Content-Type", "text/html");
        response.end("<!doctype html><title>DOM text measurement</title>");
      } else if (request.url.startsWith("/dist/")) {
        const source = await readFile(new URL(`..${request.url}`, import.meta.url));
        response.setHeader("Content-Type", "text/javascript");
        response.end(source);
      } else {
        response.writeHead(404).end();
      }
    } catch {
      response.writeHead(404).end();
    }
  });
  let browser;
  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    browser = await puppeteer.launch({
      executablePath: process.env.PUPPETEER_EXECUTABLE_PATH,
      headless: true,
    });
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const evidence = await page.evaluate(async () => {
      const { createBrowserTextMeasurementSession } = await import("/dist/runtime-render.js");
      await document.fonts.ready;
      const base = {
        phase: "layout",
        text: "iiiiMMMMWWWW",
        font_family: "monospace",
        font_size: 20,
        font_weight: "normal",
        font_style: "normal",
        max_width: null,
        has_max_width: false,
        line_height: 30,
        letter_spacing: 0,
        word_spacing: 0,
        wrap_mode: "svg-like",
        direction: "auto",
        white_space: "normal",
      };
      const paths = [
        { operation: "raw-bbox-width" },
        { operation: "computed-length" },
        { operation: "wrapped" },
        { operation: "wrapped", wrap_mode: "html-like" },
        { operation: "create-text-bbox-y-offset" },
        { operation: "mermaid-calculate-text-dimensions" },
      ];
      const secondFont = {
        font_family: '"A; B, C",    serif',
        font_weight: " 700 ",
        font_style: " oblique 10deg ",
        font_size: 28,
      };
      const invalidValues = [
        { font_family: "sans-serif; font-size: 1px" },
        { font_family: "" },
        { font_weight: "not-a-weight" },
        { font_weight: "" },
        { font_style: "not-a-style" },
        { font_style: "" },
        { font_size: NaN },
        { font_size: Infinity },
        { letter_spacing: NaN },
        { word_spacing: Infinity },
      ];
      const freshMeasurement = (request) => {
        const fresh = createBrowserTextMeasurementSession();
        try { return fresh.measure(request); }
        finally { fresh.dispose(); }
      };
      const session = createBrowserTextMeasurementSession();
      try {
        const nativeComparisons = [];
        const nativeSvg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        const nativeText = document.createElementNS("http://www.w3.org/2000/svg", "text");
        const nativeDiv = document.createElement("div");
        nativeSvg.setAttribute("width", "0");
        nativeSvg.setAttribute("height", "0");
        nativeSvg.style.cssText = "position:fixed;left:-10000px;top:-10000px;visibility:hidden;overflow:visible";
        nativeText.setAttribute("x", "0");
        nativeText.setAttribute("y", "0");
        nativeText.setAttribute("text-anchor", "start");
        nativeSvg.append(nativeText);
        nativeDiv.style.cssText = "position:fixed;left:-10000px;top:-10000px;visibility:hidden;display:inline-block;box-sizing:border-box;padding:0;margin:0;border:0;white-space:nowrap";
        nativeDiv.dir = "auto";
        document.body.append(nativeSvg, nativeDiv);
        const computedFont = element => {
          const style = getComputedStyle(element);
          return Object.fromEntries(["fontFamily", "fontSize", "fontWeight", "fontStyle"]
            .map(property => [property, style[property]]));
        };
        try {
          for (const html of [false, true]) {
            for (const field of ["font_family", "font_weight", "font_style"]) {
              // Change one field at a time and restore the base font between cases.
              for (const value of [base[field], secondFont[field], base[field]]) {
                const input = {
                  ...base,
                  operation: html ? "wrapped" : "raw-bbox-width",
                  wrap_mode: html ? "html-like" : "svg-like",
                  [field]: value,
                };
                const control = html ? nativeDiv : nativeText;
                control.style.setProperty("font-family", input.font_family);
                control.style.setProperty("font-size", `${input.font_size}px`);
                control.style.setProperty("font-weight", input.font_weight);
                control.style.setProperty("font-style", input.font_style);
                control.style.setProperty("line-height", `${input.line_height}px`);
                control.style.setProperty("letter-spacing", `${input.letter_spacing}px`);
                control.style.setProperty("word-spacing", `${input.word_spacing}px`);
                if (!html) control.style.setProperty("white-space", input.white_space);
                control.textContent = input.text;
                const bounds = html ? control.getBoundingClientRect() : control.getBBox();
                const result = session.measure(input);
                const probe = html
                  ? document.querySelector('[data-merman-text-measure-probe="html"]')
                  : document.querySelector('[data-merman-text-measure-probe="svg"] text');
                nativeComparisons.push({
                  html, field, value, result,
                  actualFont: computedFont(probe),
                  expectedFont: computedFont(control),
                  expected: html
                    ? { kind: "metrics", width: bounds.width, height: bounds.height, line_count: 1 }
                    : { kind: "length", length: bounds.width },
                });
              }
            }
          }
        } finally {
          nativeSvg.remove();
          nativeDiv.remove();
        }
        const assignments = paths.map(path => {
          const first = { ...base, ...path };
          const second = { ...first, ...secondFont };
          const expectedFirst = freshMeasurement(first);
          const expectedSecond = freshMeasurement(second);
          // Mermaid's dimensions operation intentionally consumes only these three fields.
          const invalid = path.operation === "mermaid-calculate-text-dimensions"
            ? invalidValues.filter(value => ["font_family", "font_size", "font_weight"].includes(Object.keys(value)[0]))
            : [...invalidValues, ...(path.wrap_mode === "html-like" ? [{ line_height: Infinity }] : [])];
          return {
            path,
            expectedFirst,
            expectedSecond,
            sequences: invalid.map(value => ({
              field: Object.keys(value)[0],
              first: session.measure(first),
              rejected: session.measure({ ...first, ...value }),
              second: session.measure(second),
            })),
          };
        });
        const selector = "[data-merman-text-measure-probe]";
        const inputs = paths.map(path => ({ ...base, ...path }));
        const expected = inputs.map(input => session.measure(input));
        const detachments = ["html", "svg", "mermaid-dimensions", "all"].map(marker => {
          const retired = [...document.querySelectorAll(selector)];
          for (const probe of retired) {
            if (marker === "all" || probe.getAttribute("data-merman-text-measure-probe") === marker) {
              probe.remove();
            }
          }
          return {
            marker,
            results: inputs.map(input => session.measure(input)),
            retiredDisconnected: retired.every(probe => !probe.isConnected),
            attachedCount: document.querySelectorAll(selector).length,
          };
        });
        session.dispose();
        return {
          nativeComparisons,
          assignments,
          detachments,
          expected,
          remainingProbes: document.querySelectorAll(selector).length,
          disposedResult: session.measure(inputs[0]),
        };
      } finally {
        session.dispose();
      }
    });
    assert.equal(evidence.nativeComparisons.length, 18);
    for (const { html, field, value, result, actualFont, expectedFont, expected } of evidence.nativeComparisons) {
      const label = `${html ? "HTML" : "SVG"} ${field}=${JSON.stringify(value)}`;
      assert.deepEqual(actualFont, expectedFont, `${label}: computed font must match the native control`);
      assert.deepEqual(result, expected, `${label}: geometry must match the native control`);
    }
    for (const { path, expectedFirst, expectedSecond, sequences } of evidence.assignments) {
      const label = `${path.operation}/${path.wrap_mode ?? "svg-like"}`;
      assert.ok(expectedFirst, `${label}: first font must be measured`);
      assert.ok(expectedSecond, `${label}: canonicalized second font must be measured`);
      for (const sequence of sequences) {
        assert.deepEqual(sequence.first, expectedFirst, `${label}: first font before ${sequence.field}`);
        assert.equal(sequence.rejected, undefined, `${label}: reject ${sequence.field} without stale metrics`);
        assert.deepEqual(sequence.second, expectedSecond, `${label}: recover after ${sequence.field}`);
      }
    }
    assert.ok(evidence.expected.every(Boolean));
    for (const entry of evidence.detachments) {
      assert.deepEqual(entry.results, evidence.expected, `recover after removing ${entry.marker}`);
      assert.equal(entry.retiredDisconnected, true, `release old ${entry.marker} probes`);
      assert.equal(entry.attachedCount, 3, "reconstruction must not leak sibling probes");
    }
    assert.equal(evidence.remainingProbes, 0);
    assert.equal(evidence.disposedResult, undefined);
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
});
