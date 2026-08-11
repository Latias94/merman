import { expect, test } from "@playwright/test";

import {
  auditMountedSvg,
  ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
} from "./root-viewport-oracle.ts";

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
  ];

  for (const fixture of fixtures) {
    const audit = await page.evaluate(auditMountedSvg, {
      svgSource: fixture.svg,
      quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
    });
    expect(audit.root, fixture.name).not.toBeNull();
    expect(audit.paintedElementCount, fixture.name).toBeGreaterThan(0);
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
    },
    {
      name: "whole diagram translation",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><g transform="translate(80 80)"><rect x="10" y="10" width="30" height="30"/></g></svg>',
    },
    {
      name: "foreignObject overflow",
      svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50"><foreignObject x="80" y="10" width="40" height="30"><div xmlns="http://www.w3.org/1999/xhtml" style="width:40px;height:30px">label</div></foreignObject></svg>',
    },
  ];

  for (const mutation of mutations) {
    const audit = await page.evaluate(auditMountedSvg, {
      svgSource: mutation.svg,
      quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
    });
    expect(audit.violations.length, mutation.name).toBeGreaterThan(0);
  }
});

test("the sole crop epsilon is coordinate quantization, not a fixture tolerance", async ({
  page,
}) => {
  const audit = await page.evaluate(auditMountedSvg, {
    svgSource:
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="0" y="0" width="100.02" height="100"/></svg>',
    quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
  });
  expect(audit.violations).toHaveLength(1);
});
