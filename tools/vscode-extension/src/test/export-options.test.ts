import * as assert from "node:assert/strict";
import * as path from "node:path";
import { describe, it } from "node:test";

import {
  EXPORT_PRESETS,
  defaultExportPath,
  displayExportBasename,
  exportFilters,
  exportPresetForFormat,
  pngClipboardArgs,
  pngClipboardAvailable,
  pngClipboardCommand,
} from "../export-options.js";

describe("export options", () => {
  it("builds predictable export paths beside the source file", () => {
    assert.equal(
      defaultExportPath("/workspace/docs/notes.md", "notes-mermaid-2", "svg"),
      path.join("/workspace/docs", "notes-mermaid-2.svg"),
    );
  });

  it("routes supported export formats to save dialog filters", () => {
    assert.deepEqual(exportFilters("svg"), { "SVG image": ["svg"] });
    assert.deepEqual(exportFilters("png"), { "PNG image": ["png"] });
  });

  it("displays only the saved file basename across URI shapes", () => {
    assert.equal(
      displayExportBasename({
        fsPath: "C:\\Users\\frank\\diagram.svg",
        path: "/c%3A/Users/frank/diagram.svg",
      }),
      "diagram.svg",
    );
    assert.equal(
      displayExportBasename({
        fsPath: "C:\\Users\\frank\\diagram.png",
      }),
      "diagram.png",
    );
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

  it("disables PNG clipboard copy for remote extension hosts", () => {
    assert.equal(pngClipboardAvailable("win32", undefined), true);
    assert.equal(pngClipboardAvailable("linux", "ssh-remote"), false);
    assert.equal(pngClipboardAvailable("freebsd", undefined), false);
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

  it("passes Windows clipboard paths as arguments instead of PowerShell source", () => {
    const dangerousPath = "C:\\tmp\\diagram$(Write-Output injected)'`().png";
    const args = pngClipboardArgs("win32", dangerousPath);
    const script = args[3] ?? "";

    assert.equal(args.at(-1), dangerousPath);
    assert.match(script, /param\(\[string\]\$imagePath\)/);
    assert.doesNotMatch(script, /\$args\b/);
    assert.doesNotMatch(script, /\$\(Write-Output injected\)/);
    assert.doesNotMatch(script, /diagram/);
  });
});
