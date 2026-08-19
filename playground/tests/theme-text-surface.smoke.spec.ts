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
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  const wasmResponse = page.waitForResponse((response) =>
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/u.test(response.url()),
  );
  await page.goto(`./#${hash}`, { waitUntil: "domcontentloaded" });
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

test("terminal observation exposes a tspan moved outside its owning surface", async ({
  page,
}) => {
  const svg = fixtureSvg({ tspanDy: "4em" });
  const movedProbe: ThemeTextSurfaceProbe = {
    ...PROBE,
    textSelector: `${NOTE_TEXT} tspan`,
  };
  const [observation] = await observeThemeTextSurfaces(page, svg, [movedProbe]);

  expect(containsTextBounds(observation, 1_000)).toBe(false);
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
        <rect data-c6-surface="note-0" x="20" y="20" width="200" height="48" fill="#422006" />
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
  expect(containsTextBounds(observation, 1_000)).toBe(true);
  expect(observation.artifactDigest).toMatch(/^[0-9a-f]{64}$/u);
}
