import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { expect, test } from "@playwright/test";

import { openPlayground } from "./helpers/playground";
import { fullWebPackageProvenance, verifyAssembledFullWebPackage } from "./helpers/web-package.ts";

const packageRoot = path.resolve(import.meta.dirname, "../../platforms/web/packages/full");
const source = "flowchart LR\nA[Start] -->|Approved| B[Done]";
const layoutIds = ["grid", "layered"] as const;

test("Playground loads the selected Web WASM and admits only its layout providers", async ({ page }) => {
  await verifyAssembledFullWebPackage("local layout browser smoke");
  const provenance = await fullWebPackageProvenance();
  const response = await openPlayground(page);
  const assembledWasm = await readFile(path.join(packageRoot, "artifacts/wasm/merman_wasm_bg.wasm"));
  expect(response.ok()).toBe(true);
  // The WASM can exceed Chromium's inspector response cache limit.
  const servedWasm = await page.request.get(response.url());
  expect(servedWasm.ok()).toBe(true);
  expect(sha256(await servedWasm.body())).toBe(sha256(assembledWasm));
  await servedWasm.dispose();

  await page.route("**/__layout_profile/**", async (route) => {
    const relative = new URL(route.request().url()).pathname.split("/__layout_profile/")[1];
    if (relative === "") {
      await route.fulfill({ contentType: "text/html", body: "<!doctype html><body></body>" });
      return;
    }
    const file = path.resolve(packageRoot, relative);
    expect(file.startsWith(packageRoot + path.sep)).toBe(true);
    await route.fulfill({
      contentType: file.endsWith(".wasm") ? "application/wasm" : "text/javascript",
      body: await readFile(file),
    });
  });
  await page.goto("./__layout_profile/");

  const result = await page.evaluate(async ({ source, layoutIds }) => {
    const moduleUrl = new URL("dist/package-entries/full.js", location.href).href;
    const api: typeof import("../../platforms/web/packages/full/dist/package-entries/full.js") = await import(moduleUrl);
    await api.initMerman();
    const measurement = api.createBrowserTextMeasurementSession();
    const probe = (operation: () => unknown) => {
      try {
        return { value: operation() };
      } catch (error) {
        if (!api.isBindingErrorPayload(error)) throw error;
        return { error: { code: error.code_name, kind: error.kind, capability: error.capability_id } };
      }
    };
    try {
      return {
        capabilities: api.runtimeCatalog().capabilities.capability_ids,
        rows: layoutIds.flatMap((layout) => [
          { input: `---\nconfig:\n  layout: ${layout}\n---\n${source}`, options: undefined },
          { input: source, options: { site_config: { layout } } },
        ].map(({ input, options }) => ({
          layout,
          plan: probe(() => api.svgPlanJson(input, options)),
          geometry: probe(() => Boolean(JSON.parse(api.layoutJson(input, options)).layout?.FlowchartV2)),
          svg: probe(() =>
            new DOMParser().parseFromString(api.renderSvgWithTextMeasurer(input, measurement.measure, options), "image/svg+xml")
              .documentElement.localName,
          ),
        }))),
      };
    } finally {
      measurement.dispose();
    }
  }, { source, layoutIds });

  expect(result.capabilities).toEqual(provenance.runtime_capability_ids);
  for (const row of result.rows) {
    const capability = `layout-${row.layout}`;
    if (result.capabilities.includes(capability)) {
      expect(row.plan).toEqual({ value: expect.objectContaining({
        diagram_type: "flowchart-v2", ready: true,
        required_capability_ids: [capability], missing_capability_ids: [],
      }) });
      expect(row.geometry).toEqual({ value: true });
      expect(row.svg).toEqual({ value: "svg" });
    } else {
      const refused = { error: { code: "MERMAN_UNSUPPORTED_OPERATION", kind: "generic", capability: null } };
      expect(row.plan).toEqual(refused);
      expect(row.geometry).toEqual(refused);
      expect(row.svg).toEqual(refused);
    }
  }
});

function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}
