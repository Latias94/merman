import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { freezeRenderOperation } from "../runtime/merman-operation-input.ts";
import type { RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import {
  captureProjectHandoff,
  generateProjectHandoff,
  projectHandoffMatchesWorkspace,
  ProjectHandoffError,
} from "./project-handoff.ts";

const versions = { merman: "0.10.0-alpha.1", mermaid: "12.3.0" };
const presets = [{ id: "one-dark", available: true }];
const recipe = '{\n  "schema_version": 1, "kind": "design",\n  "design": {"tokens":{"ink":{"type":"paint","value":"#123456"}},"styles":{"text":{"fill":{"ref":"ink"}}}}\n}';
const workspace: WorkspaceSnapshot = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: '---\nconfig:\n  theme: dark\n---\n%%{init: {"theme":"neutral"}}%%\nflowchart LR\nA --> B',
  mermaidConfig: '{ "theme":"forest", "flowchart":{"htmlLabels":false,"look":"neo"}, "fontFamily":"Authored font" }',
  diagramTheme: "default",
  diagramFont: "trebuchet",
  textMeasurementMode: "browser",
  themeRecipeJson: recipe,
  themeRecipeLabel: { name: "Atlas", source: "exploration" },
  svgPipeline: "resvg-safe",
};

function completed(value: WorkspaceSnapshot, extra = {}): RenderCoordinatorState {
  const operation = freezeRenderOperation({
    workspace: value,
    asciiEnabled: false, compareEnabled: false, diagnosticsEnabled: false,
    layoutEnvironment: { containerWidth: 923, containerHeight: 611, screenAvailableWidth: 1440 },
    versions, viewport: null, themePresets: presets,
    ...extra,
  });
  // Only the coordinator's completion and operation fields are read by the handoff.
  return { status: "success", merman: { status: "success" }, snapshot: { operation, publicationId: 7 } } as RenderCoordinatorState;
}

function capture(value = workspace, state = completed(value)) {
  return captureProjectHandoff({
    workspace: value, renderState: state, presets, versions, capturedAt: "2026-11-05T00:00:00.000Z",
    // Fixture recipes are already accepted; rejection cases supply their own compiler result.
    validateThemeRecipe: () => undefined,
  });
}

test("current-diagram handoff preserves the exact frozen source, options and configuration owners", () => {
  const state = completed(workspace);
  const snapshot = capture(workspace, state);
  assert.equal(snapshot.publicationId, 7);
  for (const target of ["rust", "cli", "web"] as const) {
    const bundle = generateProjectHandoff(snapshot, { scope: "diagram", target });
    const options = JSON.parse(bundle.files["options.json"]);
    assert.equal(bundle.files["source.mmd"], workspace.code);
    assert.equal(bundle.files["playground-config.json"], workspace.mermaidConfig);
    assert.equal(bundle.files["diagram.mmd"], snapshot.operation!.configuredSource);
    assert.match(bundle.files["diagram.mmd"], /---\n%%\{init: \{"theme":"forest"/);
    assert.ok(bundle.files["diagram.mmd"].indexOf('"theme":"forest"') < bundle.files["diagram.mmd"].indexOf('"theme":"neutral"'));
    assert.equal(options.site_config.theme, "forest");
    assert.equal(options.site_config.flowchart, undefined);
    assert.notEqual(options.site_config.fontFamily, "Authored font");
    assert.deepEqual(JSON.parse(bundle.files["site-config.json"]), options.site_config);
    assert.deepEqual(options.layout, { container_width: 923, container_height: 611, screen_available_width: 1440 });
    assert.deepEqual(JSON.parse(bundle.files["handoff.json"]).versions, versions);
  }
});

test("independent recipes remain authored bytes and never turn exploration names into preset IDs", () => {
  const snapshot = capture();
  for (const target of ["rust", "cli", "web"] as const) {
    const bundle = generateProjectHandoff(snapshot, { scope: "diagram", target });
    assert.equal(bundle.files["theme.json"], recipe);
    assert.ok(bundle.files["options.json"].includes(`"theme":${recipe}`));
    assert.deepEqual(JSON.parse(bundle.files["options.json"]).theme, JSON.parse(recipe));
    assert.doesNotMatch(bundle.code, /(?:preset[^\n]*atlas|from_id\("atlas"\))/i);
    assert.doesNotMatch(bundle.files["options.json"], /"recipe"\s*:/);
  }
  const rust = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  assert.match(rust.code, /compile_recipe/);
  assert.match(rust.files["Cargo.toml"], /path = "\.\.\/merman\/crates\/merman"/);
  const web = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  assert.equal(JSON.parse(web.files["package.json"]).dependencies["@mermanjs/web"], "file:../merman/platforms/web/packages/full");
  assert.match(web.code, /renderSvgResultWithTextMeasurer/);
  assert.match(web.code, /createBrowserTextMeasurementSession/);
});

test("Rust asset files remain the default with paths relative to src/main.rs", () => {
  const snapshot = capture();
  for (const scope of ["diagram", "theme"] as const) {
    const bundle = generateProjectHandoff(snapshot, { scope, target: "rust" });
    assert.deepEqual(generateProjectHandoff(snapshot, { scope, target: "rust", rustAssets: "files" }), bundle);
    assert.match(bundle.code, /include_str!\("\.\.\/theme\.json"\)/);
    assert.match(bundle.code, /include_str!\("\.\.\/diagram\.mmd"\)/);
    assert.match(bundle.code, /include_str!\("\.\.\/site-config\.json"\)/);
    const readme = bundle.files["README.txt"];
    assert.match(readme, /include_str! paths are relative to src\/main\.rs/);
    assert.match(readme, /project root beside Cargo\.toml/);
    assert.ok(readme.includes("project/\n  Cargo.toml\n  README.txt\n  diagram.mmd\n"));
    assert.ok(readme.includes("\n  src/\n    main.rs\n  theme.json\n"));
  }
});

test("inline Rust embeds the untouched recipe, configured source and frozen site config", () => {
  // Keep duplicate fields intact so the typed Rust deserializer can reject them.
  const rawRecipe = `${recipe.replace('"schema_version": 1', '"schema_version": 1, "schema_version": 1')}\n\n`;
  const value = { ...workspace, themeRecipeJson: rawRecipe, code: `${workspace.code}\n%% Keep "##, \\n and café ✓\n` };
  const snapshot = capture(value);
  const operation = snapshot.operation;
  const files = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  const inline = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust", rustAssets: "inline" });
  assert.ok(inline.code.includes(`const THEME_JSON: &str = r##"${rawRecipe}"##;`));
  assert.ok(inline.code.includes(`const DIAGRAM_SOURCE: &str = r###"${operation!.configuredSource}"###;`));
  assert.ok(inline.code.includes(`const SITE_CONFIG_JSON: &str = r#"${files.files["site-config.json"]}"#;`));
  assert.match(inline.code, /let recipe: ThemeRecipeV1 = serde_json::from_str\(THEME_JSON\)/);
  assert.ok(inline.code.indexOf("compile_recipe") < inline.code.indexOf("const THEME_JSON"));
  assert.match(inline.code, /DiagramThemeCompiler::new\(\)\.compile_recipe\(recipe\)/);
  assert.doesNotMatch(inline.code, /include_str!|serde_json::json!/);
  for (const [filename, content] of Object.entries(files.files)) {
    if (filename !== "src/main.rs" && filename !== "README.txt") assert.equal(inline.files[filename], content, filename);
  }
  assert.equal(inline.files["theme.json"], rawRecipe);
  assert.equal(inline.files["src/main.rs"], inline.code);
  assert.deepEqual(inline.notices, files.notices);
  assert.equal(snapshot.operation, operation);
  assert.equal(snapshot.publicationId, 7);
  assert.match(inline.files["README.txt"], /All rendering inputs are embedded in src\/main\.rs/);
  assert.match(inline.files["README.txt"], /serde_json Cargo dependencies are still required/);
  assert.doesNotMatch(inline.files["README.txt"], /include_str! paths/);
});

test("inline Rust raw strings escape terminator collisions and preserve CRLF bytes", () => {
  const cases = [
    { source: 'flowchart LR\nA --> B\n%% "# "## "### \\path ✓', literal: 'r####"flowchart LR\nA --> B\n%% "# "## "### \\path ✓"####' },
    { source: "flowchart LR\r\nA --> B\r", literal: 'concat!(r#"flowchart LR"#, "\\r", r#"\nA --> B"#, "\\r", r#""#)' },
    { source: `flowchart LR\nA --> B\n%% "${"#".repeat(254)} end`, literal: `r${"#".repeat(255)}"flowchart LR\nA --> B\n%% "${"#".repeat(254)} end"${"#".repeat(255)}` },
    { source: `flowchart LR\nA --> B\n%% "${"#".repeat(300)} end`, literal: `concat!(r#"flowchart LR\nA --> B\n%% "#, "\\\"", r#"${"#".repeat(300)} end"#)` },
    { source: "", literal: 'r#""#' },
  ];
  for (const { source, literal } of cases) {
    const value = { ...DEFAULT_WORKSPACE_SNAPSHOT, code: source };
    const bundle = generateProjectHandoff(capture(value), { scope: "diagram", target: "rust", rustAssets: "inline" });
    assert.ok(bundle.code.includes(`const DIAGRAM_SOURCE: &str = ${literal};`), literal);
    assert.equal(bundle.files["diagram.mmd"], source);
    assert.doesNotMatch(bundle.code, /\r/);
  }
  const crlfRecipe = '{"schema_version":1,\r\n"kind":"design","design":{}}';
  const bundle = generateProjectHandoff(capture({ ...workspace, themeRecipeJson: crlfRecipe }), {
    scope: "theme", target: "rust", rustAssets: "inline",
  });
  assert.ok(bundle.code.includes('const THEME_JSON: &str = concat!(r#"{"schema_version":1,"#, "\\r", r#"\n"kind":"design","design":{}}"#);'));
  assert.equal(bundle.files["theme.json"], crlfRecipe);
});

test("Rust optional inputs are emitted only when present, for either asset mode", () => {
  const snapshot = capture({ ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA --> B" });
  for (const rustAssets of ["files", "inline"] as const) {
    const bundle = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust", rustAssets });
    assert.equal(bundle.files["theme.json"], undefined);
    assert.equal(bundle.files["site-config.json"], undefined);
    assert.match(bundle.code, /let engine = MermaidEngine::new\(\);/);
    assert.doesNotMatch(bundle.code, /ThemeRecipeV1|SourcePresentationConfig|\.with_theme\(/);
    assert.doesNotMatch(bundle.files["README.txt"], /\.\.\/(?:theme|site-config)\.json loads/);
    assert.ok(bundle.files["README.txt"].includes("\n  src/\n    main.rs\n"));
    assert.doesNotMatch(bundle.files["README.txt"], /\n {2}(?:theme|site-config)\.json\n/);
  }
});

test("CLI and Web handoffs ignore the Rust asset selection", () => {
  const snapshot = capture();
  for (const target of ["cli", "web"] as const) {
    for (const scope of ["diagram", "theme"] as const) {
      const bundle = generateProjectHandoff(snapshot, { target, scope });
      for (const rustAssets of ["files", "inline"] as const) {
        assert.deepEqual(generateProjectHandoff(snapshot, { target, scope, rustAssets }), bundle);
      }
      assert.match(bundle.files["README.txt"], /No extra sample-config step is needed/);
      assert.match(bundle.files["README.txt"], /theme\.json, when selected, is the authored standalone visual style recipe/);
      assert.match(bundle.files["README.txt"], /no second manual theme\.json load or configuration merge is needed/);
      assert.doesNotMatch(bundle.files["README.txt"], /Rust input placement/);
    }
  }
});

test("runtime catalog presets use the actual preset APIs", () => {
  const value = { ...workspace, themeRecipeJson: null, themePresetId: "one-dark" };
  const snapshot = capture(value);
  for (const rustAssets of ["files", "inline"] as const) {
    const rust = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust", rustAssets });
    assert.match(rust.code, /ThemePreset::from_id\("one-dark"\)/);
    assert.equal(rust.files["theme.json"], undefined);
  }
  const cli = generateProjectHandoff(snapshot, { scope: "diagram", target: "cli" });
  assert.match(cli.code, /--theme-preset one-dark/);
  const web = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  assert.deepEqual(JSON.parse(web.files["options.json"]).theme, { preset: "one-dark" });
});

test("generated Rust consumers include mathematical labels available in the Web runtime", () => {
  const math = { ...workspace, themeRecipeJson: null, code: 'flowchart TD\nA["$$x^2$$"] --> B' };
  const bundle = generateProjectHandoff(capture(math), { scope: "diagram", target: "rust" });
  assert.match(bundle.files["Cargo.toml"], /features = \["math"\]/);
  assert.match(bundle.files["diagram.mmd"], /\$\$x\^2\$\$/);
  const cli = generateProjectHandoff(capture(math), { scope: "diagram", target: "cli" });
  assert.match(cli.files["README.txt"], /cargo build --locked -p merman-cli --features math/);
  assert.match(cli.files["README.txt"], /merman-cli capabilities/);
});

test("theme-only handoff validates the original recipe even when diagram rendering failed", () => {
  const invalid = { ...workspace, themeRecipeJson: '{"schema_version":1,"kind":"complete_spec","complete_spec":{"not_a_theme_field":true}}' };
  const failure = { status: "failed", merman: { status: "failure" } } as RenderCoordinatorState;
  let received = "";
  const rejected = captureProjectHandoff({
    workspace: invalid, renderState: failure, versions, capturedAt: "2026-11-05T00:00:00.000Z",
    validateThemeRecipe: (raw) => { received = raw; throw { message: "unknown field not_a_theme_field", code_name: "INVALID_THEME" }; },
  });
  assert.equal(received, invalid.themeRecipeJson);
  for (const target of ["rust", "cli", "web"] as const) {
    assert.throws(() => generateProjectHandoff(rejected, { scope: "theme", target }),
      (error) => error instanceof ProjectHandoffError && error.code === "invalid-configuration" && error.message.includes("not_a_theme_field"));
  }
  const unavailable = captureProjectHandoff({ workspace, renderState: failure, capturedAt: "2026-11-05T00:00:00.000Z" });
  assert.throws(() => generateProjectHandoff(unavailable, { scope: "theme", target: "web" }),
    (error) => error instanceof ProjectHandoffError && error.code === "recipe-unverified");
  const accepted = captureProjectHandoff({
    workspace: { ...workspace, code: "flowchart LR\nA -->" }, renderState: failure, versions,
    capturedAt: "2026-11-05T00:00:00.000Z", validateThemeRecipe: (raw) => { assert.equal(raw, recipe); },
  });
  assert.equal(generateProjectHandoff(accepted, { scope: "theme", target: "rust" }).files["theme.json"], recipe);
  assert.throws(() => generateProjectHandoff(accepted, { scope: "diagram", target: "rust" }),
    (error) => error instanceof ProjectHandoffError && error.code === "render-not-current");
});

test("unknown or unavailable preset fallback never generates equivalent-looking adoption code", () => {
  for (const id of ["atlas", "missing-preset"]) {
    const value = { ...workspace, themeRecipeJson: null, themePresetId: id };
    const snapshot = capture(value);
    assert.equal(snapshot.operation!.unavailableThemePresetId, id);
    for (const scope of ["theme", "diagram"] as const) {
      assert.throws(() => generateProjectHandoff(snapshot, { scope, target: "web" }),
        (error) => error instanceof ProjectHandoffError && error.code === "preset-unavailable");
    }
  }
});

test("only a completed successful operation matching every rendering input is current", () => {
  const state = completed(workspace);
  for (const change of [
    { code: "flowchart LR\nB --> C" }, { mermaidConfig: "{}" }, { diagramTheme: "dark" as const },
    { themeRecipeJson: `${recipe}\n` }, { themePresetId: "one-dark", themeRecipeJson: null }, { diagramFont: "arial" as const },
    { svgPipeline: "parity" as const }, { textMeasurementMode: "headless" as const },
  ]) {
    const snapshot = capture({ ...workspace, ...change }, state);
    assert.equal(snapshot.operation, null);
    assert.throws(() => generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" }),
      (error) => error instanceof ProjectHandoffError && error.code === "render-not-current");
  }
  for (const unavailable of [
    { status: "empty" }, { status: "pending", snapshot: { publicationId: 8 } },
    { status: "updating", previous: state, snapshot: { publicationId: 8 } },
    { ...state, status: "failed", merman: { status: "failure" } },
  ] as RenderCoordinatorState[]) assert.equal(capture(workspace, unavailable).operation, null);
  assert.equal(captureProjectHandoff({ workspace, renderState: state, capturedAt: "2026-11-05T00:00:00.000Z", versions: { ...versions, merman: "different" } }).operation, null);
});

test("a handoff stays frozen when the workspace changes and labels do not invalidate a render", () => {
  const mutable = { ...workspace, themeRecipeLabel: { name: "Atlas", source: "exploration" as const } };
  const snapshot = capture(mutable);
  const before = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  mutable.code = "flowchart LR\nChanged --> Later";
  mutable.themeRecipeLabel.name = "Changed";
  assert.equal(projectHandoffMatchesWorkspace(snapshot, mutable), false);
  assert.equal(snapshot.workspace.themeRecipeLabel!.name, "Atlas");
  assert.deepEqual(generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" }), before);
  assert.notEqual(capture({ ...workspace, themeRecipeLabel: { name: "Imported", source: "imported" } }, completed(workspace)).operation, null);
});

test("theme-only uses a separate example without the current diagram config, font, pipeline or layout", () => {
  const snapshot = capture(workspace, { status: "empty" });
  for (const target of ["rust", "cli", "web"] as const) {
    const bundle = generateProjectHandoff(snapshot, { scope: "theme", target });
    const options = JSON.parse(bundle.files["options.json"]);
    assert.equal(bundle.files["diagram.mmd"], "flowchart LR\nA[Theme] --> B[Your project]");
    assert.equal(bundle.files["theme.json"], recipe);
    assert.equal(options.site_config.theme, "default");
    assert.equal(options.site_config.fontFamily, undefined);
    assert.deepEqual(options.layout, { container_width: 800, container_height: 600 });
    assert.equal(options.svg, undefined);
    assert.equal(bundle.files["playground-config.json"], undefined);
    assert.doesNotMatch(bundle.files["diagram.mmd"], /forest|neutral|Authored font/);
    if (target === "web") assert.match(bundle.code, /renderSvgResult\(source, optionsJson\)/);
  }
  assert.throws(() => generateProjectHandoff(capture({ ...DEFAULT_WORKSPACE_SNAPSHOT }), { scope: "theme", target: "web" }),
    (error) => error instanceof ProjectHandoffError && error.code === "no-theme");
});

for (const themeExample of [
  {
    source: "sequenceDiagram\nClient->>Server: Request\nServer-->>Client: Response",
    configJson: '{ "theme":"dark", "sequence":{"mirrorActors":false,"actorMargin":72}, "fontFamily":"Example font" }',
  },
  {
    source: "classDiagram\nclass Project {\n  +String name\n}\nProject --> Theme",
    configJson: '{ "theme":"neutral", "class":{"hideEmptyMembersBox":true,"padding":24}, "fontFamily":"Example font" }',
  },
] as const) {
  test(`theme adoption preserves a custom ${themeExample.source.split("\n")[0]} source and config for every target without render proof`, () => {
    const value = { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: recipe };
    let validatedRecipe: string | null = null;
    const snapshot = captureProjectHandoff({
      workspace: value, renderState: { status: "empty" }, versions, capturedAt: "2026-11-05T00:00:00.000Z",
      validateThemeRecipe: (raw) => { validatedRecipe = raw; },
    });
    assert.equal(validatedRecipe, recipe);
    assert.equal(snapshot.operation, null);
    assert.equal(snapshot.publicationId, null);
    const config = JSON.parse(themeExample.configJson);
    for (const target of ["rust", "cli", "web"] as const) {
      const bundle = generateProjectHandoff(snapshot, { scope: "theme", target, themeExample });
      const options = JSON.parse(bundle.files["options.json"]);
      const metadata = JSON.parse(bundle.files["handoff.json"]);
      const directive = `%%{init: ${JSON.stringify(config)}}%%`;
      assert.equal(bundle.files["diagram.mmd"], `${directive}\n${themeExample.source}`);
      assert.equal(bundle.files["theme.json"], recipe);
      assert.deepEqual(options.theme, JSON.parse(recipe));
      assert.deepEqual(options.site_config, { theme: config.theme });
      assert.deepEqual(JSON.parse(bundle.files["site-config.json"]), options.site_config);
      assert.deepEqual(options.layout, { container_width: 800, container_height: 600 });
      assert.equal(bundle.files["source.mmd"], undefined);
      assert.equal(bundle.files["playground-config.json"], undefined);
      assert.equal(metadata.scope, "theme");
      assert.equal(metadata.publication_id, null);
      assert.deepEqual(metadata.versions, versions);
      assert.equal(metadata.configuration.original_source_file, null);
      assert.equal(metadata.configuration.authored_config_file, null);
      assert.equal(metadata.configuration.authored_config_location, null);
      assert.deepEqual(metadata.configuration.injected_config_span, { start: 0, end: new TextEncoder().encode(directive).byteLength });
      assert.equal(metadata.environment.layout, null);
      assert.equal(metadata.environment.text_measurement, "headless");
      assert.equal(metadata.environment.consumer_text_measurement, "deterministic");
      assert.match(bundle.files["README.txt"], /Theme adoption with a separate example diagram/);
      assert.match(bundle.files["README.txt"], /Render publication: not applicable/);
      assert.throws(() => generateProjectHandoff(snapshot, { scope: "diagram", target, themeExample }),
        (error) => error instanceof ProjectHandoffError && error.code === "render-not-current");
    }
    assert.equal(snapshot.operation, null);
    assert.equal(snapshot.publicationId, null);
    assert.deepEqual(snapshot.workspace, value);
  });
}

test("diagram scope ignores theme examples and keeps the captured operation and publication", () => {
  const snapshot = capture();
  const operation = snapshot.operation;
  const themeExample = { source: "sequenceDiagram\nA->>B: Theme example", configJson: "not JSON" };
  for (const target of ["rust", "cli", "web"] as const) {
    const before = generateProjectHandoff(snapshot, { scope: "diagram", target });
    assert.deepEqual(generateProjectHandoff(snapshot, { scope: "diagram", target, themeExample }), before);
    assert.equal(JSON.parse(before.files["handoff.json"]).publication_id, 7);
    const theme = generateProjectHandoff(snapshot, { scope: "theme", target, themeExample: { ...themeExample, configJson: "{}" } });
    assert.equal(theme.files["diagram.mmd"], themeExample.source);
    assert.equal(JSON.parse(theme.files["handoff.json"]).publication_id, null);
    assert.equal(JSON.parse(theme.files["handoff.json"]).environment.layout, null);
    assert.deepEqual(generateProjectHandoff(snapshot, { scope: "diagram", target }), before);
  }
  assert.equal(snapshot.operation, operation);
  assert.equal(snapshot.publicationId, 7);
});

test("invalid theme example configuration is rejected through the frozen operation", () => {
  const snapshot = capture({ ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: recipe }, { status: "empty" });
  const themeExample = { source: "classDiagram\nProject --> Theme", configJson: "not JSON" };
  for (const target of ["rust", "cli", "web"] as const) {
    assert.throws(() => generateProjectHandoff(snapshot, { scope: "theme", target, themeExample }),
      (error) => error instanceof ProjectHandoffError && error.code === "invalid-configuration");
  }
});

test("headless and browser host differences are explicit, including CLI's screen-width fallback", () => {
  const snapshot = capture();
  const rust = generateProjectHandoff(snapshot, { scope: "diagram", target: "rust" });
  assert.ok(rust.notices.includes("native-measurement"));
  assert.match(rust.code, /screen_available_width = Some\(1440\.0\)/);
  const cli = generateProjectHandoff(snapshot, { scope: "diagram", target: "cli" });
  assert.ok(cli.notices.includes("cli-screen-width"));
  assert.match(cli.code, /--width 923 --height 611/);
  assert.match(cli.code, /--font-measurement deterministic --font-system off/);
  assert.doesNotMatch(cli.code, /--config-file playground-config/);
  const headless = { ...workspace, textMeasurementMode: "headless" as const };
  const web = generateProjectHandoff(capture(headless), { scope: "diagram", target: "web" });
  assert.doesNotMatch(web.code, /createBrowserTextMeasurementSession/);
  assert.match(web.code, /renderSvgResult\(source, optionsJson\)/);
});

test("invalid configured inputs block current-diagram handoff without blocking independent theme use", () => {
  const invalid = { ...workspace, mermaidConfig: "not JSON" };
  const snapshot = capture(invalid);
  assert.throws(() => generateProjectHandoff(snapshot, { scope: "diagram", target: "web" }),
    (error) => error instanceof ProjectHandoffError && error.code === "invalid-configuration");
  assert.equal(generateProjectHandoff(snapshot, { scope: "theme", target: "web" }).files["theme.json"], recipe);
});

test("Web code admits and hardens the parsed SVG before DOM publication", () => {
  const bundle = generateProjectHandoff(capture(), { scope: "diagram", target: "web" });
  assert.doesNotMatch(bundle.code, /innerHTML/);
  const admission = bundle.code.indexOf("assertNavigableSvgForDom(result.svg)");
  const hardening = bundle.code.indexOf("prepareNavigableSvgForDomMount(admission, root, document)");
  const publication = bundle.code.indexOf("replaceChildren(root)");
  assert.ok(admission > 0 && hardening > admission && publication > hardening);
});

test("checkout identity is frozen as source provenance and never claimed as an artifact hash", () => {
  const buildIdentity = { channel: "local" as const, commit: "1234567890abcdef", ref: "workbench", dirty: true, repository: "Latias94/merman" };
  const snapshot = captureProjectHandoff({ workspace, renderState: completed(workspace), buildIdentity, presets, versions, capturedAt: "2026-11-05T00:00:00.000Z" });
  buildIdentity.commit = "changed-later";
  const bundle = generateProjectHandoff(snapshot, { scope: "diagram", target: "web" });
  assert.deepEqual(JSON.parse(bundle.files["handoff.json"]).playground_checkout, {
    channel: "local", commit: "1234567890abcdef", ref: "workbench", dirty: true, repository: "Latias94/merman",
  });
  assert.match(bundle.files["README.txt"], /1234567890abcdef/);
  assert.match(bundle.files["README.txt"], /Dirty: true/);
  assert.match(bundle.files["README.txt"], /not a WASM or binary hash/);
  assert.equal(JSON.parse(generateProjectHandoff(capture(), { scope: "diagram", target: "web" }).files["handoff.json"]).playground_checkout, null);
});
