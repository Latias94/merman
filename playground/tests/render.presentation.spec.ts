import { expect, test, type Page } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";
import { GENERATED_EXAMPLES } from "../src/generated/examples.ts";

const UPSTREAM_C_SCALE_0 = {
  default: "hsl(240, 100%, 76.2745098039%)",
  dark: "#1f2020",
  forest: "hsl(78.1578947368, 58.4615384615%, 64.5098039216%)",
  neutral: "#555",
  base: "hsl(40.5882352941, 100%, 68.3333333333%)",
} as const;

// Mindmap's root `section--1` fill is overridden by the later `section-root` rule. The first
// visible child is `section-0`, which Mermaid's generated styles bind to cScale1.
const UPSTREAM_MINDMAP_SECTION_0 = {
  default: "hsl(60, 100%, 73.5294117647%)",
  dark: "#0b0000",
  forest: "hsl(98.961038961, 100%, 74.9019607843%)",
  neutral: "#F4F4F4",
  base: "hsl(-79.4117647059, 100%, 68.3333333333%)",
} as const;

const UPSTREAM_KANBAN_SECTION_1 = {
  default: "hsl(80, 100%, 86.2745098039%)",
  dark: "hsl(321.6393442623, 65.5913978495%, 28.2352941176%)",
  forest: "hsl(78.1578947368, 58.4615384615%, 84.5098039216%)",
  neutral: "hsl(0, 0%, 43.3333333333%)",
  base: "hsl(220.5882352941, 100%, 83.3333333333%)",
} as const;

type ThemeName = keyof typeof UPSTREAM_C_SCALE_0;

const BLOCK_SYSTEM_ARCHITECTURE_EXAMPLE = GENERATED_EXAMPLES.find(
  (example) => example.id === "block-system-architecture"
);
if (!BLOCK_SYSTEM_ARCHITECTURE_EXAMPLE) {
  throw new Error("Missing the Block system architecture Playground example.");
}

const C4_CONTAINER_EXAMPLE = GENERATED_EXAMPLES.find(
  (example) => example.id === "c4-container-banking"
);
if (!C4_CONTAINER_EXAMPLE) {
  throw new Error("Missing the C4 container Playground example.");
}

const C4_DYNAMIC_EXAMPLE = GENERATED_EXAMPLES.find(
  (example) => example.id === "c4-dynamic-banking"
);
if (!C4_DYNAMIC_EXAMPLE) {
  throw new Error("Missing the C4 dynamic Playground example.");
}

const ZENUML_INTERACTION_EXAMPLE = GENERATED_EXAMPLES.find(
  (example) => example.id === "zenuml-interaction"
);
if (!ZENUML_INTERACTION_EXAMPLE) {
  throw new Error("Missing the ZenUML interaction Playground example.");
}

test("SVG mount failures stay inside the preview pane", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  await page.evaluate(() => {
    Object.defineProperty(window, "XMLSerializer", {
      configurable: true,
      value: class FailingXmlSerializer {
        serializeToString(): string {
          throw new Error("forced SVG mount failure");
        }
      },
    });
  });
  await page.getByRole("button", { name: "View SVG source" }).click();
  await page.getByRole("button", { name: "View SVG preview" }).click();

  const failure = page.locator(
    '[data-merman-render-error="true"][data-merman-error-stage="svg-mount"]',
  );
  await expect(failure).toBeVisible();
  await expect(failure).toContainText("forced SVG mount failure");
  await expect(
    page.getByRole("button", { name: "Examples", exact: true }),
  ).toBeVisible();
  errors.assertNone();
});

test("Event Model keeps Mermaid HTML labels readable in a dark Playground", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await page.emulateMedia({ colorScheme: "dark" });
  await openPlayground(page);
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  await dialog
    .getByRole("searchbox", { name: "Search examples" })
    .fill("Event Model");
  const eventModelCard = dialog.getByRole("button", {
    name: /^Event Model Flow/u,
  });
  await expect(eventModelCard).toHaveCount(1);
  await eventModelCard.click();

  await waitForPreviewSvg(page);
  await expect
    .poll(() => previewSvgText(page))
    .toContain("ItemAdded");

  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const mermaidPane = page.locator('[data-merman-compare-engine="mermaid"]');
  const mermaidHost = mermaidPane.locator(".preview-container > div");
  await expect(mermaidPane).toBeVisible();
  await expect
    .poll(() =>
      mermaidHost.evaluate((host) =>
        Boolean(host.shadowRoot?.querySelector("svg")),
      ),
    )
    .toBe(true);
  const colors = await mermaidHost.evaluate((host) => {
    const svg = host.shadowRoot?.querySelector("svg");
    if (!(svg instanceof SVGSVGElement)) {
      throw new Error("Missing mounted Mermaid SVG.");
    }
    const labels = svg.querySelectorAll("foreignObject b, foreignObject code");
    return {
      host: getComputedStyle(host).color,
      labels: [...new Set([...labels].map((label) => getComputedStyle(label).color))],
      svgFill: getComputedStyle(svg).fill,
    };
  });
  expect(colors.host).not.toBe(colors.svgFill);
  expect(colors.labels).toEqual([colors.svgFill]);
  await expect(
    page.getByRole("button", { name: "Examples", exact: true }),
  ).toBeVisible();
  errors.assertNone();
});

test("ZenUML Mermaid comparison remains self-contained", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const zenUmlFontRequests: string[] = [];
  page.on("request", (request) => {
    const url = request.url();
    if (url.includes("MS%20Sans%20Serif.ttf") || url.includes("MS Sans Serif.ttf")) {
      zenUmlFontRequests.push(request.url());
    }
  });

  await openPlayground(page);
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  await dialog
    .getByRole("searchbox", { name: "Search examples" })
    .fill(ZENUML_INTERACTION_EXAMPLE.title);
  const exampleCard = dialog.getByRole("button", {
    name: /^ZenUML Interaction/u,
  });
  await expect(exampleCard).toHaveCount(1);
  await exampleCard.click();
  await waitForPreviewSvg(page);
  await expect.poll(() => previewSvgText(page)).toContain("Alice");
  await page.getByRole("tab", { name: "Compare", exact: true }).click();

  const mermaidHost = page.locator(
    '[data-merman-compare-engine="mermaid"] .preview-container > div'
  );
  await expect
    .poll(() =>
      mermaidHost.evaluate((host) =>
        Boolean(host.shadowRoot?.querySelector("svg"))
      )
    )
    .toBe(true);
  expect(zenUmlFontRequests).toEqual([]);
  errors.assertNone();
});

test("Compare keeps Mermaid JS failures owned by the Mermaid pane", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart TD\n  Alpha --> Beta");
  await waitForPreviewSvg(page);
  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const mermaidPane = page.locator('[data-merman-compare-engine="mermaid"]');
  await expect(mermaidPane).toBeVisible();
  await expect
    .poll(() =>
      mermaidPane
        .locator(".preview-container > div")
        .evaluate((host) => Boolean(host.shadowRoot?.querySelector("svg"))),
    )
    .toBe(true);

  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");

  const mermaidError = mermaidPane.locator('[data-merman-render-error="true"]');
  await expect(mermaidError).toBeVisible();
  await expect(mermaidError).toHaveAttribute(
    "data-merman-error-engine",
    "Mermaid JS",
  );
  await expect(mermaidError).toHaveAttribute(
    "data-merman-error-stage",
    /.+/,
  );
  await expect(mermaidError).toContainText("Mermaid JS · Render Error");
  await mermaidError.locator("details").evaluate((element) => {
    (element as HTMLDetailsElement).open = true;
  });
  await expect(mermaidError.locator("pre")).toContainText(/hash|token|expected/i);
  await expect(mermaidError).not.toContainText("MERMAN_PARSE_ERROR");
  errors.assertNone();
});

test("font-only theme config preserves the computed shared palette", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  const cases = [
    [
      "radar",
      [
        "radar-beta",
        "  title Frontend Framework Comparison",
        '  axis perf["Performance"], dx["Dev Experience"], eco["Ecosystem"]',
        '  curve react["React"]{4, 4, 5}',
        "  max 5",
        "  min 0",
      ].join("\n"),
      UPSTREAM_C_SCALE_0,
      ".radarCurve-0",
    ],
    [
      "kanban",
      "kanban\n  todo[Todo]\n    docs[Create documentation]",
      UPSTREAM_KANBAN_SECTION_1,
      ".sections > .section-1 > rect",
    ],
    [
      "mindmap",
      "mindmap\n  root((Root))\n    child(Child)",
      UPSTREAM_MINDMAP_SECTION_0,
      ".mindmap-node.section-0 > .basic.label-container",
    ],
    [
      "timeline",
      "timeline\n  section Release\n    Plan : Build",
      UPSTREAM_C_SCALE_0,
      ".timeline-node.section--1 .node-bkg",
    ],
  ] as const;

  for (const theme of Object.keys(UPSTREAM_C_SCALE_0) as ThemeName[]) {
    for (const [family, source, palette, selector] of cases) {
      const expected = await browserComputedFill(page, palette[theme]);
      await renderSource(page, `${fontOnlyConfig(theme)}\n${source}`);
      await expect
        .poll(() => computedFill(page, selector), {
          message: `${family} ${theme} computed palette`,
        })
        .toBe(expected);
    }
  }

  errors.assertNone();
});

test("Quadrant keeps raw parity color while inheriting Mermaid's root fill", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(
    page,
    [
      "quadrantChart",
      "  title Reach and engagement of campaigns",
      "  x-axis Low Reach --> High Reach",
      "  y-axis Low Engagement --> High Engagement",
      "  quadrant-1 We should expand",
      "  Campaign A: [0.3, 0.6]",
    ].join("\n")
  );

  await expect.poll(() => quadrantPointPresentation(page)).toEqual({
    rawFill: "hsl(240, 100%, NaN%)",
    computedFill: "rgb(51, 51, 51)",
  });
  errors.assertNone();
});

test("Block circle edges contact the browser-visible shape boundary", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(
    page,
    [
      "block-beta",
      "  columns 3",
      '  user(("User")):3',
      "  space:3",
      '  ui["Web UI"] api["API Server"] db[("Database")]',
      "",
      "  user --> ui",
      "  ui --> api",
      "  api --> db",
    ].join("\n")
  );

  await expect.poll(() => blockCircleEndpointError(page)).toBeLessThanOrEqual(0.01);
  errors.assertNone();
});

test("Block class definitions match Mermaid computed fills", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(page, BLOCK_SYSTEM_ARCHITECTURE_EXAMPLE.source);

  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const expected = {
    front: await browserComputedFill(page, "#696"),
    back: [
      await browserComputedFill(page, "#969"),
      await browserComputedFill(page, "#969"),
    ],
  };

  await expect
    .poll(() => blockClassDefinitionFills(page, "merman"), {
      message: "Merman Block classDef computed fills",
    })
    .toEqual(expected);
  await expect
    .poll(() => blockClassDefinitionFills(page, "mermaid"), {
      message: "Mermaid JS Block classDef computed fills",
    })
    .toEqual(expected);
  errors.assertNone();
});

test("C4 uses the same browser layout environment in both compare panes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(page, C4_CONTAINER_EXAMPLE.source);
  await page.getByRole("tab", { name: "Compare", exact: true }).click();

  await expect
    .poll(() => compareViewBoxesMatch(page), {
      message: "Merman and Mermaid JS C4 viewBoxes",
    })
    .toBe(true);
  errors.assertNone();
});

test("C4 relation labels and lines match Mermaid computed presentation", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  for (const source of [
    C4_DYNAMIC_EXAMPLE.source,
    `${fontOnlyConfig("forest")}\n${C4_DYNAMIC_EXAMPLE.source}`,
  ]) {
    await renderSource(page, source);
    await page.getByRole("tab", { name: "Compare", exact: true }).click();

    await expect
      .poll(() => c4RelationPresentations(page, 2), {
        message: "C4Dynamic relation 2 computed presentation",
      })
      .toMatchObject({
        merman: {
          text: { fill: "rgb(255, 0, 0)" },
        },
        mermaid: {
          text: { fill: "rgb(255, 0, 0)" },
        },
        matches: true,
      });
  }

  errors.assertNone();
});

test("Merman Gantt presents non-overlapping date ticks", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(
    page,
    [
      "gantt",
      "  title A Gantt Diagram",
      "  dateFormat  YYYY-MM-DD",
      "  section A",
      "  Task1 :a1, 2024-01-01, 1d",
    ].join("\n")
  );

  await expect.poll(() => ganttTickOverlapCount(page)).toBe(0);
  errors.assertNone();
});

test("a 100-million-unit SVG stays bounded in preview and export", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await renderSource(
    page,
    [
      "---",
      "config:",
      "  xyChart:",
      "    width: 100000000",
      "    height: 1000000",
      "---",
      "xychart-beta",
      "  x-axis [a, b]",
      "  y-axis 0 --> 10",
      "  line [1, 9]",
    ].join("\n")
  );

  await expect.poll(() => largeSvgPreviewMetrics(page)).toMatchObject({
    rootWidth: "100%",
    rootHeight: "100%",
    viewBoxWidth: 100_000_000,
    viewBoxHeight: 1_000_000,
  });

  const metrics = await largeSvgPreviewMetrics(page);
  expect(metrics).not.toBeNull();
  const viewport = page.viewportSize() ?? { width: 1280, height: 720 };
  expect(metrics!.hostLayoutWidth).toBeLessThanOrEqual(viewport.width + 1);
  expect(metrics!.hostLayoutHeight).toBeLessThanOrEqual(viewport.height + 1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    viewport.width + 1
  );

  const fittedWidth = await previewSvgWidth(page);
  expect(fittedWidth).toBeGreaterThan(0);
  expect(fittedWidth).toBeLessThanOrEqual(viewport.width + 1);
  await expect(page.locator("span.tabular-nums").first()).not.toHaveText("0%");

  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await expect.poll(() => previewSvgWidth(page)).toBeGreaterThan(fittedWidth * 1.1);
  await page.getByRole("button", { name: "Fit to view", exact: true }).click();
  await expect.poll(() => previewSvgWidth(page)).toBeLessThanOrEqual(fittedWidth + 1);

  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog.getByRole("status")).toHaveText("Ready");
  const svgDownloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download", exact: true }).click();
  const svgDownload = await svgDownloadPromise;
  expect(await downloadText(svgDownload)).toContain(
    'viewBox="0 0 100000000 1000000"'
  );

  await dialog.getByRole("button", { name: "PNG", exact: true }).click();
  await expect(dialog.getByRole("status")).toHaveText("Ready");
  const downloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download", exact: true }).click();
  const download = await downloadPromise;
  expect(await pngDownloadDimensions(download)).toEqual({
    width: 4096,
    height: 41,
  });
  await expect(dialog.getByRole("status")).toHaveText("Downloaded");

  errors.assertNone();
});

function fontOnlyConfig(theme: string): string {
  return [
    "---",
    "config:",
    `  theme: ${theme}`,
    "  themeVariables:",
    "    fontFamily: Inter, sans-serif",
    "---",
  ].join("\n");
}

async function renderSource(page: Page, source: string): Promise<void> {
  await replaceEditorSource(page, source);
  await waitForPreviewSvg(page);
}

function previewHost(page: Page) {
  return page.locator(".preview-container > div").first();
}

async function computedFill(page: Page, selector: string): Promise<string | null> {
  return previewHost(page).evaluate((host, targetSelector) => {
    const element = host.shadowRoot?.querySelector<SVGGraphicsElement>(targetSelector);
    if (!element || element.getBoundingClientRect().width <= 0) return null;
    return getComputedStyle(element).fill;
  }, selector);
}

async function browserComputedFill(page: Page, fill: string): Promise<string> {
  return page.evaluate((value) => {
    const namespace = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(namespace, "svg");
    const rect = document.createElementNS(namespace, "rect");
    rect.setAttribute("fill", value);
    svg.append(rect);
    document.body.append(svg);
    const computed = getComputedStyle(rect).fill;
    svg.remove();
    return computed;
  }, fill);
}

async function quadrantPointPresentation(
  page: Page
): Promise<{ rawFill: string | null; computedFill: string } | null> {
  return previewHost(page).evaluate((host) => {
    const point = host.shadowRoot?.querySelector<SVGCircleElement>(".data-point circle");
    if (!point) return null;
    return {
      rawFill: point.getAttribute("fill"),
      computedFill: getComputedStyle(point).fill,
    };
  });
}

async function blockCircleEndpointError(page: Page): Promise<number> {
  return previewHost(page).evaluate((host) => {
    const svg = host.shadowRoot?.querySelector("svg");
    if (!svg) return Number.POSITIVE_INFINITY;
    const user = svg.querySelector<SVGGElement>('g.node[id$="-user"]');
    const circle = user?.querySelector<SVGCircleElement>("circle");
    const edge = [...svg.querySelectorAll<SVGPathElement>("path.flowchart-link")].find(
      (path) =>
        path.id.includes("user-ui") ||
        [...path.classList].some((className) => className === "LS-user")
    );
    const edgeMatrix = edge?.getCTM();
    const circleMatrix = circle?.getCTM();
    if (!circle || !edge || !edgeMatrix || !circleMatrix) {
      return Number.POSITIVE_INFINITY;
    }

    const edgeStart = edge.getPointAtLength(0).matrixTransform(edgeMatrix);
    const localStart = edgeStart.matrixTransform(circleMatrix.inverse());
    const centerX = circle.cx.baseVal.value;
    const centerY = circle.cy.baseVal.value;
    const radius = circle.r.baseVal.value;
    return Math.abs(Math.hypot(localStart.x - centerX, localStart.y - centerY) - radius);
  });
}

async function blockClassDefinitionFills(
  page: Page,
  engine: "merman" | "mermaid"
): Promise<{ front: string | null; back: Array<string | null> } | null> {
  const host = page.locator(
    `[data-merman-compare-engine="${engine}"] .preview-container > div`
  );
  return host.evaluate((preview) => {
    const svg = preview.shadowRoot?.querySelector("svg");
    if (!svg) return null;

    const shapeFill = (node: Element): string | null => {
      const shape = node.querySelector<SVGGraphicsElement>(
        ":scope > rect, :scope > circle, :scope > ellipse, :scope > polygon, :scope > path"
      );
      return shape ? getComputedStyle(shape).fill : null;
    };
    const front = svg.querySelector("g.node.front");
    const back = [...svg.querySelectorAll("g.node.back")];
    if (!front || back.length !== 2) return null;

    return {
      front: shapeFill(front),
      back: back.map(shapeFill),
    };
  });
}

async function compareViewBoxesMatch(page: Page): Promise<boolean | null> {
  const viewBoxes = await Promise.all(
    (["merman", "mermaid"] as const).map((engine) =>
      page
        .locator(
          `[data-merman-compare-engine="${engine}"] .preview-container > div`
        )
        .evaluate((host) => {
          const svg = host.shadowRoot?.querySelector<SVGSVGElement>("svg");
          if (!svg) return null;
          const viewBox = svg.viewBox.baseVal;
          return [viewBox.x, viewBox.y, viewBox.width, viewBox.height];
        })
    )
  );
  if (viewBoxes.some((viewBox) => viewBox === null)) return null;
  const [left, right] = viewBoxes;
  return left?.every((value, index) => value === right?.[index]) ?? null;
}

type C4RelationPresentation = {
  text: {
    fill: string;
    fontSize: string;
    fontFamily: string;
    fontWeight: string;
    fontStyle: string;
    textAnchor: string;
    dominantBaseline: string;
  };
  line: {
    fill: string;
    stroke: string;
    strokeWidth: string;
  };
};

async function c4RelationPresentations(
  page: Page,
  relationIndex: number
): Promise<{
  merman: C4RelationPresentation;
  mermaid: C4RelationPresentation;
  matches: boolean;
} | null> {
  const read = async (
    engine: "merman" | "mermaid"
  ): Promise<C4RelationPresentation | null> => {
    const host = page.locator(
      `[data-merman-compare-engine="${engine}"] .preview-container > div`
    );
    return host.evaluate((preview, index) => {
      const svg = preview.shadowRoot?.querySelector("svg");
      const label = [...(svg?.querySelectorAll<SVGTextElement>("text") ?? [])].find(
        (candidate) => candidate.textContent?.startsWith(`${index}: `)
      );
      const line = label?.previousElementSibling;
      if (
        !label ||
        !line ||
        !["line", "path"].includes(line.tagName.toLowerCase())
      ) {
        return null;
      }

      const textStyle = getComputedStyle(label);
      const lineStyle = getComputedStyle(line);
      return {
        text: {
          fill: textStyle.fill,
          fontSize: textStyle.fontSize,
          fontFamily: textStyle.fontFamily,
          fontWeight: textStyle.fontWeight,
          fontStyle: textStyle.fontStyle,
          textAnchor: textStyle.textAnchor,
          dominantBaseline: textStyle.dominantBaseline,
        },
        line: {
          fill: lineStyle.fill,
          stroke: lineStyle.stroke,
          strokeWidth: lineStyle.strokeWidth,
        },
      };
    }, relationIndex);
  };

  const [merman, mermaid] = await Promise.all([read("merman"), read("mermaid")]);
  if (!merman || !mermaid) return null;
  return {
    merman,
    mermaid,
    matches: JSON.stringify(merman) === JSON.stringify(mermaid),
  };
}

async function ganttTickOverlapCount(page: Page): Promise<number | null> {
  return previewHost(page).evaluate((host) => {
    const ticks = [
      ...(host.shadowRoot?.querySelectorAll<SVGTextElement>(".tick text") ?? []),
    ]
      .map((tick) => tick.getBoundingClientRect())
      .filter((rect) => rect.width > 0)
      .sort((left, right) => left.top - right.top || left.left - right.left);
    if (ticks.length < 2) return null;

    return ticks.slice(1).filter((current, index) => {
      const previous = ticks[index];
      return Math.abs(previous.top - current.top) < 2 && previous.right > current.left;
    }).length;
  });
}

async function largeSvgPreviewMetrics(page: Page): Promise<{
  rootWidth: string | null;
  rootHeight: string | null;
  viewBoxWidth: number;
  viewBoxHeight: number;
  renderedWidth: number;
  renderedHeight: number;
  hostLayoutWidth: number;
  hostLayoutHeight: number;
} | null> {
  return previewHost(page).evaluate((host) => {
    const svg = host.shadowRoot?.querySelector<SVGSVGElement>("svg");
    if (!svg) return null;
    const rendered = svg.getBoundingClientRect();
    return {
      rootWidth: svg.getAttribute("width"),
      rootHeight: svg.getAttribute("height"),
      viewBoxWidth: svg.viewBox.baseVal.width,
      viewBoxHeight: svg.viewBox.baseVal.height,
      renderedWidth: rendered.width,
      renderedHeight: rendered.height,
      hostLayoutWidth: host.clientWidth,
      hostLayoutHeight: host.clientHeight,
    };
  });
}

async function previewSvgWidth(page: Page): Promise<number> {
  return (await largeSvgPreviewMetrics(page))?.renderedWidth ?? 0;
}

async function pngDownloadDimensions(
  download: import("@playwright/test").Download
): Promise<{ width: number; height: number }> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("PNG download stream is unavailable");

  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of stream) {
    const buffer = Buffer.from(chunk);
    chunks.push(buffer);
    length += buffer.length;
    if (length >= 24) break;
  }
  stream.destroy();
  const header = Buffer.concat(chunks, length);
  if (
    header.length < 24 ||
    header.subarray(1, 4).toString("ascii") !== "PNG"
  ) {
    throw new Error("Downloaded artifact is not a PNG");
  }
  return {
    width: header.readUInt32BE(16),
    height: header.readUInt32BE(20),
  };
}

async function downloadText(
  download: import("@playwright/test").Download
): Promise<string> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("Download stream is unavailable");

  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}
