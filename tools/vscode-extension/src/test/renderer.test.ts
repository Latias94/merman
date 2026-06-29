import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { renderMermanArgs } from "../render-options.js";

describe("renderer arguments", () => {
  it("passes preview Mermaid themes through to merman-cli", () => {
    assert.deepEqual(renderMermanArgs({ format: "svg", theme: "forest" }), [
      "-q",
      "-i",
      "-",
      "-o",
      "-",
      "-e",
      "svg",
      "--theme",
      "forest",
    ]);
  });

  it("omits source/default theme overrides", () => {
    assert.deepEqual(renderMermanArgs({ format: "svg", theme: "source" }), [
      "-q",
      "-i",
      "-",
      "-o",
      "-",
      "-e",
      "svg",
    ]);
  });
});
