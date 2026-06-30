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

  it("passes text preview formats and explicit backgrounds through to merman-cli", () => {
    assert.deepEqual(renderMermanArgs({ format: "ascii", background: "transparent" }), [
      "-q",
      "-i",
      "-",
      "-o",
      "-",
      "-e",
      "ascii",
      "--background-color",
      "transparent",
    ]);
    assert.deepEqual(renderMermanArgs({ format: "unicode", background: "white" }), [
      "-q",
      "-i",
      "-",
      "-o",
      "-",
      "-e",
      "unicode",
      "--background-color",
      "white",
    ]);
  });

  it("maps preview background choices to exported render backgrounds", () => {
    assert.equal(previewCliBackground("paper"), "white");
    assert.equal(previewCliBackground("transparent"), "transparent");
    assert.equal(previewCliBackground("dark"), PREVIEW_DARK_BACKGROUND_COLOR);
  });
});
