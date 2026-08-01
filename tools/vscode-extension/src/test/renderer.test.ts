import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { renderMermanArgs } from "../render-options.js";
import {
  PREVIEW_DARK_BACKGROUND_COLOR,
  previewCliBackground,
} from "../preview-background.js";

describe("renderer arguments", () => {
  it("passes preview Mermaid themes through to merman-cli", () => {
    assert.deepEqual(renderMermanArgs({ format: "svg", theme: "forest" }), [
      "render",
      "-",
      "--output",
      "-",
      "--format",
      "svg",
      "--quiet",
      "--theme",
      "forest",
    ]);
  });

  it("omits source/default theme overrides", () => {
    assert.deepEqual(renderMermanArgs({ format: "svg", theme: "source" }), [
      "render",
      "-",
      "--output",
      "-",
      "--format",
      "svg",
      "--quiet",
    ]);
  });

  it("uses the native output path and background options for graphical output", () => {
    assert.deepEqual(
      renderMermanArgs({
        format: "png",
        outputPath: "diagram.png",
        background: "transparent",
      }),
      [
        "render",
        "-",
        "--output",
        "diagram.png",
        "--format",
        "png",
        "--quiet",
        "--background",
        "transparent",
      ],
    );
  });

  it("does not pass graphical background options to text output", () => {
    assert.deepEqual(renderMermanArgs({ format: "ascii", background: "transparent" }), [
      "render",
      "-",
      "--output",
      "-",
      "--format",
      "ascii",
      "--quiet",
    ]);
    assert.deepEqual(renderMermanArgs({ format: "unicode", background: "white" }), [
      "render",
      "-",
      "--output",
      "-",
      "--format",
      "unicode",
      "--quiet",
    ]);
  });

  it("maps preview background choices to exported render backgrounds", () => {
    assert.equal(previewCliBackground("paper"), "white");
    assert.equal(previewCliBackground("transparent"), "transparent");
    assert.equal(previewCliBackground("dark"), PREVIEW_DARK_BACKGROUND_COLOR);
  });
});
