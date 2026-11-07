import type { ThemePresetCatalogEntry, ThemeAppearance, ThemeCatalog } from "@mermanjs/web";

import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { freezeRenderOperation, matchesWorkspaceAppearance, type FrozenRenderOperation, type RenderOperationVersions } from "../runtime/merman-operation-input.ts";
import type { RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import { projectError } from "../runtime/error-projection.ts";
import { resolveThemeSelection, type ResolvedThemeSelection, type ThemeAppearanceSupport } from "./theme-appearance.ts";

export type ProjectUseTarget = "rust" | "cli" | "web";
export type ProjectUseScope = "theme" | "diagram";
export type ProjectHandoffNotice = "fonts" | "native-measurement" | "cli-screen-width" | "local-source";
export type ProjectHandoffErrorCode = "render-not-current" | "invalid-configuration" | "recipe-unverified" | "preset-unavailable" | "no-theme" | "theme-conflict";

export class ProjectHandoffError extends Error {
  readonly code: ProjectHandoffErrorCode;

  constructor(code: ProjectHandoffErrorCode, detail?: string) {
    super(detail ?? code);
    this.name = "ProjectHandoffError";
    this.code = code;
  }
}

/** Playground source checkout identity only; it does not identify a WASM binary. */
export interface ProjectHandoffBuildIdentity {
  readonly channel: "main" | "preview" | "tag" | "local";
  readonly commit: string | null;
  readonly ref: string | null;
  readonly dirty: boolean;
  readonly repository: string | null;
}

export interface ProjectHandoffInput {
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly renderState: RenderCoordinatorState;
  readonly presets?: readonly Pick<ThemePresetCatalogEntry, "id" | "available">[];
  readonly themeCatalog?: ThemeCatalog | null;
  readonly themeRecipeInspection?: ThemeAppearanceSupport | null;
  readonly hostAppearance?: ThemeAppearance;
  readonly versions?: Readonly<RenderOperationVersions>;
  readonly buildIdentity?: Readonly<ProjectHandoffBuildIdentity>;
  readonly validateThemeRecipe?: (recipeJson: string) => unknown;
  readonly capturedAt: string;
}

export interface ProjectHandoffSnapshot {
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly operation: FrozenRenderOperation | null;
  readonly publicationId: number | null;
  readonly presetAvailable: boolean;
  readonly versions: Readonly<RenderOperationVersions> | null;
  readonly buildIdentity: Readonly<ProjectHandoffBuildIdentity> | null;
  readonly recipeValidation: "valid" | "invalid" | "unavailable";
  readonly recipeError: string | null;
  readonly capturedAt: string;
  readonly selection: ResolvedThemeSelection;
}

export interface ProjectHandoffBundle {
  readonly target: ProjectUseTarget;
  readonly scope: ProjectUseScope;
  readonly code: string;
  readonly codeFilename: string;
  readonly files: Readonly<Record<string, string>>;
  readonly notices: readonly ProjectHandoffNotice[];
}

export interface ProjectHandoffThemeExample {
  readonly source: string;
  readonly configJson: string;
}

/** Capture materials once. A visible previous render is never current render evidence. */
export function captureProjectHandoff(input: ProjectHandoffInput): ProjectHandoffSnapshot {
  const { renderState: state, workspace, versions } = input;
  const completed = state.status === "success" || state.status === "partial" || state.status === "failed";
  const candidate = completed && state.merman.status === "success" ? state.snapshot.operation : null;
  const operation = candidate && operationMatchesWorkspace(candidate, workspace, input.hostAppearance) &&
    (!versions || (versions.merman === candidate.versions.merman && versions.mermaid === candidate.versions.mermaid))
    ? candidate : null;
  const presetId = workspace.themePresetId?.trim();
  const selection = operation ? Object.freeze({
    preference: operation.themeAppearancePreference,
    appearance: operation.resolvedThemeAppearance,
    mermaidTheme: operation.mermaidTheme,
    followsApp: operation.themeAppearancePreference === "follow",
    supported_appearances: operation.supportedThemeAppearances,
    default_appearance: operation.defaultThemeAppearance,
  }) : resolveThemeSelection(workspace, input.hostAppearance ?? "light", input.themeCatalog, input.themeRecipeInspection);
  const presets = input.themeCatalog ? input.themeCatalog.presets.flatMap((preset) => [
    { id: preset.id, available: preset.available },
    ...preset.appearances.map((entry) => ({ id: entry.concrete_preset_id, available: preset.available })),
  ]) : input.presets;
  const presetAvailable = Boolean(presetId && (
    presets?.find((preset) => preset.id === presetId)?.available ||
    (operation?.unavailableThemePresetId === null && operation.bindingOptions.theme &&
      "preset" in operation.bindingOptions.theme && operation.bindingOptions.theme.preset === presetId)
  ));
  let recipeValidation: ProjectHandoffSnapshot["recipeValidation"] = workspace.themeRecipeJson === null || operation
    ? "valid" : "unavailable";
  let recipeError: string | null = null;
  if (workspace.themeRecipeJson !== null && input.validateThemeRecipe) {
    try { input.validateThemeRecipe(workspace.themeRecipeJson); recipeValidation = "valid"; }
    catch (error) { recipeValidation = "invalid"; recipeError = projectError(error).summary; }
  }
  return Object.freeze({
    workspace: Object.freeze({
      ...workspace,
      ...(workspace.themeRecipeLabel ? { themeRecipeLabel: Object.freeze({ ...workspace.themeRecipeLabel }) } : {}),
    }),
    operation,
    publicationId: operation && completed ? state.snapshot.publicationId : null,
    presetAvailable,
    versions: operation?.versions ?? (versions ? Object.freeze({ ...versions }) : null),
    buildIdentity: input.buildIdentity ? Object.freeze({ ...input.buildIdentity }) : null,
    recipeValidation,
    recipeError,
    capturedAt: input.capturedAt,
    selection,
  });
}

function operationMatchesWorkspace(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>, hostAppearance?: ThemeAppearance): boolean {
  return operation.source === workspace.code && operation.configJson === workspace.mermaidConfig &&
    operation.theme === workspace.diagramTheme && operation.themePresetId === workspace.themePresetId &&
    operation.themeRecipeJson === workspace.themeRecipeJson && operation.diagramFont === workspace.diagramFont &&
    operation.textMeasurementMode === workspace.textMeasurementMode && operation.svgPipeline === workspace.svgPipeline &&
    matchesWorkspaceAppearance(operation, workspace, hostAppearance);
}

export function projectHandoffMatchesWorkspace(snapshot: ProjectHandoffSnapshot, workspace: Readonly<WorkspaceSnapshot>, hostAppearance?: ThemeAppearance): boolean {
  const frozen = snapshot.workspace;
  return frozen.code === workspace.code && frozen.mermaidConfig === workspace.mermaidConfig &&
    frozen.diagramTheme === workspace.diagramTheme && frozen.themePresetId === workspace.themePresetId &&
    frozen.themeRecipeJson === workspace.themeRecipeJson && frozen.diagramFont === workspace.diagramFont &&
    frozen.textMeasurementMode === workspace.textMeasurementMode && frozen.svgPipeline === workspace.svgPipeline &&
    frozen.themeRecipeLabel?.name === workspace.themeRecipeLabel?.name && frozen.themeRecipeLabel?.source === workspace.themeRecipeLabel?.source &&
    frozen.themeRecipeLabel?.derivedFrom === workspace.themeRecipeLabel?.derivedFrom &&
    (frozen.themeAppearance ?? "default") === (workspace.themeAppearance ?? "default") &&
    (snapshot.selection.preference !== "follow" || !hostAppearance || snapshot.selection.appearance ===
      (snapshot.selection.supported_appearances.includes(hostAppearance) ? hostAppearance : snapshot.selection.default_appearance));
}

export function generateProjectHandoff(
  snapshot: ProjectHandoffSnapshot,
  { target, scope, repositoryPath = "../merman", themeExample, rustAssets = "files" }: {
    readonly target: ProjectUseTarget;
    readonly scope: ProjectUseScope;
    /** Source checkout relative to the extracted consumer, or an absolute local path. */
    readonly repositoryPath?: string;
    /** Independent theme adoption inputs; never current-diagram render evidence. */
    readonly themeExample?: ProjectHandoffThemeExample;
    /** Rust rendering inputs are separate files by default, or embedded in src/main.rs. */
    readonly rustAssets?: "files" | "inline";
  },
): ProjectHandoffBundle {
  const workspace = snapshot.workspace;
  if (snapshot.selection.error) throw new ProjectHandoffError("invalid-configuration", snapshot.selection.error);
  if (workspace.themePresetId !== null && workspace.themeRecipeJson !== null) throw new ProjectHandoffError("theme-conflict");
  if (workspace.themeRecipeJson !== null && snapshot.recipeValidation !== "valid") {
    throw new ProjectHandoffError(snapshot.recipeValidation === "invalid" ? "invalid-configuration" : "recipe-unverified", snapshot.recipeError ?? undefined);
  }
  if (workspace.themePresetId !== null && (!snapshot.presetAvailable || snapshot.operation?.unavailableThemePresetId)) {
    throw new ProjectHandoffError("preset-unavailable");
  }
  if (scope === "theme" && workspace.themePresetId === null && workspace.themeRecipeJson === null && workspace.diagramTheme === "auto") {
    throw new ProjectHandoffError("no-theme");
  }
  const operation = scope === "diagram" ? snapshot.operation : themeExampleOperation(snapshot, themeExample);
  if (!operation) throw new ProjectHandoffError("render-not-current");
  if (operation.configurationError) throw new ProjectHandoffError("invalid-configuration", operation.configurationError.summary);
  const optionsJson = operation.bindingOptionsJson ?? json(operation.bindingOptions);
  const files: Record<string, string> = {
    "diagram.mmd": operation.configuredSource,
    "options.json": optionsJson,
  };
  if (scope === "diagram") {
    files["source.mmd"] = workspace.code;
    files["playground-config.json"] = workspace.mermaidConfig;
  }
  if (workspace.themeRecipeJson !== null) files["theme.json"] = workspace.themeRecipeJson;
  if (operation.bindingOptions.site_config && Object.keys(operation.bindingOptions.site_config).length > 0) {
    files["site-config.json"] = json(operation.bindingOptions.site_config);
  }
  const browserMeasurement = scope === "diagram" && operation.textMeasurementMode === "browser";
  const notices: ProjectHandoffNotice[] = ["fonts", "local-source"];
  if (browserMeasurement && target !== "web") notices.push("native-measurement");
  const layout = operation.bindingOptions.layout;
  if (target === "cli" && layout?.screen_available_width !== undefined && layout.screen_available_width !== layout.container_width) {
    notices.push("cli-screen-width");
  }
  const repo = repositoryPath.replaceAll("\\", "/").replace(/\/$/, "");
  let code: string;
  let codeFilename: string;
  if (target === "rust") {
    code = rustConsumer(operation, files, rustAssets);
    codeFilename = "src/main.rs";
    files["Cargo.toml"] = `[package]\nname = "merman-project-use"\nversion = "0.1.0"\nedition = "2024"\n\n[workspace]\n\n[dependencies]\nmerman = { path = ${JSON.stringify(`${repo}/crates/merman`)}, features = ["math"] }\nserde_json = "1"\n`;
    files[codeFilename] = code;
  } else if (target === "cli") {
    const args = cliArguments(operation, files);
    code = ["merman-cli", ...args].map(shellArgument).join(" ");
    codeFilename = "render-command.txt";
    files[codeFilename] = `${code}\n`;
    files["render.py"] = `from pathlib import Path\nimport argparse\nimport subprocess\n\nparser = argparse.ArgumentParser(description="Render the captured Merman materials.")\nparser.add_argument("--cli", default="merman-cli", help="Path to merman-cli built from the matching checkout")\nargs = parser.parse_args()\nsubprocess.run([args.cli, *${JSON.stringify(args)}], cwd=Path(__file__).resolve().parent, check=True)\n`;
  } else {
    code = webConsumer(browserMeasurement);
    codeFilename = "main.js";
    files[codeFilename] = code;
    files["index.html"] = '<!doctype html>\n<html lang="en">\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1">\n<title>Merman project example</title>\n<body>\n<p id="status" role="status">Loading diagram…</p>\n<div id="diagram"></div>\n<script type="module" src="./main.js"></script>\n</body>\n</html>\n';
    files["package.json"] = json({
      name: "merman-project-use", private: true, type: "module",
      scripts: { dev: "vite --host 127.0.0.1", build: "vite build" },
      dependencies: { "@mermanjs/web": `file:${repo}/platforms/web/packages/full` },
      devDependencies: { vite: "^8.3.5" },
    });
  }
  files["handoff.json"] = json({
    schema_version: 1, scope, target, captured_at: snapshot.capturedAt,
    publication_id: scope === "diagram" ? snapshot.publicationId : null,
    versions: snapshot.versions,
    playground_checkout: snapshot.buildIdentity,
    theme: {
      preset_id: workspace.themePresetId,
      recipe_file: workspace.themeRecipeJson === null ? null : "theme.json",
      label: workspace.themeRecipeLabel ?? null,
      mermaid_base: workspace.diagramTheme,
      appearance_preference: snapshot.selection.preference,
      resolved_appearance: operation.resolvedThemeAppearance,
      resolved_mermaid_theme: operation.mermaidTheme,
    },
    configuration: {
      source_file: "diagram.mmd",
      original_source_file: scope === "diagram" ? "source.mmd" : null,
      authored_config_file: scope === "diagram" ? "playground-config.json" : null,
      authored_config_location: scope === "diagram" ? "source init directive after frontmatter" : null,
      site_config_file: files["site-config.json"] ? "site-config.json" : null,
      injected_config_span: operation.injectedConfigSpan,
      binding_options_file: "options.json",
    },
    environment: {
      layout: scope === "diagram" ? operation.layoutEnvironment : null,
      text_measurement: operation.textMeasurementMode,
      consumer_text_measurement: target === "web" && browserMeasurement ? "browser" : "deterministic",
      diagram_font: operation.diagramFont,
      svg_pipeline: operation.svgPipeline,
      fonts_embedded: false,
      cli_screen_width: target === "cli" ? "container width fallback; no screen width flag" : null,
    },
  });
  files["README.txt"] = consumerReadme(snapshot, target, scope, notices, files, rustAssets);
  return Object.freeze({ target, scope, code, codeFilename, files: Object.freeze(files), notices: Object.freeze(notices) });
}

function themeExampleOperation(snapshot: ProjectHandoffSnapshot, example?: ProjectHandoffThemeExample): FrozenRenderOperation {
  const native = snapshot.workspace.themePresetId === null && snapshot.workspace.themeRecipeJson === null;
  const operation = freezeRenderOperation({
    workspace: {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: example?.source ?? "flowchart LR\nA[Theme] --> B[Your project]",
      mermaidConfig: example?.configJson ?? "{}",
      diagramTheme: snapshot.selection.mermaidTheme,
      themeAppearance: native ? "default" : snapshot.selection.appearance ?? "default",
      themePresetId: snapshot.workspace.themePresetId,
      themeRecipeJson: snapshot.workspace.themeRecipeJson,
      textMeasurementMode: "headless",
    },
    asciiEnabled: false, compareEnabled: false, diagnosticsEnabled: false,
    layoutEnvironment: { containerWidth: 800, containerHeight: 600 },
    versions: snapshot.versions ?? { merman: "unknown", mermaid: "unknown" },
    viewport: null,
    themeRecipeInspection: snapshot.selection,
  });
  return native ? Object.freeze({
    ...operation,
    themeAppearancePreference: snapshot.selection.appearance ?? "default",
    resolvedThemeAppearance: snapshot.selection.appearance,
    supportedThemeAppearances: snapshot.selection.supported_appearances,
    defaultThemeAppearance: snapshot.selection.default_appearance,
  }) : operation;
}

function rustConsumer(operation: FrozenRenderOperation, files: Readonly<Record<string, string>>, assets: "files" | "inline"): string {
  const recipe = files["theme.json"] !== undefined;
  const preset = operation.themePresetId?.trim();
  const inputNames = { "theme.json": "THEME_JSON", "diagram.mmd": "DIAGRAM_SOURCE", "site-config.json": "SITE_CONFIG_JSON" } as const;
  const input = (filename: keyof typeof inputNames) => assets === "inline"
    ? inputNames[filename] : `include_str!("../${filename}")`;
  const embedded = assets === "inline" ? Object.entries(inputNames)
    .filter(([filename]) => files[filename] !== undefined)
    .map(([filename, name]) => `\nconst ${name}: &str = ${rustRawString(files[filename])};\n`).join("") : "";
  const appearance = operation.resolvedThemeAppearance;
  const selector = appearance ? `Some(ThemeAppearance::${appearance === "dark" ? "Dark" : "Light"})` : "None";
  const appearanceImport = appearance ? ", ThemeAppearance" : "";
  const imports = recipe ? `use merman::diagram_theme::{DiagramThemeCompiler, ThemeRecipeV1${appearanceImport}};\n`
    : preset ? `use merman::diagram_theme::{DiagramThemeCompiler${appearanceImport}};\n` : "";
  const theme = recipe
    ? `    let recipe: ThemeRecipeV1 = serde_json::from_str(${input("theme.json")})?;\n    let theme = DiagramThemeCompiler::new().compile_document(recipe, ${selector})?;\n`
    : preset ? `    let theme = DiagramThemeCompiler::new().compile_preset_selection(${JSON.stringify(preset)}, ${selector})?;\n` : "";
  const site = files["site-config.json"]
    ? `MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(serde_json::from_str(${input("site-config.json")})?))`
    : "MermaidEngine::new()";
  const pipeline = { parity: "parity", readable: "readable", "resvg-safe": "resvg_safe" }[operation.svgPipeline];
  const layout = operation.bindingOptions.layout;
  return `${imports}use merman::svg::SvgPipeline;\nuse merman::{MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest${files["site-config.json"] ? ", SourcePresentationConfig" : ""}};\n\nfn main() -> Result<(), Box<dyn std::error::Error>> {\n${theme}    let source = ${input("diagram.mmd")};\n    let engine = ${site};\n    let mut svg_request = SvgRequest::default();\n    svg_request.pipeline = Some(SvgPipeline::${pipeline}());\n${layout ? `    svg_request.layout.container_width = ${rustFloat(layout.container_width ?? 800)};\n    svg_request.layout.container_height = ${rustFloat(layout.container_height ?? 600)};\n` : ""}${layout?.screen_available_width === undefined ? "" : `    svg_request.layout.screen_available_width = Some(${rustFloat(layout.screen_available_width)});\n`}    // Native consumers use deterministic measurement; browser fonts are external host resources.\n    let request = RenderRequest::svg(source, OperationControl::new(), svg_request)${recipe || preset ? ".with_theme(theme)" : ""};\n    let output = Renderer::new().with_presentation_engine(engine).render(request)?;\n    let RenderOutput::Svg(Some(svg)) = output else {\n        return Err("No Mermaid diagram detected".into());\n    };\n    std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("diagram.svg"), svg.svg())?;\n    Ok(())\n}\n${embedded}`;
}

function cliArguments(operation: FrozenRenderOperation, files: Readonly<Record<string, string>>): string[] {
  const layout = operation.bindingOptions.layout;
  return [
    "render", "diagram.mmd",
    ...(files["site-config.json"] ? ["--config-file", "site-config.json"] : []),
    ...(files["theme.json"] !== undefined ? ["--theme-file", "theme.json"]
      : operation.themePresetId ? ["--theme-preset", operation.themePresetId.trim()] : []),
    ...((files["theme.json"] !== undefined || operation.themePresetId) && operation.resolvedThemeAppearance
      ? ["--theme-appearance", operation.resolvedThemeAppearance] : []),
    "--svg-pipeline", operation.svgPipeline,
    "--font-measurement", "deterministic", "--font-system", "off",
    ...(layout?.container_width === undefined ? [] : ["--width", String(layout.container_width)]),
    ...(layout?.container_height === undefined ? [] : ["--height", String(layout.container_height)]),
    "-o", "diagram.svg",
  ];
}

function webConsumer(browserMeasurement: boolean): string {
  const render = browserMeasurement ? `  // Load application-provided fonts before creating the measurement session.
  await document.fonts.ready;
  const measurement = createBrowserTextMeasurementSession();
  let result;
  try {
    result = renderSvgResultWithTextMeasurer(source, measurement.measure, optionsJson);
  } finally {
    measurement.dispose();
  }
` : "  const result = renderSvgResult(source, optionsJson);\n";
  return `import { initMerman, ${browserMeasurement ? "createBrowserTextMeasurementSession, renderSvgResultWithTextMeasurer" : "renderSvgResult"}, assertNavigableSvgForDom, prepareNavigableSvgForDomMount } from "@mermanjs/web";

async function readText(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(\`Failed to load \${url}: \${response.status}\`);
  return response.text();
}

try {
  await initMerman();
  const [source, optionsJson] = await Promise.all([
    readText(new URL("./diagram.mmd", import.meta.url)),
    readText(new URL("./options.json", import.meta.url)),
  ]);
  // Keep options as raw JSON so authored recipes retain duplicate-field validation.
${render}  // Preserve the SDK's admission and mount-time navigation/resource checks.
  const admission = assertNavigableSvgForDom(result.svg);
  const parsed = new DOMParser().parseFromString(result.svg, "image/svg+xml");
  if (parsed.querySelector("parsererror") || parsed.documentElement.localName !== "svg") {
    throw new Error("Merman output is not a valid SVG document.");
  }
  const root = document.importNode(parsed.documentElement, true);
  const shadow = document.getElementById("diagram").attachShadow({ mode: "open" });
  prepareNavigableSvgForDomMount(admission, root, document);
  shadow.replaceChildren(root);
  document.getElementById("status").textContent = "Rendered. Fonts and host measurement can change geometry.";
  console.log(result.metadata);
} catch (error) {
  document.getElementById("status").textContent = String(error);
  console.error(error);
}
`;
}

function consumerReadme(snapshot: ProjectHandoffSnapshot, target: ProjectUseTarget, scope: ProjectUseScope, notices: readonly ProjectHandoffNotice[], files: Readonly<Record<string, string>>, rustAssets: "files" | "inline"): string {
  const setup = {
    rust: "Use the matching Merman source checkout. Cargo.toml contains the local path to crates/merman; adjust the merman dependency path to your checkout. Run cargo run from this directory.",
    cli: "Use merman-cli built from the matching source checkout with native-fonts (included by default). Mathematical labels also require math: build with cargo build --locked -p merman-cli --features math and inspect merman-cli capabilities. Run python render.py, or python render.py --cli /absolute/path/to/merman-cli. The render-command.txt file contains the equivalent direct command. Measurement is explicitly deterministic with system fonts off.",
    web: "Use the matching Merman source checkout, including its prepared Web dist, glue and WASM. package.json links ../merman/platforms/web/packages/full; adjust that local file dependency as needed. Follow that checkout's Web build instructions first. Then run npm install and npm run dev in this directory. Serve over HTTP; do not open index.html as file://.",
  }[target];
  const directory = [...Object.keys(files), "README.txt"].sort()
    .map((filename) => filename === "src/main.rs" ? "  src/\n    main.rs" : `  ${filename}`).join("\n");
  const rustInputs = target !== "rust" ? "" : rustAssets === "inline"
    ? "\nRust input placement\nAll rendering inputs are embedded in src/main.rs. The separate diagram, theme and site-config files are reference copies, when present; this program does not read them. Copy main.rs and the Cargo.toml dependencies into your Cargo project, or run this directory as generated. The matching merman checkout and serde_json Cargo dependencies are still required.\n"
    : `\nRust input placement\ninclude_str! paths are relative to src/main.rs, not the working directory. Keep these files at the project root beside Cargo.toml:\n- ../diagram.mmd loads the configured diagram.\n${files["theme.json"] !== undefined ? "- ../theme.json loads the authored theme recipe.\n" : ""}${files["site-config.json"] !== undefined ? "- ../site-config.json loads the host initialization config.\n" : ""}`;
  const checkout = snapshot.buildIdentity;
  return `Merman project materials

Scope: ${scope === "diagram" ? "Captured current diagram" : "Theme adoption with a separate example diagram"}
Captured: ${snapshot.capturedAt}
Render publication: ${scope === "diagram" ? snapshot.publicationId : "not applicable"}
Merman: ${snapshot.versions?.merman ?? "unknown"}; Mermaid JS: ${snapshot.versions?.mermaid ?? "unknown"}

Playground source checkout identity
Repository: ${checkout?.repository ?? "unknown"}
Commit: ${checkout?.commit ?? "unknown"}
Ref: ${checkout?.ref ?? "unknown"}
Channel: ${checkout?.channel ?? "unknown"}
Dirty: ${checkout ? String(checkout.dirty) : "unknown"}
This identifies the Playground checkout, not a WASM or binary hash. A dirty checkout includes changes absent from that commit. Matching render artifacts still require compatible versions, facade/glue and WASM from the same revision; this record does not verify their binary identity.

${setup}

Generated directory
project/
${directory}
${rustInputs}
Appearance selection
- Playground preference: ${snapshot.selection.preference}.
- Exported appearance: ${snapshot.selection.appearance ?? "the concrete document or source default"}.
- Generated code pins the captured appearance when one is declared. Follow app is resolved by the Playground, never passed as an automatic mode to a headless renderer.
- theme.json, when present, keeps every authored appearance and shared reference. Change the explicit appearance selector to render another declared appearance; do not duplicate or flatten the theme.

Configuration provenance
- diagram.mmd is the render-ready configured source, including the sample or authored diagram configuration. No extra sample-config step is needed. Do not inject playground-config.json again.
- For current-diagram materials, source.mmd and playground-config.json retain the original editor inputs.
- Authored Playground config stays in a source init directive after frontmatter. It is never moved into site_config or CLI --config-file.
- site-config.json, when present, contains only the frozen operation's host initialization config. The generated consumer already applies it.
- theme.json, when selected, is the authored standalone visual style recipe. It supplies styling, while diagram.mmd supplies the diagram and its configuration. Do not wrap the recipe in {recipe: ...}.
- options.json retains the complete Web binding request, including the selected theme and host initialization config. The Web consumer already loads them through options.json; no second manual theme.json load or configuration merge is needed.
- handoff.json records versions, layout, measurement and file provenance. Builtin preset IDs require a matching runtime catalog; a package version alone does not pin a source revision.

Host dependencies
Font names do not bundle font files. Supply the same licensed fonts in your host and load them before measuring. Browser text measurement and deterministic native measurement can produce different geometry. No cross-host pixel equivalence is promised.
${notices.includes("native-measurement") ? "This captured diagram used browser measurement. This Rust/CLI consumer uses deterministic measurement; it reproduces the source and presentation inputs with this explicit host difference.\n" : ""}${notices.includes("cli-screen-width") ? "The CLI has no screen-available-width flag. Its C4 layout falls back to container width; handoff.json and options.json preserve the captured screen width for Rust/Web use.\n" : ""}
`;
}

function json(value: unknown): string { return `${JSON.stringify(value, null, 2)}\n`; }
function rustRawString(value: string): string {
  // Rust normalizes source CRLF and limits raw delimiters to 255 hashes.
  const parts = value.split(/(\r|"(?=#{255}))/).map((part, index) => {
    if (index % 2 === 1) return part === "\r" ? '"\\r"' : '"\\\""';
    let hashes = "#";
    while (part.includes(`"${hashes}`)) hashes += "#";
    return `r${hashes}"${part}"${hashes}`;
  });
  return parts.length === 1 ? parts[0] : `concat!(${parts.join(", ")})`;
}
function rustFloat(value: number): string { return Number.isInteger(value) && !String(value).includes("e") ? `${value}.0` : String(value); }
function shellArgument(value: string): string { return /^[a-zA-Z0-9_./-]+$/.test(value) ? value : `'${value.replaceAll("'", "'\\''")}'`; }
