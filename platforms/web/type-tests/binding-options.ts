import { withResourceOptions } from "../src/runtime-core.js";
import type { BindingDiagnosticErrorDetails } from "../src/public-catalog.js";
import type {
  AsciiBindingOptions,
  CommonBindingOptions,
  EditorBindingOptions,
  EditorResourceOptions,
  ResourceOptions,
  SvgBindingOptions,
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
void mixedAnalysisRoot;
void mixedMermanRoot;
void duplicateWrapperRoot;
void mixedCommonRoot;
void mixedSvgRoot;
void looserEditorProfile;
void rendererOnlyEditorLimit;
