import type { ThemeEffectGraph } from "../src/index.js";
import { withResourceOptions } from "../src/runtime-core.js";
import {
  describeThemeSupport,
  exportThemePreset,
  layoutJsonWithTextMeasurer,
  materializeTheme,
  renderSvgWithTextMeasurer,
} from "../src/runtime-render.js";
import type { BindingDiagnosticErrorDetails } from "../src/public-catalog.js";
import type {
  AsciiBindingOptions,
  CommonBindingOptions,
  EditorBindingOptions,
  EditorResourceOptions,
  HostTextMeasurerSvgBindingOptions,
  ResourceOptions,
  SvgBindingOptions,
  ThemeLinearGradientRepetition,
  ThemePaint,
  ThemeRadialGradientRepetition,
  ThemeCapabilityDescriptorV1,
  ThemeAuthoringTypographyV1,
  ThemeAuthoringOptions,
  ThemeDefinitionV1,
  ThemeRecipeV1,
  ThemeStylePatch,
  ThemeDiagnostic,
  ThemeExecutionEvidenceV1,
  ThemeSupportQueryV1,
  ThemeSupportUnknownSubjectKindV1,
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
    trim_trailing_spaces: true,
  },
  merman: { resources },
  parse: { suppress_errors: true },
};
const svgOptions: SvgBindingOptions = {
  fixed_today: "2026-08-12",
  parse: { suppress_errors: true },
  svg: { diagram_id: "example" },
};
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

const themeDefinition: ThemeDefinitionV1 = {
  authoring_schema_version: 1,
  expansion_version: 1,
  tokens: { text: "#123456", accent: "#abcdef" },
};
const materializedTheme = materializeTheme(themeDefinition, {
  resources: { profile: "constrained", limits: { max_theme_encoded_bytes: 1024 } },
});
const themeAuthoringOptions: ThemeAuthoringOptions = {
  resources: { limits: { max_theme_encoded_bytes: 1024 } },
};
materializeTheme(themeDefinition, themeAuthoringOptions);
// @ts-expect-error theme-only limits must not leak into ordinary SVG options.
const invalidSvgOptions: SvgBindingOptions = { resources: { limits: { max_theme_encoded_bytes: 1 } } };
exportThemePreset("editor-light", themeAuthoringOptions);
materializeTheme(themeDefinition, { analysis: themeAuthoringOptions });
materializeTheme(themeDefinition, { merman: themeAuthoringOptions });
materializeTheme(themeDefinition, {
  // @ts-expect-error source limits do not apply to authoring operations.
  resources: { limits: { max_source_bytes: 1024 } },
});
exportThemePreset("editor-light", {
  // @ts-expect-error hard caps are implementation-owned.
  resources: { limits: { theme_encoded_bytes_hard_cap: 1024 } },
});
// @ts-expect-error wrapped resources cannot be combined with direct resources.
const mixedThemeOptions: ThemeAuthoringOptions = { resources: {}, analysis: { resources: {} } };
// @ts-expect-error only one analysis wrapper is allowed.
const ambiguousThemeOptions: ThemeAuthoringOptions = { analysis: {}, merman: {} };
materializedTheme.spec.styles;

const supportQuery: ThemeSupportQueryV1 = {
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  subject: { kind: "base-typography", property: "font-stack" },
};
const supportDescriptor: ThemeCapabilityDescriptorV1 =
  describeThemeSupport(supportQuery, themeAuthoringOptions);
supportDescriptor.state;

const futureSubjectKind = "future-subject" as ThemeSupportUnknownSubjectKindV1;
const futureSupportQuery: ThemeSupportQueryV1 = {
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  subject: { kind: futureSubjectKind, future_option: true },
};

const incompleteRuleSupportQuery = {
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  // @ts-expect-error known rule subjects require both target and facet.
  subject: { kind: "rule" },
} satisfies ThemeSupportQueryV1;

const incompleteOrdinalSupportQuery = {
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  // @ts-expect-error known ordinal-palette subjects require target.
  subject: { kind: "ordinal-palette" },
} satisfies ThemeSupportQueryV1;

const incompleteTypographySupportQuery = {
  schema_version: 1,
  family: "sequence",
  output: "standalone-svg",
  // @ts-expect-error known base-typography subjects require property.
  subject: { kind: "base-typography" },
} satisfies ThemeSupportQueryV1;

// @ts-expect-error the Rust authoring wire rejects null for the outer stroke patch.
const nullOuterStroke = { stroke: null } satisfies ThemeStylePatch;

// @ts-expect-error the Rust authoring wire rejects null for the outer typography patch.
const nullOuterTypography = { typography: null } satisfies ThemeStylePatch;

const compactTypographyWithLineHeight = {
  // @ts-expect-error compact authoring typography has no portable base line-height consumer.
  line_height: "normal",
} satisfies ThemeAuthoringTypographyV1;

const srgbEffectGraph: ThemeEffectGraph = {
  kind: "graph",
  id: "glow",
  color_space: "srgb",
  primitives: [{ kind: "gaussian-blur", std_deviation: 2 }],
};
const linearEffectGraph: ThemeEffectGraph = {
  ...srgbEffectGraph,
  color_space: "linear-rgb",
};
const defaultEffectGraph: ThemeEffectGraph = {
  kind: "graph",
  id: "default-glow",
  primitives: [],
};
const invalidEffectColorSpace = {
  ...srgbEffectGraph,
  // @ts-expect-error only the Rust wire's interpolation spaces are supported.
  color_space: "display-p3",
} satisfies ThemeEffectGraph;
const nullEffectColorSpace = {
  ...srgbEffectGraph,
  // @ts-expect-error omission selects the default, but explicit null is invalid.
  color_space: null,
} satisfies ThemeEffectGraph;
const cyberpunkExport = exportThemePreset("cyberpunk");
if (cyberpunkExport.kind === "complete_spec") {
  for (const effect of cyberpunkExport.complete_spec.effects ?? []) {
    if (effect.kind === "graph") {
      const colorSpace: "linear-rgb" | "srgb" | undefined = effect.color_space;
      void colorSpace;
    }
  }
}
void linearEffectGraph;
void defaultEffectGraph;
void invalidEffectColorSpace;
void nullEffectColorSpace;

const presetExport: ThemeRecipeV1 = exportThemePreset("editor-light");
const exportedThemeOptions: SvgBindingOptions = { theme: presetExport };
if (presetExport.kind === "complete_spec") {
  presetExport.complete_spec;
}
const authoredRecipe = {
  schema_version: 1,
  kind: "definition",
  definition: themeDefinition,
} satisfies ThemeRecipeV1;
const authoredThemeOptions: SvgBindingOptions = { theme: authoredRecipe };
const completeRecipe = {
  schema_version: 1,
  kind: "complete_spec",
  complete_spec: { styles: [] },
} satisfies ThemeRecipeV1;
const completeThemeOptions: SvgBindingOptions = { theme: completeRecipe };
const futureRecipe = { ...authoredRecipe, schema_version: 2 } as const;
// @ts-expect-error unknown recipe versions require runtime migration, not unchecked admission.
const futureThemeOptions: SvgBindingOptions = { theme: futureRecipe };
const mixedPresetRecipe = { ...authoredRecipe, preset: "editor-light" };
// @ts-expect-error recipe and preset selection must not compete for ownership.
const mixedPresetThemeOptions: SvgBindingOptions = { theme: mixedPresetRecipe };
const mixedSpecRecipe = { ...completeRecipe, spec: {} };
// @ts-expect-error recipe and direct spec selection are mutually exclusive.
const mixedSpecThemeOptions: SvgBindingOptions = { theme: mixedSpecRecipe };
const ambiguousRecipe = { ...authoredRecipe, complete_spec: {} };
// @ts-expect-error a recipe has exactly one payload, including structurally assigned values.
const ambiguousRecipeOptions: SvgBindingOptions = { theme: ambiguousRecipe };
const unversionedRecipe = { kind: "definition", definition: themeDefinition } as const;
// @ts-expect-error shared recipes always carry their wire version.
const unversionedThemeOptions: SvgBindingOptions = { theme: unversionedRecipe };

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
void explicitNullTextMeasurementOptions;
void hostMeasuredSvgOptions;
void themeDefinition;
void materializedTheme;
void supportQuery;
void supportDescriptor;
void futureSupportQuery;
void incompleteRuleSupportQuery;
void incompleteOrdinalSupportQuery;
void incompleteTypographySupportQuery;
void nullOuterStroke;
void nullOuterTypography;
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


const strictThemeOptions = {
  environment: { theme_portability: "require-portable" },
} satisfies SvgBindingOptions;
void strictThemeOptions;
// @ts-expect-error Unknown request policies must not silently weaken admission.
const unknownThemePolicy = { environment: { theme_portability: "future-policy" } } satisfies SvgBindingOptions;
void unknownThemePolicy;

// Open diagnostic identifiers must survive newer renderer implementations.
const themeDiagnostic: ThemeDiagnostic = {
  code: "future-code",
  subject: "future-subject",
  target: "future-target",
  property: "future-property",
  source_document: "future-document",
  source_paths: ["/styles/2"],
  generated: false,
};
const themeOutcome: ThemeExecutionEvidenceV1 = {
  version: 1,
  family_id: "class",
  theme_status: "residual",
  output_mutated: false,
  target_kind: "svg",
  target_status: "rejected",
  target_reason_ids: ["theme_evidence_incomplete"],
  font_source: "embedded",
  diagnostics: [themeDiagnostic],
};
const legacyThemeOutcome: ThemeExecutionEvidenceV1 = {
  ...themeOutcome,
  diagnostics: undefined,
};
legacyThemeOutcome.diagnostics?.map((diagnostic) => diagnostic.source_paths);
const malformedThemeDiagnostic: ThemeDiagnostic = {
  ...themeDiagnostic,
  // @ts-expect-error source pointers are strings, not compiled rule indexes.
  source_paths: [2],
};
void malformedThemeDiagnostic;
