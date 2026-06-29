import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { defaultExportPath, exportFilters } from "../export-options.js";

describe("export options", () => {
  it("builds predictable export paths beside the source file", () => {
    assert.equal(
      defaultExportPath("/workspace/docs/notes.md", "notes-mermaid-2", "svg"),
      "/workspace/docs/notes-mermaid-2.svg",
    );
  });

  it("routes supported export formats to save dialog filters", () => {
    assert.deepEqual(exportFilters("svg"), { "SVG image": ["svg"] });
    assert.deepEqual(exportFilters("png"), { "PNG image": ["png"] });
  });
});
