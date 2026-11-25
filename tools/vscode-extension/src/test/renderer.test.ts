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
    assert.equal(previewCliBackground("theme"), undefined);
    assert.equal(previewCliBackground("paper"), "white");
    assert.equal(previewCliBackground("transparent"), "transparent");
    assert.equal(previewCliBackground("dark"), PREVIEW_DARK_BACKGROUND_COLOR);
  });

  it("uses the same theme ID and concrete appearance for SVG and colored text output", () => {
    for (const format of ["svg", "ascii", "unicode"] as const) {
      assert.deepEqual(renderMermanArgs({ format, theme: "studio", themeAppearance: "dark" }), [
        "render", "-", "--output", "-", "--format", format, "--quiet",
        "--theme", "studio", "--appearance", "dark", "--theme-precedence", "host",
        ...(format === "svg" ? [] : ["--ascii-color", "truecolor"]),
      ]);
    }
  });

  it("passes the frozen custom theme as a path in both graphical and terminal modes", () => {
    for (const format of ["svg", "ascii", "unicode"] as const) {
      assert.deepEqual(renderMermanArgs({ format, themeFilePath: "/private/theme.json", themeAppearance: "light" }).slice(7, 13), [
        "--theme", "/private/theme.json", "--appearance", "light", "--theme-precedence", "host",
      ]);
    }
  });

  it("keeps the graphical canvas override with an explicit theme selection", () => {
    assert.deepEqual(renderMermanArgs({
      format: "svg",
      theme: "studio",
      themeAppearance: "dark",
      background: "transparent",
    }).slice(7), ["--theme", "studio", "--appearance", "dark", "--theme-precedence", "host", "--background", "transparent"]);
  });

  it("identifies sanitized SVG input explicitly when encoding an image", () => {
    const args = renderMermanArgs({ format: "png", inputKind: "svg", outputPath: "diagram.png" });
    assert.deepEqual(args.slice(-2), ["--input-kind", "svg"]);
  });
});
