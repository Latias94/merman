mod builtin;
mod context;
mod final_validation;
mod font_embedding;
mod policy;
mod prepared_text;
mod preset;
mod resource_closure;

pub(crate) use builtin::GitGraphBranchLabelBaselinePostprocessor;
pub(crate) use builtin::set_root_background_color;
pub use builtin::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor,
};
pub use context::{SvgPostprocessContext, SvgPostprocessMetadata};
pub(crate) use final_validation::validate_well_formed_svg;
pub(crate) use font_embedding::SvgFontSeal;
pub use policy::SvgOutputPolicy;
pub(crate) use prepared_text::partition_prepared_text_label_ids;
pub use preset::SvgPipelinePreset;
pub use resource_closure::{SvgResourceClosure, SvgResourceFingerprint};

use crate::environment::RenderSession;
use crate::resources::ResourceLimitPhase;
use crate::text::{PreparedTextEvidenceLease, PreparedTextTerminalReceipt};
use crate::{Error, Result};
use merman_core::OperationPhase;
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
/// raster data URLs, `feImage` accepts either form, and renderer-owned full-font data URLs require
/// an exact prepared-text embedding plan.
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
    reference_plan: SvgReferencePlan,
    resource_closure: SvgResourceClosure,
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
/// Raw SVG drafts do not carry this report. It is created only after the resvg-safe pipeline has
/// completed XML, CSS, reference, and resource validation.
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

    /// Returns the number of terminal SVG `<text>` elements considered by font admission.
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
                .map(|postprocessor| postprocessor.name().to_string())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            drop_native_duplicate_fallbacks: pipeline.drop_native_duplicate_fallbacks,
            reference_plan: terminal.reference_plan.clone(),
            resource_closure: terminal.resource_closure.clone(),
            text_element_count: terminal.text_elements,
            font_seal: terminal.font_seal.clone(),
        }
    }

    fn refresh_terminal(&mut self, terminal: &final_validation::TerminalSvgValidation) {
        self.reference_plan = terminal.reference_plan.clone();
        self.resource_closure = terminal.resource_closure.clone();
        self.text_element_count = terminal.text_elements;
        self.font_seal = terminal.font_seal.clone();
    }
}

impl ResvgCompatibleSvg {
    fn finalized(
        svg: String,
        reference_plan: SvgReferencePlan,
        resource_closure: SvgResourceClosure,
        finalization_report: SvgFinalizationReport,
        session: &RenderSession,
    ) -> Self {
        let font_catalog = session.font_catalog().clone();
        let font_source_policy = session.font_source_policy().clone();
        let resource_fingerprint = resource_closure::fingerprint_svg_resources(
            &svg,
            font_catalog.fingerprint().as_bytes(),
            &font_source_policy,
        );
        Self {
            svg,
            prepared_text_svg: None,
            prepared_text_evidence: PreparedTextEvidenceLease::default(),
            prepared_text_terminal_receipt: None,
            prepared_text_evidence_valid: true,
            reference_plan,
            resource_closure,
            finalization_report,
            font_catalog,
            font_source_policy,
            resource_fingerprint,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.svg
    }

    pub(crate) fn attach_prepared_text_evidence(
        mut self,
        evidence: PreparedTextEvidenceLease,
        evidence_valid: bool,
        resource_policy: crate::resources::RenderResourcePolicy,
    ) -> Result<Self> {
        self.prepared_text_evidence_valid = evidence_valid;
        self.prepared_text_evidence = if evidence_valid {
            evidence
        } else {
            PreparedTextEvidenceLease::default()
        };
        if let Some(plan) = font_embedding::SvgFontEmbeddingPlan::for_prepared_text(
            &self.font_catalog,
            self.prepared_text_evidence.entries(),
            &self.svg,
        ) {
            let embedded = plan.inject(&self.svg)?;
            resource_policy.check_svg_bytes(&embedded, ResourceLimitPhase::SvgPostprocess)?;
            let terminal = final_validation::validate_resvg_compatible_svg_with_font_plan(
                &embedded,
                resource_policy,
                &plan,
            )?;
            self.reference_plan = terminal.reference_plan.clone();
            self.resource_closure = terminal.resource_closure.clone();
            self.finalization_report.refresh_terminal(&terminal);
            self.svg = embedded;
        }
        if self.prepared_text_evidence.is_empty() {
            self.prepared_text_svg = None;
            self.prepared_text_terminal_receipt = None;
        } else {
            let (public_svg, tokenized_svg) = partition_prepared_text_label_ids(
                std::mem::take(&mut self.svg),
                self.prepared_text_evidence.entries(),
            )?;
            self.svg = public_svg;
            self.prepared_text_svg = tokenized_svg.map(Arc::from);
            self.prepared_text_terminal_receipt = self
                .prepared_text_svg
                .as_deref()
                .and_then(|tokenized_svg| {
                    PreparedTextTerminalReceipt::from_ledger(
                        tokenized_svg,
                        self.prepared_text_evidence.entries(),
                    )
                })
                .ok_or_else(|| {
                    Error::svg_postprocess(
                        "prepared-text-terminal-receipt",
                        "prepared-text terminal receipt could not be frozen from the sealed artifact",
                    )
                })
                .map(Some)?;
        }
        self.resource_fingerprint = resource_closure::fingerprint_svg_resources(
            self.native_export_svg(),
            self.font_catalog.fingerprint().as_bytes(),
            &self.font_source_policy,
        );
        Ok(self)
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

    /// Projects the sealed artifact to SVG text and discards its export resources and proofs.
    pub fn into_string(self) -> String {
        self.svg
    }

    /// Returns the reference-expansion preflight retained at terminal finalization.
    pub const fn reference_plan(&self) -> &SvgReferencePlan {
        &self.reference_plan
    }

    /// Returns the terminal same-document and inline-resource inventory.
    pub const fn resource_closure(&self) -> &SvgResourceClosure {
        &self.resource_closure
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
            .field("reference_plan", &self.reference_plan)
            .field("resource_closure", &self.resource_closure)
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
            && self.reference_plan == other.reference_plan
            && self.resource_closure == other.resource_closure
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

#[derive(Clone)]
pub struct SvgPipeline {
    preset: SvgPipelinePreset,
    postprocessors: Vec<Arc<dyn SvgPostprocessor>>,
    drop_native_duplicate_fallbacks: bool,
}

impl fmt::Debug for SvgPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = self
            .postprocessors
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>();

        f.debug_struct("SvgPipeline")
            .field("preset", &self.preset)
            .field("postprocessors", &names)
            .field(
                "drop_native_duplicate_fallbacks",
                &self.drop_native_duplicate_fallbacks,
            )
            .finish()
    }
}

impl Default for SvgPipeline {
    fn default() -> Self {
        Self::parity()
    }
}

impl SvgPipeline {
    pub fn parity() -> Self {
        Self::from_preset(SvgPipelinePreset::Parity)
    }

    pub fn readable() -> Self {
        Self::from_preset(SvgPipelinePreset::Readable)
    }

    pub fn resvg_safe() -> Self {
        Self::from_preset(SvgPipelinePreset::ResvgSafe)
    }

    pub fn from_preset(preset: SvgPipelinePreset) -> Self {
        Self {
            preset,
            postprocessors: Vec::new(),
            drop_native_duplicate_fallbacks: false,
        }
    }

    /// Returns whether the configured draft pipeline has no untrusted transformation that could
    /// rewrite typed root or family output after the renderer emitted it.
    ///
    /// The terminal preset itself is renderer-owned and does not invalidate root evidence. A
    /// custom postprocessor is conservatively treated as a mutation boundary until it provides a
    /// dedicated preservation contract.
    pub(crate) fn preserves_typed_theme_evidence(&self) -> bool {
        self.postprocessors.is_empty()
    }

    /// Returns whether the pipeline can preserve renderer-owned prepared-label locators.
    pub(crate) fn preserves_prepared_text_evidence(&self) -> bool {
        self.postprocessors.is_empty()
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

    pub fn with_postprocessor<P>(mut self, postprocessor: P) -> Self
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
        self
    }

    pub fn with_shared_postprocessor(mut self, postprocessor: Arc<dyn SvgPostprocessor>) -> Self {
        self.postprocessors.push(postprocessor);
        self
    }

    pub fn push_postprocessor<P>(&mut self, postprocessor: P)
    where
        P: SvgPostprocessor + 'static,
    {
        self.postprocessors.push(Arc::new(postprocessor));
    }

    pub fn process<'a>(&self, svg: &'a str, session: &RenderSession) -> Result<Cow<'a, str>> {
        let metadata = SvgPostprocessMetadata::from_svg(svg);
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
            .process_cow_with_reference_plan(svg, metadata, session)?
            .0)
    }

    fn process_cow_with_reference_plan<'a>(
        &self,
        svg: Cow<'a, str>,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<(
        Cow<'a, str>,
        Option<final_validation::TerminalSvgValidation>,
    )> {
        session.checkpoint(OperationPhase::Postprocess)?;
        let mut current = svg;
        session
            .resource_policy()
            .check_svg_bytes(current.as_ref(), ResourceLimitPhase::SvgPostprocess)?;

        for (index, postprocessor) in self.postprocessors.iter().enumerate() {
            session.checkpoint(OperationPhase::Postprocess)?;
            let ctx = SvgPostprocessContext::new(
                self.preset,
                index,
                postprocessor.name(),
                metadata,
                session,
            );
            current = match postprocessor.process(current, &ctx) {
                Ok(current) => current,
                Err(error @ (Error::Cancelled(_) | Error::ResourceLimitExceeded(_))) => {
                    return Err(error);
                }
                Err(error) => {
                    session.checkpoint(OperationPhase::Postprocess)?;
                    return Err(Error::svg_postprocess(
                        postprocessor.name(),
                        error.to_string(),
                    ));
                }
            };
            session.checkpoint(OperationPhase::Postprocess)?;
            session
                .resource_policy()
                .check_svg_bytes(current.as_ref(), ResourceLimitPhase::SvgPostprocess)?;
        }

        session.checkpoint(OperationPhase::Postprocess)?;
        let finalized = preset::apply_preset_cow(
            self.preset,
            current,
            metadata,
            session,
            self.drop_native_duplicate_fallbacks,
        )?;
        let finalized = crate::xml::strip_forbidden_xml_1_0_chars_cow(finalized);
        session.checkpoint(OperationPhase::Postprocess)?;
        session
            .resource_policy()
            .check_svg_bytes(finalized.as_ref(), ResourceLimitPhase::SvgPostprocess)?;
        let terminal = if self.preset == SvgPipelinePreset::ResvgSafe {
            session.checkpoint(OperationPhase::Postprocess)?;
            Some(final_validation::validate_resvg_compatible_svg(
                finalized.as_ref(),
                session.resource_policy(),
            )?)
        } else {
            session.checkpoint(OperationPhase::Postprocess)?;
            final_validation::validate_well_formed_svg(
                finalized.as_ref(),
                session.resource_policy(),
            )?;
            None
        };
        session.checkpoint(OperationPhase::Postprocess)?;
        Ok((finalized, terminal))
    }

    pub fn process_to_string(&self, svg: &str, session: &RenderSession) -> Result<String> {
        Ok(self.process(svg, session)?.into_owned())
    }

    pub fn process_owned_to_string(&self, svg: String, session: &RenderSession) -> Result<String> {
        let metadata = SvgPostprocessMetadata::from_svg(&svg);
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

    pub fn process_resvg_compatible(
        &self,
        svg: &str,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        let metadata = SvgPostprocessMetadata::from_svg(svg);
        self.process_resvg_compatible_with_metadata(svg, &metadata, session)
    }

    pub fn process_resvg_compatible_with_metadata(
        &self,
        svg: &str,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let (svg, terminal) =
            self.process_cow_with_reference_plan(Cow::Borrowed(svg), metadata, session)?;
        let terminal = terminal.expect("resvg-safe processing always produces terminal evidence");
        let finalization_report = SvgFinalizationReport::from_pipeline(self, &terminal);
        Ok(ResvgCompatibleSvg::finalized(
            svg.into_owned(),
            terminal.reference_plan,
            terminal.resource_closure,
            finalization_report,
            session,
        ))
    }

    pub fn process_owned_resvg_compatible_with_metadata(
        &self,
        svg: String,
        metadata: &SvgPostprocessMetadata,
        session: &RenderSession,
    ) -> Result<ResvgCompatibleSvg> {
        self.ensure_resvg_safe_contract()?;
        let (svg, terminal) =
            self.process_cow_with_reference_plan(Cow::Owned(svg), metadata, session)?;
        let terminal = terminal.expect("resvg-safe processing always produces terminal evidence");
        let finalization_report = SvgFinalizationReport::from_pipeline(self, &terminal);
        Ok(ResvgCompatibleSvg::finalized(
            svg.into_owned(),
            terminal.reference_plan,
            terminal.resource_closure,
            finalization_report,
            session,
        ))
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
    fn sealed_svg_retains_and_fingerprints_the_authorized_font_catalog() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text>Alpha</text></svg>"#;
        let default_session = render_session();
        let font_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let custom_catalog =
            crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
                "excalifont",
                font_bytes,
            )])
            .compile(&crate::diagram_theme::ThemeResourcePolicy::interactive())
            .unwrap();
        let custom_session = crate::environment::RenderEnvironment::deterministic()
            .with_font_catalog(custom_catalog.clone())
            .begin_session()
            .unwrap();

        let default_svg = finalize_resvg_svg(svg, &default_session).unwrap();
        let custom_svg = finalize_resvg_svg(svg, &custom_session).unwrap();

        assert_eq!(
            custom_svg.finalization_report().preset(),
            SvgPipelinePreset::ResvgSafe
        );
        assert!(
            custom_svg
                .finalization_report()
                .postprocessor_names()
                .is_empty()
        );
        assert!(
            custom_svg
                .finalization_report()
                .resource_closure()
                .is_closed()
        );
        assert_eq!(
            custom_svg.finalization_report().reference_plan(),
            custom_svg.reference_plan()
        );

        assert_eq!(default_svg.as_str(), custom_svg.as_str());
        assert_ne!(
            default_svg.resource_fingerprint(),
            custom_svg.resource_fingerprint()
        );
        assert_eq!(
            custom_svg.font_catalog().fingerprint(),
            custom_catalog.fingerprint()
        );
        assert!(custom_svg.resource_closure().is_closed());
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
