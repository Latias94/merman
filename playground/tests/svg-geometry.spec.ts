import path from "node:path";
import { expect, test } from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";

import { GENERATED_EXAMPLES } from "../src/generated/examples.ts";

let sourceServer: ViteDevServer | null = null;
let sourceOrigin = "";

const EVENT_MODEL_EXAMPLE = GENERATED_EXAMPLES.find(
  (example) => example.id === "event-model",
);
if (!EVENT_MODEL_EXAMPLE) {
  throw new Error("Missing the Event Model Playground example.");
}

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

test("raster export changes only the root canvas and encodes explicit alpha or JPEG background", async ({
  page,
}) => {
  await page.goto(sourceOrigin);
  const result = await page.evaluate(async () => {
    const { projectNavigableInlineSvg } = await import(
      "/src/runtime/" + "render-artifact.ts"
    );
    const { inspectSvgForRasterExport, prepareSvgForRasterExport } =
      await import("/src/lib/" + "svg-geometry.ts");
    const { planRasterExport } = await import(
      "/src/lib/" + "raster-export-plan.ts"
    );
    const { encodeRasterExport } = await import("/src/lib/" + "export.ts");
    const artifact = projectNavigableInlineSvg(
      [
        '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" style="background-color: white !important; background-image: linear-gradient(red, red) !important;">',
        '<rect x="8" y="8" width="16" height="16" fill="white"/>',
        "</svg>",
      ].join("")
    );
    const source = inspectSvgForRasterExport(artifact);
    const transparentPlan = planRasterExport(source, {
      format: "png",
      background: { mode: "transparent" },
      sizing: { mode: "scale", scale: 1 },
    });
    const transparentSvg = prepareSvgForRasterExport(
      artifact,
      transparentPlan
    );
    if (!transparentSvg) throw new Error("Expected a prepared transparent SVG.");
    const transparent = await encodeRasterExport(artifact, transparentPlan);

    const jpegPlan = planRasterExport(source, {
      format: "jpeg",
      background: { mode: "custom", color: "#12abef" },
      quality: 100,
      sizing: { mode: "scale", scale: 1 },
    });
    const jpeg = await encodeRasterExport(artifact, jpegPlan);

    async function pixels(blob: Blob) {
      const bitmap = await createImageBitmap(blob);
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) throw new Error("Missing test canvas context.");
      context.drawImage(bitmap, 0, 0);
      const corner = [...context.getImageData(1, 1, 1, 1).data];
      const center = [...context.getImageData(16, 16, 1, 1).data];
      bitmap.close();
      return { width: canvas.width, height: canvas.height, corner, center };
    }

    const unsizedArtifact = projectNavigableInlineSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" style="background-color:white" />'
    );
    const unsizedPlan = planRasterExport(
      inspectSvgForRasterExport(unsizedArtifact),
      {
        format: "png",
        background: { mode: "transparent" },
        sizing: { mode: "scale", scale: 1 },
      }
    );
    const unsizedPrepared = prepareSvgForRasterExport(
      unsizedArtifact,
      unsizedPlan
    );
    if (!unsizedPrepared) throw new Error("Expected fallback SVG geometry.");
    const unsizedParsed = new DOMParser().parseFromString(
      unsizedPrepared.svg,
      "image/svg+xml"
    );
    const unsized = await encodeRasterExport(unsizedArtifact, unsizedPlan);

    const parsed = new DOMParser().parseFromString(
      transparentSvg.svg,
      "image/svg+xml"
    );
    return {
      sourceBackground: source.originalBackground,
      rootBackground: (parsed.documentElement as unknown as SVGSVGElement).style
        .backgroundColor,
      rootBackgroundPriority: (
        parsed.documentElement as unknown as SVGSVGElement
      ).style.getPropertyPriority("background-color"),
      rootBackgroundImage: (
        parsed.documentElement as unknown as SVGSVGElement
      ).style.backgroundImage,
      descendantFill: parsed.querySelector("rect")?.getAttribute("fill"),
      pngType: transparent.type,
      png: await pixels(transparent),
      jpegType: jpeg.type,
      jpeg: await pixels(jpeg),
      unsized: {
        viewBox: unsizedParsed.documentElement.getAttribute("viewBox"),
        rootBackground: (
          unsizedParsed.documentElement as unknown as SVGSVGElement
        ).style.backgroundColor,
        raster: await pixels(unsized),
      },
    };
  });

  expect(result.sourceBackground).toEqual({ color: "white", opaque: true });
  expect(result.rootBackground).toBe("transparent");
  expect(result.rootBackgroundPriority).toBe("important");
  expect(result.rootBackgroundImage).toBe("none");
  expect(result.descendantFill).toBe("white");
  expect(result.pngType).toBe("image/png");
  expect(result.png).toEqual({
    width: 32,
    height: 32,
    corner: [0, 0, 0, 0],
    center: [255, 255, 255, 255],
  });
  expect(result.jpegType).toBe("image/jpeg");
  expect(result.jpeg.width).toBe(32);
  expect(result.jpeg.height).toBe(32);
  expect(result.jpeg.corner[0]).toBeCloseTo(18, -1);
  expect(result.jpeg.corner[1]).toBeCloseTo(171, -1);
  expect(result.jpeg.corner[2]).toBeCloseTo(239, -1);
  expect(result.jpeg.corner[3]).toBe(255);
  expect(result.unsized).toEqual({
    viewBox: "0 0 300 150",
    rootBackground: "transparent",
    raster: {
      width: 300,
      height: 150,
      corner: [0, 0, 0, 0],
      center: [0, 0, 0, 0],
    },
  });
});

test("preview preserves safe HTML-compatible SVG through inert DOM parsing", async ({
  page,
}) => {
  await page.goto(sourceOrigin);
  const rendered = await page.evaluate(async () => {
    const { projectNavigableInlineSvg } = await import(
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
        projectNavigableInlineSvg(svg),
        document
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

test("responsive preview preserves renderer viewBox ownership", async ({ page }) => {
  await page.goto(sourceOrigin);
  const result = await page.evaluate(async () => {
    const { projectNavigableInlineSvg } = await import(
      "/src/runtime/" + "render-artifact.ts"
    );
    const { prepareSvgForResponsivePreview } = await import(
      "/src/lib/" + "svg-geometry.ts"
    );
    const sources = [
      '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40" viewBox="5 6 120 40" style="background-color: white"><text>bounded</text></svg>',
      '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><text>intrinsic</text></svg>',
    ];

    return sources.map((source) => {
      const artifact = projectNavigableInlineSvg(source);
      const preview = prepareSvgForResponsivePreview(artifact, document);
      if (!preview) throw new Error("Expected prepared preview geometry.");
      const root = preview.takeNode();
      return {
        backgroundColor: (root as SVGElement).style.backgroundColor,
        sourceUnchanged: artifact.svg === source,
        viewBox: root.getAttribute("viewBox"),
        width: root.getAttribute("width"),
        height: root.getAttribute("height"),
      };
    });
  });

  expect(result).toEqual([
    {
      backgroundColor: "",
      sourceUnchanged: true,
      viewBox: "5 6 120 40",
      width: "100%",
      height: "100%",
    },
    {
      backgroundColor: "",
      sourceUnchanged: true,
      viewBox: null,
      width: "120",
      height: "40",
    },
  ]);
});

test("preview hardens external anchors without mutating the export source", async ({
  page,
}) => {
  await page.goto(sourceOrigin);
  const result = await page.evaluate(async () => {
    const { projectNavigableInlineSvg } = await import(
      "/src/runtime/" + "render-artifact.ts"
    );
    const { prepareSvgForResponsivePreview } = await import(
      "/src/lib/" + "svg-geometry.ts"
    );
    const source = [
      '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="240" height="80">',
      '<a href="https://example.test/browse/MC-1" target="_top" rel="nofollow"><text>https</text></a>',
      '<a xlink:href="mailto:maintainer@example.test"><text>mail</text></a>',
      '<a href="tel:+1234567890"><text>tel</text></a>',
      '<a href="../browse/MC-2"><text>relative</text></a>',
      '<a href="#local-ticket" target="_top"><text>fragment</text></a>',
      "</svg>",
    ].join("");
    const artifact = projectNavigableInlineSvg(source);
    const preview = prepareSvgForResponsivePreview(artifact, document);
    if (!preview) throw new Error("Expected a prepared SVG preview.");

    const anchors = [...preview.takeNode().querySelectorAll("a")].map(
      (anchor) => ({
        href:
          anchor.getAttribute("href") ??
          anchor.getAttributeNS("http://www.w3.org/1999/xlink", "href") ??
          anchor.getAttribute("xlink:href"),
        rel: anchor.getAttribute("rel"),
        target: anchor.getAttribute("target"),
      })
    );
    return {
      anchors,
      exportSourceUnchanged: artifact.svg === source,
      exportSourceHasHardening:
        artifact.svg.includes('target="_blank"') ||
        artifact.svg.includes("noopener") ||
        artifact.svg.includes("noreferrer"),
    };
  });

  expect(result).toEqual({
    anchors: [
      {
        href: "https://example.test/browse/MC-1",
        rel: "nofollow noopener noreferrer",
        target: "_blank",
      },
      {
        href: "mailto:maintainer@example.test",
        rel: "noopener noreferrer",
        target: "_blank",
      },
      {
        href: "tel:+1234567890",
        rel: "noopener noreferrer",
        target: "_blank",
      },
      {
        href: "../browse/MC-2",
        rel: "noopener noreferrer",
        target: "_blank",
      },
      {
        href: "#local-ticket",
        rel: null,
        target: "_self",
      },
    ],
    exportSourceHasHardening: false,
    exportSourceUnchanged: true,
  });
});

test("Event Model payload SVG survives responsive preview mount", async ({
  page,
}) => {
  await page.goto(sourceOrigin);
  const result = await page.evaluate(async (source) => {
    const { ensureMermanReady } = await import(
      "/src/runtime/" + "merman.ts"
    );
    const { configuredMermanOperationInput } = await import(
      "/src/runtime/" + "merman-operation-input.ts"
    );
    const { prepareSvgForResponsivePreview } = await import(
      "/src/lib/" + "svg-geometry.ts"
    );
    const facade = await ensureMermanReady();
    const rendered = facade.render(
      configuredMermanOperationInput(source, "default", "{}", {
        textMeasurementMode: "headless",
      }),
    );
    if (rendered.status === "failure") {
      throw new Error(rendered.error.message);
    }
    const preview = prepareSvgForResponsivePreview(
      rendered.artifact,
      document,
    );
    if (!preview) return null;

    const host = document.createElement("div");
    document.body.append(host);
    const root = host.attachShadow({ mode: "open" });
    root.replaceChildren(preview.takeNode());
    const svg = root.querySelector("svg");
    const mounted = {
      foreignObjectCount: svg?.querySelectorAll("foreignObject").length ?? 0,
      lineBreakCount: svg?.querySelectorAll("foreignObject br").length ?? 0,
      isSvg: svg instanceof SVGSVGElement,
      text: svg?.textContent ?? null,
    };
    host.remove();
    return mounted;
  }, EVENT_MODEL_EXAMPLE.source);

  expect(result).toEqual({
    foreignObjectCount: 5,
    lineBreakCount: 5,
    isSvg: true,
    text: expect.stringContaining("ItemAdded"),
  });
});

test("preview binds fragment references to the actual mount document", async ({
  page,
}) => {
  await page.route("**/mount-document-fixture", (route) =>
    route.fulfill({
      body: [
        "<!doctype html>",
        '<html><head><base href="https://collector.example/external.svg"></head>',
        "<body></body></html>",
      ].join(""),
      contentType: "text/html",
    })
  );
  await page.goto(sourceOrigin);
  const result = await page.evaluate(async () => {
    const { projectNavigableInlineSvg } = await import(
      "/src/runtime/" + "render-artifact.ts"
    );
    const { prepareSvgForResponsivePreview } = await import(
      "/src/lib/" + "svg-geometry.ts"
    );
    const frame = document.createElement("iframe");
    const loaded = new Promise<void>((resolve, reject) => {
      frame.addEventListener("load", () => resolve(), { once: true });
      frame.addEventListener(
        "error",
        () => reject(new Error("Mount-document fixture failed to load.")),
        { once: true }
      );
    });
    frame.src = "/mount-document-fixture";
    document.body.append(frame);
    await loaded;
    const mountDocument = frame.contentDocument;
    if (!mountDocument) {
      throw new Error("Mount-document fixture is not same-origin.");
    }

    let fragmentError = "";
    try {
      prepareSvgForResponsivePreview(
        projectNavigableInlineSvg(
          '<svg xmlns="http://www.w3.org/2000/svg"><use href="#node"/></svg>'
        ),
        mountDocument
      )?.takeNode();
    } catch (error) {
      fragmentError = error instanceof Error ? error.message : String(error);
    }

    const fragmentFreePreview = prepareSvgForResponsivePreview(
      projectNavigableInlineSvg(
        '<svg xmlns="http://www.w3.org/2000/svg"><text>safe</text></svg>'
      ),
      mountDocument
    );
    const result = {
      fragmentError,
      fragmentFreePrepared:
        fragmentFreePreview?.takeNode().ownerDocument === mountDocument,
    };
    frame.remove();
    return result;
  });

  expect(result.fragmentError).toMatch(/base URI differs/u);
  expect(result.fragmentFreePrepared).toBe(true);
});
