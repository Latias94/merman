import assert from "node:assert/strict";
import test from "node:test";

import * as coreRuntime from "../dist/runtime-core.js";
import { svgPlanJson } from "../dist/runtime-render.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";
import { webPackages } from "./surface-manifest.mjs";

test("SVG plan facade preserves the generated object payload and encodes options", async () => {
  const calls = [];
  const expected = {
    schema_version: 1,
    planned_operation_id: "svg",
    diagram_type: "flowchart-v2",
    required_capability_ids: ["layout-elk", "svg"],
    missing_capability_ids: ["layout-elk"],
    ready: false,
  };
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      svgPlanJson(source, optionsJson) {
        calls.push({ source, optionsJson });
        return expected;
      },
    }),
    {
      initMerman: coreRuntime.initMerman,
      svgPlanJson,
    },
  );

  await runtime.initMerman();
  const result = runtime.svgPlanJson("flowchart TD\n  A --> B", {
    site_config: { layout: "elk" },
  });

  assert.equal(result, expected);
  assert.deepEqual(calls, [
    {
      source: "flowchart TD\n  A --> B",
      optionsJson: JSON.stringify({ site_config: { layout: "elk" } }),
    },
  ]);
});

test("only SVG renderer package entries expose the SVG plan facade", () => {
  for (const descriptor of webPackages) {
    const exposesSvgPlan = descriptor.runtimeExportNames.includes("svgPlanJson");
    assert.equal(
      exposesSvgPlan,
      descriptor.id === "full" || descriptor.id === "render",
      `${descriptor.name} has the wrong SVG plan facade`,
    );
  }
});
