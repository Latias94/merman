import { withResourceOptions } from "../src/runtime-core.js";
import type {
  AsciiBindingOptions,
  CommonBindingOptions,
  EditorBindingOptions,
  ResourceOptions,
  SvgBindingOptions,
} from "../src/public-types.js";

const resources: ResourceOptions = { profile: "interactive" };

const directEditorOptions: EditorBindingOptions = {
  fixed_today: "2026-08-12",
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

const tightenedSvgOptions = withResourceOptions(
  {
    analysis: { fixed_today: "2026-08-12" },
    svg: { diagram_id: "example" },
  } satisfies SvgBindingOptions,
  resources,
);
tightenedSvgOptions.svg.diagram_id;

// @ts-expect-error direct analysis options cannot be mixed with the analysis wrapper.
const mixedAnalysisRoot: EditorBindingOptions = {
  fixed_today: "2026-08-12",
  analysis: {},
};

// @ts-expect-error direct analysis options cannot be mixed with the merman wrapper.
const mixedMermanRoot: EditorBindingOptions = {
  resources,
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
