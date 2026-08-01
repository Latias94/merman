import path from "node:path";
import { expect, test } from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";

let sourceServer: ViteDevServer | null = null;
let sourceOrigin = "";

test.beforeAll(async () => {
  sourceServer = await createServer({
    root: path.resolve(import.meta.dirname, ".."),
    server: { host: "127.0.0.1", port: 0 },
  });
  await sourceServer.listen();
  const address = sourceServer.httpServer?.address();
  if (!address || typeof address === "string") {
    throw new Error("Vite source server did not expose a TCP address.");
  }
  sourceOrigin = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
  await sourceServer?.close();
  sourceServer = null;
});

test("preview preserves safe HTML-compatible SVG through inert DOM parsing", async ({
  page,
}) => {
  await page.goto(sourceOrigin);
  const rendered = await page.evaluate(async () => {
    const { projectSafeInlineSvg } = await import(
      "/src/runtime/" + "render-artifact.ts"
    );
    const { prepareSvgForResponsivePreview } = await import(
      "/src/lib/" + "svg-geometry.ts"
    );
    const cases = [
      '<svg width="120" height="40"><text>no namespace</text></svg>',
      '<svg xmlns="http://www.w3.org/2000/svg"><text>A & B</text></svg>',
      [
        '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40">',
        '<foreignObject width="120" height="40">',
        '<div><p>HTML label</p></div>',
        '</foreignObject></svg>',
      ].join(""),
      '<SVG width="120" height="40"><RECT width="120" height="40" /></SVG>',
    ];

    return cases.map((svg) => {
      const preview = prepareSvgForResponsivePreview(
        projectSafeInlineSvg(svg)
      );
      if (!preview) return { prepared: false };

      const host = document.createElement("div");
      host.style.width = "120px";
      host.style.height = "40px";
      document.body.append(host);
      const root = host.attachShadow({ mode: "open" });
      const firstNode = preview.takeNode();
      root.replaceChildren(firstNode);
      const first = root.querySelector("svg");

      const parallelHost = document.createElement("div");
      document.body.append(parallelHost);
      const parallelRoot = parallelHost.attachShadow({ mode: "open" });
      parallelRoot.replaceChildren(preview.takeNode());
      const parallel = parallelRoot.querySelector("svg");
      const firstStayedMounted = root.querySelector("svg") === first;

      root.replaceChildren();
      root.replaceChildren(preview.takeNode());
      const remounted = root.querySelector("svg");
      const htmlLabel = remounted?.querySelector("foreignObject div") ?? null;
      const rectangle = remounted?.querySelector("rect") ?? null;
      const result = {
        prepared: true,
        firstIsSvg: first instanceof SVGSVGElement,
        firstUsedPreparedNode: first === firstNode,
        firstStayedMounted,
        parallelIsSvg: parallel instanceof SVGSVGElement,
        remountedIsSvg: remounted instanceof SVGSVGElement,
        htmlLabelIsHtml:
          htmlLabel === null ? null : htmlLabel instanceof HTMLDivElement,
        htmlLabelHasLayout:
          htmlLabel === null
            ? null
            : htmlLabel.getBoundingClientRect().width > 0 &&
              htmlLabel.getBoundingClientRect().height > 0,
        rectangleIsSvg:
          rectangle === null ? null : rectangle instanceof SVGRectElement,
        text: remounted?.textContent ?? null,
      };
      host.remove();
      parallelHost.remove();
      return result;
    });
  });

  expect(rendered).toEqual([
    {
      prepared: true,
      firstIsSvg: true,
      firstUsedPreparedNode: true,
      firstStayedMounted: true,
      parallelIsSvg: true,
      remountedIsSvg: true,
      htmlLabelIsHtml: null,
      htmlLabelHasLayout: null,
      rectangleIsSvg: null,
      text: "no namespace",
    },
    {
      prepared: true,
      firstIsSvg: true,
      firstUsedPreparedNode: true,
      firstStayedMounted: true,
      parallelIsSvg: true,
      remountedIsSvg: true,
      htmlLabelIsHtml: null,
      htmlLabelHasLayout: null,
      rectangleIsSvg: null,
      text: "A & B",
    },
    {
      prepared: true,
      firstIsSvg: true,
      firstUsedPreparedNode: true,
      firstStayedMounted: true,
      parallelIsSvg: true,
      remountedIsSvg: true,
      htmlLabelIsHtml: true,
      htmlLabelHasLayout: true,
      rectangleIsSvg: null,
      text: "HTML label",
    },
    {
      prepared: true,
      firstIsSvg: true,
      firstUsedPreparedNode: true,
      firstStayedMounted: true,
      parallelIsSvg: true,
      remountedIsSvg: true,
      htmlLabelIsHtml: null,
      htmlLabelHasLayout: null,
      rectangleIsSvg: true,
      text: "",
    },
  ]);
});
