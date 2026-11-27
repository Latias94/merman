mod builtin;
mod context;
mod embedding;
mod final_validation;
mod policy;
mod preset;
mod resource_closure;
mod standalone;
mod static_validation;
mod step_animation;
mod text_tracking;

pub use builtin::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor,
};
pub(crate) use builtin::{GitGraphBranchLabelBaselinePostprocessor, RebaseSvgIdsPostprocessor};
pub(crate) use context::SvgPostprocessExecution;
pub use context::{SvgPostprocessContext, SvgPostprocessMetadata};
pub(crate) use final_validation::{SvgStructureMetrics, validate_well_formed_svg_with_controls};
pub(crate) use merman_render_core::svg_scan::{
    SvgTagScanner, checkpoint_loop, end_tag_name, escape_xml_attr_with_checkpoints,
    escape_xml_text_with_checkpoints, extract_exact_double_quoted_attr_with_checkpoints,
    find_tag_end_with_checkpoints, find_with_checkpoints, next_svg_quoted_attr_with_checkpoints,
    rfind_with_checkpoints, start_tag_name,
};
pub use policy::SvgOutputPolicy;
pub use preset::SvgPipelinePreset;
pub use resource_closure::SvgResourceClosure;
pub use standalone::{StandaloneSvgArtifact, StandaloneSvgTerminalStatus};
pub(crate) use text_tracking::SvgFontSeal;

pub(crate) fn is_css_value_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "style"
            | "background"
            | "background-image"
            | "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "cursor"
            | "color-profile"
    )
}

pub(crate) fn is_svg_idref_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "aria-activedescendant"
            | "aria-controls"
            | "aria-describedby"
            | "aria-details"
            | "aria-errormessage"
            | "aria-flowto"
            | "aria-labelledby"
            | "aria-owns"
    )
}

/// Internal cross-crate adapter for rebasing ids in Merman-produced SVG.
#[doc(hidden)]
pub fn rebase_svg_ids(
    svg: &str,
    prefix: impl Into<String>,
    session: &RenderSession,
) -> Result<String> {
    SvgPipeline::parity()
        .with_rebased_ids(prefix)
        .process_to_string(svg, session)
}

/// Validates sanitized SVG for Merman's known-host static-inline embedding policy.
///
/// This policy permits ordinary navigation links and same-document fragment references. It is not
/// a general DOM-mount admission contract: the host must not inject or inherit a `<base>` URL that
/// changes how fragment-only references resolve.
#[doc(hidden)]
pub fn validate_static_inline_svg(svg: &str, session: &RenderSession) -> Result<()> {
    static_validation::validate_rustdoc_static_svg(svg, SvgPostprocessExecution::new(session))
}

/// Validates renderer output before static-inline fallback and compatibility transformations.
#[doc(hidden)]
pub fn validate_static_inline_svg_admission(svg: &str, session: &RenderSession) -> Result<()> {
    static_validation::validate_rustdoc_admission_svg(svg, SvgPostprocessExecution::new(session))
}

use crate::environment::RenderSession;
use crate::math::{PreparedMathEvidenceLease, PreparedMathTerminalReceipt};
use crate::svg::SvgOutputProfile;
use crate::{Error, Result};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PipelineTerminalValidation {
    Immediate,
    Deferred,
}

struct PipelineRunResult<'a> {
    svg: Cow<'a, str>,
    terminal: Option<final_validation::TerminalSvgValidation>,
}

pub trait SvgPostprocessor: Send + Sync {
    fn name(&self) -> &'static str;

    /// Host CSS injected by this pass after layout, when applicable.
    fn scoped_css(&self) -> Option<&str> {
        None
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>>;
}

/// SVG that has passed the terminal resvg compatibility and rendering-resource finalizer.
///
/// The inner string cannot be constructed directly. Custom postprocessors operate on an SVG draft
/// before finalization and therefore cannot claim this type. Structural resources are limited to
/// same-document fragments, ordinary image elements require approved, syntactically valid inline
/// raster data URLs, and `feImage` accepts either form. Theme font resources are not embedded.
///
/// ```compile_fail
/// use merman_render::svg::ResvgCompatibleSvg;
///
/// let forged = ResvgCompatibleSvg { svg: "<svg/>".to_string() };
/// ```
#[derive(Clone)]
pub struct ResvgCompatibleSvg {
    svg: String,
    inline_raster_sources: Box<[(usize, final_validation::InlineRasterHref)]>,

    prepared_math_evidence: PreparedMathEvidenceLease,
    prepared_math_terminal_receipt: Option<PreparedMathTerminalReceipt>,
    prepared_math_evidence_valid: bool,
    finalization_report: SvgFinalizationReport,
    font_catalog: crate::text::FontCatalog,
    font_source_policy: crate::text::FontSourcePolicy,
}

/// Reference-expansion work retained by a sealed resvg-compatible SVG.
///
/// The occurrence slice is ordered by source-document element encounter order. Exporters use it
/// to charge each inline resource once for every `<use>`-expanded instance before asking usvg to
/// decode that resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgReferencePlan {
    expanded_elements: usize,
    max_tree_depth: usize,
    raw_element_occurrences: Box<[usize]>,
}

impl SvgReferencePlan {
    /// Returns the element count after same-document `<use>` expansion.
    pub const fn expanded_elements(&self) -> usize {
        self.expanded_elements
    }

    /// Returns the maximum resolved XML-tree depth after `<use>` expansion.
    pub const fn max_tree_depth(&self) -> usize {
        self.max_tree_depth
    }

    /// Returns expanded instance counts for source elements in document encounter order.
    pub fn raw_element_occurrences(&self) -> &[usize] {
        &self.raw_element_occurrences
    }
}

/// Immutable evidence produced by the terminal SVG finalizer.
///
/// Raw SVG drafts do not carry this report. It is created only after the exact selected artifact
/// has completed XML, CSS, reference, and resource validation. The retained preset identifies the
/// pipeline that produced those exact bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgFinalizationReport {
    preset: SvgPipelinePreset,
    postprocessor_names: Box<[String]>,
    drop_native_duplicate_fallbacks: bool,
    reference_plan: SvgReferencePlan,
    resource_closure: SvgResourceClosure,
    text_element_count: usize,
    font_seal: SvgFontSeal,
    fallback_css_residual: bool,
}

impl SvgFinalizationReport {
    pub const fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    pub fn postprocessor_names(&self) -> &[String] {
        &self.postprocessor_names
    }

    pub const fn drop_native_duplicate_fallbacks(&self) -> bool {
        self.drop_native_duplicate_fallbacks
    }

    pub const fn reference_plan(&self) -> &SvgReferencePlan {
        &self.reference_plan
    }

    pub const fn resource_closure(&self) -> &SvgResourceClosure {
        &self.resource_closure
    }

    /// Counts authored SVG `<text>` elements with descendant character data (including
    /// whitespace) or an effective local `<tref>`. It does not inspect the referenced text,
    /// count `<use>` instances, or predict visibility, whitespace trimming, or native font use.
    pub const fn text_element_count(&self) -> usize {
        self.text_element_count
    }

    /// Returns whether readable HTML projection encountered CSS whose native rendering may differ.
    ///
    /// This diagnostic only lowers fidelity admission; it does not establish resource or font safety.
    pub const fn fallback_css_residual(&self) -> bool {
        self.fallback_css_residual
    }

    pub(crate) const fn font_seal(&self) -> &SvgFontSeal {
        &self.font_seal
    }

    fn from_pipeline(
        pipeline: &SvgPipeline,
        terminal: final_validation::TerminalSvgValidation,
    ) -> Self {
        Self {
            preset: pipeline.preset,
            postprocessor_names: pipeline
                .postprocessors
                .iter()
                .map(|entry| entry.processor.name().to_string())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            drop_native_duplicate_fallbacks: pipeline.drop_native_duplicate_fallbacks,
            reference_plan: terminal.reference_plan,
            resource_closure: terminal.resource_closure,
            text_element_count: terminal.text_elements,
            font_seal: terminal.font_seal,
            fallback_css_residual: terminal.fallback_css_residual,
        }
    }
}

impl ResvgCompatibleSvg {
    fn seal(
        svg: String,
        terminal: Option<final_validation::TerminalSvgValidation>,

        prepared_math_evidence: PreparedMathEvidenceLease,
        prepared_math_evidence_valid: bool,
        pipeline: &SvgPipeline,
        session: &RenderSession,
    ) -> Result<Self> {
        let execution = SvgPostprocessExecution::new(session);
        let prepared_math_evidence = if prepared_math_evidence_valid {
            prepared_math_evidence
        } else {
            PreparedMathEvidenceLease::default()
        };
        // Math evidence errors precede terminal resource errors. The receipt remains bound to
        // the exact SVG bytes retained by the sealed artifact.
        let prepared_math_terminal_receipt = if prepared_math_evidence.is_empty() {
            None
        } else {
            PreparedMathTerminalReceipt::from_terminal_svg(&svg, &prepared_math_evidence).map_err(
                |message| Error::svg_postprocess("prepared-math-terminal-receipt", message),
            )?
        };
        let mut terminal = match terminal {
            Some(terminal) => terminal,
            None => {
                final_validation::validate_resvg_compatible_svg_with_execution(&svg, execution)?
            }
        };
        execution.checkpoint()?;
        let font_catalog = session.font_catalog().clone();
        let font_source_policy = session.font_source_policy().clone();
        execution.checkpoint()?;
        Ok(Self {
            svg,
            inline_raster_sources: std::mem::take(&mut terminal.inline_raster_sources),

            prepared_math_evidence,
            prepared_math_terminal_receipt,
            prepared_math_evidence_valid,
            finalization_report: SvgFinalizationReport::from_pipeline(pipeline, terminal),
            font_catalog,
            font_source_policy,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.svg
    }

    pub(crate) const fn prepared_math_terminal_receipt(
        &self,
    ) -> Option<&PreparedMathTerminalReceipt> {
        self.prepared_math_terminal_receipt.as_ref()
    }

    pub(crate) fn prepared_math_evidence_count(&self) -> usize {
        self.prepared_math_evidence.entries().len()
    }

    pub(crate) const fn prepared_math_evidence_valid(&self) -> bool {
        self.prepared_math_evidence_valid
    }

    /// Projects the sealed artifact to SVG text and discards its export resources and proofs.
    pub fn into_string(self) -> String {
        self.svg
    }

    /// Returns the reference-expansion preflight retained at terminal finalization.
    pub const fn reference_plan(&self) -> &SvgReferencePlan {
        self.finalization_report.reference_plan()
    }

    /// Returns the terminal same-document and inline-resource inventory.
    pub const fn resource_closure(&self) -> &SvgResourceClosure {
        self.finalization_report.resource_closure()
    }

    /// Yields each effective inline raster href with its reference-expanded occurrence count.
    #[doc(hidden)]
    pub fn inline_raster_sources(&self) -> impl Iterator<Item = (&str, usize)> {
        self.inline_raster_sources.iter().map(|(element, href)| {
            (
                href.as_str(self.as_str()),
                self.reference_plan().raw_element_occurrences()[*element],
            )
        })
    }

    /// Returns the exact font catalog authorized by the render session.
    pub const fn font_catalog(&self) -> &crate::text::FontCatalog {
        &self.font_catalog
    }

    /// Returns the host-authorized source priority retained for native font resolution.
    pub const fn font_source_policy(&self) -> &crate::text::FontSourcePolicy {
        &self.font_source_policy
    }

    pub const fn finalization_report(&self) -> &SvgFinalizationReport {
        &self.finalization_report
    }
}

impl fmt::Debug for ResvgCompatibleSvg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResvgCompatibleSvg")
            .field("svg", &self.svg)
            .field(
                "prepared_math_occurrence_count",
                &self.prepared_math_evidence.entries().len(),
            )
            .field(
                "prepared_math_evidence_valid",
                &self.prepared_math_evidence_valid,
            )
            .field(
                "prepared_math_terminal_receipt",
                &self.prepared_math_terminal_receipt,
            )
            .field("finalization_report", &self.finalization_report)
            .field("font_catalog", &self.font_catalog.fingerprint())
            .field("font_source_policy", &self.font_source_policy)
            .finish()
    }
}

impl PartialEq for ResvgCompatibleSvg {
    fn eq(&self, other: &Self) -> bool {
        self.svg == other.svg
            && self.prepared_math_evidence == other.prepared_math_evidence
            && self.prepared_math_terminal_receipt == other.prepared_math_terminal_receipt
            && self.prepared_math_evidence_valid == other.prepared_math_evidence_valid
            && self.finalization_report == other.finalization_report
            && self.font_catalog.fingerprint() == other.font_catalog.fingerprint()
            && self.font_source_policy == other.font_source_policy
    }
}

impl Eq for ResvgCompatibleSvg {}

impl AsRef<str> for ResvgCompatibleSvg {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineContract {
    Browser,
    Static,
}

#[derive(Clone)]
struct SvgPostprocessorEntry {
    processor: Arc<dyn SvgPostprocessor>,
    family: Option<crate::DiagramFamilyId>,
    preserves_prepared_math: bool,
    preserves_palette_slots: bool,
}

impl SvgPostprocessorEntry {
    fn untrusted(processor: Arc<dyn SvgPostprocessor>) -> Self {
        Self {
            processor,
            family: None,
            preserves_prepared_math: false,
            preserves_palette_slots: false,
        }
    }

    // Unknown family identity remains conservative for both execution and evidence decisions.
    fn applies_to(&self, family: Option<crate::DiagramFamilyId>) -> bool {
        match (self.family, family) {
            (Some(required), Some(actual)) => required == actual,
            _ => true,
        }
    }
}

/// Selects which generated styles an SVG consumer needs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SvgGeneration {
    /// Retain the complete generated styles for Mermaid-compatible and mutable SVGs.
    #[default]
    Full,
    /// Generate container palette rules only for slots used by the static document.
    ///
    /// This preserves authored CSS and SVG content. Consumer compatibility, animation
    /// projection, and embedding admission are handled by the selected output pipeline.
    Static,
}

#[derive(Clone)]
pub struct SvgPipeline {
    preset: SvgPipelinePreset,
    generation: Option<SvgGeneration>,
    postprocessors: Vec<SvgPostprocessorEntry>,
    drop_native_duplicate_fallbacks: bool,
    inline_contract: Option<InlineContract>,
    isolate_ids: bool,
}

impl fmt::Debug for SvgPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .postprocessors
            .iter()
            .map(|pass| pass.processor.name())
            .collect::<Vec<_>>();

        f.debug_struct("SvgPipeline")
            .field("preset", &self.preset)
            .field("generation", &self.generation)
            .field("postprocessors", &names)
            .field(
                "drop_native_duplicate_fallbacks",
                &self.drop_native_duplicate_fallbacks,
            )
            .field("inline_contract", &self.inline_contract)
            .field("isolate_ids", &self.isolate_ids)
            .finish()
    }
}

impl Default for SvgPipeline {
    fn default() -> Self {
        Self::parity()
    }
}

impl SvgPipeline {
    /// Default browser-facing SVG; preserves Mermaid labels and styles without text overlays.
    pub fn parity() -> Self {
        Self::from_preset(SvgPipelinePreset::Parity)
    }

    /// Advanced text overlay for consumers that ignore HTML labels.
    ///
    /// Browsers may show duplicate text. Prefer [`Self::parity`] for browser previews or
    /// [`Self::resvg_safe`] for resvg/usvg compatibility. See [`SvgPipelinePreset::Readable`].
    pub fn readable() -> Self {
        Self::from_preset(SvgPipelinePreset::Readable)
    }

    /// Converts HTML labels and applies terminal resvg compatibility cleanup and validation.
    pub fn resvg_safe() -> Self {
        Self::from_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn from_preset(preset: SvgPipelinePreset) -> Self {
        Self {
            preset,
            generation: None,
            postprocessors: Vec::new(),
            drop_native_duplicate_fallbacks: false,
            inline_contract: None,
            isolate_ids: false,
        }
    }

    /// Returns whether a host-provided postprocessor applies to this output.
    pub(crate) fn has_custom_postprocessors(&self, family: Option<crate::DiagramFamilyId>) -> bool {
        self.postprocessors
            .iter()
            .any(|entry| entry.applies_to(family))
    }

    /// Returns whether the pipeline can preserve renderer-owned prepared-math occurrences.
    pub(crate) fn preserves_prepared_math_evidence(
        &self,
        family: Option<crate::DiagramFamilyId>,
    ) -> bool {
        self.postprocessors
            .iter()
            .all(|entry| !entry.applies_to(family) || entry.preserves_prepared_math)
    }

    /// Returns whether this pipeline needs renderer-owned prepared-math evidence in order to
    /// project internal browser markers before its opaque publication passes run.
    pub(crate) fn requires_prepared_math_projection(&self) -> bool {
        self.inline_contract.is_some()
    }

    pub fn preset(&self) -> SvgPipelinePreset {
        self.preset
    }

    pub(crate) fn validate_output_profile(&self, profile: SvgOutputProfile) -> Result<()> {
        if profile == SvgOutputProfile::NativeCompact
            && (self.preset != SvgPipelinePreset::Parity || !self.postprocessors.is_empty())
        {
            return Err(Error::UnsupportedSvgOutputProfile {
                profile,
                reason: "the compact profile does not support SVG transformations",
            });
        }
        Ok(())
    }

    /// Selects the generation policy for renderer-owned Mermaid SVG drafts.
    ///
    /// Ordinary SVG defaults to [`SvgGeneration::Full`]. Encoded export targets default to
    /// [`SvgGeneration::Static`]. Opaque postprocessors receive the full draft because they can
    /// introduce new palette slots. Processing existing SVG runs the same stages in either mode.
    pub fn with_generation(mut self, generation: SvgGeneration) -> Self {
        self.generation = Some(generation);
        self
    }

    pub(crate) fn generation_for(
        &self,
        family: crate::DiagramFamilyId,
        default: SvgGeneration,
    ) -> SvgGeneration {
        match self.generation.unwrap_or(default) {
            SvgGeneration::Static
                if self.inline_contract != Some(InlineContract::Browser)
                    && self.postprocessors.iter().all(|entry| {
                        !entry.applies_to(Some(family)) || entry.preserves_palette_slots
                    }) =>
            {
                SvgGeneration::Static
            }
            _ => SvgGeneration::Full,
        }
    }

    /// Keeps every configured draft transformation while replacing the terminal output contract.
    pub fn with_preset(mut self, preset: SvgPipelinePreset) -> Self {
        self.preset = preset;
        self
    }

    pub fn into_resvg_safe(self) -> Self {
        self.with_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn with_drop_native_duplicate_fallbacks(mut self, drop: bool) -> Self {
        self.drop_native_duplicate_fallbacks = drop;
        self
    }

    /// Adds Merman's static-inline publication contract to this pipeline.
    ///
    /// Admission runs before any lossy compatibility transform. Final validation runs after all
    /// configured transforms and XML 1.0 character cleanup, while the renderer-owned session is
    /// still available for cancellation and cumulative resource accounting.
    #[doc(hidden)]
    pub fn with_static_inline_contract(mut self, id_prefix: impl Into<String>) -> Self {
        self.inline_contract = Some(InlineContract::Static);
        self.push_postprocessor(ForeignObjectFallbackPostprocessor);
        self.push_postprocessor(SanitizeCssPostprocessor);
        self.push_postprocessor(SanitizeSvgAttributesPostprocessor);
        self.with_rebased_ids(id_prefix)
    }

    /// Adds Merman's browser-inline publication contract without converting HTML labels.
    ///
    /// Resource and selector-scope checks run before any configured transformation and again
    /// after terminal processing. Safe XHTML labels and browser CSS remain available; select a
    /// compatible preset separately when the consumer also needs a text-only SVG. Custom CSS
    /// animation names remain document-global, matching Mermaid.
    ///
    /// As with the static-inline contract, the host must not introduce a `<base>` URL that changes
    /// the resolution of same-document fragment references.
    #[doc(hidden)]
    pub fn with_browser_inline_contract(mut self, id_prefix: impl Into<String>) -> Self {
        self.inline_contract = Some(InlineContract::Browser);
        self.with_rebased_ids(id_prefix)
    }

    /// Isolates SVG IDs and their references without imposing an embedding safety contract.
    ///
    /// Renderer-certified Mindmap output first drops the redundant background-shape ID retained
    /// by raw Mermaid parity. Other duplicate IDs remain errors.
    #[doc(hidden)]
    pub fn with_rebased_ids(mut self, id_prefix: impl Into<String>) -> Self {
        self.isolate_ids = true;
        self.with_postprocessor(RebaseSvgIdsPostprocessor::new(id_prefix))
    }

    pub fn with_postprocessor<P>(mut self, postprocessor: P) -> Self
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors
            .push(SvgPostprocessorEntry::untrusted(Arc::new(postprocessor)));
        self
    }

    pub fn with_shared_postprocessor(mut self, postprocessor: Arc<dyn SvgPostprocessor>) -> Self {
        self.postprocessors
            .push(SvgPostprocessorEntry::untrusted(postprocessor));
        self
    }

    pub fn push_postprocessor<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors
            .push(SvgPostprocessorEntry::untrusted(Arc::new(postprocessor)));
    }

    /// Registers a renderer-owned pass whose implementation is proven not to alter prepared-math
    /// occurrence markers, native projection bytes, or palette participants. This is intentionally
    /// crate-private: opaque host postprocessors use the public registration methods above.
    pub(crate) fn push_postprocessor_preserving_prepared_math<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(SvgPostprocessorEntry {
            processor: Arc::new(postprocessor),
            family: None,
            preserves_prepared_math: true,
            preserves_palette_slots: true,
        });
    }

    /// The renderer owns this scope; public host passes cannot opt out of evidence invalidation.
    pub(crate) fn push_family_postprocessor_preserving_prepared_math<P>(
        &mut self,
        family: crate::DiagramFamilyId,
        postprocessor: P,
    ) where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(SvgPostprocessorEntry {
            processor: Arc::new(postprocessor),
            family: Some(family),
            preserves_prepared_math: true,
            preserves_palette_slots: true,
        });
    }

    fn push_css_postprocessor<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(SvgPostprocessorEntry {
            processor: Arc::new(postprocessor),
            family: None,
            preserves_prepared_math: false,
            preserves_palette_slots: true,
        });
    }

    /// Whether the pipeline injects host CSS after text layout has completed.
    pub fn has_scoped_css(&self) -> bool {
        self.postprocessors.iter().any(|entry| {
            entry
                .processor
                .scoped_css()
                .is_some_and(|css| !css.trim().is_empty())
        })
    }

    pub fn process<'a>(&self, svg: &'a str, session: &RenderSession) -> Result<Cow<'a, str>> {
        let execution = SvgPostprocessExecution::new(session);
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(svg, execution)?;
        self.process_with_metadata(svg, &metadata, session)
    }

    pub fn process_with_metadata<'a>(
        &self,
        svg: &'a str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<Cow<'a, str>> {
        self.process_cow_with_metadata(Cow::Borrowed(svg), metadata, session)
    }

    fn process_cow_with_metadata<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<Cow<'a, str>> {
        Ok(self
            .run(
                svg,
                metadata,
                session,
                None,
                PipelineTerminalValidation::Immediate,
            )?
            .svg)
    }

    fn is_identity_for(&self, metadata: &SvgPostprocessMetadata) -> bool {
        self.preset == SvgPipelinePreset::Parity
            && self.inline_contract.is_none()
            && !self.isolate_ids
            && self
                .postprocessors
                .iter()
                .all(|entry| !entry.applies_to(metadata.family_id()))
    }

    fn run<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
        terminal_validation: PipelineTerminalValidation,
    ) -> Result<PipelineRunResult<'a>> {
        let execution = SvgPostprocessExecution::new(session);
        execution.checkpoint()?;
        execution.preflight_svg_byte_count(svg.len())?;
        let mut current =
            crate::xml::strip_forbidden_xml_1_0_chars_cow_with_checkpoints(svg, || {
                execution.checkpoint()
            })?;
        execution.preflight_svg_byte_count(current.len())?;
        if self.is_identity_for(metadata) {
            // Raw SVG enters through admission once; generated identity output bypasses run.
            final_validation::check_svg_resource_budget_with_execution(
                current.as_ref(),
                execution,
            )?;
            execution.checkpoint()?;
            return Ok(PipelineRunResult {
                svg: current,
                terminal: None,
            });
        }

        let mut structure = match final_validation::validate_well_formed_svg_with_execution(
            current.as_ref(),
            execution,
        ) {
            Ok(structure) => structure,
            Err(initial_error) if self.preset == SvgPipelinePreset::ResvgSafe => {
                // Browser-facing SVG may contain HTML named entities in serialized attribute
                // values (for example `&colon;`). They are not XML entities, so run the same
                // resource-aware attribute sanitizer once before retrying structural validation.
                // Text/entity errors remain rejected because this pass never rewrites text nodes.
                let sanitized =
                    merman_render_core::svg_safety::sanitize_element_attributes_cow_with_checkpoints(
                        current.clone(),
                        &mut || execution.checkpoint(),
                    )?;
                execution.preflight_svg_byte_count(sanitized.len())?;
                if sanitized.as_ref() == current.as_ref() {
                    return Err(initial_error);
                }
                current = sanitized;
                final_validation::validate_well_formed_svg_with_execution(
                    current.as_ref(),
                    execution,
                )?
            }
            Err(error) => return Err(error),
        };
        if self.inline_contract.is_some() {
            // Materialize the private prepared-math payload before inline publication and
            // admission. Browser XHTML supplies occurrence placement only; native geometry comes
            // from the renderer-owned evidence lease and never from browser markup.
            execution.checkpoint()?;
            let (projected, changed) = preset::BuiltinSvgStage::PreparedMathProjection.apply(
                current,
                metadata,
                execution,
                structure,
                prepared_math_evidence,
            )?;
            current = projected;
            execution.checkpoint()?;
            execution.preflight_svg_byte_count(current.len())?;
            if changed {
                structure = final_validation::validate_well_formed_svg_with_execution(
                    current.as_ref(),
                    execution,
                )?;
            }
        }
        if self.inline_contract == Some(InlineContract::Static) {
            current = step_animation::project_static(current, execution)?;
        }
        for stage in preset::builtin_stages_before_postprocessors(self.preset) {
            execution.checkpoint()?;
            let (output, _) = stage.apply(
                current,
                metadata,
                execution,
                structure,
                prepared_math_evidence,
            )?;
            current = output;
            execution.checkpoint()?;
            execution.preflight_svg_byte_count(current.len())?;
        }
        if self.isolate_ids {
            current = embedding::normalize_renderer_ids(current, metadata, execution)?;
        }
        if self.inline_contract.is_some() {
            static_validation::validate_rustdoc_admission_svg(current.as_ref(), execution)?;
        }

        let mut opaque_output = false;
        for (index, entry) in self.postprocessors.iter().enumerate() {
            execution.checkpoint()?;
            if !entry.applies_to(metadata.family_id()) {
                continue;
            }
            opaque_output = true;
            let postprocessor = &entry.processor;
            let ctx = SvgPostprocessContext::new(
                self.preset,
                index,
                postprocessor.name(),
                metadata,
                session,
            );
            let result = postprocessor.process(current, &ctx);
            execution.checkpoint()?;
            current = match result {
                Ok(current) => current,
                Err(Error::Cancelled(error)) => return Err(Error::Cancelled(error)),
                Err(Error::ResourceLimitExceeded(error)) => {
                    return Err(execution.terminate_resource_error(error));
                }
                Err(error) => {
                    return Err(Error::svg_postprocess(
                        postprocessor.name(),
                        error.to_string(),
                    ));
                }
            };
            execution.preflight_svg_byte_count(current.len())?;
        }

        if opaque_output
            && (self.preset != SvgPipelinePreset::Parity || self.inline_contract.is_some())
        {
            current =
                crate::xml::strip_forbidden_xml_1_0_chars_cow_with_checkpoints(current, || {
                    execution.checkpoint()
                })?;
            structure = final_validation::validate_well_formed_svg_with_execution(
                current.as_ref(),
                execution,
            )?;
        }
        execution.checkpoint()?;
        let (finalized, _) = preset::apply_preset_cow(
            self.preset,
            current,
            metadata,
            execution,
            structure,
            self.drop_native_duplicate_fallbacks,
            prepared_math_evidence,
        )?;
        let finalized =
            crate::xml::strip_forbidden_xml_1_0_chars_cow_with_checkpoints(finalized, || {
                execution.checkpoint()
            })?;
        execution.preflight_svg_byte_count(finalized.len())?;
        match self.inline_contract {
            Some(InlineContract::Static) => {
                static_validation::validate_rustdoc_static_svg(finalized.as_ref(), execution)?;
            }
            Some(InlineContract::Browser) => {
                static_validation::validate_rustdoc_admission_svg(finalized.as_ref(), execution)?;
            }
            None => {}
        }
        let terminal = if self.preset == SvgPipelinePreset::ResvgSafe
            && terminal_validation == PipelineTerminalValidation::Immediate
        {
            if let Some(evidence) = prepared_math_evidence.filter(|evidence| !evidence.is_empty()) {
                PreparedMathTerminalReceipt::from_terminal_svg(finalized.as_ref(), evidence)
                    .map_err(|message| {
                        Error::svg_postprocess("prepared-math-terminal-receipt", message)
                    })?
                    .ok_or_else(|| {
                        Error::svg_postprocess(
                            "prepared-math-terminal-receipt",
                            "prepared-math evidence did not produce a terminal receipt",
                        )
                    })?;
            }
            Some(
                final_validation::validate_resvg_compatible_svg_with_execution(
                    finalized.as_ref(),
                    execution,
                )?,
            )
        } else {
            if self.preset != SvgPipelinePreset::ResvgSafe && self.inline_contract.is_none() {
                final_validation::check_svg_resource_budget_with_execution(
                    finalized.as_ref(),
                    execution,
                )?;
            }
            None
        };
        // The deferred Resvg path transfers both terminal validation and its trailing checkpoint
        // to `seal`, keeping math-receipt failures ahead of terminal validation failures.
        if self.preset != SvgPipelinePreset::ResvgSafe
            || terminal_validation == PipelineTerminalValidation::Immediate
        {
            execution.checkpoint()?;
        }
        Ok(PipelineRunResult {
            svg: finalized,
            terminal,
        })
    }

    pub fn process_to_string(&self, svg: &str, session: &RenderSession) -> Result<String> {
        Ok(self.process(svg, session)?.into_owned())
    }

    pub fn process_owned_to_string(&self, svg: String, session: &RenderSession) -> Result<String> {
        let execution = SvgPostprocessExecution::new(session);
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(&svg, execution)?;
        self.process_owned_to_string_with_metadata(svg, &metadata, session)
    }

    pub fn process_to_string_with_metadata(
        &self,
        svg: &str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<String> {
        Ok(self
            .process_with_metadata(svg, metadata, session)?
            .into_owned())
    }

    pub fn process_owned_to_string_with_metadata(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<String> {
        Ok(self
            .process_cow_with_metadata(Cow::Owned(svg), metadata, session)?
            .into_owned())
    }

    pub(crate) fn process_generated_with_metadata_and_math_evidence(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
    ) -> Result<String> {
        // An identity pipeline has no input-admission responsibility for our own output.
        // Opaque callbacks remain outside this producer-owned path even if they return equal bytes.
        if self.is_identity_for(metadata) {
            let execution = SvgPostprocessExecution::new(session);
            execution.checkpoint()?;
            execution.preflight_svg_byte_count(svg.len())?;
            return Ok(svg);
        }
        Ok(self
            .run(
                Cow::Owned(svg),
                metadata,
                session,
                prepared_math_evidence,
                PipelineTerminalValidation::Immediate,
            )?
            .svg
            .into_owned())
    }

    pub fn process_resvg_compatible(
        &self,
        svg: &str,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        let execution = SvgPostprocessExecution::new(session);
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(svg, execution)?;
        self.process_resvg_compatible_with_metadata(svg, &metadata, session)
    }

    pub fn process_resvg_compatible_with_metadata(
        &self,
        svg: &str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let result = self.run(
            Cow::Borrowed(svg),
            metadata,
            session,
            None,
            PipelineTerminalValidation::Immediate,
        )?;
        ResvgCompatibleSvg::seal(
            result.svg.into_owned(),
            result.terminal,
            PreparedMathEvidenceLease::default(),
            true,
            self,
            session,
        )
    }

    pub fn process_owned_resvg_compatible_with_metadata(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let result = self.run(
            Cow::Owned(svg),
            metadata,
            session,
            None,
            PipelineTerminalValidation::Immediate,
        )?;
        ResvgCompatibleSvg::seal(
            result.svg.into_owned(),
            result.terminal,
            PreparedMathEvidenceLease::default(),
            true,
            self,
            session,
        )
    }

    pub(crate) fn process_owned_resvg_compatible_with_metadata_and_evidence(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,

        prepared_math_evidence: PreparedMathEvidenceLease,
        prepared_math_evidence_valid: bool,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let result = self.run(
            Cow::Owned(svg),
            metadata,
            session,
            prepared_math_evidence_valid.then_some(&prepared_math_evidence),
            // The seal preserves math-receipt errors before terminal-resource errors.
            PipelineTerminalValidation::Deferred,
        )?;
        ResvgCompatibleSvg::seal(
            result.svg.into_owned(),
            result.terminal,
            prepared_math_evidence,
            prepared_math_evidence_valid,
            self,
            session,
        )
    }

    fn ensure_resvg_safe_contract(&self) -> Result<()> {
        if self.preset != SvgPipelinePreset::ResvgSafe {
            return Err(Error::svg_postprocess(
                "resvg-finalize",
                "ResvgCompatibleSvg requires the resvg-safe terminal preset",
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "native-compact")]
pub(super) fn validate_native_compact_svg(svg: &str, session: &RenderSession) -> Result<()> {
    final_validation::validate_well_formed_svg_with_execution(
        svg,
        SvgPostprocessExecution::new(session),
    )
    .map(|_| ())
}

/// Finalizes arbitrary SVG for resvg/raster consumption without a family capability.
pub fn finalize_resvg_svg(svg: &str, session: &RenderSession) -> Result<ResvgCompatibleSvg> {
    SvgPipeline::resvg_safe().process_resvg_compatible(svg, session)
}

#[cfg(test)]
mod tests;
