import { expect, test } from "@playwright/test";

import {
  auditMountedSvg,
  classifyRootViewportContainment,
  exactRootViewportResidualEvidenceIsEligible,
  type PaintAuditIndeterminateReason,
  type RootViewportAudit,
} from "./root-viewport-oracle.ts";
import {
  matchingRootViewportResidual,
  parseRootViewportResidualCatalog,
} from "./root-viewport-residuals.ts";

test("exact root viewport residuals bind both SVG artifacts", () => {
  const local = "a".repeat(64);
  const upstream = "b".repeat(64);
  const catalog = parseRootViewportResidualCatalog(
    JSON.stringify({
      schemaVersion: 1,
      comparisonRevision: "browser-root-paint-containment-v10",
      entries: [
        {
          fixture: "xychart/probe",
          localSvgSha256: local,
          upstreamSvgSha256: upstream,
          reason: "deterministic-text-measurement-out-of-domain-extrapolation",
        },
      ],
    }),
  );

  expect(matchingRootViewportResidual(catalog, "xychart/probe", local, upstream)).not.toBeNull();
  expect(
    matchingRootViewportResidual(catalog, "xychart/probe", "c".repeat(64), upstream),
  ).toBeNull();
  expect(() =>
    parseRootViewportResidualCatalog(
      JSON.stringify({
        schemaVersion: 1,
        comparisonRevision: "stale",
        entries: [],
      }),
    ),
  ).toThrow(/revision drifted/u);

  const collected = fixtureAudit({ paintedPixelCount: 0 });
  expect(exactRootViewportResidualEvidenceIsEligible(collected, collected)).toBe(true);
  expect(exactRootViewportResidualEvidenceIsEligible(collected, null)).toBe(false);

  const missingRoot: RootViewportAudit = {
    ...collected,
    root: null,
    paintAudit: { ...collected.paintAudit, status: "missing-root" },
  };
  expect(exactRootViewportResidualEvidenceIsEligible(missingRoot, collected)).toBe(false);

  for (const reason of [
    "capture-boundary",
    "capture-limit",
    "marker-capture-unbounded",
  ] as const) {
    const indeterminate = fixtureAudit({
      status: "indeterminate",
      indeterminateReasons: [reason],
      paintedPixelCount: 0,
    });
    expect(
      exactRootViewportResidualEvidenceIsEligible(indeterminate, collected),
      reason,
    ).toBe(false);
  }
});

test("upstream comparison blocks new edges and deeper structural overflow", () => {
  const upstreamOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "right", depth: 2, paintedPixelCount: 10 }],
  });
  const sameDepth = fixtureAudit({
    structuralOverflows: [
      { edge: "right", depth: 2, paintedPixelCount: 30, tangentStart: 60 },
    ],
  });
  expect(classifyRootViewportContainment(sameDepth, upstreamOverflow)).toBe(
    "upstream-inherited",
  );

  const worseOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "right", depth: 3 }],
  });
  expect(classifyRootViewportContainment(worseOverflow, upstreamOverflow)).toBe("blocking");

  const sparseDistantOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "right", depth: 1, outwardGap: 50 }],
  });
  expect(classifyRootViewportContainment(sparseDistantOverflow, upstreamOverflow)).toBe(
    "blocking",
  );

  const upstreamBottomStrip = fixtureAudit({
    structuralOverflows: [{ edge: "bottom", depth: 1, paintedPixelCount: 100 }],
  });
  const widerBottomStrip = fixtureAudit({
    structuralOverflows: [{ edge: "bottom", depth: 1, paintedPixelCount: 110 }],
  });
  expect(classifyRootViewportContainment(widerBottomStrip, upstreamBottomStrip)).toBe(
    "upstream-inherited",
  );

  const movedLeftStroke = fixtureAudit({
    structuralOverflows: [
      { edge: "left", depth: 4, paintedPixelCount: 20, tangentStart: 30 },
    ],
  });
  const upstreamLeftStroke = fixtureAudit({
    structuralOverflows: [
      { edge: "left", depth: 4, paintedPixelCount: 20, tangentStart: 45 },
    ],
  });
  expect(classifyRootViewportContainment(movedLeftStroke, upstreamLeftStroke)).toBe(
    "upstream-inherited",
  );

  const newBottomOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "bottom", depth: 1 }],
  });
  expect(classifyRootViewportContainment(newBottomOverflow, upstreamOverflow)).toBe(
    "blocking",
  );

  const cornerOverflow = fixtureAudit({
    structuralOverflows: [
      { edge: "top", depth: 1 },
      { edge: "right", depth: 1 },
    ],
  });
  const topOnlyOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "top", depth: 1 }],
  });
  expect(classifyRootViewportContainment(cornerOverflow, topOnlyOverflow)).toBe("blocking");
});

test("browser-owned overflow remains diagnostic without structural paint", () => {
  const browserOwnedOnly = fixtureAudit({
    paintedPixelCount: 10,
    structuralPaintedPixelCount: 0,
  });
  expect(classifyRootViewportContainment(browserOwnedOnly, null)).toBe(
    "browser-owned-diagnostic",
  );

  const browserOwnedAtCaptureBoundary = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-boundary"],
    structuralPaintedPixelCount: 0,
  });
  expect(classifyRootViewportContainment(browserOwnedAtCaptureBoundary, null)).toBe(
    "blocking",
  );

  const structuralOverflow = fixtureAudit({
    structuralOverflows: [{ edge: "right", depth: 1 }],
  });
  expect(classifyRootViewportContainment(structuralOverflow, null)).toBe("blocking");

  const indeterminateBrowserOwned = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter"],
    structuralPaintedPixelCount: 0,
  });
  expect(classifyRootViewportContainment(indeterminateBrowserOwned, null)).toBe("blocking");
  expect(
    classifyRootViewportContainment(indeterminateBrowserOwned, indeterminateBrowserOwned),
  ).toBe("upstream-inherited");
});

test("indeterminate evidence requires a matching no-worse upstream witness", () => {
  const upstreamCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-limit"],
    captureWidthCssPx: 6000,
    paintedPixelCount: 0,
  });
  const localCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-limit"],
    captureWidthCssPx: 5000,
    paintedPixelCount: 0,
  });
  expect(classifyRootViewportContainment(localCaptureLimit, upstreamCaptureLimit)).toBe(
    "upstream-inherited",
  );

  const largerCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-limit"],
    captureWidthCssPx: 6001,
    paintedPixelCount: 0,
  });
  expect(classifyRootViewportContainment(largerCaptureLimit, upstreamCaptureLimit)).toBe(
    "blocking",
  );

  const upstreamBalancedCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-limit"],
    captureWidthCssPx: 6000,
    paintedPixelCount: 0,
    geometryUnion: {
      left: -1000,
      top: 0,
      right: 5000,
      bottom: 100,
      width: 6000,
      height: 100,
    },
  });
  const localDeeperLeftCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-limit"],
    captureWidthCssPx: 5000,
    paintedPixelCount: 0,
    geometryUnion: {
      left: -2000,
      top: 0,
      right: 3000,
      bottom: 100,
      width: 5000,
      height: 100,
    },
  });
  expect(
    classifyRootViewportContainment(
      localDeeperLeftCaptureLimit,
      upstreamBalancedCaptureLimit,
    ),
  ).toBe("blocking");

  const ambiguousCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter", "capture-limit"],
    captureWidthCssPx: 5000,
    paintedPixelCount: 0,
  });
  const upstreamAmbiguousCaptureLimit = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter", "capture-limit"],
    captureWidthCssPx: 6000,
    paintedPixelCount: 0,
  });
  expect(
    classifyRootViewportContainment(
      ambiguousCaptureLimit,
      upstreamAmbiguousCaptureLimit,
    ),
  ).toBe("blocking");

  const noWorseFilteredPaint = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter"],
    structuralOverflows: [{ edge: "right", depth: 1 }],
  });
  expect(classifyRootViewportContainment(noWorseFilteredPaint, noWorseFilteredPaint)).toBe(
    "upstream-inherited",
  );

  const newFilteredPaint = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter"],
    structuralOverflows: [{ edge: "right", depth: 2 }],
  });
  expect(classifyRootViewportContainment(newFilteredPaint, noWorseFilteredPaint)).toBe(
    "blocking",
  );

  const collectedLocal = fixtureAudit({
    structuralOverflows: [{ edge: "right", depth: 1 }],
  });
  expect(classifyRootViewportContainment(collectedLocal, upstreamCaptureLimit)).toBe("blocking");

  const differentIndeterminateReason = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["active-filter"],
    paintedPixelCount: 0,
  });
  expect(
    classifyRootViewportContainment(localCaptureLimit, differentIndeterminateReason),
  ).toBe("blocking");

  const boundary = fixtureAudit({
    status: "indeterminate",
    indeterminateReasons: ["capture-boundary"],
  });
  expect(classifyRootViewportContainment(boundary, boundary)).toBe("blocking");
});

test("browser-mounted painted content remains inside the root viewport", async ({
  page,
}) => {
  const fixtures = [
    {
      name: "svg shapes",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80"/><text x="50" y="55" text-anchor="middle">ok</text></svg>',
    },
    {
      name: "foreignObject and HTML",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 60"><foreignObject x="10" y="10" width="100" height="40"><div xmlns="http://www.w3.org/1999/xhtml" style="width:100px;height:40px">label</div></foreignObject></svg>',
    },
    {
      name: "root CSS sizing preserves max-width at a painted edge",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" width="100%" viewBox="0 0 2412 512" style="max-width:512px"><path d="M128 128 C57.31 128 0 185.31 0 256 S57.31 384 128 384 S256 326.69 256 256 S198.69 128 128 128 Z"/></svg>',
      rootWidth: 512,
    },
  ];

  for (const fixture of fixtures) {
    const audit = await auditMountedSvg(page, {
      svgSource: fixture.svg,
    });
    expect(audit.root, fixture.name).not.toBeNull();
    if (fixture.rootWidth !== undefined) {
      expect(audit.root?.width, fixture.name).toBe(fixture.rootWidth);
    }
    expect(audit.paintedElementCount, fixture.name).toBeGreaterThan(0);
    expect(audit.paintAudit.status, fixture.name).toBe("collected");
    expect(audit.violations, fixture.name).toEqual([]);
  }
});

test("browser-owned text and RoughJS paint remain diagnostic", async ({ page }) => {
  for (const svgSource of [
    '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><text x="95" y="50">browser text width</text></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><foreignObject x="90" y="20" width="30" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="white-space:nowrap">browser text width</div></foreignObject></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><g class="label"><foreignObject x="90" y="20" width="30" height="30" style="overflow:visible"><div xmlns="http://www.w3.org/1999/xhtml" style="display:table-cell;white-space:nowrap;background:black">browser-owned label background</div></foreignObject></g></svg>',
    '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><path data-look="handDrawn" d="M90 50 L120 50" stroke="black"/></svg>',
  ]) {
    const audit = await auditMountedSvg(page, { svgSource });
    expect(audit.violations.length).toBeGreaterThan(0);
    expect(audit.structuralViolations).toEqual([]);
    expect(classifyRootViewportContainment(audit, null)).toBe(
      "browser-owned-diagnostic",
    );
  }
});

test("browser-owned suppression preserves structural foreignObject imagery", async ({
  page,
}) => {
  const fixtures = [
    {
      name: "label CSS background image",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><g class="label"><foreignObject x="90" y="20" width="30" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:30px;height:30px;background-image:linear-gradient(black,black)"></div></foreignObject></g></svg>',
    },
    {
      name: "non-label currentColor SVG",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><foreignObject x="90" y="20" width="30" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:30px;height:30px;color:black"><svg xmlns="http://www.w3.org/2000/svg" width="30" height="30"><rect width="30" height="30" fill="currentColor"/></svg></div></foreignObject></svg>',
    },
  ];

  for (const fixture of fixtures) {
    const audit = await auditMountedSvg(page, { svgSource: fixture.svg });
    expect(audit.structuralViolations.length, fixture.name).toBeGreaterThan(0);
    expect(
      audit.structuralViolations.map((violation) => violation.edge),
      fixture.name,
    ).toContain("right");
    expect(classifyRootViewportContainment(audit, null), fixture.name).toBe("blocking");
  }
});

test("hand-drawn containers retain structural HTML and marker paint", async ({ page }) => {
  const fixtures = [
    {
      name: "foreignObject under a hand-drawn group",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><g data-look="handDrawn"><path d="M10 10 L20 20" stroke="black"/><foreignObject x="90" y="20" width="30" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:30px;height:30px;background:black"></div></foreignObject></g></svg>',
    },
    {
      name: "marker attached to a hand-drawn edge",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><marker id="arrow" viewBox="0 0 10 10" refX="0" refY="5" markerWidth="20" markerHeight="20" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L10 5 L0 10 Z" fill="black"/></marker></defs><path data-look="handDrawn" d="M10 50 L90 50" stroke="black" marker-end="url(#arrow)"/></svg>',
    },
  ];

  for (const fixture of fixtures) {
    const audit = await auditMountedSvg(page, { svgSource: fixture.svg });
    expect(audit.structuralViolations.length, fixture.name).toBeGreaterThan(0);
    expect(
      audit.structuralViolations.map((violation) => violation.edge),
      fixture.name,
    ).toContain("right");
  }
});

test("root crop and layout-offset mutations fail the independent browser oracle", async ({
  page,
}) => {
  const mutations = [
    {
      name: "cropped viewBox",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 50 50"><rect x="60" y="60" width="20" height="20"/></svg>',
      edge: "bottom",
    },
    {
      name: "whole diagram translation",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><g transform="translate(80 80)"><rect x="10" y="10" width="30" height="30"/></g></svg>',
      edge: "bottom",
    },
    {
      name: "foreignObject overflow",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><foreignObject x="80" y="10" width="40" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:40px;height:30px;background:black">label</div></foreignObject></svg>',
      edge: "right",
    },
    {
      name: "line stroke overflow",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><line x1="10" y1="0" x2="90" y2="0" stroke="black" stroke-width="12"/></svg>',
      edge: "top",
    },
    {
      name: "endpoint marker overflow",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><marker id="arrow" viewBox="0 0 10 10" refX="0" refY="5" markerWidth="20" markerHeight="20" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L10 5 L0 10 Z"/></marker></defs><line x1="10" y1="50" x2="100" y2="50" stroke="black" stroke-width="2" marker-end="url(#arrow)"/></svg>',
      edge: "right",
    },
  ];

  for (const mutation of mutations) {
    const audit = await auditMountedSvg(page, {
      svgSource: mutation.svg,
    });
    expect(audit.structuralViolations.length, mutation.name).toBeGreaterThan(0);
    expect(
      audit.structuralViolations.map((violation) => violation.edge),
      mutation.name,
    ).toContain(mutation.edge);
  }
});

test("the sole crop epsilon is coordinate quantization, not a fixture tolerance", async ({
  page,
}) => {
  const audit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="0" y="0" width="100.02" height="100"/></svg>',
  });
  expect(audit.structuralViolations).toHaveLength(1);
});

test("corner paint is attributed to every crossed root edge", async ({ page }) => {
  const audit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="150" y="-10" width="20" height="20"/></svg>',
  });
  const edges = audit.structuralViolations.map((violation) => violation.edge);
  expect(edges).toContain("top");
  expect(edges).toContain("right");
});

test("geometry expands pixel capture without becoming the containment verdict", async ({
  page,
}) => {
  const painted = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="1500" y="20" width="20" height="20"/></svg>',
  });
  expect(painted.paintAudit.status).toBe("collected");
  expect(painted.paintAudit.captureWidthCssPx).toBeGreaterThan(1500);
  expect(painted.violations.map((violation) => violation.edge)).toContain("right");

  const geometryOnly = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="1500" y="20" width="20" height="20" fill="none" stroke="none"/></svg>',
  });
  expect(geometryOnly.paintAudit.captureWidthCssPx).toBeGreaterThan(1500);
  expect(geometryOnly.paintAudit.status).toBe("collected");
  expect(geometryOnly.violations).toEqual([]);
});

test("distant marker paint cannot escape the bounded capture", async ({ page }) => {
  const audit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><marker id="arrow" viewBox="0 0 10 10" refX="-5000" refY="5" markerWidth="20" markerHeight="20" markerUnits="userSpaceOnUse" orient="auto"><path d="M0 0 L10 5 L0 10 Z" fill="black"/></marker></defs><path d="M10 50 L90 50" stroke="black" marker-end="url(#arrow)"/></svg>',
  });

  expect(audit.paintAudit.status).toBe("indeterminate");
  expect(audit.paintAudit.indeterminateReasons).toContain("capture-limit");
  expect(classifyRootViewportContainment(audit, null)).toBe("blocking");
});

test("unbounded filters and over-limit captures fail closed as indeterminate", async ({
  page,
}) => {
  const filtered = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="10" y="10" width="20" height="20" style="filter:drop-shadow(1000px 0 0 red)"/></svg>',
  });
  expect(filtered.paintAudit.status).toBe("indeterminate");
  expect(filtered.paintAudit.indeterminateReasons.join(" ")).toContain("filter");
  expect(filtered.violations).toEqual([]);

  const visiblyFiltered = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="70" y="20" width="20" height="20" style="filter:drop-shadow(20px 0 0 red)"/></svg>',
  });
  expect(visiblyFiltered.paintAudit.status).toBe("indeterminate");
  expect(visiblyFiltered.violations.map((violation) => violation.edge)).toContain("right");

  const boundaryPaint = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><line x1="10" y1="0" x2="90" y2="0" stroke="black" stroke-width="80"/></svg>',
  });
  expect(boundaryPaint.paintAudit.status).toBe("indeterminate");
  expect(boundaryPaint.paintAudit.indeterminateReasons.join(" ")).toContain("boundary");
  expect(boundaryPaint.violations.map((violation) => violation.edge)).toContain("top");

  const tallButBounded = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="4400" viewBox="0 0 100 4400"><rect x="20" y="4380" width="20" height="20"/></svg>',
  });
  expect(tallButBounded.paintAudit.status).toBe("collected");
  expect(tallButBounded.paintAudit.captureHeightCssPx).toBeGreaterThan(4096);

  const overDimensionLimit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="20000" y="20" width="20" height="20"/></svg>',
  });
  expect(overDimensionLimit.paintAudit.status).toBe("indeterminate");
  expect(overDimensionLimit.paintAudit.captureWidthCssPx).toBeGreaterThan(16_384);
  expect(overDimensionLimit.paintAudit.indeterminateReasons).toContain("capture-limit");
  expect(overDimensionLimit.violations).toEqual([]);

  const overAreaLimit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="5000" height="4000" viewBox="0 0 5000 4000"><rect x="20" y="20" width="20" height="20"/></svg>',
  });
  expect(overAreaLimit.paintAudit.status).toBe("indeterminate");
  expect(overAreaLimit.paintAudit.indeterminateReasons).toContain("capture-limit");
  expect(overAreaLimit.violations).toEqual([]);

  const brokenImage = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><image href="data:image/png;base64,invalid" x="10" y="10" width="20" height="20"/></svg>',
  });
  expect(brokenImage.paintAudit.status).toBe("indeterminate");
  expect(brokenImage.paintAudit.indeterminateReasons.join(" ")).toContain("image");
});

function fixtureAudit({
  status = "collected",
  indeterminateReasons = [],
  captureWidthCssPx = 132,
  captureHeightCssPx = 132,
  paintedPixelCount = 10,
  structuralPaintedPixelCount = paintedPixelCount,
  paintedOverflows,
  structuralOverflows,
  geometryUnion,
  rootWidth = 100,
  rootHeight = 100,
}: {
  status?: RootViewportAudit["paintAudit"]["status"];
  indeterminateReasons?: PaintAuditIndeterminateReason[];
  captureWidthCssPx?: number;
  captureHeightCssPx?: number;
  paintedPixelCount?: number;
  structuralPaintedPixelCount?: number;
  paintedOverflows?: FixtureOverflow[];
  structuralOverflows?: FixtureOverflow[];
  geometryUnion?: RootViewportAudit["geometryUnion"];
  rootWidth?: number;
  rootHeight?: number;
} = {}): RootViewportAudit {
  const root = {
    left: 0,
    top: 0,
    right: rootWidth,
    bottom: rootHeight,
    width: rootWidth,
    height: rootHeight,
  };
  const resolvedPaintedOverflows =
    paintedOverflows ??
    structuralOverflows ??
    (paintedPixelCount === 0
      ? []
      : [{ edge: "right" as const, depth: 1, paintedPixelCount }]);
  const resolvedStructuralOverflows =
    structuralOverflows ??
    (structuralPaintedPixelCount === 0
      ? []
      : resolvedPaintedOverflows.map((overflow) => ({
          ...overflow,
          paintedPixelCount: structuralPaintedPixelCount,
        })));
  const violations = resolvedPaintedOverflows.map((overflow) =>
    fixtureViolation(root, overflow),
  );
  const structuralViolations = resolvedStructuralOverflows.map((overflow) =>
    fixtureViolation(root, overflow),
  );
  const structuralPixelCount = structuralViolations.reduce(
    (total, violation) => total + violation.paintedPixelCount,
    0,
  );
  return {
    root,
    geometryUnion: geometryUnion === undefined ? root : geometryUnion,
    paintedElementCount: 1,
    paintAudit: {
      status,
      guardCssPx: 16,
      captureWidthCssPx,
      captureHeightCssPx,
      indeterminateReasons,
    },
    violations,
    structuralViolations,
    structuralPixelKeys: Array.from(
      { length: structuralPixelCount },
      (_, index) => `${index},0`,
    ),
  };
}

type FixtureOverflow = {
  edge: "top" | "right" | "bottom" | "left";
  depth: number;
  outwardGap?: number;
  paintedPixelCount?: number;
  tangentStart?: number;
  tangentLength?: number;
};

function fixtureViolation(
  root: NonNullable<RootViewportAudit["root"]>,
  {
    edge,
    depth,
    outwardGap = 0,
    paintedPixelCount = 10,
    tangentStart = 20,
    tangentLength = 10,
  }: FixtureOverflow,
): RootViewportAudit["violations"][number] {
  const horizontal = edge === "top" || edge === "bottom";
  const left = horizontal
    ? tangentStart
    : edge === "left"
      ? -depth - outwardGap
      : Math.ceil(root.width) + outwardGap;
  const top = horizontal
    ? edge === "top"
      ? -depth - outwardGap
      : Math.ceil(root.height) + outwardGap
    : tangentStart;
  const width = horizontal ? tangentLength : depth;
  const height = horizontal ? depth : tangentLength;
  return {
    edge,
    paintedPixelCount,
    rect: {
      left,
      top,
      right: left + width,
      bottom: top + height,
      width,
      height,
    },
    reachesAuditBoundary: false,
  };
}
