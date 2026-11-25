import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { freezeRenderOperation } from "../runtime/merman-operation-input.ts";
import type { RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import { captureProjectHandoff, generateProjectHandoff, projectHandoffMatchesWorkspace, ProjectHandoffError } from "./project-handoff.ts";
import { themeCatalogFixture, themeResolutionFixture } from "./theme-appearance.test-fixture.ts";

const versions = { merman: "0.11.0", mermaid: "12.3.0" };
const catalog = themeCatalogFixture();
const raw = '{\n  "schema_version": 2, "extends": "studio",\n  "seed": { "accent": "#123456" }\n}\n';
const workspace: WorkspaceSnapshot = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: '---\nconfig:\n  theme: dark\n---\n%%{init: {"theme":"neutral"}}%%\nflowchart LR\nA --> B',
  mermaidConfig: '{ "theme":"forest", "flowchart":{"htmlLabels":false}, "fontFamily":"Authored font" }',
  diagramFont: "trebuchet",
  themeDocumentJson: raw,
  themeAppearance: "dark",
  svgPipeline: "resvg-safe",
};
const targets = ["rust", "cli", "web"] as const;

function completed(value: WorkspaceSnapshot): RenderCoordinatorState {
  const operation = freezeRenderOperation({
    workspace: value, themeCatalog: catalog, versions, viewport: null,
    asciiEnabled: false, compareEnabled: false, diagnosticsEnabled: false,
    layoutEnvironment: { containerWidth: 923, containerHeight: 611, screenAvailableWidth: 1440 },
    runtimeSnapshot: { unixMillis: 1_794_633_825_678, localOffsetMinutes: 345 },
  });
  return { status: "success", merman: { status: "success" }, snapshot: { operation, publicationId: 7 } } as RenderCoordinatorState;
}

function capture(value = workspace, state = completed(value)) {
  return captureProjectHandoff({
    workspace: value, renderState: state, themeCatalog: catalog, versions, capturedAt: "2026-11-05T00:00:00.000Z",
    resolveTheme: themeResolutionFixture,
  });
}

test("diagram handoff preserves configured source, runtime clock and the authored Theme v2 bytes", () => {
  const snapshot = capture();
  for (const target of targets) {
    const bundle = generateProjectHandoff(snapshot, { scope: "diagram", target });
    const options = JSON.parse(bundle.files["options.json"]);
    assert.equal(bundle.files["source.mmd"], workspace.code);
    assert.equal(bundle.files["playground-config.json"], workspace.mermaidConfig);
    assert.equal(bundle.files["diagram.mmd"], snapshot.operation!.configuredSource);
    assert.match(bundle.files["diagram.mmd"], /---\n%%\{init: \{"theme":"forest"/);
    assert.deepEqual(JSON.parse(bundle.files["site-config.json"]), options.site_config);
    assert.equal(options.site_config.flowchart, undefined);
    assert.notEqual(options.site_config.fontFamily, "Authored font");
    assert.deepEqual(options.layout, { container_width: 923, container_height: 611, screen_available_width: 1440 });
    assert.equal(options.fixed_unix_millis, 1_794_633_825_678);
    assert.equal(options.fixed_local_offset_minutes, 345);
    assert.equal(bundle.files["theme.json"], raw);
    assert.ok(bundle.files["options.json"].includes(`"document":${raw}`));
    assert.deepEqual(options.theme, { document: JSON.parse(raw), appearance: "dark" });
    assert.equal(JSON.parse(bundle.files["handoff.json"]).publication_id, 7);
  }
  const rust = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  assert.match(rust.code, /ThemeCompiler::new\(\)\.compile_json\(include_str!\("\.\.\/theme\.json"\)\.as_bytes\(\), Some\(Appearance::Dark\)\)/);
  assert.match(rust.code, /\.with_native_theme\(theme\)/);
  assert.match(rust.code, /\.with_fixed_unix_millis\(1794633825678\)/);
  assert.match(rust.files["Cargo.toml"], /features = \["math"\]/);
  const cli = generateProjectHandoff(snapshot, { scope: "diagram", target: "cli" });
  assert.match(cli.code, /--theme theme\.json --appearance dark/);
  assert.match(cli.code, /--fixed-unix-millis 1794633825678 --fixed-local-offset-minutes 345/);
  const web = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  assert.match(web.code, /renderSvgResultWithTextMeasurer\(source, measurement.measure, optionsJson\)/);
  assert.ok(web.code.indexOf("assertNavigableSvgForDom(result.svg)") < web.code.indexOf("shadow.replaceChildren(root)"));
});

test("theme-only handoff uses an independent source and validates the document without a diagram render", () => {
  const input = { workspace, renderState: { status: "empty" } as const, capturedAt: "2026-11-05T00:00:00.000Z" };
  const snapshot = captureProjectHandoff({ ...input, resolveTheme: (received, appearance) => {
    assert.equal(received, raw);
    assert.equal(appearance, "dark");
    return themeResolutionFixture();
  } });
  for (const target of targets) {
    const bundle = generateProjectHandoff(snapshot, { scope: "theme", target });
    const options = JSON.parse(bundle.files["options.json"]);
    assert.equal(bundle.files["diagram.mmd"], "flowchart LR\nA[Theme] --> B[Your project]");
    assert.equal(bundle.files["theme.json"], raw);
    assert.deepEqual(options.theme, { document: JSON.parse(raw), appearance: "dark" });
    assert.equal(options.fixed_unix_millis, 0);
    assert.equal(bundle.files["site-config.json"], undefined);
    assert.equal(bundle.files["source.mmd"], undefined);
  }
  const invalid = captureProjectHandoff({ ...input, resolveTheme: () => ({ ok: false, line: null, column: null, path: "seed.accent", message: "Invalid color" }) });
  assert.throws(() => generateProjectHandoff(invalid, { scope: "theme", target: "web" }),
    (error) => error instanceof ProjectHandoffError && error.code === "invalid-configuration" && error.message === "Invalid color");
});

test("preset handoff uses catalog IDs and no extra theme file", () => {
  const selected = { ...workspace, themeDocumentJson: null, themePresetId: "studio" };
  const snapshot = capture(selected);
  const rust = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  assert.match(rust.code, /BuiltinPreset::from_id\("studio"\)/);
  assert.match(rust.code, /ThemeSelection::preset\(preset, Some\(Appearance::Dark\)\)/);
  assert.equal(rust.files["theme.json"], undefined);
  const cli = generateProjectHandoff(snapshot, { scope: "theme", target: "cli" });
  assert.match(cli.code, /--theme studio --appearance dark/);
  const web = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  assert.deepEqual(JSON.parse(web.files["options.json"]).theme, { preset: "studio", appearance: "dark" });
  const unavailable = capture({ ...selected, themePresetId: "retired-preset" });
  assert.throws(() => generateProjectHandoff(unavailable, { scope: "theme", target: "web" }),
    (error) => error instanceof ProjectHandoffError && error.code === "preset-unavailable");
});

test("diagram handoff requires the current render and freezes its capture against later workspace changes", () => {
  const current = completed(workspace);
  const snapshot = capture(workspace, current);
  const original = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  for (const edit of [
    { code: "flowchart LR\nC --> D" }, { mermaidConfig: "{}" }, { themeAppearance: "light" as const },
    { themeDocumentJson: `${raw}\n` }, { svgPipeline: "parity" as const },
  ]) {
    const next = { ...workspace, ...edit };
    assert.equal(projectHandoffMatchesWorkspace(snapshot, next), false);
    const stale = capture(next, current);
    assert.throws(() => generateProjectHandoff(stale, { scope: "diagram", target: "web" }),
      (error) => error instanceof ProjectHandoffError && error.code === "render-not-current");
  }
  assert.deepEqual(generateProjectHandoff(snapshot, { scope: "diagram", target: "web" }), original);
});
