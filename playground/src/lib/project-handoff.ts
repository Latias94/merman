import type { ThemePresetCatalogEntry } from "@mermanjs/web";

import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { freezeRenderOperation, type FrozenRenderOperation, type RenderOperationVersions } from "../runtime/merman-operation-input.ts";
import type { RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import { projectError } from "../runtime/error-projection.ts";

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
}

export interface ProjectHandoffBundle {
  readonly target: ProjectUseTarget;
  readonly scope: ProjectUseScope;
  readonly code: string;
  readonly codeFilename: string;
  readonly files: Readonly<Record<string, string>>;
  readonly notices: readonly ProjectHandoffNotice[];
}

/** Capture materials once. A visible previous render is never current render evidence. */
export function captureProjectHandoff(input: ProjectHandoffInput): ProjectHandoffSnapshot {
  const { renderState: state, workspace, versions } = input;
  const completed = state.status === "success" || state.status === "partial" || state.status === "failed";
  const candidate = completed && state.merman.status === "success" ? state.snapshot.operation : null;
  const operation = candidate && operationMatchesWorkspace(candidate, workspace) &&
    (!versions || (versions.merman === candidate.versions.merman && versions.mermaid === candidate.versions.mermaid))
    ? candidate : null;
  const presetId = workspace.themePresetId?.trim();
  const presetAvailable = Boolean(presetId && (
    input.presets?.some((preset) => preset.id === presetId && preset.available) ||
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
  });
}

function operationMatchesWorkspace(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>): boolean {
  return operation.source === workspace.code && operation.configJson === workspace.mermaidConfig &&
    operation.theme === workspace.diagramTheme && operation.themePresetId === workspace.themePresetId &&
    operation.themeRecipeJson === workspace.themeRecipeJson && operation.diagramFont === workspace.diagramFont &&
    operation.textMeasurementMode === workspace.textMeasurementMode && operation.svgPipeline === workspace.svgPipeline;
}

export function projectHandoffMatchesWorkspace(snapshot: ProjectHandoffSnapshot, workspace: Readonly<WorkspaceSnapshot>): boolean {
  const frozen = snapshot.workspace;
  return frozen.code === workspace.code && frozen.mermaidConfig === workspace.mermaidConfig &&
    frozen.diagramTheme === workspace.diagramTheme && frozen.themePresetId === workspace.themePresetId &&
    frozen.themeRecipeJson === workspace.themeRecipeJson && frozen.diagramFont === workspace.diagramFont &&
    frozen.textMeasurementMode === workspace.textMeasurementMode && frozen.svgPipeline === workspace.svgPipeline &&
    frozen.themeRecipeLabel?.name === workspace.themeRecipeLabel?.name && frozen.themeRecipeLabel?.source === workspace.themeRecipeLabel?.source;
}

export function generateProjectHandoff(
  snapshot: ProjectHandoffSnapshot,
  { target, scope, repositoryPath = "../merman" }: {
    readonly target: ProjectUseTarget;
    readonly scope: ProjectUseScope;
    /** Source checkout relative to the extracted consumer, or an absolute local path. */
    readonly repositoryPath?: string;
  },
): ProjectHandoffBundle {
  const workspace = snapshot.workspace;
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
  const operation = scope === "diagram" ? snapshot.operation : themeExampleOperation(snapshot);
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
    code = rustConsumer(operation, files);
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
  files["README.txt"] = consumerReadme(snapshot, target, scope, notices);
  return Object.freeze({ target, scope, code, codeFilename, files: Object.freeze(files), notices: Object.freeze(notices) });
}

function themeExampleOperation(snapshot: ProjectHandoffSnapshot): FrozenRenderOperation {
  return freezeRenderOperation({
    workspace: {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: "flowchart LR\nA[Theme] --> B[Your project]",
      mermaidConfig: "{}",
      diagramTheme: snapshot.workspace.diagramTheme,
      themePresetId: snapshot.workspace.themePresetId,
      themeRecipeJson: snapshot.workspace.themeRecipeJson,
      textMeasurementMode: "headless",
    },
    asciiEnabled: false, compareEnabled: false, diagnosticsEnabled: false,
    layoutEnvironment: { containerWidth: 800, containerHeight: 600 },
    versions: snapshot.versions ?? { merman: "unknown", mermaid: "unknown" },
    viewport: null,
  });
}

function rustConsumer(operation: FrozenRenderOperation, files: Readonly<Record<string, string>>): string {
  const recipe = files["theme.json"] !== undefined;
  const preset = operation.themePresetId?.trim();
  const imports = recipe ? "use merman::diagram_theme::{DiagramThemeCompiler, ThemeRecipeV1};\n"
    : preset ? "use merman::diagram_theme::{DiagramThemeCompiler, ThemePreset};\n" : "";
  const theme = recipe
    ? '    let recipe: ThemeRecipeV1 = serde_json::from_str(include_str!("../theme.json"))?;\n    let theme = DiagramThemeCompiler::new().compile_recipe(recipe)?;\n'
    : preset ? `    let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::from_id(${JSON.stringify(preset)})?)?;\n` : "";
  const site = files["site-config.json"]
    ? 'MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(serde_json::from_str(include_str!("../site-config.json"))?))'
    : "MermaidEngine::new()";
  const pipeline = { parity: "parity", readable: "readable", "resvg-safe": "resvg_safe" }[operation.svgPipeline];
  const layout = operation.bindingOptions.layout;
  return `${imports}use merman::svg::SvgPipeline;\nuse merman::{MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest${files["site-config.json"] ? ", SourcePresentationConfig" : ""}};\n\nfn main() -> Result<(), Box<dyn std::error::Error>> {\n${theme}    let source = include_str!("../diagram.mmd");\n    let engine = ${site};\n    let mut svg_request = SvgRequest::default();\n    svg_request.pipeline = Some(SvgPipeline::${pipeline}());\n${layout ? `    svg_request.layout.container_width = ${rustFloat(layout.container_width ?? 800)};\n    svg_request.layout.container_height = ${rustFloat(layout.container_height ?? 600)};\n` : ""}${layout?.screen_available_width === undefined ? "" : `    svg_request.layout.screen_available_width = Some(${rustFloat(layout.screen_available_width)});\n`}    // Native consumers use deterministic measurement; browser fonts are external host resources.\n    let request = RenderRequest::svg(source, OperationControl::new(), svg_request)${recipe || preset ? ".with_theme(theme)" : ""};\n    let output = Renderer::new().with_presentation_engine(engine).render(request)?;\n    let RenderOutput::Svg(Some(svg)) = output else {\n        return Err("No Mermaid diagram detected".into());\n    };\n    std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("diagram.svg"), svg.svg())?;\n    Ok(())\n}\n`;
}

function cliArguments(operation: FrozenRenderOperation, files: Readonly<Record<string, string>>): string[] {
  const layout = operation.bindingOptions.layout;
  return [
    "render", "diagram.mmd",
    ...(files["site-config.json"] ? ["--config-file", "site-config.json"] : []),
    ...(files["theme.json"] !== undefined ? ["--theme-file", "theme.json"]
      : operation.themePresetId ? ["--theme-preset", operation.themePresetId.trim()] : []),
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

function consumerReadme(snapshot: ProjectHandoffSnapshot, target: ProjectUseTarget, scope: ProjectUseScope, notices: readonly ProjectHandoffNotice[]): string {
  const setup = {
    rust: "Use the matching Merman source checkout. Cargo.toml points to ../merman/crates/merman; adjust the path to your checkout. Run cargo run from this directory.",
    cli: "Use merman-cli built from the matching source checkout with native-fonts (included by default). Mathematical labels also require math: build with cargo build --locked -p merman-cli --features math and inspect merman-cli capabilities. Run python render.py, or python render.py --cli /absolute/path/to/merman-cli. The render-command.txt file contains the equivalent direct command. Measurement is explicitly deterministic with system fonts off.",
    web: "Use the matching Merman source checkout, including its prepared Web dist, glue and WASM. package.json links ../merman/platforms/web/packages/full; adjust that local file dependency as needed. Follow that checkout's Web build instructions first. Then run npm install and npm run dev in this directory. Serve over HTTP; do not open index.html as file://.",
  }[target];
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

Configuration provenance
- diagram.mmd is the render-ready configured source. Do not inject playground-config.json again.
- For current-diagram materials, source.mmd and playground-config.json retain the original editor inputs.
- Authored Playground config stays in a source init directive after frontmatter. It is never moved into site_config or CLI --config-file.
- site-config.json, when present, contains only the frozen operation's host initialization config.
- options.json retains the complete Web binding request; theme.json is the authored standalone recipe, when selected. Do not wrap it in {recipe: ...}.
- handoff.json records versions, layout, measurement and file provenance. Builtin preset IDs require a matching runtime catalog; a package version alone does not pin a source revision.

Host dependencies
Font names do not bundle font files. Supply the same licensed fonts in your host and load them before measuring. Browser text measurement and deterministic native measurement can produce different geometry. No cross-host pixel equivalence is promised.
${notices.includes("native-measurement") ? "This captured diagram used browser measurement. This Rust/CLI consumer uses deterministic measurement; it reproduces the source and presentation inputs with this explicit host difference.\n" : ""}${notices.includes("cli-screen-width") ? "The CLI has no screen-available-width flag. Its C4 layout falls back to container width; handoff.json and options.json preserve the captured screen width for Rust/Web use.\n" : ""}
`;
}

function json(value: unknown): string { return `${JSON.stringify(value, null, 2)}\n`; }
function rustFloat(value: number): string { return Number.isInteger(value) && !String(value).includes("e") ? `${value}.0` : String(value); }
function shellArgument(value: string): string { return /^[a-zA-Z0-9_./-]+$/.test(value) ? value : `'${value.replaceAll("'", "'\\''")}'`; }
