import { expect, test } from "@playwright/test";

import { auditMountedSvg } from "./root-viewport-oracle.ts";

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
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><foreignObject x="80" y="10" width="40" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:40px;height:30px">label</div></foreignObject></svg>',
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
    expect(audit.violations.length, mutation.name).toBeGreaterThan(0);
    expect(
      audit.violations.map((violation) => violation.edge),
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
  expect(audit.violations).toHaveLength(1);
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

  const overLimit = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect x="5000" y="20" width="20" height="20"/></svg>',
  });
  expect(overLimit.paintAudit.status).toBe("indeterminate");
  expect(overLimit.paintAudit.captureWidthCssPx).toBeGreaterThan(4096);
  expect(overLimit.paintAudit.indeterminateReasons.join(" ")).toContain("exceeds");
  expect(overLimit.violations).toEqual([]);

  const brokenImage = await auditMountedSvg(page, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><image href="data:image/png;base64,invalid" x="10" y="10" width="20" height="20"/></svg>',
  });
  expect(brokenImage.paintAudit.status).toBe("indeterminate");
  expect(brokenImage.paintAudit.indeterminateReasons.join(" ")).toContain("image");
});
