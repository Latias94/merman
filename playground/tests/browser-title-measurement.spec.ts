import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

import { expect, test } from "@playwright/test";
import type { HostTextMeasureRequest } from "../../platforms/web/packages/full/dist/public-types.js";
import {
  auditMountedSvg,
  classifyRootViewportContainment,
} from "./root-viewport-oracle.ts";

const repositoryRoot = path.resolve(import.meta.dirname, "../..");
const packageRoot = path.join(repositoryRoot, "platforms/web/packages/full");
const fixture = "flowchart/stress_flowchart_title_padding_subgraph_029";

test.beforeAll(async () => {
  await promisify(execFile)(
    process.execPath,
    ["platforms/web/scripts/verify-wasm-inputs.mjs", "--package", "full"],
    { cwd: repositoryRoot },
  );
  // Bind the served package to the verified WASM output and current compiled JS closure.
  const checkerUrl = pathToFileURL(
    path.join(repositoryRoot, "platforms/web/scripts/prepack-check.mjs"),
  ).href;
  const { assertArtifactFileEvidence } = await import(checkerUrl);
  const provenance = JSON.parse(
    await readFile(path.join(packageRoot, "artifacts/provenance.json"), "utf8"),
  );
  assertArtifactFileEvidence({
    packageWasmRoot: path.join(packageRoot, "artifacts/wasm"),
    sourceWasmRoot: path.join(repositoryRoot, "platforms/web/pkg/full"),
    packageDistRoot: path.join(packageRoot, "dist"),
    sourceDistRoot: path.join(repositoryRoot, "platforms/web/dist"),
    packageId: "full",
    artifactFiles: provenance.artifact_files,
    label: "browser title/UTF-16 regression package",
  });
});

test.beforeEach(async ({ page }) => {
  // Serve the assembled public package, including its actual WASM, without a test-only renderer.
  await page.route("**/__title_measurement/**", async (route) => {
    const relative = new URL(route.request().url()).pathname.split(
      "/__title_measurement/",
    )[1];
    if (relative === "") {
      await route.fulfill({
        contentType: "text/html",
        body: "<!doctype html><body></body>",
      });
      return;
    }
    const file = path.resolve(packageRoot, relative);
    expect(file.startsWith(packageRoot + path.sep)).toBe(true);
    await route.fulfill({
      contentType: file.endsWith(".wasm")
        ? "application/wasm"
        : "text/javascript",
      body: await readFile(file),
    });
  });
  await page.goto("./__title_measurement/");
});

for (const fontFamily of [undefined, "monospace"]) {
  const label = fontFamily ?? "configured default";
  test(`public browser measurement includes title bounds with ${label}`, async ({
    page,
  }, testInfo) => {
    const source = await readFile(
      path.join(repositoryRoot, "fixtures", `${fixture}.mmd`),
      "utf8",
    );
    const rendered = await page.evaluate(
      async ({ input, fontFamily }) => {
        const moduleUrl = new URL("dist/package-entries/full.js", location.href)
          .href;
        const api: typeof import("../../platforms/web/packages/full/dist/package-entries/full.js") =
          await import(moduleUrl);
        await api.initMerman();
        const session = api.createBrowserTextMeasurementSession();
        const options = fontFamily
          ? { site_config: { fontFamily } }
          : undefined;
        const requests: HostTextMeasureRequest[] = [];
        const measure: typeof session.measure = (request) => {
          requests.push(request);
          return session.measure(request);
        };
        try {
          // A ready promise before the first probe does not initiate a configured webfont load.
          // Discover the actual requests, load their fonts, then perform the authoritative render.
          api.renderSvgWithTextMeasurer(input, measure, options);
          await Promise.all(
            requests.map((request) =>
              document.fonts.load(
                `${request.font_style ?? "normal"} ${request.font_weight ?? "normal"} ${request.font_size}px ${request.font_family ?? "sans-serif"}`,
                request.text,
              ),
            ),
          );
          await document.fonts.ready;
          requests.length = 0;
          const svg = api.renderSvgWithTextMeasurer(input, measure, options);
          const deterministic = api.renderSvg(input, options);
          const declined = api.renderSvgWithTextMeasurer(
            input,
            () => undefined,
            options,
          );
          const titleRequests = requests.filter(
            (request) => request.operation === "title-bbox-x",
          );
          const host = document.createElement("div");
          document.body.appendChild(host);
          host.innerHTML = svg;
          const root = host.querySelector("svg")!;
          const title = root.querySelector<SVGTextElement>(
            ".flowchartTitleText",
          )!;
          const bbox = title.getBBox();
          const viewBox = root.viewBox.baseVal;
          const titleBounds = {
            left: bbox.x,
            right: bbox.x + bbox.width,
            rootLeft: viewBox.x,
            rootRight: viewBox.x + viewBox.width,
          };
          host.remove();
          return { svg, deterministic, declined, titleRequests, titleBounds };
        } finally {
          session.dispose();
        }
      },
      { input: source, fontFamily },
    );

    expect(rendered.titleRequests.length).toBeGreaterThan(0);
    expect(
      rendered.titleRequests.every(
        (request) =>
          request.text === "Stress flowchart title and padding with subgraphs",
      ),
    ).toBe(true);
    expect(rendered.declined).toBe(rendered.deterministic);
    // SVG root serialization rounds to three decimals; getBBox retains browser precision.
    expect(rendered.titleBounds.left).toBeGreaterThanOrEqual(
      rendered.titleBounds.rootLeft - 0.001,
    );
    expect(rendered.titleBounds.right).toBeLessThanOrEqual(
      rendered.titleBounds.rootRight + 0.001,
    );
    if (fontFamily !== undefined) return;
    const upstreamSvg = await readFile(
      path.join(repositoryRoot, "fixtures/upstream-svgs", `${fixture}.svg`),
      "utf8",
    );
    const local = await auditMountedSvg(page, { svgSource: rendered.svg });
    const upstream = await auditMountedSvg(page, { svgSource: upstreamSvg });
    const classification = classifyRootViewportContainment(local, upstream);
    await testInfo.attach("title-browser-measurement.json", {
      body: JSON.stringify(
        {
          titleRequests: rendered.titleRequests,
          titleBounds: rendered.titleBounds,
          classification,
          local,
          upstream,
        },
        null,
        2,
      ),
      contentType: "application/json",
    });
    expect(classification).not.toBe("blocking");
  });
}

test("public Web preserves distinct UTF-16 JSON keys through semantic and SVG APIs", async ({
  page,
}) => {
  const source = String.raw`usecase-beta
json Data@{"\ud800":"high","\udc00":"low","�":"replacement","\\ud800":"literal"}
`;
  const result = await page.evaluate(async (input) => {
    const moduleUrl = new URL("dist/package-entries/full.js", location.href)
      .href;
    const api: typeof import("../../platforms/web/packages/full/dist/package-entries/full.js") =
      await import(moduleUrl);
    await api.initMerman();
    const parsed = JSON.parse(api.parseJson(input));
    const node = parsed.jsonNodes[0];
    const decoded = Object.entries(node.value).map(([key, value]) => ({
      key: JSON.parse(key) as string,
      value: JSON.parse(value as string) as string,
    }));
    const svg = api.renderSvg(input);
    const document = new DOMParser().parseFromString(svg, "image/svg+xml");
    return {
      encoding: node.stringEncoding,
      encodedKeys: Object.keys(node.value),
      decoded: decoded.map(({ key, value }) => ({
        codeUnits: Array.from({ length: key.length }, (_, index) =>
          key.charCodeAt(index),
        ),
        value,
      })),
      text: document.documentElement.textContent,
      parserErrors: document.querySelectorAll("parsererror").length,
    };
  }, source);
  expect(result.encoding).toBe("json-utf16");
  expect(new Set(result.encodedKeys).size).toBe(4);
  expect(result.decoded).toEqual([
    { codeUnits: [0xd800], value: "high" },
    { codeUnits: [0xdc00], value: "low" },
    { codeUnits: [0xfffd], value: "replacement" },
    { codeUnits: [0x5c, 0x75, 0x64, 0x38, 0x30, 0x30], value: "literal" },
  ]);
  expect(result.parserErrors).toBe(0);
  for (const value of ["high", "low", "replacement", "literal", "�"]) {
    expect(result.text).toContain(value);
  }
});
