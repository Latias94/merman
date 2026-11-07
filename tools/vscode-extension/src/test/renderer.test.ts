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

  it("passes a concrete Merman selection without sending host preferences to the CLI", () => {
    const args = renderMermanArgs({ format: "svg", themePreset: "editor", themeAppearance: "dark" });
    assert.deepEqual(args.slice(-4), ["--theme-preset", "editor", "--theme-appearance", "dark"]);
    assert.equal(args.includes("follow-editor"), false);
    assert.equal(args.includes("--theme"), false);
    assert.deepEqual(renderMermanArgs({
      format: "svg",
      themeFilePath: "/private/theme.json",
      themeAppearance: "light",
    }).slice(-4), ["--theme-file", "/private/theme.json", "--theme-appearance", "light"]);
  });

  it("keeps graphical theme options out of both terminal render modes", () => {
    for (const format of ["ascii", "unicode"] as const) {
      assert.deepEqual(renderMermanArgs({
        format,
        themePreset: "editor",
        themeFilePath: "/private/theme.json",
        themeAppearance: "dark",
        background: "white",
      }), ["render", "-", "--output", "-", "--format", format, "--quiet"]);
    }
  });

  it("identifies sanitized SVG input explicitly when encoding an image", () => {
    const args = renderMermanArgs({ format: "png", inputKind: "svg", outputPath: "diagram.png" });
    assert.deepEqual(args.slice(-2), ["--input-kind", "svg"]);
  });
});
