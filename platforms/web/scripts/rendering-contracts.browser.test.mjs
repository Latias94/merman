import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import test from "node:test";

// Requires compiled TypeScript and a locally built full browser WASM artifact.
test("browser Class bounds and scoped icon services use the actual WASM renderer", {
  skip: !process.env.PUPPETEER_EXECUTABLE_PATH && "Set PUPPETEER_EXECUTABLE_PATH to run Chromium",
  timeout: 60_000,
}, async () => {
  const require = createRequire(new URL("../../../tools/mermaid-cli/package.json", import.meta.url));
  const puppeteer = require(process.env.MERMAN_PUPPETEER_MODULE ?? "puppeteer");
  const server = createServer(async (request, response) => {
    try {
      if (request.url === "/") {
        response.setHeader("Content-Type", "text/html");
        response.end("<!doctype html><title>Rendering contracts</title><main></main>");
      } else {
        const file = new URL(`..${request.url}`, import.meta.url);
        const source = await readFile(file);
        response.setHeader("Content-Type", file.pathname.endsWith(".wasm") ? "application/wasm" : "text/javascript");
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
    page.on("pageerror", error => console.error(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const result = await page.evaluate(async () => {
      const core = await import("/dist/runtime-core.js");
      const { createBrowserTextMeasurementSession } = await import("/dist/runtime-render.js");
      const { createRenderSession } = await import("/dist/runtime-render-session.js");
      const { BINDING_OPTIONS_SCHEMA_VERSION: version } = await import("/dist/generated/resource-contract.js");
      await core.initMerman(() => import("/pkg/full/merman_wasm.js"));
      await document.fonts.ready;
      const measurements = createBrowserTextMeasurementSession();
      const pack = path => JSON.stringify({
        prefix: "original", width: 16, height: 16,
        icons: { marker: { body: `<path d="${path}"/>` } },
      });
      const a = createRenderSession({ iconPacks: [{ json: pack("M1 1h14v14H1z"), registrationName: "fleet" }] });
      const b = createRenderSession({ iconPacks: [{ json: pack("M2 2h12v12H2z"), registrationName: "fleet" }] });
      const empty = createRenderSession();
      const iconSource = 'flowchart TD\nA@{ icon: "fleet:marker", label: "A" }';
      const first = a.renderSvg(iconSource);
      const second = b.renderSvg(iconSource);
      const absent = empty.renderSvg(iconSource);
      const ordinary = core.getMerman().renderSvg(iconSource);
      const classSource = `classDiagram
class Vehicle {
  +String licensePlate
  +void getInfo()
}
class Driver {
  +void drive(Vehicle v)
}
Driver --> Vehicle : drives`;
      const bounds = [];
      const rows = [];
      for (const family of ["monospace", "serif", "default"]) {
        const svg = a.renderSvgWithTextMeasurer(classSource, measurements.measure, {
          version,
          site_config: family === "default" ? { htmlLabels: true } : {
            theme: "default", look: "classic", htmlLabels: true, fontFamily: "sans-serif",
            themeVariables: { fontFamily: family, fontSize: "16px" },
          },
          svg: { diagram_id: `class-${family}` },
        });
        const mount = document.createElement("section");
        mount.innerHTML = svg;
        document.querySelector("main").append(mount);
        for (const fo of mount.querySelectorAll("foreignObject")) {
          const text = fo.textContent.trim();
          if (!text) continue;
          const range = document.createRange();
          range.selectNodeContents(fo);
          const actual = range.getBoundingClientRect();
          const container = fo.getBoundingClientRect();
          rows.push({ family, text, width: actual.width, height: actual.height,
            containerWidth: container.width, containerHeight: container.height });
        }
        for (const title of ["Vehicle", "Driver"]) {
          const fo = [...mount.querySelectorAll("foreignObject")].find(node => node.textContent.trim() === title);
          if (!fo) throw new Error(`Missing ${title} foreignObject`);
          const range = document.createRange();
          range.selectNodeContents(fo);
          bounds.push({
            family, title,
            container: fo.getBoundingClientRect().width,
            text: range.getBoundingClientRect().width,
          });
        }
      }
      let invalid;
      try {
        createRenderSession({ iconPacks: [{ json: '{"prefix":"bad","icons":{"x":{"body":"<path>"}}}' }] });
      } catch (error) {
        invalid = error;
      }
      let reentry;
      a.renderSvgWithTextMeasurer(classSource, request => {
        if (!reentry) {
          try { a.layoutJson(classSource); } catch (error) { reentry = error; }
        }
        return measurements.measure(request);
      });
      b.renderSvgWithTextMeasurer(classSource, request => {
        b.dispose();
        return measurements.measure(request);
      });
      let disposed = false;
      try { b.renderSvg(iconSource); } catch { disposed = true; }
      const remains = a.renderSvg(iconSource);
      const catalog = core.runtimeCatalog();
      a.dispose();
      empty.dispose();
      measurements.dispose();
      return { first, second, absent, ordinary, remains, bounds, rows, invalid, reentry, disposed, services: catalog.constructor_service_ids };
    });
    assert.match(result.first, /M1 1h14v14H1z/);
    assert.match(result.second, /M2 2h12v12H2z/);
    assert.doesNotMatch(result.first, /M2 2h12v12H2z/);
    assert.doesNotMatch(result.absent, /M[12] [12]h1[24]v1[24]H[12]z/);
    assert.doesNotMatch(result.ordinary, /M[12] [12]h1[24]v1[24]H[12]z/);
    assert.match(result.remains, /M1 1h14v14H1z/);
    assert.equal(result.invalid?.details?.icon_registry?.kind_id, "invalid_xml");
    assert.equal(result.reentry?.kind, "reentrant-call");
    assert.equal(result.disposed, true);
    assert.ok(result.services.includes("icon-registry"));
    assert.equal(result.bounds.length, 6);
    assert.ok(result.rows.some(row => row.text.includes("licensePlate")));
    assert.ok(result.rows.some(row => row.text.includes("drive(")));
    for (const row of result.rows) {
      assert.ok(row.width <= row.containerWidth + 0.1, `Clipped row width: ${JSON.stringify(row)}`);
      assert.ok(row.height <= row.containerHeight + 0.1, `Clipped row height: ${JSON.stringify(row)}`);
    }
    for (const bound of result.bounds) {
      assert.ok(bound.text > 0, JSON.stringify(bound));
      assert.ok(bound.text <= bound.container + 0.1, `Clipped title: ${JSON.stringify(bound)}`);
    }
    console.log(JSON.stringify({ bounds: result.bounds, services: result.services }));
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
});
