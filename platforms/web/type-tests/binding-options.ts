import { withResourceOptions } from "../src/runtime-core.js";
import {
  describeThemeSupport,
  exportThemePreset,
  materializeTheme,
} from "../src/runtime-render.js";
import type {
  AsciiBindingOptions,
  CommonBindingOptions,
  EditorBindingOptions,
  EditorResourceOptions,
  ResourceOptions,
  SvgBindingOptions,
  ThemeLinearGradientRepetition,
  ThemePaint,
  ThemeRadialGradientRepetition,
  ThemeCapabilityDescriptorV2,
  ThemeDefinitionV1,
  ThemeSupportQueryV2,
} from "../src/public-types.js";

const resources: ResourceOptions = { profile: "interactive" };
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
  ascii: { charset: "unicode" },
  merman: { resources },
  parse: { suppress_errors: true },
};
const svgOptions: SvgBindingOptions = {
  fixed_today: "2026-08-12",
  parse: { suppress_errors: true },
  svg: { diagram_id: "example" },
};

const themeDefinition: ThemeDefinitionV1 = {
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: { text: "#123456", accent: "#abcdef" },
};
const materializedTheme = materializeTheme(themeDefinition, {
  resources: { profile: "constrained" },
});
materializedTheme.spec.styles;

const supportQuery: ThemeSupportQueryV2 = {
  schema_version: 2,
  family: "sequence",
  output: "standalone-svg",
  subject: { kind: "base-typography", property: "font-stack" },
};
const supportDescriptor: ThemeCapabilityDescriptorV2 =
  describeThemeSupport(supportQuery);
supportDescriptor.state;

const presetExport = exportThemePreset("editor-light");
if (presetExport.kind === "complete_spec") {
  presetExport.complete_spec;
}

const tightenedSvgOptions = withResourceOptions(
  {
    analysis: { fixed_today: "2026-08-12" },
    svg: { diagram_id: "example" },
  } satisfies SvgBindingOptions,
  resources,
);
tightenedSvgOptions.svg.diagram_id;

const repeatedLinearCanvas: ThemePaint = {
  kind: "linear-gradient",
  angle_degrees: 135,
  stops: [
    { offset: 0, color: "#0f172a" },
    { offset: 1, color: "#f8fafc" },
  ],
  repetition: { kind: "repeating", period_px: 16 },
};
const tiledRadialCanvas: ThemePaint = {
  kind: "radial-gradient",
  center_x: { percent: 50 },
  center_y: { percent: 50 },
  radius: { percent: 50 },
  stops: [
    { offset: 0, color: "#22d3ee55" },
    { offset: 1, color: "#22d3ee00" },
  ],
  repetition: { kind: "tiled", width_px: 20, height_px: 20 },
};

// @ts-expect-error repeating linear gradients require one explicit period.
const implicitLinearRepetition = { kind: "repeating" } satisfies ThemeLinearGradientRepetition;

const ambiguousLinearRepetition = {
  kind: "repeating",
  period_px: 16,
  // @ts-expect-error repetition variants cannot mix period and tile geometry.
  width_px: 20,
  height_px: 20,
} satisfies ThemeLinearGradientRepetition;

// @ts-expect-error radial repetition uses its authored radius and accepts no linear period.
const radialWithLinearPeriod = { kind: "repeating", period_px: 16 } satisfies ThemeRadialGradientRepetition;

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
void themeDefinition;
void materializedTheme;
void supportQuery;
void supportDescriptor;
void presetExport;
void mixedAnalysisRoot;
void mixedMermanRoot;
void duplicateWrapperRoot;
void mixedCommonRoot;
void mixedSvgRoot;
void looserEditorProfile;
void rendererOnlyEditorLimit;
void repeatedLinearCanvas;
void tiledRadialCanvas;
void implicitLinearRepetition;
void ambiguousLinearRepetition;
void radialWithLinearPeriod;
