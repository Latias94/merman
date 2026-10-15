import assert from "node:assert/strict";
import test from "node:test";
import { themeFamilyTreatment } from "./theme-design.ts";

test("only explicit known family designs affect the displayed treatment", () => {
  const preset = {
    available: true,
    qualified_cells: [{ family_id: "state", admission_status: "portable" }],
    family_designs: [
      { family_id: "class", treatment: "base_only" },
      { family_id: "flowchart", treatment: "dedicated" },
      { family_id: "sequence", treatment: "future-design" },
    ],
  };
  assert.equal(themeFamilyTreatment(preset, "class"), "base_only");
  assert.equal(themeFamilyTreatment(preset, "flowchart"), "dedicated");
  for (const family of ["state", "sequence", "future-family", undefined]) {
    assert.equal(themeFamilyTreatment(preset, family), "unreviewed");
  }
  assert.equal(themeFamilyTreatment(undefined, "flowchart"), "unreviewed");
  assert.equal(themeFamilyTreatment({ family_designs: [] }, "flowchart"), "unreviewed");
  assert.equal(preset.family_designs[2].treatment, "future-design");
});
