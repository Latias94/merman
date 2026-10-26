import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import test from "node:test";

// Run after compiling TypeScript, with PUPPETEER_EXECUTABLE_PATH set to Chromium.
// MERMAN_PUPPETEER_MODULE may point to an existing Puppeteer installation.
test("Canvas font assignment and state isolation in Chromium", {
  skip: !process.env.PUPPETEER_EXECUTABLE_PATH && "Set PUPPETEER_EXECUTABLE_PATH to run Chromium",
  timeout: 30_000,
}, async () => {
  const require = createRequire(new URL("../../../tools/mermaid-cli/package.json", import.meta.url));
  const puppeteer = require(process.env.MERMAN_PUPPETEER_MODULE ?? "puppeteer");
  const server = createServer(async (request, response) => {
    try {
      if (request.url === "/") {
        response.setHeader("Content-Type", "text/html");
        response.end("<!doctype html><title>Canvas measurement</title>");
      } else {
        const source = await readFile(new URL(`..${request.url}`, import.meta.url));
        response.setHeader("Content-Type", "text/javascript");
        response.end(source);
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
      const control = document.createElement("canvas").getContext("2d");
      const getContext = HTMLCanvasElement.prototype.getContext;
      let context;
      let canvasCreates = 0;
      let saves = 0;
      let restores = 0;
      const measuredFonts = [];
      HTMLCanvasElement.prototype.getContext = function (...args) {
        context = getContext.apply(this, args);
        canvasCreates += 1;
        const save = context.save;
        const restore = context.restore;
        const measureText = context.measureText;
        context.save = function () { saves += 1; return save.call(this); };
        context.restore = function () { restores += 1; return restore.call(this); };
        context.measureText = function (text) {
          measuredFonts.push(this.font);
          return measureText.call(this, text);
        };
        return context;
      };
      const session = createBrowserTextMeasurementSession();
      const base = {
        operation: "canvas-measure-text-width",
        phase: "layout",
        text: "Vehicle",
        font_style: "normal",
        font_weight: "normal",
        font_size: 16,
        font_family: "sans-serif",
        max_width: null,
        has_max_width: false,
        line_height: 24,
        letter_spacing: 0,
        word_spacing: 0,
        wrap_mode: "svg-like",
        direction: "ltr",
        white_space: "normal",
      };
      const cases = [
        { font_family: "sans-serif;", invalid: true },
        { font_weight: "bolder", font_size: 14 },
        { font_weight: "bolder", font_size: 16 },
        { font_family: "sans-serif;", invalid: true },
        { font_weight: "lighter" },
        { font_family: '"A; B, C", sans-serif', font_size: 18 },
        { font_family: '"A\\"; B", serif', font_size: 18 },
        { font_family: "sans-serif", font_size: 10 },
        { font_family: "serif", font_size: 12 },
        { font_weight: "not-a-weight", invalid: true },
        { font_family: "", invalid: true },
        { font_style: "italic", font_size: 24, font_family: "monospace" },
      ];
      try {
        const measurements = cases.map(({ invalid = false, ...font }, index) => {
          const request = { ...base, ...font };
          if (index > 0) context.font = "italic bold 31px serif";
          const beforeFont = context?.font ?? "10px sans-serif";
          const callsBefore = measuredFonts.length;
          control.font = "10px sans-serif";
          control.font = `${request.font_style} ${request.font_weight} ${request.font_size}px ${request.font_family}`;
          const result = session.measure(request);
          return {
            invalid,
            result,
            expectedWidth: control.measureText(request.text).width,
            expectedFont: control.font,
            measuredFont: measuredFonts.at(-1),
            measuredCalls: measuredFonts.length - callsBefore,
            beforeFont,
            afterFont: context.font,
          };
        });
        context.save();
        context.font = "italic 27px serif";
        const nestedFont = context.font;
        session.measure(base);
        const nestedAfter = context.font;
        context.restore();
        const restoredFont = context.font;
        session.dispose();
        return {
          measurements,
          nestedFont,
          nestedAfter,
          restoredFont,
          canvasCreates,
          saves,
          restores,
          remainingProbes: document.querySelectorAll("[data-merman-text-measure-probe]").length,
          disposedResult: session.measure(base),
        };
      } finally {
        session.dispose();
        HTMLCanvasElement.prototype.getContext = getContext;
      }
    });
    for (const entry of evidence.measurements) {
      assert.equal(entry.afterFont, entry.beforeFont);
      if (entry.invalid) {
        assert.equal(entry.expectedFont, "10px sans-serif", "the control must reject the font");
        assert.equal(entry.result, undefined);
        assert.equal(entry.measuredCalls, 0);
      } else {
        assert.deepEqual(entry.result, { kind: "length", length: entry.expectedWidth });
        assert.equal(entry.measuredFont, entry.expectedFont);
        assert.equal(entry.measuredCalls, 1);
      }
    }
    assert.notEqual(evidence.measurements[1].result.length, evidence.measurements[2].result.length);
    assert.equal(evidence.nestedAfter, evidence.nestedFont);
    assert.equal(evidence.restoredFont, "italic bold 31px serif");
    assert.equal(evidence.canvasCreates, 1);
    assert.equal(evidence.saves, evidence.restores);
    assert.equal(evidence.remainingProbes, 0);
    assert.equal(evidence.disposedResult, undefined);
  } finally {
    await browser?.close();
    await new Promise((resolve) => server.close(resolve));
  }
});
