mod builtin;
mod context;
mod embedding;
mod final_validation;
mod policy;
mod prepared_text;
mod preset;
mod resource_closure;
mod standalone;
mod static_validation;
mod text_tracking;

pub(crate) use builtin::set_root_background_color;
pub(crate) use builtin::util::{
    SvgTagScanner, checkpoint_loop, end_tag_name, escape_xml_attr_with_checkpoints,
    escape_xml_text_with_checkpoints, extract_exact_double_quoted_attr_with_checkpoints,
    find_tag_end_with_checkpoints, find_with_checkpoints, rfind_with_checkpoints, start_tag_name,
    trim_with_checkpoints,
};
pub use builtin::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor,
};
pub(crate) use builtin::{GitGraphBranchLabelBaselinePostprocessor, RebaseSvgIdsPostprocessor};
pub(crate) use context::SvgPostprocessExecution;
pub use context::{SvgPostprocessContext, SvgPostprocessMetadata};
pub(crate) use final_validation::{SvgStructureMetrics, validate_well_formed_svg_with_controls};
pub use policy::SvgOutputPolicy;
pub(crate) use prepared_text::partition_prepared_text_label_ids;
pub use preset::SvgPipelinePreset;
pub use resource_closure::{SvgResourceClosure, SvgResourceFingerprint};
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
use crate::text::{PreparedTextEvidenceLease, PreparedTextTerminalReceipt};
use crate::{Error, Result};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

pub trait SvgPostprocessor: Send + Sync {
    fn name(&self) -> &'static str;

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
    prepared_text_svg: Option<Arc<str>>,
    prepared_text_evidence: PreparedTextEvidenceLease,
    prepared_text_terminal_receipt: Option<PreparedTextTerminalReceipt>,
    prepared_text_evidence_valid: bool,
    prepared_math_evidence: PreparedMathEvidenceLease,
    prepared_math_terminal_receipt: Option<PreparedMathTerminalReceipt>,
    prepared_math_evidence_valid: bool,
    finalization_report: SvgFinalizationReport,
    font_catalog: crate::diagram_theme::FontCatalog,
    font_source_policy: crate::diagram_theme::FontSourcePolicy,
    resource_fingerprint: SvgResourceFingerprint,
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

    /// Returns the number of character-bearing terminal SVG `<text>` elements considered by font
    /// admission. Empty Mermaid parity nodes do not select a font face and are excluded.
    pub const fn text_element_count(&self) -> usize {
        self.text_element_count
    }

    pub(crate) const fn font_seal(&self) -> &SvgFontSeal {
        &self.font_seal
    }

    fn from_pipeline(
        pipeline: &SvgPipeline,
        terminal: &final_validation::TerminalSvgValidation,
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
            reference_plan: terminal.reference_plan.clone(),
            resource_closure: terminal.resource_closure.clone(),
            text_element_count: terminal.text_elements,
            font_seal: terminal.font_seal.clone(),
        }
    }
}

impl ResvgCompatibleSvg {
    fn seal(
        mut svg: String,
        terminal: Option<final_validation::TerminalSvgValidation>,
        prepared_text_evidence: PreparedTextEvidenceLease,
        prepared_text_evidence_valid: bool,
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
        // Math evidence errors precede terminal resource errors. Partitioning retains this exact
        // string as the native projection, so the receipt remains bound to the sealed bytes.
        let prepared_math_terminal_receipt = if prepared_math_evidence.is_empty() {
            None
        } else {
            PreparedMathTerminalReceipt::from_terminal_svg(&svg, &prepared_math_evidence).map_err(
                |message| Error::svg_postprocess("prepared-math-terminal-receipt", message),
            )?
        };
        let terminal = match terminal {
            Some(terminal) => terminal,
            None => {
                final_validation::validate_resvg_compatible_svg_with_execution(&svg, execution)?
            }
        };
        execution.checkpoint()?;
        let prepared_text_evidence = if prepared_text_evidence_valid {
            prepared_text_evidence
        } else {
            PreparedTextEvidenceLease::default()
        };
        // Always scan the reserved namespace, including an empty ledger. Otherwise a custom
        // postprocessor could introduce an unowned prepared-text token after the regular stages.
        let (public_svg, tokenized_svg) =
            partition_prepared_text_label_ids(svg, prepared_text_evidence.entries(), execution)?;
        svg = public_svg;
        let prepared_text_svg = if prepared_text_evidence.is_empty() {
            None
        } else {
            Some(tokenized_svg.map(Arc::from).ok_or_else(|| {
                Error::svg_postprocess(
                    "prepared-text-terminal-receipt",
                    "prepared-text evidence did not produce a native projection",
                )
            })?)
        };
        let prepared_text_terminal_receipt = prepared_text_svg
            .as_deref()
            .map(|tokenized_svg| {
                PreparedTextTerminalReceipt::from_ledger(
                    tokenized_svg,
                    prepared_text_evidence.entries(),
                )
                .ok_or_else(|| {
                    Error::svg_postprocess(
                        "prepared-text-terminal-receipt",
                        "prepared-text terminal receipt could not be frozen from the sealed artifact",
                    )
                })
            })
            .transpose()?;
        let native_svg = prepared_text_svg.as_deref().unwrap_or(svg.as_str());
        // The native projection is the original postprocessed SVG. Prepared-text partitioning
        // only strips renderer-owned IDs from the public projection, so the terminal validation
        // already attached to these bytes remains authoritative and must not be rerun.
        let font_catalog = session.font_catalog().clone();
        let font_source_policy = session.font_source_policy().clone();
        let resource_fingerprint = resource_closure::fingerprint_svg_resources(
            native_svg,
            font_catalog.fingerprint().as_bytes(),
            &font_source_policy,
        );
        Ok(Self {
            svg,
            prepared_text_svg,
            prepared_text_evidence,
            prepared_text_terminal_receipt,
            prepared_text_evidence_valid,
            prepared_math_evidence,
            prepared_math_terminal_receipt,
            prepared_math_evidence_valid,
            finalization_report: SvgFinalizationReport::from_pipeline(pipeline, &terminal),
            font_catalog,
            font_source_policy,
            resource_fingerprint,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.svg
    }

    pub(crate) fn native_export_svg(&self) -> &str {
        self.prepared_text_svg
            .as_deref()
            .unwrap_or(self.svg.as_str())
    }

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> &[crate::text::PreparedTextLabelLedgerEntry] {
        self.prepared_text_evidence.entries()
    }

    pub(crate) const fn prepared_text_terminal_receipt(
        &self,
    ) -> Option<&PreparedTextTerminalReceipt> {
        self.prepared_text_terminal_receipt.as_ref()
    }

    pub(crate) const fn prepared_text_evidence_valid(&self) -> bool {
        self.prepared_text_evidence_valid
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

    /// Returns the exact font catalog authorized by the render session.
    pub const fn font_catalog(&self) -> &crate::diagram_theme::FontCatalog {
        &self.font_catalog
    }

    /// Returns the host-authorized source priority retained for native font resolution.
    pub const fn font_source_policy(&self) -> &crate::diagram_theme::FontSourcePolicy {
        &self.font_source_policy
    }

    /// Returns a stable identity for the finalized SVG, retained catalog, and font-source policy.
    pub const fn resource_fingerprint(&self) -> SvgResourceFingerprint {
        self.resource_fingerprint
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
                "prepared_text_label_count",
                &self.prepared_text_evidence.entries().len(),
            )
            .field(
                "prepared_text_evidence_valid",
                &self.prepared_text_evidence_valid,
            )
            .field(
                "prepared_text_terminal_receipt",
                &self.prepared_text_terminal_receipt,
            )
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
            .field("resource_fingerprint", &self.resource_fingerprint)
            .finish()
    }
}

impl PartialEq for ResvgCompatibleSvg {
    fn eq(&self, other: &Self) -> bool {
        self.svg == other.svg
            && self.prepared_text_svg == other.prepared_text_svg
            && self.prepared_text_evidence == other.prepared_text_evidence
            && self.prepared_text_terminal_receipt == other.prepared_text_terminal_receipt
            && self.prepared_text_evidence_valid == other.prepared_text_evidence_valid
            && self.prepared_math_evidence == other.prepared_math_evidence
            && self.prepared_math_terminal_receipt == other.prepared_math_terminal_receipt
            && self.prepared_math_evidence_valid == other.prepared_math_evidence_valid
            && self.finalization_report == other.finalization_report
            && self.font_catalog.fingerprint() == other.font_catalog.fingerprint()
            && self.font_source_policy == other.font_source_policy
            && self.resource_fingerprint == other.resource_fingerprint
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
}

impl SvgPostprocessorEntry {
    fn untrusted(processor: Arc<dyn SvgPostprocessor>) -> Self {
        Self {
            processor,
            family: None,
            preserves_prepared_math: false,
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

#[derive(Clone)]
pub struct SvgPipeline {
    preset: SvgPipelinePreset,
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
            postprocessors: Vec::new(),
            drop_native_duplicate_fallbacks: false,
            inline_contract: None,
            isolate_ids: false,
        }
    }

    /// Returns whether the configured draft pipeline has no untrusted transformation that could
    /// rewrite typed root or family output after the renderer emitted it.
    ///
    /// The terminal preset itself is renderer-owned and does not invalidate root evidence. A
    /// custom postprocessor is conservatively treated as a mutation boundary until it provides a
    /// dedicated preservation contract.
    pub(crate) fn preserves_typed_theme_evidence(
        &self,
        family: Option<crate::DiagramFamilyId>,
    ) -> bool {
        !self
            .postprocessors
            .iter()
            .any(|entry| entry.applies_to(family))
    }

    /// Returns whether the pipeline can preserve renderer-owned prepared-label locators.
    pub(crate) fn preserves_prepared_text_evidence(
        &self,
        family: Option<crate::DiagramFamilyId>,
    ) -> bool {
        !self
            .postprocessors
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
    /// occurrence markers or native projection bytes. This is intentionally crate-private: opaque
    /// host postprocessors remain fail-closed through the public registration methods above.
    pub(crate) fn push_postprocessor_preserving_prepared_math<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(SvgPostprocessorEntry {
            processor: Arc::new(postprocessor),
            family: None,
            preserves_prepared_math: true,
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
        });
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
        self.process_cow_with_metadata_and_math_evidence(svg, metadata, session, None)
    }

    fn process_cow_with_metadata_and_math_evidence<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
    ) -> Result<Cow<'a, str>> {
        Ok(self
            .process_cow_with_reference_plan(
                svg,
                metadata,
                session,
                prepared_math_evidence,
                true,
                true,
            )?
            .0)
    }

    fn process_cow_with_reference_plan<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
        validate_resvg_terminal: bool,
        validate_prepared_math_receipt: bool,
    ) -> Result<(
        Cow<'a, str>,
        Option<final_validation::TerminalSvgValidation>,
    )> {
        let execution = SvgPostprocessExecution::new(session);
        execution.checkpoint()?;
        execution.preflight_svg_byte_count(svg.len())?;
        let mut current =
            crate::xml::strip_forbidden_xml_1_0_chars_cow_with_checkpoints(svg, || {
                execution.checkpoint()
            })?;
        execution.preflight_svg_byte_count(current.len())?;
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
                    builtin::attr_sanitize::sanitize_element_attributes_cow_with_checkpoints(
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
            // Prepared math is emitted as a renderer-owned `<template>` projection inside a
            // browser fallback. It must be materialized before inline publication and
            // admission runs; otherwise the internal evidence marker is rejected as unsupported
            // XHTML before the renderer-owned projection can consume it. The projection is
            // bounded by an opaque renderer evidence fingerprint, so untrusted or forged native
            // geometry still fails closed before the general admission pass.
            execution.checkpoint()?;
            current = preset::BuiltinSvgStage::PreparedMathProjection.apply(
                current,
                metadata,
                execution,
                structure,
                prepared_math_evidence,
            )?;
            execution.checkpoint()?;
            execution.preflight_svg_byte_count(current.len())?;
            structure = final_validation::validate_well_formed_svg_with_execution(
                current.as_ref(),
                execution,
            )?;
        }
        if self.isolate_ids {
            current = embedding::normalize_renderer_ids(current, metadata, execution)?;
            structure = final_validation::validate_well_formed_svg_with_execution(
                current.as_ref(),
                execution,
            )?;
        }
        if self.inline_contract.is_some() {
            static_validation::validate_rustdoc_admission_svg(current.as_ref(), execution)?;
        }

        for (index, entry) in self.postprocessors.iter().enumerate() {
            execution.checkpoint()?;
            if !entry.applies_to(metadata.family_id()) {
                continue;
            }
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
            structure = final_validation::validate_well_formed_svg_with_execution(
                current.as_ref(),
                execution,
            )?;
        }

        execution.checkpoint()?;
        let finalized = preset::apply_preset_cow(
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
        let terminal = if self.preset == SvgPipelinePreset::ResvgSafe && validate_resvg_terminal {
            if validate_prepared_math_receipt
                && let Some(evidence) =
                    prepared_math_evidence.filter(|evidence| !evidence.is_empty())
            {
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
            if self.preset != SvgPipelinePreset::ResvgSafe {
                final_validation::validate_well_formed_svg_with_execution(
                    finalized.as_ref(),
                    execution,
                )?;
            }
            None
        };
        // The deferred Resvg path transfers both terminal validation and its trailing checkpoint
        // to `seal`, keeping math-receipt failures ahead of terminal validation failures.
        if self.preset != SvgPipelinePreset::ResvgSafe || validate_resvg_terminal {
            execution.checkpoint()?;
        }
        Ok((finalized, terminal))
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

    pub(crate) fn process_owned_to_string_with_metadata_and_math_evidence(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
        prepared_math_evidence: Option<&PreparedMathEvidenceLease>,
    ) -> Result<String> {
        Ok(self
            .process_cow_with_metadata_and_math_evidence(
                Cow::Owned(svg),
                metadata,
                session,
                prepared_math_evidence,
            )?
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
        let (svg, terminal) = self.process_cow_with_reference_plan(
            Cow::Borrowed(svg),
            metadata,
            session,
            None,
            true,
            false,
        )?;
        ResvgCompatibleSvg::seal(
            svg.into_owned(),
            terminal,
            PreparedTextEvidenceLease::default(),
            true,
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
        let (svg, terminal) = self.process_cow_with_reference_plan(
            Cow::Owned(svg),
            metadata,
            session,
            None,
            true,
            false,
        )?;
        ResvgCompatibleSvg::seal(
            svg.into_owned(),
            terminal,
            PreparedTextEvidenceLease::default(),
            true,
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
        prepared_text_evidence: PreparedTextEvidenceLease,
        prepared_text_evidence_valid: bool,
        prepared_math_evidence: PreparedMathEvidenceLease,
        prepared_math_evidence_valid: bool,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let (svg, terminal) = self.process_cow_with_reference_plan(
            Cow::Owned(svg),
            metadata,
            session,
            prepared_math_evidence_valid.then_some(&prepared_math_evidence),
            // The seal preserves math-receipt, terminal-resource, then text-partition ordering.
            false,
            false,
        )?;
        ResvgCompatibleSvg::seal(
            svg.into_owned(),
            terminal,
            prepared_text_evidence,
            prepared_text_evidence_valid,
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

/// Finalizes arbitrary SVG for resvg/raster consumption without a family capability.
pub fn finalize_resvg_svg(svg: &str, session: &RenderSession) -> Result<ResvgCompatibleSvg> {
    SvgPipeline::resvg_safe().process_resvg_compatible(svg, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    fn render_session() -> RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn general_resvg_seal_fingerprints_final_native_svg_once() {
        let session = render_session();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1"/></svg>"#;
        let (sealed, work) = resource_closure::measure_resource_fingerprint_work(|| {
            SvgPipeline::resvg_safe()
                .process_resvg_compatible(svg, &session)
                .unwrap()
        });

        assert_eq!(sealed.native_export_svg(), svg);
        assert_eq!(work.calls, 1);
        assert_eq!(work.svg_bytes, sealed.native_export_svg().len());
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn family_resvg_seal_with_empty_ledger_fingerprints_final_native_svg_once() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA --> B\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let rendered =
            crate::family::prepare(parsed, &crate::LayoutOptions::default(), render_session())
                .unwrap()
                .render_svg(
                    &crate::svg::SvgRenderOptions::default(),
                    &crate::svg::SvgDebugOptions::default(),
                )
                .unwrap();
        let (sealed, work) = resource_closure::measure_resource_fingerprint_work(|| {
            rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap()
        });

        assert!(sealed.svg().prepared_text_label_ledger().is_empty());
        assert_eq!(work.calls, 1);
        assert_eq!(work.svg_bytes, sealed.svg().native_export_svg().len());
    }

    #[test]
    fn prepared_text_resvg_seal_fingerprints_final_native_svg_once() {
        // Exercise the internal projection boundary directly: ordinary family rendering currently
        // uses the asset-free font catalog and does not produce this non-empty prepared ledger.
        let session = render_session();
        let id =
            crate::text::PreparedTextLabelId::new(crate::text::PreparedTextLabelFamily::State, 0);
        let evidence = PreparedTextEvidenceLease::new(
            vec![crate::text::PreparedTextLabelLedgerEntry::for_test(
                id, "label",
            )],
            Vec::new(),
        );
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}">label</text></svg>"#,
            id.as_svg_id(),
        );
        let metadata =
            SvgPostprocessMetadata::from_svg(&svg).with_family_id(crate::DiagramFamilyId::STATE);
        let (sealed, work) = resource_closure::measure_resource_fingerprint_work(|| {
            SvgPipeline::resvg_safe()
                .process_owned_resvg_compatible_with_metadata_and_evidence(
                    svg.clone(),
                    &metadata,
                    &session,
                    evidence,
                    true,
                    PreparedMathEvidenceLease::default(),
                    true,
                )
                .unwrap()
        });

        assert_eq!(sealed.native_export_svg(), svg);
        assert_ne!(sealed.as_str(), sealed.native_export_svg());
        assert_eq!(sealed.prepared_text_label_ledger().len(), 1);
        assert!(
            sealed
                .prepared_text_terminal_receipt()
                .unwrap()
                .artifact_matches(sealed.native_export_svg())
        );
        assert_eq!(work.calls, 1);
        assert_eq!(work.svg_bytes, sealed.native_export_svg().len());

        // Verification hashes run outside the observation scope, so they cannot inflate the count.
        let fingerprint = resource_closure::fingerprint_svg_resources(
            sealed.native_export_svg(),
            session.font_catalog().fingerprint().as_bytes(),
            session.font_source_policy(),
        );
        assert_eq!(sealed.resource_fingerprint(), fingerprint);
        assert_ne!(
            sealed.resource_fingerprint(),
            resource_closure::fingerprint_svg_resources(
                sealed.as_str(),
                session.font_catalog().fingerprint().as_bytes(),
                session.font_source_policy(),
            ),
        );
    }

    #[test]
    fn prepared_text_seal_reuses_supplied_terminal_validation() {
        let session = render_session();
        let pipeline = SvgPipeline::resvg_safe();
        let id =
            crate::text::PreparedTextLabelId::new(crate::text::PreparedTextLabelFamily::State, 0);
        let evidence = PreparedTextEvidenceLease::new(
            vec![crate::text::PreparedTextLabelLedgerEntry::for_test(
                id, "label",
            )],
            Vec::new(),
        );
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="{}">label</text></svg>"#,
            id.as_svg_id()
        );
        let metadata =
            SvgPostprocessMetadata::from_svg(&svg).with_family_id(crate::DiagramFamilyId::STATE);

        let (sealed, work) = final_validation::measure_terminal_validation_work(|| {
            let (svg, terminal) = pipeline
                .process_cow_with_reference_plan(
                    Cow::Owned(svg),
                    &metadata,
                    &session,
                    None,
                    true,
                    false,
                )
                .unwrap();
            ResvgCompatibleSvg::seal(
                svg.into_owned(),
                terminal,
                evidence,
                true,
                PreparedMathEvidenceLease::default(),
                true,
                &pipeline,
                &session,
            )
            .unwrap()
        });

        assert_eq!(sealed.prepared_text_label_ledger().len(), 1);
        assert_eq!(sealed.native_export_svg().len(), work.svg_bytes);
        assert_eq!(
            sealed.reference_plan(),
            sealed.finalization_report().reference_plan()
        );
        assert_eq!(
            sealed.resource_closure(),
            sealed.finalization_report().resource_closure()
        );
        assert_eq!(work.calls, 1);
    }

    #[test]
    fn strict_parity_artifact_is_compatible_when_its_bytes_satisfy_the_terminal_contract() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(
                crate::diagram_theme::ThemePortabilityRequirement::RequirePortable,
            )
            .begin_session()
            .unwrap();
        let artifact = StandaloneSvgArtifact::finalize_exact(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><path d="M0 0h1v1z"/></svg>"#.to_owned(),
            None,
            PreparedTextEvidenceLease::default(),
            true,
            &SvgPipeline::parity(),
            &session,
        )
        .unwrap();

        assert_eq!(
            artifact.terminal_status(),
            StandaloneSvgTerminalStatus::Compatible
        );
        assert_eq!(artifact.selected_pipeline(), SvgPipelinePreset::Parity);
        assert!(artifact.finalization_report().is_some());
        assert!(artifact.text_fonts_are_self_contained());
    }

    #[test]
    fn parity_pipeline_preserves_svg_exactly() {
        let svg = r#"<svg><style>@keyframes a{to{opacity:1}}</style><rect width="10"/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity().process(svg, &session).unwrap();
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, svg);
    }

    #[test]
    fn browser_inline_contract_preserves_html_labels_and_rebases_references() {
        let svg = r##"<svg id="root" xmlns="http://www.w3.org/2000/svg" aria-labelledby="title"><title id="title">Diagram</title><defs><marker id="arrow"/></defs><style>@keyframes dash{to{stroke-dashoffset:0}}#root .edge{animation:dash 20s linear infinite}#root #label{color:red}</style><path class="edge" marker-end="url(#arrow)"/><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml" id="label" style="transform:rotate(5deg)"><strong>Safe label</strong></div></foreignObject></svg>"##;
        let output = SvgPipeline::parity()
            .with_browser_inline_contract("embed")
            .process_to_string(svg, &render_session())
            .unwrap();

        assert!(output.contains("<foreignObject"), "{output}");
        assert!(output.contains("<strong>Safe label</strong>"), "{output}");
        assert!(output.contains("transform:rotate(5deg)"), "{output}");
        assert!(output.contains("animation:dash 20s"), "{output}");
        assert!(output.contains(r#"id="embed-root""#), "{output}");
        assert!(
            output.contains(r#"aria-labelledby="embed-title""#),
            "{output}"
        );
        assert!(output.contains("url(#embed-arrow)"), "{output}");
        assert!(output.contains("#embed-root #embed-label"), "{output}");
    }

    #[test]
    fn browser_inline_contract_accepts_renderer_owned_family_styles() {
        for (name, source) in [
            ("error", "error"),
            #[cfg(feature = "diagram-event-modeling")]
            ("eventmodeling", "eventmodeling\ntf 01 event Start"),
            #[cfg(feature = "diagram-ishikawa")]
            (
                "ishikawa",
                "ishikawa-beta\n    Problem\n        Cause\n            Detail",
            ),
            #[cfg(feature = "diagram-tree-view")]
            ("treeView", "treeView-beta\n    root/\n        file.txt"),
            #[cfg(feature = "diagram-zenuml")]
            ("zenuml", "zenuml\nAlice->Bob: Hello"),
            #[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
            ("mindmap", "mindmap\n  root\n    a\n    b"),
            #[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
            (
                "architecture",
                "architecture-beta\nservice worker \"<a href='https://example.test'>Docs</a>\" [Worker]",
            ),
        ] {
            let parsed = merman_core::Engine::new()
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .unwrap_or_else(|error| panic!("{name}: {error}"))
                .unwrap();
            let artifact =
                crate::family::prepare(parsed, &crate::LayoutOptions::default(), render_session())
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            let rendered = artifact
                .render_svg(
                    &crate::svg::SvgRenderOptions {
                        diagram_id: Some(name.to_string()),
                        ..Default::default()
                    },
                    &crate::svg::SvgDebugOptions::default(),
                )
                .unwrap_or_else(|error| panic!("{name}: {error}"))
                .apply_pipeline(&SvgPipeline::parity().with_browser_inline_contract("embed"))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(rendered.svg().contains(&format!("id=\"embed-{name}\"")));
        }
    }

    #[test]
    fn browser_inline_contract_rejects_external_html_and_escaped_css_resources() {
        for (svg, expected) in [
            (
                r#"<svg><foreignObject><img xmlns="http://www.w3.org/1999/xhtml" src="https://tracker.test/image.png"/></foreignObject></svg>"#,
                "forbidden <img>",
            ),
            (
                r#"<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml" style="background:u\72l(https://tracker.test/image.png)">Label</div></foreignObject></svg>"#,
                "non-local CSS URL",
            ),
            (
                r#"<svg id="root"><style>#root .label{background:u\72l('https://tracker.test/image.png')}</style></svg>"#,
                "non-local CSS URL",
            ),
        ] {
            let error = SvgPipeline::parity()
                .with_browser_inline_contract("embed")
                .process_to_string(svg, &render_session())
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{svg}: {error}");
        }
    }

    #[test]
    fn inline_contract_admits_before_any_lossy_postprocessor() {
        struct MustNotRun;
        impl SvgPostprocessor for MustNotRun {
            fn name(&self) -> &'static str {
                "must-not-run"
            }

            fn process<'a>(
                &self,
                _: Cow<'a, str>,
                _: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                panic!("unsafe SVG must be rejected before postprocessing")
            }
        }
        for pipeline in [
            SvgPipeline::parity()
                .with_postprocessor(MustNotRun)
                .with_browser_inline_contract("embed"),
            SvgPipeline::parity()
                .with_postprocessor(MustNotRun)
                .with_static_inline_contract("embed"),
        ] {
            let error = pipeline
                .process_to_string("<svg><script/></svg>", &render_session())
                .unwrap_err();
            assert!(error.to_string().contains("forbidden <script>"), "{error}");
        }
    }

    #[test]
    fn inline_contract_validates_after_all_custom_postprocessors() {
        struct InjectResource;
        impl SvgPostprocessor for InjectResource {
            fn name(&self) -> &'static str {
                "inject-resource"
            }

            fn process<'a>(
                &self,
                _: Cow<'a, str>,
                _: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                Ok(Cow::Borrowed(
                    r#"<svg><image href="https://tracker.test/image.png"/></svg>"#,
                ))
            }
        }
        for pipeline in [
            SvgPipeline::parity().with_browser_inline_contract("embed"),
            SvgPipeline::parity().with_static_inline_contract("embed"),
        ] {
            let error = pipeline
                .with_postprocessor(InjectResource)
                .process_to_string("<svg/>", &render_session())
                .unwrap_err();
            assert!(error.to_string().contains("non-local resource"), "{error}");
        }
    }

    #[test]
    fn static_inline_contract_still_converts_html_and_removes_animation() {
        let svg = r#"<svg id="root"><style>@keyframes dash{to{opacity:0}}#root .edge{animation:dash 1s}</style><foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml">Safe label</div></foreignObject></svg>"#;
        let output = SvgPipeline::parity()
            .with_static_inline_contract("embed")
            .process_to_string(svg, &render_session())
            .unwrap();

        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(!output.contains("@keyframes"), "{output}");
        assert!(!output.contains("animation:"), "{output}");
        assert!(output.contains("Safe label"), "{output}");
        assert!(output.contains(r#"id="embed-root""#), "{output}");
    }

    #[test]
    fn rebasing_without_inline_contract_preserves_external_resources() {
        let svg = r#"<svg id="root"><image href="https://example.test/image.png"/></svg>"#;
        let output = SvgPipeline::parity()
            .with_rebased_ids("embed")
            .process_to_string(svg, &render_session())
            .unwrap();

        assert!(output.contains(r#"id="embed-root""#), "{output}");
        assert!(
            output.contains("https://example.test/image.png"),
            "{output}"
        );
    }

    #[test]
    fn parity_pipeline_returns_owned_svg_without_reallocating() {
        let svg = String::from(r#"<svg><rect width="10"/></svg>"#);
        let allocation = svg.as_ptr();
        let session = render_session();

        let out = SvgPipeline::parity()
            .process_owned_to_string(svg, &session)
            .unwrap();

        assert_eq!(out.as_ptr(), allocation);
    }

    #[test]
    fn inline_pipelines_project_renderer_owned_math_before_admission() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
            r#"<path d="M0 0h1v1z"/></g>"#,
        );
        let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
            crate::DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
        );
        let evidence = PreparedMathEvidenceLease::new(
            vec![crate::math::PreparedMathExpectation::available(
                occurrence_id.clone(),
                crate::math::PreparedMathProjectionFingerprint::from_projection(projection),
                1,
            )],
            Vec::new(),
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<foreignObject width="10" height="11"><div xmlns="http://www.w3.org/1999/xhtml">"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">browser"#,
                r#"<template data-merman-prepared-math-projection="v1">{}</template>"#,
                r#"</span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );
        let session = render_session();
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_family_id(crate::DiagramFamilyId::FLOWCHART);

        for pipeline in [
            SvgPipeline::parity().with_static_inline_contract("static-math"),
            SvgPipeline::parity().with_browser_inline_contract("browser-math"),
        ] {
            let output = pipeline
                .process_cow_with_metadata_and_math_evidence(
                    Cow::Borrowed(&svg),
                    &metadata,
                    &session,
                    Some(&evidence),
                )
                .unwrap()
                .into_owned();

            assert!(!output.contains("<template"), "{output}");
            assert!(!output.contains("<foreignObject"), "{output}");
            assert!(output.contains(r#"class="merman-prepared-math-native""#));
            validate_static_inline_svg(&output, &session).unwrap();
        }
    }

    #[test]
    fn resvg_seal_preserves_math_receipt_error_before_resource_closure_error() {
        let projection = "<g><path d=\"M0 0h1\"/></g>";
        let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
            crate::DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
        );
        let evidence = PreparedMathEvidenceLease::new(
            vec![crate::math::PreparedMathExpectation::available(
                occurrence_id.clone(),
                crate::math::PreparedMathProjectionFingerprint::from_projection(projection),
                1,
            )],
            Vec::new(),
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<g class="merman-prepared-math-native" data-merman-prepared-math-occurrence="{}">"#,
                r#"<g><path d="M0 0h2"/></g></g>"#,
                r##"<path fill="url(#missing)"/></svg>"##,
            ),
            occurrence_id.as_str(),
        );
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_family_id(crate::DiagramFamilyId::FLOWCHART);
        let session = render_session();
        let error = SvgPipeline::resvg_safe()
            .process_owned_resvg_compatible_with_metadata_and_evidence(
                svg,
                &metadata,
                &session,
                PreparedTextEvidenceLease::default(),
                true,
                evidence,
                true,
            )
            .unwrap_err();

        assert!(
            matches!(
                error,
                Error::SvgPostprocess { ref pass, ref message }
                    if pass == "prepared-math-terminal-receipt"
                        && message.contains("mismatched projection")
            ),
            "{error}"
        );
    }

    #[test]
    fn inline_pipelines_reject_forged_math_projection_without_evidence() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="10" height="11"><div xmlns="http://www.w3.org/1999/xhtml">"#,
            r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
            r#"data-merman-prepared-math-occurrence="flowchart/node-label/0">browser"#,
            r#"<template data-merman-prepared-math-projection="v1">"#,
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
            r#"<path d="M0 0h1v1z"/></g></template></span></div></foreignObject></svg>"#,
        );
        let session = render_session();
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);

        for pipeline in [
            SvgPipeline::parity().with_static_inline_contract("static-math"),
            SvgPipeline::parity().with_browser_inline_contract("browser-math"),
        ] {
            let error = pipeline
                .process_with_metadata(svg, &metadata, &session)
                .unwrap_err();
            assert!(matches!(
                error,
                Error::SvgPostprocess { ref pass, ref message }
                    if pass == "prepared-math-projection"
                        && message.contains("renderer-owned occurrence evidence")
            ));
        }
    }

    #[test]
    fn static_inline_pipeline_rejects_math_evidence_without_an_occurrence_marker() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
            r#"<path d="M0 0h1v1z"/></g>"#,
        );
        let evidence = PreparedMathEvidenceLease::new(
            vec![crate::math::PreparedMathExpectation::available(
                crate::math::PreparedMathOccurrenceId::indexed(
                    crate::DiagramFamilyId::FLOWCHART,
                    "node-label",
                    0,
                ),
                crate::math::PreparedMathProjectionFingerprint::from_projection(projection),
                1,
            )],
            Vec::new(),
        );
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>"#;
        let session = render_session();
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(crate::DiagramFamilyId::FLOWCHART);

        let error = SvgPipeline::parity()
            .with_static_inline_contract("static-math")
            .process_cow_with_metadata_and_math_evidence(
                Cow::Borrowed(svg),
                &metadata,
                &session,
                Some(&evidence),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            Error::SvgPostprocess { ref pass, ref message }
                if pass == "prepared-math-projection"
                    && message.contains("no terminal occurrence marker")
        ));
    }

    #[test]
    fn every_pipeline_preset_enforces_the_xml_1_0_character_contract() {
        let svg = "<svg><text>A\u{0}B\u{1c}C\u{fffe}D</text></svg>";
        let session = render_session();

        for preset in [
            SvgPipelinePreset::Parity,
            SvgPipelinePreset::Readable,
            SvgPipelinePreset::ResvgSafe,
        ] {
            let out = SvgPipeline::from_preset(preset)
                .process_to_string(svg, &session)
                .unwrap();
            assert_eq!(out, "<svg><text>ABCD</text></svg>", "{preset:?}");
            roxmltree::Document::parse(&out).expect("pipeline output must remain XML 1.0");
        }
    }

    #[test]
    fn every_pipeline_preset_rejects_unknown_xml_entities() {
        let session = render_session();

        for preset in [
            SvgPipelinePreset::Parity,
            SvgPipelinePreset::Readable,
            SvgPipelinePreset::ResvgSafe,
        ] {
            let error = SvgPipeline::from_preset(preset)
                .process_to_string("<svg><text>&unknown;</text></svg>", &session)
                .unwrap_err();
            assert!(
                error.to_string().contains("invalid XML"),
                "{preset:?}: {error}"
            );
        }
    }

    #[test]
    fn readable_pipeline_matches_foreign_object_fallback() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(10,20)"><foreignObject width="80" height="48"><div xmlns="http://www.w3.org/1999/xhtml"><p>Layer 7\nHTTP</p></div></foreignObject></g></svg>"#;
        let session = render_session();
        let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Wrap);

        let expected = super::builtin::foreign_object::foreign_object_fallback_svg(svg, &measurer);
        let out = SvgPipeline::readable()
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(out, expected);
        assert!(out.contains(">Layer 7</text>"));
        assert!(out.contains(">HTTP</text>"));
    }

    #[test]
    fn resvg_safe_pipeline_strips_generic_raster_hazards() {
        let svg = r#"<svg id="test" xmlns="http://www.w3.org/2000/svg"><style type="text/css">@keyframes bounce { 0% { transform: scale(1); } 100% { transform: scale(1.1); } } #test :root { --bg: white; } .node rect { animation: dash 1s linear; transform: rotate(45deg); fill: red; }</style><g transform="translate(undefined,NaN)"><foreignObject width="10" height="10"><div xmlns="http://www.w3.org/1999/xhtml"><p>Hello</p></div></foreignObject><rect width="10px" height="12px" stroke="" style="fill: ; stroke: #333; transform: rotate(45deg); animation: dash 1s;"/><rect width="10px" height="" fill="hsl(240, 100%, NaN%)"/></g></svg>"#;
        let session = render_session();

        let out = SvgPipeline::resvg_safe()
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes"));
        assert!(!out.contains(":root"));
        assert!(!out.contains("animation"));
        assert!(!out.contains("deg"));
        assert!(!out.contains("NaN"));
        assert!(!out.contains("undefined"));
        assert!(!out.contains(r#"height="""#));
        assert!(!out.contains(r#"fill="hsl"#));
        assert!(!out.contains(r#"stroke="""#));
        assert!(out.contains(r#"width="10""#));
        assert!(out.contains(r#"height="12""#));
        assert!(out.contains("stroke:#333"));
        assert!(out.contains(">Hello</text>"));
    }

    #[test]
    fn resvg_safe_pipeline_sanitizes_url_attributes_after_malformed_text() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
"#shape"/>
  <path stroke="url(javascript:alert(1))" style="filterBurl('file://юmp/xter:');svroke:#033"/>
</svg>"##;
        let session = render_session();

        let out = SvgPipeline::resvg_safe()
            .process_to_string(svg, &session)
            .expect("resvg-safe pipeline should sanitize the fuzz regression input");

        assert!(!out.to_ascii_lowercase().contains("javascript"), "{out}");
        assert!(!out.contains(r#"stroke="url("#), "{out}");
    }

    #[test]
    fn resvg_safe_finalization_drops_unclosed_inline_css_blocks_idempotently() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="filter:5rl('file:///{animatiEtroke:#333"/></svg>"##;
        let session = render_session();

        let once = super::finalize_resvg_svg(svg, &session).unwrap();
        let twice = super::finalize_resvg_svg(once.as_str(), &session).unwrap();

        assert_eq!(twice, once);
        assert!(!once.as_str().contains("style="), "{}", once.as_str());
    }

    #[test]
    fn resvg_safe_pipeline_bounds_css_nesting_on_a_small_thread_stack() {
        const CSS_NESTING_LIMIT: usize = 64;

        fn nested_function(depth: usize, leaf: &str) -> String {
            format!("{}{}{}", "f(".repeat(depth), leaf, ")".repeat(depth))
        }

        fn nested_media(depth: usize, rule: &str) -> String {
            format!(
                "{}{}{}",
                "@media all{".repeat(depth),
                rule,
                "}".repeat(depth)
            )
        }

        std::thread::Builder::new()
            .name("bounded-css-nesting".into())
            .stack_size(512 * 1024)
            .spawn(|| {
                let style_exact = nested_function(CSS_NESTING_LIMIT - 1, "red");
                let style_over = nested_function(CSS_NESTING_LIMIT, "red");
                let inline_exact = nested_function(CSS_NESTING_LIMIT, "red");
                let selector_exact = format!(
                    "{}.selector-exact{}",
                    ":is(".repeat(CSS_NESTING_LIMIT),
                    ")".repeat(CSS_NESTING_LIMIT)
                );
                let selector_over = format!(
                    "{}.selector-over{}",
                    ":is(".repeat(CSS_NESTING_LIMIT + 1),
                    ")".repeat(CSS_NESTING_LIMIT + 1)
                );
                let media_exact = nested_media(CSS_NESTING_LIMIT - 1, ".media-exact{fill:red}");
                let media_over = nested_media(CSS_NESTING_LIMIT, ".media-over{fill:red}");
                let very_deep = nested_function(4_096, "red");
                let svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg"><style>
.style-exact{{fill:{style_exact}}}
.style-over{{fill:{style_over};stroke:blue}}
{selector_exact}{{fill:red}}
{selector_over}{{fill:red}}
{media_exact}
{media_over}
</style>
<path id="inline-exact" style="fill:{inline_exact};stroke:blue"/>
<path id="inline-over" style="fill:{very_deep};stroke:blue"/>
<path id="presentation-over" fill="{very_deep}" stroke="blue"/>
</svg>"#
                );

                let session = render_session();
                let out = SvgPipeline::resvg_safe()
                    .process_to_string(&svg, &session)
                    .expect("resvg-safe CSS sanitization must remain bounded");

                assert!(out.contains(".style-exact{fill:"), "{out}");
                assert!(out.contains(".style-over{stroke:blue}"), "{out}");
                assert!(out.contains(".selector-exact"), "{out}");
                assert!(out.contains("){fill:red}"), "{out}");
                assert!(!out.contains("selector-over"), "{out}");
                assert!(out.contains("media-exact"), "{out}");
                assert!(!out.contains("media-over"), "{out}");

                let document = roxmltree::Document::parse(&out).expect("valid sanitized SVG");
                let inline_exact = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("inline-exact"))
                    .unwrap();
                assert!(inline_exact.attribute("style").unwrap().contains("fill:"));
                let inline_over = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("inline-over"))
                    .unwrap();
                assert_eq!(inline_over.attribute("style"), Some("stroke:blue"));
                let presentation_over = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("presentation-over"))
                    .unwrap();
                assert_eq!(presentation_over.attribute("fill"), None);
                assert_eq!(presentation_over.attribute("stroke"), Some("blue"));
            })
            .expect("small-stack CSS regression thread must start")
            .join()
            .expect("bounded CSS traversal must not overflow the small thread stack");
    }

    struct AppendPass(&'static str);

    impl SvgPostprocessor for AppendPass {
        fn name(&self) -> &'static str {
            self.0
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Ok(Cow::Owned(format!(
                "{}<!--{}:{}:{:?}:{}:{}:{}-->",
                svg,
                ctx.pass_index(),
                ctx.pass_name(),
                ctx.preset(),
                ctx.diagram_type().unwrap_or("none"),
                ctx.diagram_title().unwrap_or("none"),
                ctx.svg_id().unwrap_or("none")
            )))
        }
    }

    struct CancelOperationPass(OperationControl);

    impl SvgPostprocessor for CancelOperationPass {
        fn name(&self) -> &'static str {
            "cancel-operation"
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            self.0.cancel();
            Ok(svg)
        }
    }

    #[test]
    fn pipeline_checks_control_after_an_opaque_postprocessor() {
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(control.clone())
            .unwrap();

        let error = SvgPipeline::parity()
            .with_postprocessor(CancelOperationPass(control))
            .process_to_string("<svg/>", &session)
            .unwrap_err();

        let Error::Cancelled(error) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(error.phase, OperationPhase::Postprocess);
        assert_eq!(error.reason, CancelReason::Requested);
    }

    #[test]
    fn custom_postprocessors_run_before_builtin_finalizer_in_order() {
        let svg = r#"<svg><foreignObject width="10" height="10"><div><p>Hello</p></div></foreignObject></svg>"#;
        let pipeline = SvgPipeline::readable()
            .with_postprocessor(AppendPass("first"))
            .with_postprocessor(AppendPass("second"));
        let session = render_session();

        let out = pipeline.process_to_string(svg, &session).unwrap();

        let first = out.find("<!--0:first:Readable").unwrap();
        let second = out.find("<!--1:second:Readable").unwrap();
        assert!(first < second);
        assert!(out.contains("data-merman-foreignobject"));
    }

    #[test]
    fn custom_postprocessor_output_is_cleaned_by_resvg_finalizer() {
        struct InjectActiveContent;

        impl SvgPostprocessor for InjectActiveContent {
            fn name(&self) -> &'static str {
                "inject-active-content"
            }

            fn process<'a>(
                &self,
                svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                Ok(Cow::Owned(svg.replace(
                    "</svg>",
                    r#"<script>alert(1)</script><rect animation="spin 1s"/></svg>"#,
                )))
            }
        }

        let session = render_session();
        let output = SvgPipeline::resvg_safe()
            .with_postprocessor(InjectActiveContent)
            .process_resvg_compatible("<svg></svg>", &session)
            .unwrap();

        assert!(!output.as_str().contains("script"));
        assert!(!output.as_str().contains("animation"));
    }

    #[test]
    fn resvg_seal_rejects_unowned_prepared_text_tokens() {
        let session = render_session();
        let error = SvgPipeline::resvg_safe()
            .process_resvg_compatible(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="merman-prepared-flowchart-0">label</text></svg>"#,
                &session,
            )
            .expect_err("an empty ledger must reject reserved prepared-text tokens");

        assert!(
            matches!(
                error,
                Error::SvgPostprocess { ref pass, ref message }
                    if pass == "prepared-text-token"
                        && message.contains("has no matching ledger entry")
            ),
            "{error}"
        );
    }

    #[test]
    fn expanded_draft_is_budgeted_before_terminal_xml_validation() {
        struct ExpandDraft;

        impl SvgPostprocessor for ExpandDraft {
            fn name(&self) -> &'static str {
                "expand-draft"
            }

            fn process<'a>(
                &self,
                _svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                Ok(Cow::Owned(format!("<svg>{}</svg>", "x".repeat(128))))
            }
        }

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 64)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        let error = SvgPipeline::resvg_safe()
            .with_postprocessor(ExpandDraft)
            .process_resvg_compatible("<svg/>", &session)
            .unwrap_err();

        assert!(error.to_string().contains("max_svg_bytes"), "{error}");
    }

    #[test]
    fn provided_metadata_admits_raw_svg_bytes_before_xml_cleanup() {
        let svg = "<svg>\u{0}</svg>";
        let maximum = svg.len() - 1;
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, maximum)
                    .unwrap(),
            )
            .begin_session_with_control(control.clone())
            .unwrap();
        let metadata = SvgPostprocessMetadata::new();

        let error = SvgPipeline::parity()
            .process_with_metadata(svg, &metadata, &session)
            .expect_err("raw SVG bytes must be admitted before XML cleanup allocates");
        let Error::ResourceLimitExceeded(first) = error else {
            panic!("expected a resource rejection");
        };
        assert_eq!(first.limit, "max_svg_bytes");
        assert_eq!(first.actual, svg.len());
        assert_eq!(first.max, maximum);

        control.cancel();
        let replayed = SvgPipeline::parity()
            .process_with_metadata("<svg/>", &metadata, &session)
            .expect_err("the first resource terminal must remain sticky");
        let Error::ResourceLimitExceeded(replayed) = replayed else {
            panic!("expected the resource rejection to replay");
        };
        assert_eq!(replayed, first);
    }

    #[test]
    fn structure_limit_rejects_before_custom_postprocessors_run() {
        struct MustNotRun;

        impl SvgPostprocessor for MustNotRun {
            fn name(&self) -> &'static str {
                "must-not-run"
            }

            fn process<'a>(
                &self,
                _svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> Result<Cow<'a, str>> {
                panic!("structure admission must precede custom postprocessing")
            }
        }

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxSvgElements, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        let error = SvgPipeline::parity()
            .with_postprocessor(MustNotRun)
            .process_to_string("<svg><g/></svg>", &session)
            .unwrap_err();

        assert!(error.to_string().contains("max_svg_elements"), "{error}");
    }

    #[test]
    fn fallback_generated_elements_are_rejected_before_the_overlay_is_completed() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxSvgElements, 4)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="10" height="10">"#,
            r#"<div xmlns="http://www.w3.org/1999/xhtml">Hello</div>"#,
            "</foreignObject></svg>",
        );
        let error = SvgPipeline::readable()
            .process_to_string(svg, &session)
            .unwrap_err();

        assert!(error.to_string().contains("max_svg_elements"), "{error}");
    }

    #[test]
    fn non_resvg_pipeline_cannot_construct_resvg_compatible_svg() {
        let session = render_session();
        let error = SvgPipeline::parity()
            .process_resvg_compatible("<svg/>", &session)
            .unwrap_err();

        assert!(error.to_string().contains("resvg-safe terminal preset"));
    }

    #[test]
    fn custom_postprocessor_context_exposes_metadata() {
        let svg = r#"<svg id="host-diagram"><rect width="10"/></svg>"#;
        let metadata = SvgPostprocessMetadata::from_svg(svg)
            .with_diagram_type("flowchart-v2")
            .with_diagram_title("Host Diagram");
        let pipeline = SvgPipeline::parity().with_postprocessor(AppendPass("meta"));
        let session = render_session();

        let out = pipeline
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();

        assert!(out.contains("<!--0:meta:Parity:flowchart-v2:Host Diagram:host-diagram-->"));
    }

    struct ErrorPass;

    impl SvgPostprocessor for ErrorPass {
        fn name(&self) -> &'static str {
            "error-pass"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            Err(Error::InvalidModel {
                message: "boom".to_string(),
            })
        }
    }

    #[test]
    fn custom_postprocessor_errors_surface_with_pass_name() {
        let session = render_session();
        let err = SvgPipeline::parity()
            .with_postprocessor(ErrorPass)
            .process_to_string("<svg/>", &session)
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("error-pass"));
        assert!(message.contains("boom"));
    }

    struct CancelAndErrorPass(OperationControl);

    impl SvgPostprocessor for CancelAndErrorPass {
        fn name(&self) -> &'static str {
            "cancel-and-error"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> Result<Cow<'a, str>> {
            self.0.cancel();
            Err(Error::InvalidModel {
                message: "opaque failure".to_string(),
            })
        }
    }

    #[test]
    fn cancellation_observed_after_a_failing_postprocessor_wins_over_wrapping() {
        let control = OperationControl::new();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_control(control.clone())
            .unwrap();

        let error = SvgPipeline::parity()
            .with_postprocessor(CancelAndErrorPass(control))
            .process_to_string("<svg/>", &session)
            .unwrap_err();

        let Error::Cancelled(error) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(error.phase, OperationPhase::Postprocess);
        assert_eq!(error.reason, CancelReason::Requested);
    }
}
