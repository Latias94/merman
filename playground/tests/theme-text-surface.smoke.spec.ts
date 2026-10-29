import { readFile } from "node:fs/promises";

import { expect, test } from "@playwright/test";
import initMermanWasm, {
  renderSvg as renderBindingSvg,
} from "../../platforms/web/packages/full/artifacts/wasm/merman_wasm.js";

import {
  containsTextBounds,
  observeMountedThemeTextSurfaces,
  observeThemeTextSurfaces,
  textSurfaceContrastRatio,
  type ThemeTextSurfaceProbe,
} from "./helpers/theme-text-surface";
import {
  monitorBrowserErrors,
  waitForPreviewSvg,
} from "./helpers/playground";
import { encodeShareHash } from "../src/lib/share";

const NOTE_TEXT = "[data-c6-role='note-label'][data-c6-occurrence='note-0-line-0']";
const NOTE_SURFACE = "[data-c6-surface='note-0']";
const PROBE: ThemeTextSurfaceProbe = {
  role: "note-label",
  occurrenceId: "note-0-line-0",
  textSelector: NOTE_TEXT,
  surfaceSelector: NOTE_SURFACE,
};

const DARK_CANVAS = [0x0f, 0x17, 0x2a] as const;

test.beforeAll(async () => {
  const wasmBytes = await readFile(
    new URL(
      "../../platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm",
      import.meta.url,
    ),
  );
  await initMermanWasm({ module_or_path: wasmBytes });
});

test("observes State dark native Note text after the production Playground mount", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  const source = [
    "stateDiagram-v2",
    "  A",
    "  note right of A : Terminal note",
  ].join("\n");
  const hash = encodeShareHash({
    code: source,
    mermaidConfig: '{"htmlLabels":false}',
    diagramTheme: "default",
    themePresetId: "editor-dark",
    themeRecipeJson: null,
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  const wasmResponse = page.waitForResponse((response) =>
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/u.test(response.url()),
  );
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await wasmResponse;
  await waitForPreviewSvg(page);

  const mountedSvg = page
    .locator(".preview-container > div")
    .first()
    .locator("svg");
  await expect(mountedSvg).toHaveCount(1);
  await expect(mountedSvg.locator(".statediagram-note foreignObject")).toHaveCount(
    0,
  );
  const [observation] = await observeMountedThemeTextSurfaces(mountedSvg, [
    {
      role: "note-label",
      occurrenceId: "state-note-0",
      textSelector: ".statediagram-note .noteLabel text",
      surfaceSelector:
        ".statediagram-note .basic.label-container.outer-path > path:first-child",
    },
  ]);

  expectReadableProductionSurface(observation, [0xfe, 0xf3, 0xc7], [
    0x42, 0x20, 0x06,
  ]);
  errors.assertNone();
});

test("observes Packet dark byte, title, and field-label role surfaces", async ({
  page,
}) => {
  const svg = renderBindingSvg(
    [
      "packet",
      "title Packet Typography",
      '0-7: "Header"',
      '8-15: "Payload"',
    ].join("\n"),
    JSON.stringify({
      theme: {
        spec: {
          styles: [
            packetFillRule("packet-byte-label", "#e5e7eb"),
            packetFillRule("packet-field-label", "#0f172a"),
            packetFillRule("title", "#e5e7eb"),
          ],
        },
      },
      svg: {
        diagram_id: "browser-packet-dark",
        root_background_color: "#0f172a",
      },
    }),
  );
  expect(
    [
      "#browser-packet-dark .packetByte.start{fill:#e5e7eb;}",
      "#browser-packet-dark .packetLabel{fill:#0f172a;",
      "#browser-packet-dark .packetTitle{fill:#e5e7eb;",
    ].every((rule) => svg.includes(rule)),
    "the checked-in Web binding must expose all Packet terminal text-role fills",
  ).toBe(true);

  const observations = await observeThemeTextSurfaces(page, svg, [
    {
      role: "packet-byte-label",
      occurrenceId: "packet-byte-start-0",
      textSelector: "g > text.packetByte.start:nth-of-type(2)",
      surfaceSelector: ":scope",
      surfacePaintProperty: "background-color",
    },
    {
      role: "title",
      occurrenceId: "packet-title-0",
      textSelector: "text.packetTitle",
      surfaceSelector: ":scope",
      surfacePaintProperty: "background-color",
    },
    {
      role: "packet-field-label",
      occurrenceId: "packet-field-label-0",
      textSelector: "g > text.packetLabel:first-of-type",
      surfaceSelector: "g > rect.packetBlock:first-of-type",
    },
  ]);

  expectReadableProductionSurface(observations[0], [0xe5, 0xe7, 0xeb], [
    ...DARK_CANVAS,
  ]);
  expectReadableProductionSurface(observations[1], [0xe5, 0xe7, 0xeb], [
    ...DARK_CANVAS,
  ]);
  expectReadableProductionSurface(observations[2], [0x0f, 0x17, 0x2a], [
    0xef, 0xef, 0xef,
  ]);
});

test("observes a Sequence role paint expressed by the checked-in theme spec", async ({
  page,
}) => {
  const svg = renderSequenceRoleSvg();
  expect(svg).toContain("Render through the product binding");
  const [observation] = await observeThemeTextSurfaces(page, svg, [
    {
      role: "message-label",
      occurrenceId: "message-0",
      textSelector: "text.messageText",
      surfaceSelector: ":scope",
      surfacePaintProperty: "background-color",
    },
  ]);

  expectReadableProductionSurface(observation, [0xf8, 0xfa, 0xfc], [
    ...DARK_CANVAS,
  ]);
});

test("browser computed style owns Sequence role cascade resolution", async ({
  page,
}) => {
  const svg = renderSequenceRoleSvg().replace(
    "</svg>",
    "<style>#browser-sequence-role [class~='messageText']{fill:#000!important;}</style></svg>",
  );
  const [observation] = await observeThemeTextSurfaces(page, svg, [
    {
      role: "message-label",
      occurrenceId: "message-0",
      textSelector: "text.messageText",
      surfaceSelector: ":scope",
      surfacePaintProperty: "background-color",
    },
  ]);

  expect(observation.finalTextPaint).toEqual([0, 0, 0]);
  expect(observation.finalTextPaint).not.toEqual([0xf8, 0xfa, 0xfc]);
});

test("missing production Sequence role capability fails instead of skipping", async ({
  page,
}) => {
  const svgWithoutMessageRole = renderSequenceRoleSvg().replaceAll(
    "messageText",
    "removedMessageText",
  );

  await expect(
    observeThemeTextSurfaces(page, svgWithoutMessageRole, [
      {
        role: "message-label",
        occurrenceId: "message-0",
        textSelector: "text.messageText",
        surfaceSelector: ":scope",
        surfacePaintProperty: "background-color",
      },
    ]),
  ).rejects.toThrow("Expected one terminal text");
});

test("terminal observation sees a higher-specificity compatibility CSS winner", async ({
  page,
}) => {
  const svg = fixtureSvg({
    extraCss: "#diagram .legacy-note text { fill: #333333; }",
  });
  const [observation] = await observeThemeTextSurfaces(page, svg, [PROBE]);

  expect(observation.finalTextPaint).toEqual([0x33, 0x33, 0x33]);
  expect(observation.finalTextPaint).not.toEqual([0xfe, 0xf3, 0xc7]);
  expect(observation.artifactDigest).toMatch(/^[0-9a-f]{64}$/u);
});

test("terminal observation exposes a role whose paint declaration is missing", async ({
  page,
}) => {
  const svg = fixtureSvg({ declaredTextPaint: null });
  const [observation] = await observeThemeTextSurfaces(page, svg, [PROBE]);

  expect(observation.finalTextPaint).toEqual([0, 0, 0]);
  expect(observation.finalTextPaint).not.toEqual([0xfe, 0xf3, 0xc7]);
});

test("terminal observation rejects a tspan moved outside its owning surface", async ({
  page,
}) => {
  const svg = fixtureSvg({ tspanDy: "4em" });
  const movedProbe: ThemeTextSurfaceProbe = {
    ...PROBE,
    textSelector: `${NOTE_TEXT} tspan`,
  };
  await expect(observeThemeTextSurfaces(page, svg, [movedProbe])).rejects.toThrow(
    "Terminal surface must supply the opaque background at every visible text pixel",
  );
});

test("terminal observation checks background behind a displaced child tspan", async ({ page }) => {
  await expect(observeThemeTextSurfaces(
    page, fixtureSvg({ tspanDy: "4em" }), [PROBE],
  )).rejects.toThrow("Terminal surface must supply the opaque background at every visible text pixel");
});

test("terminal text geometry follows the surface transform", async ({ page }) => {
  const svg = fixtureSvg({}).replace(
    '<g class="legacy-note">', '<g class="legacy-note" transform="translate(80,30) scale(0.5)">',
  );
  const [observation] = await observeThemeTextSurfaces(page, svg, [PROBE]);
  expect(containsTextBounds(observation, 1_000), JSON.stringify(observation)).toBe(true);
});

for (const [name, extraCss] of [
  ["hidden text", `${NOTE_TEXT}{visibility:hidden}`],
  ["transparent text", `${NOTE_TEXT}{opacity:0}`],
  ["undisplayed text", `${NOTE_TEXT}{display:none}`],
  ["zero-size text", `${NOTE_TEXT}{font-size:0}`],
  ["transparent ancestor", ".legacy-note{opacity:0}"],
  ["undisplayed ancestor", ".legacy-note{display:none}"],
  ["transparent SVG fill", `${NOTE_TEXT}{fill-opacity:0}`],
  ["fully clipped text", `${NOTE_TEXT}{clip-path:inset(100%)}`],
] as const) {
  test(`terminal observation rejects ${name}`, async ({ page }) => {
    await expect(
      observeThemeTextSurfaces(page, fixtureSvg({ extraCss }), [PROBE]),
    ).rejects.toThrow(/visible|positive area/u);
  });
}

test("pixel observation restores authored text style", async ({ page }) => {
  const svg = fixtureSvg({}).replace('font-family="Inter"', 'font-family="Inter" style="font-weight:600"');
  await page.setContent(svg);
  const root = page.locator("svg");
  const before = await root.evaluate((element) => new XMLSerializer().serializeToString(element));
  await observeMountedThemeTextSurfaces(root, [PROBE]);
  expect(await root.evaluate((element) => new XMLSerializer().serializeToString(element))).toBe(before);
});

for (const [name, extraCss] of [
  ["partially clipped text", `${NOTE_TEXT}{clip-path:inset(0 80% 0 0)}`],
  ["clipped final glyph", `${NOTE_TEXT}{clip-path:inset(0 5% 0 0)}`],
  ["partially clipped ancestor", ".legacy-note{clip-path:inset(0 80% 0 0)}"],
] as const) {
  test(`terminal observation rejects ${name}`, async ({ page }) => {
    await expect(
      observeThemeTextSurfaces(page, fixtureSvg({ extraCss }), [PROBE]),
    ).rejects.toThrow("Terminal text must retain its complete visible ink");
  });
}

test("terminal observation rejects a selected tspan clipped by its text parent", async ({ page }) => {
  await expect(observeThemeTextSurfaces(page, fixtureSvg({
    extraCss: `${NOTE_TEXT}{clip-path:inset(0 80% 0 0)}`,
  }), [{ ...PROBE, textSelector: `${NOTE_TEXT} tspan` }])).rejects.toThrow(
    "Terminal text must retain its complete visible ink",
  );
});

test("terminal observation accepts clipping confined to font whitespace", async ({ page }) => {
  const svg = fixtureSvg({
    extraCss: `${NOTE_TEXT}{font-family:serif;font-size:28px;font-style:italic;white-space:pre;clip-path:inset(-20% 1% -20% -20%)}`,
  }).replace("Terminal note</tspan>", "Italic f    </tspan>")
    .replace(/>\s+<tspan/u, "><tspan")
    .replace(/<\/tspan>\s+<\/text>/u, "</tspan></text>");
  const [observation] = await observeThemeTextSurfaces(page, svg, [PROBE]);
  expect(observation.finalFontIdentity).toContain("serif");
});

test("terminal observation rejects text partly clipped by the root viewport", async ({ page }) => {
  const svg = fixtureSvg({})
    .replace('id="diagram"', 'id="diagram" style="overflow:hidden"')
    .replace(/x="30"/gu, 'x="200"')
    .replace('width="200" height="48"', 'width="300" height="48"');
  await expect(observeThemeTextSurfaces(page, svg, [PROBE])).rejects.toThrow(
    "Terminal text must retain its complete visible ink",
  );
});

test("partial clipping rejection restores every authored style", async ({ page }) => {
  await page.setContent(fixtureSvg({
    extraCss: ".legacy-note{clip-path:inset(0 80% 0 0)}",
  }));
  const root = page.locator("svg");
  const before = await root.evaluate((element) => new XMLSerializer().serializeToString(element));
  await expect(observeMountedThemeTextSurfaces(root, [PROBE])).rejects.toThrow(
    "Terminal text must retain its complete visible ink",
  );
  expect(await root.evaluate((element) => new XMLSerializer().serializeToString(element))).toBe(before);
});

test("terminal observation rejects a selector with zero matches", async ({
  page,
}) => {
  await expect(
    observeThemeTextSurfaces(page, fixtureSvg({}), [
      { ...PROBE, textSelector: "[data-c6-role='missing']" },
    ]),
  ).rejects.toThrow("Expected one terminal text");
});

test("terminal observation rejects duplicate semantic occurrences", async ({
  page,
}) => {
  await expect(
    observeThemeTextSurfaces(page, fixtureSvg({ duplicateText: true }), [PROBE]),
  ).rejects.toThrow("got 2");
});

test("terminal observation rejects transparent resolved paint", async ({
  page,
}) => {
  await expect(
    observeThemeTextSurfaces(
      page,
      fixtureSvg({ declaredTextPaint: "rgba(0, 0, 0, 0)" }),
      [PROBE],
    ),
  ).rejects.toThrow("requires an opaque resolved color");
});

test("terminal observation rejects a non-RGB computed paint", async ({
  page,
}) => {
  await expect(
    observeThemeTextSurfaces(
      page,
      fixtureSvg({ declaredTextPaint: "url(#terminal-paint)" }),
      [PROBE],
    ),
  ).rejects.toThrow("Unsupported terminal computed color");
});

for (const [name, extraCss] of [
  ["hidden surface", `${NOTE_SURFACE}{visibility:hidden}`],
  ["transparent surface", `${NOTE_SURFACE}{opacity:0}`],
  ["partially transparent surface", `${NOTE_SURFACE}{opacity:0.5}`],
  ["transparent surface fill", `${NOTE_SURFACE}{fill-opacity:0}`],
  ["surface-only transparent ancestor", ".surface-group{opacity:0}"],
  ["surface-only hidden ancestor", ".surface-group{visibility:hidden}"],
]) {
  test(`terminal observation rejects ${name}`, async ({ page }) => {
    await expect(
      observeThemeTextSurfaces(page, fixtureSvg({ extraCss }), [PROBE]),
    ).rejects.toThrow(/surface.*visible and opaque/u);
  });
}

test("terminal observation rejects a surface occluded beneath visible text", async ({ page }) => {
  const svg = fixtureSvg({}).replace(
    '<text data-c6-role="note-label"',
    '<rect x="20" y="20" width="200" height="48" fill="white"/><text data-c6-role="note-label"',
  );
  await expect(observeThemeTextSurfaces(page, svg, [PROBE])).rejects.toThrow(
    "Terminal surface must supply the opaque background at every visible text pixel",
  );
});

test("terminal observation rejects a surface occluded only beneath the glyphs", async ({ page }) => {
  const svg = fixtureSvg({}).replace(
    '<text data-c6-role="note-label"',
    '<rect x="28" y="28" width="130" height="34" fill="white"/><text data-c6-role="note-label"',
  );
  await expect(observeThemeTextSurfaces(page, svg, [PROBE])).rejects.toThrow(
    "Terminal surface must supply the opaque background at every visible text pixel",
  );
});

test("terminal observation accepts surface occlusion away from the glyphs", async ({ page }) => {
  const svg = fixtureSvg({}).replace(
    '<text data-c6-role="note-label"',
    '<rect x="200" y="28" width="18" height="34" fill="white"/><text data-c6-role="note-label"',
  );
  const [observation] = await observeThemeTextSurfaces(page, svg, [PROBE]);
  expectReadableProductionSurface(observation, [0xfe, 0xf3, 0xc7], [0x42, 0x20, 0x06]);
});

test("terminal observation rejects a partially transparent child tspan", async ({ page }) => {
  await expect(
    observeThemeTextSurfaces(page, fixtureSvg({ extraCss: "tspan{fill-opacity:0.5}" }), [PROBE]),
  ).rejects.toThrow("Terminal text fill must be visible and opaque");
});

test("terminal observation rejects a child tspan whose paint differs from the report", async ({ page }) => {
  await expect(
    observeThemeTextSurfaces(page, fixtureSvg({ extraCss: "tspan{fill:#422006}" }), [PROBE]),
  ).rejects.toThrow("Terminal text requires uniform resolved paint and font");
});

for (const [name, extraCss] of [
  ["text brightness filter", `${NOTE_TEXT}{filter:brightness(0.2)}`],
  ["text multiply blend", `${NOTE_TEXT}{mix-blend-mode:multiply}`],
] as const) {
  test(`terminal observation rejects ${name}`, async ({ page }) => {
    await expect(
      observeThemeTextSurfaces(page, fixtureSvg({ extraCss }), [PROBE]),
    ).rejects.toThrow("Terminal text requires unfiltered, normally blended paint");
  });
}

test("terminal observation rejects a text stroke that covers the reported fill", async ({ page }) => {
  await expect(observeThemeTextSurfaces(page, fixtureSvg({
    extraCss: `${NOTE_TEXT}{stroke:blue;stroke-width:5px;paint-order:fill stroke}`,
  }), [PROBE])).rejects.toThrow("Terminal text requires fill-only paint");
});

test("terminal observation rejects a text mask that changes glyph opacity", async ({ page }) => {
  const svg = fixtureSvg({ extraCss: `${NOTE_TEXT}{mask:url(#dim-text)}` }).replace(
    "</defs>",
    '<mask id="dim-text" maskUnits="userSpaceOnUse" x="0" y="0" width="240" height="120"><rect width="240" height="120" fill="white" fill-opacity="0.1"/></mask></defs>',
  );
  await expect(observeThemeTextSurfaces(page, svg, [PROBE])).rejects.toThrow(
    "Terminal text requires unmasked paint",
  );
});

interface FixtureOptions {
  declaredTextPaint?: string | null;
  duplicateText?: boolean;
  extraCss?: string;
  tspanDy?: string;
}

function fixtureSvg({
  declaredTextPaint = "#fef3c7",
  duplicateText = false,
  extraCss = "",
  tspanDy = "0",
}: FixtureOptions): string {
  const fill = declaredTextPaint === null ? "" : ` fill="${declaredTextPaint}"`;
  const duplicate = duplicateText
    ? `<text data-c6-role="note-label" data-c6-occurrence="note-0-line-0" x="30" y="60"${fill}>Duplicate</text>`
    : "";
  return `
    <svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="240" height="120" viewBox="0 0 240 120">
      <defs>
        <linearGradient id="terminal-paint"><stop offset="0" stop-color="#fef3c7" /></linearGradient>
      </defs>
      <style>${extraCss}</style>
      <g class="legacy-note">
        <g class="surface-group"><rect data-c6-surface="note-0" x="20" y="20" width="200" height="48" fill="#422006" /></g>
        <text data-c6-role="note-label" data-c6-occurrence="note-0-line-0" x="30" y="48"${fill} font-family="Inter">
          <tspan x="30" dy="${tspanDy}">Terminal note</tspan>
        </text>
        ${duplicate}
      </g>
    </svg>
  `;
}

function packetFillRule(target: string, fill: string) {
  return {
    kind: "rule",
    target,
    family: "packet",
    style: { fill },
  };
}

function renderSequenceRoleSvg(): string {
  return renderBindingSvg(
    [
      "sequenceDiagram",
      "  participant Alice",
      "  participant Bob",
      "  Alice->>Bob: Render through the product binding",
    ].join("\n"),
    JSON.stringify({
      theme: {
        spec: {
          styles: [
            {
              kind: "rule",
              target: "message-label",
              family: "sequence",
              style: { fill: "#f8fafc" },
            },
          ],
        },
      },
      svg: {
        diagram_id: "browser-sequence-role",
        root_background_color: "#0f172a",
      },
    }),
  );
}

function expectReadableProductionSurface(
  observation: Awaited<ReturnType<typeof observeThemeTextSurfaces>>[number],
  expectedTextPaint: [number, number, number],
  expectedBackgroundPaint: [number, number, number],
): void {
  expect(observation.finalTextPaint).toEqual(expectedTextPaint);
  expect(observation.backgroundPaint).toEqual(expectedBackgroundPaint);
  expect(observation.finalFontIdentity.length).toBeGreaterThan(0);
  expect(textSurfaceContrastRatio(observation)).toBeGreaterThanOrEqual(4.5);
  expect(containsTextBounds(observation, 1_000), JSON.stringify(observation)).toBe(true);
  expect(observation.artifactDigest).toMatch(/^[0-9a-f]{64}$/u);
}
