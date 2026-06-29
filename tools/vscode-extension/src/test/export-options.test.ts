import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  EXPORT_PRESETS,
  defaultExportPath,
  exportFilters,
  exportPresetForFormat,
  pngClipboardArgs,
  pngClipboardCommand,
} from "../export-options.js";

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

  it("exposes export presets for quick-pick flows", () => {
    assert.deepEqual(
      EXPORT_PRESETS.map((preset) => `${preset.label}:${preset.format}:${preset.openAfterExport}`),
      ["SVG:svg:false", "PNG:png:false", "SVG and Open:svg:true", "PNG and Open:png:true"],
    );
    assert.equal(exportPresetForFormat("svg").label, "SVG");
    assert.equal(exportPresetForFormat("png").label, "PNG");
  });

  it("selects platform clipboard commands for PNG copy", () => {
    assert.equal(pngClipboardCommand("darwin"), "osascript");
    assert.equal(pngClipboardCommand("win32"), "powershell.exe");
    assert.equal(pngClipboardCommand("linux"), "wl-copy");
    assert.equal(pngClipboardCommand("freebsd"), undefined);
  });

  it("builds platform clipboard command arguments", () => {
    assert.deepEqual(pngClipboardArgs("linux", "/tmp/chart.png"), [
      "--type",
      "image/png",
    ]);
    const darwinArgs = pngClipboardArgs("darwin", "/tmp/chart.png");
    assert.ok(darwinArgs[1]?.includes("PNGf"));
    assert.ok(pngClipboardArgs("win32", "C:\\chart.png").join(" ").includes("SetImage"));
  });
});
