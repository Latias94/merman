import { withResourceOptions } from "../src/runtime-core.js";
import { renderAscii, renderAsciiResult } from "../src/runtime-ascii.js";
import {
  exportThemePreset,
  layoutJsonWithTextMeasurer,
  resolveTheme,
  renderSvgWithTextMeasurer,
  renderSvgResult,
  svgPlanJson,
} from "../src/runtime-render.js";
import type { AsciiCapability, AsciiGraphPresentation, BindingDiagnosticErrorDetails } from "../src/public-catalog.js";
import type {
  AsciiBindingOptions,
  AsciiOutputPlan,
  AsciiRenderResult,
  BindingOperationMetadata,
  CommonBindingOptions,
  EditorBindingOptions,
  EditorResourceOptions,
  HostTextMeasurerSvgBindingOptions,
  ResourceOptions,
  SvgBindingOptions,
  ThemeAppearance,
  ThemeCatalog,
  ThemeDocumentV2,
  ThemeOutcome,
  ThemePaint,
} from "../src/public-types.js";

const asciiDiagnostic: BindingDiagnosticErrorDetails = {
  code: "merman.ascii.width_overflow",
  span: null,
  field: null,
  diagram_type: "flowchart-v2",
  requested_max_width: 10,
  actual_width: 42,
  width_profile: "unicode",
  fallback_reason: null,
};
asciiDiagnostic.actual_width;

const resources: ResourceOptions = { profile: "interactive" };

const hostClock: SvgBindingOptions = {
  version: 4,
  fixed_unix_millis: 1_794_657_600_375,
  fixed_local_offset_minutes: 480,
};
renderSvgResult("gantt\ndateFormat x\nTask: task,0,1000ms", hostClock);
renderSvgResult("gantt\ndateFormat x\nTask: task,0,1000ms", {
  analysis: { fixed_today: null, fixed_unix_millis: 375 },
});

declare const catalog: ThemeCatalog;
const preset = catalog.presets[0];
const schema: 2 = catalog.schema_version;
const appearance: ThemeAppearance = preset.default_appearance;
const swatch: string = preset.swatches[5];
const familyFlags: number = catalog.families[0].capabilities;
const gradient: ThemePaint = {
  kind: "linear_gradient",
  angle_deg: 135,
  stops: [{ offset: 0, color: "#0F172A" }, { offset: 1, color: "#F8FAFC" }],
};
const themeDocument: ThemeDocumentV2 = {
  schema_version: 2,
  extends: preset.id,
  seed: { accent: "#0052CC" },
  roles: { canvas: gradient },
  typography: { body_family: "serif", base_size_px: 16 },
  form: { marker_style: "open", elevation: "soft" },
  appearances: { dark: { canvas: { textures: [{ kind: "grid", step_px: 16 }] } } },
  overrides: [{ families: ["flowchart"], roles: { "text.primary": "#171717" } }],
};
const exportedPreset: ThemeDocumentV2 = exportThemePreset(preset.id);
const source = "flowchart LR\nA --> B";
renderSvgResult(source, { theme: preset.id });
renderSvgResult(source, { theme: "none", background: "transparent" });
renderSvgResult(source, { theme: exportedPreset });
renderSvgResult(source, { theme: { document: themeDocument, appearance, precedence: "host" } });
renderAsciiResult(source, { theme: { preset: preset.id, appearance }, ascii: { color_mode: "truecolor" } });
const themeOutcome: ThemeOutcome = renderSvgResult(source, { theme: themeDocument }).metadata.theme_outcome;
for (const notice of themeOutcome.notices) {
  if (typeof notice !== "string" && notice.kind === "low_contrast") {
    const ratio: number = notice.ratio;
    void ratio;
  }
}
const resolution = resolveTheme(themeDocument, { appearance, auto_fix_contrast: true });
resolveTheme(JSON.stringify(themeDocument));
if (resolution.ok) {
  const appearance: ThemeAppearance = resolution.resolved.appearance;
  const ink: string = resolution.resolved.roles["text.primary"];
  const color: string = resolution.resolved.data.fill[0];
  void [appearance, ink, color];
} else {
  const line: number | null = resolution.line;
  const path: string | null = resolution.path;
  void [line, path];
}
// @ts-expect-error The preset export takes one ID.
exportThemePreset(preset.id, {});
// @ts-expect-error A selector accepts exactly one of preset and document.
renderSvgResult(source, { theme: { preset: preset.id, document: themeDocument } });
// @ts-expect-error Host preferences must resolve to a concrete appearance.
renderSvgResult(source, { theme: { preset: preset.id, appearance: "auto" } });
// @ts-expect-error Appearance belongs in the selector around the document.
renderSvgResult(source, { theme: { ...themeDocument, appearance } });
// @ts-expect-error Theme documents use schema version 2.
resolveTheme({ schema_version: 1 });
// @ts-expect-error Text roles use solid colors.
resolveTheme({ schema_version: 2, roles: { "text.primary": gradient } });
// @ts-expect-error SVG output has no theme portability option.
renderSvgResult(source, { environment: { theme_portability: "require-portable" } });
void [schema, swatch, familyFlags];

const editorResources: EditorResourceOptions = {
  profile: "constrained",
  limits: { max_source_bytes: 1024, max_document_diagrams: 8 },
};

const directEditorOptions: EditorBindingOptions = {
  fixed_today: "2026-08-12",
  resources: editorResources,
};
const analysisWrappedEditorOptions: EditorBindingOptions = {
  analysis: { fixed_today: "2026-08-12" },
};
const mermanWrappedEditorOptions: EditorBindingOptions = {
  merman: { fixed_local_offset_minutes: 480 },
};

const commonOptions: CommonBindingOptions = {
  analysis: { resources },
  parse: { suppress_errors: true },
};
const asciiOptions: AsciiBindingOptions = {
  ascii: {
    charset: "unicode",
    maxWidth: 80,
    overflow: "fallback",
    graph_presentation: "adaptive",
    trim_trailing_spaces: true,
  },
  merman: { resources },
  parse: { suppress_errors: true },
  timeout_ms: 250,
};
const asciiResult: AsciiRenderResult = renderAsciiResult("flowchart LR\nA --> B", asciiOptions);
const asciiMetadata: BindingOperationMetadata = asciiResult.metadata;
const asciiPlan: AsciiOutputPlan = asciiResult.metadata.output_plan;
const asciiReportVersion: 4 = asciiPlan.schema_version;
const primaryWidth: number | null = asciiPlan.primary_width;
const primaryOverflowed: boolean | null = asciiPlan.primary_overflowed;
const emittedOverflowed: boolean = asciiPlan.emitted_overflowed;
const requestedPresentation: AsciiGraphPresentation = asciiPlan.requested_graph_presentation;
renderAscii("flowchart LR\nA --> B", { ascii: { graphPresentation: "strict" } });
renderAsciiResult("flowchart LR", '{"ascii":{"graph_presentation":"adaptive"}}');
const unmeasuredAsciiPlan: AsciiOutputPlan = {
  ...asciiPlan,
  primary_width: null,
  primary_height: null,
  layout_profile: null,
  primary_overflowed: null,
};
declare const asciiCapability: AsciiCapability;
const graphPresentations: AsciiGraphPresentation[] = asciiCapability.graph_presentations;
const invalidGraphPresentation: AsciiBindingOptions = {
  // @ts-expect-error unknown policies must not silently opt into adaptive recovery.
  ascii: { graph_presentation: "best_effort" },
};
const nullGraphPresentation: AsciiBindingOptions = {
  // @ts-expect-error only omission selects the default presentation policy.
  ascii: { graphPresentation: null },
};
const obsoleteAsciiPlan = {
  ...asciiPlan,
  // @ts-expect-error the current report writer emits schema 4.
  schema_version: 3,
} satisfies AsciiOutputPlan;
// @ts-expect-error schema 4 separates primary and emitted overflow.
asciiPlan.overflowed;
// @ts-expect-error the result exposes text instead of an SVG payload.
asciiResult.svg;
void [asciiMetadata, asciiReportVersion, primaryWidth, primaryOverflowed, emittedOverflowed,
  requestedPresentation, unmeasuredAsciiPlan, graphPresentations, invalidGraphPresentation,
  nullGraphPresentation, obsoleteAsciiPlan];
const svgOptions: SvgBindingOptions = {
  fixed_today: "2026-08-12",
  parse: { suppress_errors: true },
  svg: { diagram_id: "example" },
  layout: { adaptation: "fit-width", container_width: 421 },
};
const shoalSource = "---\nconfig:\n  layout: shoal\n---\nflowchart LR\nA --> B";
const capturedFrame = renderSvgResult(shoalSource, {
  layout: { capture_shoal_frame: true },
}).metadata.shoal_layout_frame;
renderSvgResult(shoalSource, {
  layout: {
    adaptation: "fit-width",
    container_width: 640,
    fold_wide_ranks: true,
    ...(capturedFrame && { previous_shoal_frame: capturedFrame }),
  },
});
renderSvgResult(shoalSource, {
  // @ts-expect-error Replay accepts an opaque frame object.
  layout: { previous_shoal_frame: [] },
});
const explicitNullTextMeasurementOptions: SvgBindingOptions = {
  environment: { text_measurement: null },
};
const hostMeasuredSvgOptions: HostTextMeasurerSvgBindingOptions = {
  environment: { math_renderer: "none" },
  svg: { diagram_id: "host-measured" },
};
const hostTextMeasurer = () => ({ handled: false as const });
renderSvgWithTextMeasurer("flowchart TD\nA --> B", hostTextMeasurer, hostMeasuredSvgOptions);
layoutJsonWithTextMeasurer("flowchart TD\nA --> B", hostTextMeasurer, hostMeasuredSvgOptions);
renderSvgWithTextMeasurer("flowchart TD\nA --> B", hostTextMeasurer, {
  // @ts-expect-error a host callback owns text measurement, so an explicit selector would conflict.
  environment: { text_measurement: "deterministic" },
});
layoutJsonWithTextMeasurer("flowchart TD\nA --> B", hostTextMeasurer, {
  // @ts-expect-error a host callback owns text measurement, so an explicit selector would conflict.
  environment: { text_measurement: "deterministic" },
});
renderSvgWithTextMeasurer("flowchart TD\nA --> B", hostTextMeasurer, {
  // @ts-expect-error a host callback also conflicts with an explicit null selector.
  environment: { text_measurement: null },
});

const tightenedSvgOptions = withResourceOptions(
  {
    analysis: { fixed_today: "2026-08-12" },
    svg: { diagram_id: "example" },
  } satisfies SvgBindingOptions,
  resources,
);
tightenedSvgOptions.svg.diagram_id;

// @ts-expect-error browser editor sessions cannot select a looser native profile.
const looserEditorProfile = { resources: { profile: "trusted-native" } } satisfies EditorBindingOptions;

// @ts-expect-error browser editor sessions expose only analysis-owned resource limits.
const rendererOnlyEditorLimit = { resources: { limits: { max_svg_bytes: 1024 } } } satisfies EditorBindingOptions;

// @ts-expect-error direct analysis options cannot be mixed with the analysis wrapper.
const mixedAnalysisRoot: EditorBindingOptions = {
  fixed_today: "2026-08-12",
  analysis: {},
};

// @ts-expect-error direct analysis options cannot be mixed with the merman wrapper.
const mixedMermanRoot: EditorBindingOptions = {
  resources: editorResources,
  merman: {},
};

// @ts-expect-error the analysis and merman wrappers are mutually exclusive.
const duplicateWrapperRoot: EditorBindingOptions = {
  analysis: {},
  merman: {},
};

// @ts-expect-error parse remains orthogonal but cannot make an invalid analysis root valid.
const mixedCommonRoot: CommonBindingOptions = {
  fixed_local_offset_minutes: 480,
  analysis: {},
  parse: { suppress_errors: true },
};

// @ts-expect-error renderer-specific fields cannot make duplicate wrappers valid.
const mixedSvgRoot: SvgBindingOptions = {
  analysis: {},
  merman: {},
  svg: { diagram_id: "example" },
};

void directEditorOptions;
void editorResources;
void analysisWrappedEditorOptions;
void mermanWrappedEditorOptions;
void commonOptions;
void asciiOptions;
void svgOptions;
void explicitNullTextMeasurementOptions;
void hostMeasuredSvgOptions;
void mixedAnalysisRoot;
void mixedMermanRoot;
void duplicateWrapperRoot;
void mixedCommonRoot;
void mixedSvgRoot;
void looserEditorProfile;
void rendererOnlyEditorLimit;
// Prepared redraws retain parse configuration and can select a theme per output.
import type { BrowserPreparedSource, PreparedSourceOutputOptions, HostTextMeasurerPreparedSourceOutputOptions } from "../src/public-types.js";
declare const preparedSource: BrowserPreparedSource;
preparedSource.renderSvg({ layout: { container_width: 480 }, background: "#F7F3E8", resources, timeout_ms: 100 });
preparedSource.renderSvgWithTextMeasurer(() => undefined, { svg: { diagram_id: "retained" } });
const preparedOutput: PreparedSourceOutputOptions = { layout: { container_width: 640 } };
// @ts-expect-error Parse options require a new preparation.
preparedSource.renderSvg({ parse: { suppress_errors: true } });
// @ts-expect-error Configuration requires a new preparation.
preparedSource.renderSvg({ site_config: {} });
preparedSource.renderSvg({ theme: null });
preparedSource.renderSvg({ theme: { preset: "studio", appearance: "dark" } });
// @ts-expect-error Host measurers own the output measurement selection.
const preparedHostOutput: HostTextMeasurerPreparedSourceOutputOptions = { environment: { text_measurement: "deterministic" } };
void preparedOutput;
void preparedHostOutput;

const unsupportedAdaptation: SvgBindingOptions = {
  // @ts-expect-error explicit placement modes are bounded by the Rust owner.
  layout: { adaptation: "shrink-font" },
};
void unsupportedAdaptation;

const adaptationIds: string[] = svgPlanJson("kanban").supported_layout_adaptation_ids;
const adaptation = renderSvgResult("kanban").metadata.layout_adaptation;
if (adaptation) {
  const width: number = adaptation.actual_width;
  const overflowed: boolean = adaptation.overflowed;
  void width;
  void overflowed;
}
void adaptationIds;

const mountedOptions: import("../src/public-types.js").MountDiagramOptions = {
  outputOptions: { layout: { adaptation: "fit-width" }, timeout_ms: 250 },
  textMeasurement: "browser",
};
const invalidMountedOptions: import("../src/public-types.js").MountDiagramOptions = {
  // @ts-expect-error The mount owns its SVG ID namespace.
  outputOptions: { svg: { diagram_id: "shared-id" } },
};
void [mountedOptions, invalidMountedOptions];
