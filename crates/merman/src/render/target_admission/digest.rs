use sha2::{Digest as _, Sha256};

use super::{
    DocumentPortabilityReport, RenderArtifactKind, TargetAdmissionReason, TargetAdmissionStatus,
    TargetFontSource,
};
use crate::render::{RenderEvidence, ThemeEvidenceStatus, ThemeEvidenceSummary};

const TARGET_EVIDENCE_DIGEST_DOMAIN: &[u8] = b"merman-target-evidence-experimental-v4";

#[cfg(feature = "svg")]
fn render_digest(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, domain);
    update(&mut hasher);
    hasher.finalize().into()
}

#[cfg(feature = "svg")]
fn update_digest_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(feature = "svg")]
fn update_digest_bool(hasher: &mut Sha256, value: bool) {
    hasher.update([u8::from(value)]);
}

#[cfg(feature = "svg")]
fn update_digest_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_le_bytes());
}

#[cfg(feature = "svg")]
fn update_digest_usize(hasher: &mut Sha256, value: usize) {
    update_digest_u64(hasher, u64::try_from(value).unwrap_or(u64::MAX));
}

#[cfg(feature = "pdf")]
fn update_digest_f32(hasher: &mut Sha256, value: f32) {
    hasher.update(value.to_bits().to_le_bytes());
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn update_digest_f64(hasher: &mut Sha256, value: f64) {
    hasher.update(value.to_bits().to_le_bytes());
}

#[cfg(feature = "svg")]
fn update_digest_optional_str(hasher: &mut Sha256, value: Option<&str>) {
    update_digest_bool(hasher, value.is_some());
    if let Some(value) = value {
        update_digest_field(hasher, value.as_bytes());
    }
}

#[cfg(feature = "svg")]
fn update_digest_sequence<I, T>(
    hasher: &mut Sha256,
    label: &[u8],
    values: I,
    mut update_item: impl FnMut(&mut Sha256, T),
) where
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
{
    let values = values.into_iter();
    update_digest_field(hasher, label);
    update_digest_usize(hasher, values.len());
    for value in values {
        update_item(hasher, value);
    }
}

#[cfg(feature = "svg")]
pub(in crate::render) fn artifact_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(feature = "svg")]
pub(in crate::render) fn document_digest(
    public_svg_digest: [u8; 32],
    native_svg_digest: [u8; 32],
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    evidence: &RenderEvidence,
    portability: &DocumentPortabilityReport,
) -> [u8; 32] {
    render_digest(b"merman-rendered-document-experimental-v5", |hasher| {
        update_digest_field(hasher, &public_svg_digest);
        update_digest_field(hasher, &native_svg_digest);
        update_digest_field(hasher, resource_fingerprint.as_bytes());
        update_digest_field(hasher, evidence.family_id().as_str().as_bytes());
        match evidence.theme_recipe_fingerprint() {
            Some(fingerprint) => {
                update_digest_field(hasher, b"theme-recipe");
                update_digest_field(hasher, fingerprint.as_bytes());
            }
            None => update_digest_field(hasher, b"no-theme-recipe"),
        }
        update_operation_context_digest(hasher, evidence);
        update_theme_summary_digest(hasher, evidence.theme_evidence());
        update_root_capability_digest(hasher, evidence.root_applied_capabilities());
        update_portability_requirement_digest(hasher, evidence.portability_requirement());
        update_host_admission_digest(hasher, evidence.session().theme_host_admission_report());
        update_text_measurement_digest(hasher, evidence.measurement());
        update_prepared_text_layout_digest(hasher, evidence.prepared_text_layout());
        update_digest_optional_str(
            hasher,
            evidence.text_layout_failure().map(|failure| failure.id()),
        );
        update_native_filter_digest(hasher, evidence.native_filter_receipt());
        update_digest_bool(hasher, portability.resource_closure_complete());
        update_digest_bool(hasher, portability.prepared_text_evidence_valid());
        update_prepared_math_digest(hasher, portability.prepared_math_evidence());
        update_digest_bool(hasher, portability.is_evidence_valid());
        update_digest_bool(hasher, portability.is_host_dependent());
        update_digest_sequence(
            hasher,
            b"portability-reasons",
            portability.reasons().iter().copied(),
            |hasher, reason| update_digest_field(hasher, reason.id().as_bytes()),
        );
    })
}

#[cfg(feature = "svg")]
fn update_root_capability_digest(
    hasher: &mut Sha256,
    capabilities: &[merman_render::diagram_theme::ThemeCapability],
) {
    update_digest_sequence(
        hasher,
        b"root-applied-capabilities",
        capabilities.iter().copied(),
        |hasher, capability| update_digest_field(hasher, capability.id().as_bytes()),
    );
}

#[cfg(feature = "svg")]
fn update_prepared_math_digest(
    hasher: &mut Sha256,
    summary: merman_render::__private::PreparedMathEvidenceSummary,
) {
    update_prepared_math_digest_fields(
        hasher,
        summary.evidence_valid(),
        summary.expected_occurrence_count(),
        summary.terminal_occurrence_count(),
        summary.terminal_proof_complete(),
        summary.terminal_artifact_digest(),
    );
}

#[cfg(feature = "svg")]
fn update_prepared_math_digest_fields(
    hasher: &mut Sha256,
    evidence_valid: bool,
    expected_occurrence_count: usize,
    terminal_occurrence_count: usize,
    terminal_proof_complete: bool,
    artifact_digest: Option<[u8; 32]>,
) {
    update_digest_bool(hasher, evidence_valid);
    update_digest_usize(hasher, expected_occurrence_count);
    update_digest_usize(hasher, terminal_occurrence_count);
    update_digest_bool(hasher, terminal_proof_complete);
    update_digest_bool(hasher, artifact_digest.is_some());
    if let Some(artifact_digest) = artifact_digest {
        update_digest_field(hasher, &artifact_digest);
    }
}

#[cfg(feature = "svg")]
fn update_operation_context_digest(hasher: &mut Sha256, evidence: &RenderEvidence) {
    let context = evidence.operation_context();
    hasher.update(context.unix_millis().to_le_bytes());
    update_digest_field(hasher, context.clock_source().id().as_bytes());
    let date = context.today_local();
    hasher.update(date.year().to_le_bytes());
    hasher.update(date.month().to_le_bytes());
    hasher.update(date.day().to_le_bytes());
    update_digest_bool(hasher, context.today_is_fixed());
    update_digest_u64(hasher, context.seed());
    update_digest_field(hasher, context.random_source().id().as_bytes());
    update_digest_bool(hasher, context.timing().is_some());
    let zone = evidence.local_time_zone();
    let zone_source = match zone.source() {
        merman_core::time::LocalTimeZoneSource::FixedOffset => "fixed-offset",
        merman_core::time::LocalTimeZoneSource::System => "system",
    };
    update_digest_field(hasher, zone_source.as_bytes());
    update_digest_field(hasher, zone.identifier().as_bytes());
    update_digest_optional_str(hasher, zone.rules_sha256());
}

#[cfg(feature = "svg")]
fn update_theme_summary_digest(hasher: &mut Sha256, summary: ThemeEvidenceSummary) {
    let status = match summary.status() {
        ThemeEvidenceStatus::NotApplicable => "not-applicable",
        ThemeEvidenceStatus::Verified => "verified",
        ThemeEvidenceStatus::Residual => "residual",
        ThemeEvidenceStatus::Incomplete => "incomplete",
    };
    update_digest_field(hasher, status.as_bytes());
    update_digest_bool(hasher, summary.output_mutated());
}

#[cfg(feature = "svg")]
fn update_portability_requirement_digest(
    hasher: &mut Sha256,
    requirement: merman_render::diagram_theme::ThemePortabilityRequirement,
) {
    let requirement = match requirement {
        merman_render::diagram_theme::ThemePortabilityRequirement::BestEffort => "best-effort",
        merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable => {
            "require-portable"
        }
        _ => "unknown",
    };
    update_digest_field(hasher, requirement.as_bytes());
}

#[cfg(feature = "svg")]
fn update_host_admission_digest(
    hasher: &mut Sha256,
    report: Option<&merman_render::diagram_theme::ThemeHostAdmissionReport>,
) {
    update_digest_bool(hasher, report.is_some());
    let Some(report) = report else {
        return;
    };
    update_digest_sequence(
        hasher,
        b"host-capabilities",
        report.host_allowed_capabilities(),
        |hasher, capability| update_digest_field(hasher, capability.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"host-text-capabilities",
        report.host_allowed_text_capabilities(),
        |hasher, capability| update_digest_field(hasher, capability.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"font-source-priority",
        report.font_source_policy().priority(),
        |hasher, source| update_digest_field(hasher, source.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"measurement-fallback-priority",
        report.measurement_fallback_policy().priority(),
        |hasher, fallback| update_digest_field(hasher, fallback.id().as_bytes()),
    );
    update_portability_requirement_digest(hasher, report.portability_requirement());
    update_digest_sequence(
        hasher,
        b"trusted-theme-lanes",
        report.trusted_lanes().allowed(),
        |hasher, lane| update_digest_field(hasher, lane.id().as_bytes()),
    );
}

#[cfg(feature = "svg")]
fn update_text_measurement_digest(
    hasher: &mut Sha256,
    report: &merman_render::environment::TextMeasurementReport,
) {
    update_digest_sequence(
        hasher,
        b"text-measurement-provenance",
        report.entries().iter(),
        |hasher, entry| {
            let provenance = entry.provenance();
            let phase = match provenance.phase {
                merman_render::environment::TextMeasurementPhase::Layout => "layout",
                merman_render::environment::TextMeasurementPhase::Wrap => "wrap",
                merman_render::environment::TextMeasurementPhase::SvgBBox => "svg-bbox",
                merman_render::environment::TextMeasurementPhase::ComputedLength => {
                    "computed-length"
                }
            };
            update_digest_field(hasher, phase.as_bytes());
            update_digest_field(hasher, provenance.operation.external_name().as_bytes());
            let source = match provenance.source {
                merman_render::environment::TextMeasurementSource::Profile => "profile",
                merman_render::environment::TextMeasurementSource::Host => "host",
            };
            update_digest_field(hasher, source.as_bytes());
            update_digest_field(hasher, provenance.identity.profile().as_str().as_bytes());
            update_digest_field(hasher, provenance.identity.version().as_bytes());
            update_digest_sequence(
                hasher,
                b"measurement-profile-decorators",
                provenance
                    .identity
                    .decorators()
                    .iter()
                    .map(|decorator| decorator.as_ref()),
                |hasher, decorator| update_digest_field(hasher, decorator.as_bytes()),
            );
            let fallback_reason = provenance.fallback_reason.map(|reason| match reason {
                merman_render::environment::HostFallbackReason::Missing => "missing",
                merman_render::environment::HostFallbackReason::Invalid => "invalid",
                merman_render::environment::HostFallbackReason::Error => "error",
            });
            update_digest_optional_str(hasher, fallback_reason);
            update_digest_u64(hasher, entry.count());
        },
    );
}

#[cfg(feature = "svg")]
fn update_prepared_text_layout_digest(
    hasher: &mut Sha256,
    report: Option<&merman_render::text::PreparedTextLayoutReport>,
) {
    update_digest_bool(hasher, report.is_some());
    let Some(report) = report else {
        return;
    };
    update_digest_field(hasher, report.catalog_fingerprint().as_bytes());
    update_digest_usize(hasher, report.face_count());
    update_digest_sequence(
        hasher,
        b"used-font-sources",
        report.used_font_sources().iter().copied(),
        |hasher, source| update_digest_field(hasher, source.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"used-measurement-fallbacks",
        report.used_fallbacks().iter().copied(),
        |hasher, fallback| update_digest_field(hasher, fallback.id().as_bytes()),
    );
    update_digest_u64(hasher, report.fallback_count());
    update_digest_u64(hasher, report.failed_attempt_count());
    update_digest_bool(hasher, report.is_host_dependent());
}

#[cfg(feature = "svg")]
fn update_native_filter_digest(
    hasher: &mut Sha256,
    receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
) {
    update_digest_bool(hasher, receipt.is_some());
    if let Some(receipt) = receipt {
        update_digest_u64(hasher, u64::from(receipt.drop_shadow_count()));
        update_digest_u64(hasher, u64::from(receipt.reference_count()));
        update_digest_field(hasher, receipt.identity_digest());
    }
}

#[cfg(feature = "svg")]
fn update_target_admission_digest(
    hasher: &mut Sha256,
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) {
    update_digest_field(hasher, artifact_kind.id().as_bytes());
    update_digest_field(hasher, status.id().as_bytes());
    update_digest_sequence(
        hasher,
        b"target-admission-reasons",
        reasons.iter().copied(),
        |hasher, reason| update_digest_field(hasher, reason.id().as_bytes()),
    );
}

#[cfg(feature = "svg")]
#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn target_admission_receipt_digest(
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
    font_source: TargetFontSource,
    resource_fingerprint: &[u8; 32],
    font_catalog_fingerprint: &[u8; 32],
    document_digest: [u8; 32],
    target_evidence_digest: [u8; 32],
    artifact_digest: [u8; 32],
) -> [u8; 32] {
    render_digest(
        b"merman-target-admission-receipt-experimental-v1",
        |hasher| {
            update_target_admission_digest(hasher, artifact_kind, status, reasons);
            update_digest_field(hasher, font_source.id().as_bytes());
            update_digest_field(hasher, resource_fingerprint);
            update_digest_field(hasher, font_catalog_fingerprint);
            update_digest_field(hasher, &document_digest);
            update_digest_field(hasher, &target_evidence_digest);
            update_digest_field(hasher, &artifact_digest);
        },
    )
}

#[cfg(feature = "svg")]
pub(in crate::render) fn standalone_target_evidence_digest(
    svg: &merman_render::svg::StandaloneSvgArtifact,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
    font_source: TargetFontSource,
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, RenderArtifactKind::Svg, status, reasons);
        update_digest_field(hasher, font_source.id().as_bytes());
        update_digest_field(
            hasher,
            svg_pipeline_preset_id(svg.selected_pipeline()).as_bytes(),
        );
        update_digest_field(hasher, svg.terminal_status().id().as_bytes());
        update_digest_bool(hasher, svg.finalization_report().is_some());
        if let Some(report) = svg.finalization_report() {
            update_digest_sequence(
                hasher,
                b"postprocessors",
                report.postprocessor_names().iter().map(String::as_str),
                |hasher, name| update_digest_field(hasher, name.as_bytes()),
            );
            update_digest_bool(hasher, report.drop_native_duplicate_fallbacks());
            let reference_plan = report.reference_plan();
            update_digest_usize(hasher, reference_plan.expanded_elements());
            update_digest_usize(hasher, reference_plan.max_tree_depth());
            update_digest_sequence(
                hasher,
                b"raw-element-occurrences",
                reference_plan.raw_element_occurrences().iter().copied(),
                update_digest_usize,
            );
            let closure = report.resource_closure();
            update_digest_sequence(
                hasher,
                b"available-fragment-ids",
                closure.available_fragment_ids().iter().map(String::as_str),
                |hasher, id| update_digest_field(hasher, id.as_bytes()),
            );
            update_digest_sequence(
                hasher,
                b"referenced-fragment-ids",
                closure.referenced_fragment_ids().iter().map(String::as_str),
                |hasher, id| update_digest_field(hasher, id.as_bytes()),
            );
            update_digest_sequence(
                hasher,
                b"stylesheet-fragment-ids",
                closure.stylesheet_fragment_ids().iter().map(String::as_str),
                |hasher, id| update_digest_field(hasher, id.as_bytes()),
            );
            update_digest_usize(hasher, closure.inline_data_resource_count());
            update_digest_usize(hasher, report.text_element_count());
        }
        update_digest_bool(hasher, svg.prepared_text_evidence_valid());
        update_prepared_math_digest(hasher, document.prepared_math_evidence());
        update_digest_bool(hasher, svg.text_fonts_are_self_contained());
        update_root_capability_digest(hasher, evidence.root_applied_capabilities());
        update_native_filter_digest(hasher, evidence.native_filter_receipt());
    })
}

#[cfg(feature = "svg")]
const fn svg_pipeline_preset_id(preset: merman_render::svg::SvgPipelinePreset) -> &'static str {
    match preset {
        merman_render::svg::SvgPipelinePreset::Parity => "parity",
        merman_render::svg::SvgPipelinePreset::Readable => "readable",
        merman_render::svg::SvgPipelinePreset::ResvgSafe => "resvg-safe",
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_export_font_digest(hasher: &mut Sha256, fonts: merman_export::ExportFontPlan) {
    update_digest_field(hasher, fonts.catalog_fingerprint().as_bytes());
    update_digest_field(hasher, fonts.source_mode().id().as_bytes());
    update_digest_usize(hasher, fonts.loaded_embedded_face_count());
    update_digest_bool(hasher, fonts.used_embedded_fonts());
    update_digest_bool(hasher, fonts.used_system_fonts());
    update_digest_bool(hasher, fonts.family_fallback_used());
    update_digest_bool(hasher, fonts.glyph_fallback_used());
    update_digest_bool(hasher, fonts.unresolved_font_request());
    update_digest_bool(hasher, fonts.unresolved_glyph_fallback());
    update_digest_usize(hasher, fonts.prepared_label_expected_count());
    update_digest_usize(hasher, fonts.prepared_label_verified_count());
    update_digest_usize(hasher, fonts.prepared_label_mismatch_count());
    update_digest_usize(hasher, fonts.prepared_label_host_dependent_count());
    update_digest_usize(hasher, fonts.prepared_label_terminal_incomplete_count());
    update_digest_usize(hasher, fonts.unclassified_face_count());
    update_digest_usize(hasher, fonts.notdef_glyph_count());
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_embedded_image_digest(hasher: &mut Sha256, plan: merman_export::EmbeddedImagePlan) {
    update_digest_usize(hasher, plan.data_resources);
    update_digest_usize(hasher, plan.raster_images);
    update_digest_u64(hasher, plan.largest_data_bytes);
    update_digest_u64(hasher, plan.total_data_bytes);
    update_digest_u64(hasher, plan.largest_raster_pixels);
    update_digest_u64(hasher, plan.total_pixels);
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_svg_conversion_digest(hasher: &mut Sha256, plan: merman_export::SvgConversionPlan) {
    update_digest_usize(hasher, plan.tree_nodes);
    update_digest_usize(hasher, plan.max_tree_depth);
    update_digest_usize(hasher, plan.max_isolation_depth);
    update_digest_usize(hasher, plan.filtered_groups);
    update_digest_usize(hasher, plan.filter_primitives);
    update_digest_usize(hasher, plan.subroots);
    update_digest_usize(hasher, plan.nested_svg_images);
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_export_color_digest(hasher: &mut Sha256, color: Option<merman_export::ExportRgbaColor>) {
    update_digest_bool(hasher, color.is_some());
    if let Some(color) = color {
        hasher.update([color.red(), color.green(), color.blue(), color.alpha()]);
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn raster_target_evidence_digest(
    artifact_kind: RenderArtifactKind,
    report: merman_export::RasterExportReport,
    expected_native_filter: Option<merman_render::__private::NativeSvgFilterReceipt>,
    root_applied_capabilities: &[merman_render::diagram_theme::ThemeCapability],
    prepared_math: merman_render::__private::PreparedMathEvidenceSummary,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, artifact_kind, status, reasons);
        update_digest_field(hasher, report.resource_fingerprint().as_bytes());
        update_root_capability_digest(hasher, root_applied_capabilities);
        update_prepared_math_digest(hasher, prepared_math);
        update_native_filter_digest(hasher, expected_native_filter);
        update_native_filter_digest(hasher, report.native_filter_receipt());
        let output = match report.output() {
            merman_export::RasterOutputKind::Png => "png",
            merman_export::RasterOutputKind::Jpeg => "jpeg",
            _ => "unknown",
        };
        update_digest_field(hasher, output.as_bytes());
        let plan = report.raster();
        update_digest_f64(hasher, plan.requested_width_px);
        update_digest_f64(hasher, plan.requested_height_px);
        update_digest_u64(hasher, u64::from(plan.width_px));
        update_digest_u64(hasher, u64::from(plan.height_px));
        update_digest_f64(hasher, plan.requested_scale);
        update_digest_f64(hasher, plan.effective_scale);
        update_digest_bool(hasher, plan.limited);
        update_embedded_image_digest(hasher, report.embedded_images());
        update_svg_conversion_digest(hasher, report.conversion());
        update_export_font_digest(hasher, report.fonts());
        update_export_color_digest(hasher, report.matte());
        update_digest_bool(hasher, report.matte_defaulted());
    })
}

#[cfg(feature = "pdf")]
pub(super) fn pdf_target_evidence_digest(
    report: merman_export::PdfExportReport,
    expected_native_filter: Option<merman_render::__private::NativeSvgFilterReceipt>,
    root_applied_capabilities: &[merman_render::diagram_theme::ThemeCapability],
    prepared_math: merman_render::__private::PreparedMathEvidenceSummary,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, RenderArtifactKind::Pdf, status, reasons);
        update_digest_field(hasher, report.resource_fingerprint().as_bytes());
        update_root_capability_digest(hasher, root_applied_capabilities);
        update_prepared_math_digest(hasher, prepared_math);
        update_native_filter_digest(hasher, expected_native_filter);
        update_native_filter_digest(hasher, report.native_filter_receipt());
        update_digest_bool(hasher, report.native_filter_fully_localized());
        let page = report.page();
        update_digest_f32(hasher, page.page_width_pt());
        update_digest_f32(hasher, page.page_height_pt());
        update_digest_f32(hasher, page.drawing_width_pt());
        update_digest_f32(hasher, page.drawing_height_pt());
        update_digest_f32(hasher, page.offset_x_pt());
        update_digest_f32(hasher, page.offset_y_pt());
        let filters = report.filters();
        update_digest_usize(hasher, filters.filtered_groups);
        update_digest_f32(hasher, filters.requested_scale);
        update_digest_f32(hasher, filters.effective_scale);
        update_digest_u64(hasher, filters.requested_image_pixels);
        update_digest_u64(hasher, filters.effective_image_pixels);
        update_digest_bool(hasher, filters.limited);
        update_embedded_image_digest(hasher, report.embedded_images());
        update_svg_conversion_digest(hasher, report.conversion());
        update_export_font_digest(hasher, report.fonts());
        update_export_color_digest(hasher, report.page_paint());
    })
}

#[cfg(test)]
mod tests {
    use super::{
        render_digest, target_admission_receipt_digest, update_digest_field,
        update_digest_sequence, update_native_filter_digest, update_prepared_math_digest_fields,
        update_root_capability_digest,
    };
    use crate::render::{
        RenderArtifactKind, TargetAdmissionReason, TargetAdmissionStatus, TargetFontSource,
    };
    use merman_render::__private::{NativeSvgFilterReceipt, NativeSvgHardShadow};
    use merman_render::diagram_theme::ThemeCapability;

    #[test]
    fn prepared_math_digest_binds_every_terminal_proof_field() {
        fn digest(
            evidence_valid: bool,
            expected: usize,
            terminal: usize,
            proof_complete: bool,
            artifact: Option<[u8; 32]>,
        ) -> [u8; 32] {
            render_digest(b"prepared-math-digest-test", |hasher| {
                update_prepared_math_digest_fields(
                    hasher,
                    evidence_valid,
                    expected,
                    terminal,
                    proof_complete,
                    artifact,
                );
            })
        }

        let baseline = digest(true, 2, 2, true, Some([7; 32]));
        assert_ne!(baseline, digest(false, 2, 2, true, Some([7; 32])));
        assert_ne!(baseline, digest(true, 3, 2, true, Some([7; 32])));
        assert_ne!(baseline, digest(true, 2, 1, true, Some([7; 32])));
        assert_ne!(baseline, digest(true, 2, 2, false, Some([7; 32])));
        assert_ne!(baseline, digest(true, 2, 2, true, None));
        assert_ne!(baseline, digest(true, 2, 2, true, Some([8; 32])));
    }

    #[test]
    fn root_capability_digest_binds_the_terminal_capability_set() {
        fn digest(capabilities: &[ThemeCapability]) -> [u8; 32] {
            render_digest(b"root-capability-digest-test", |hasher| {
                update_root_capability_digest(hasher, capabilities);
            })
        }

        assert_ne!(
            digest(&[ThemeCapability::SolidPaint]),
            digest(&[ThemeCapability::SolidPaint, ThemeCapability::Opacity])
        );
        assert_ne!(
            digest(&[ThemeCapability::SolidPaint]),
            digest(&[ThemeCapability::TransparentPaint])
        );
    }

    #[test]
    fn native_filter_digest_distinguishes_equal_counts_with_different_blur() {
        fn receipt(std_deviation: f32) -> NativeSvgFilterReceipt {
            NativeSvgFilterReceipt::from_drop_shadows([NativeSvgHardShadow::new(
                "state-theme-effect-shadow",
                [-0.2, -0.2, 1.4, 1.4],
                [5.0, 5.0],
                [std_deviation, std_deviation],
                "#111827",
                1,
            )
            .expect("valid drop shadow")])
            .expect("valid native filter receipt")
        }

        fn digest(receipt: NativeSvgFilterReceipt) -> [u8; 32] {
            render_digest(b"native-filter-digest-test", |hasher| {
                update_native_filter_digest(hasher, Some(receipt));
            })
        }

        let sharp = receipt(0.0);
        let blurred = receipt(8.0);

        assert_eq!(sharp.drop_shadow_count(), blurred.drop_shadow_count());
        assert_eq!(sharp.reference_count(), blurred.reference_count());
        assert_ne!(digest(sharp), digest(blurred));
    }

    #[test]
    fn target_admission_receipt_digest_binds_every_canonical_field() {
        #[derive(Clone)]
        struct DigestInputs {
            artifact_kind: RenderArtifactKind,
            status: TargetAdmissionStatus,
            reasons: Vec<TargetAdmissionReason>,
            font_source: TargetFontSource,
            resource_fingerprint: [u8; 32],
            font_catalog_fingerprint: [u8; 32],
            document_digest: [u8; 32],
            target_evidence_digest: [u8; 32],
            artifact_digest: [u8; 32],
        }

        impl DigestInputs {
            fn digest(&self) -> [u8; 32] {
                target_admission_receipt_digest(
                    self.artifact_kind,
                    self.status,
                    &self.reasons,
                    self.font_source,
                    &self.resource_fingerprint,
                    &self.font_catalog_fingerprint,
                    self.document_digest,
                    self.target_evidence_digest,
                    self.artifact_digest,
                )
            }
        }

        fn assert_field_is_bound(
            baseline: &DigestInputs,
            baseline_digest: [u8; 32],
            field: &str,
            mutate: impl FnOnce(&mut DigestInputs),
        ) {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(
                baseline_digest,
                changed.digest(),
                "receipt digest must bind {field}"
            );
        }

        let baseline = DigestInputs {
            artifact_kind: RenderArtifactKind::Svg,
            status: TargetAdmissionStatus::Portable,
            reasons: vec![
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::PreparedTextLayoutFailed,
            ],
            font_source: TargetFontSource::None,
            resource_fingerprint: [1; 32],
            font_catalog_fingerprint: [2; 32],
            document_digest: [3; 32],
            target_evidence_digest: [4; 32],
            artifact_digest: [5; 32],
        };
        let baseline_digest = baseline.digest();

        assert_field_is_bound(&baseline, baseline_digest, "artifact kind", |inputs| {
            inputs.artifact_kind = RenderArtifactKind::Png;
        });
        assert_field_is_bound(&baseline, baseline_digest, "status", |inputs| {
            inputs.status = TargetAdmissionStatus::HostDependent;
        });
        assert_field_is_bound(&baseline, baseline_digest, "ordered reasons", |inputs| {
            inputs.reasons.reverse();
        });
        assert_field_is_bound(&baseline, baseline_digest, "font source", |inputs| {
            inputs.font_source = TargetFontSource::Embedded;
        });
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "resource fingerprint",
            |inputs| {
                inputs.resource_fingerprint[0] ^= 1;
            },
        );
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "font catalog fingerprint",
            |inputs| {
                inputs.font_catalog_fingerprint[0] ^= 1;
            },
        );
        assert_field_is_bound(&baseline, baseline_digest, "document digest", |inputs| {
            inputs.document_digest[0] ^= 1;
        });
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "target evidence digest",
            |inputs| {
                inputs.target_evidence_digest[0] ^= 1;
            },
        );
        assert_field_is_bound(&baseline, baseline_digest, "artifact digest", |inputs| {
            inputs.artifact_digest[0] ^= 1;
        });
    }

    #[test]
    fn canonical_digest_sequences_bind_semantic_field_boundaries() {
        fn digest(available: &[&str], referenced: &[&str], stylesheet: &[&str]) -> [u8; 32] {
            render_digest(b"digest-sequence-boundary-test", |hasher| {
                update_digest_sequence(
                    hasher,
                    b"available",
                    available.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
                update_digest_sequence(
                    hasher,
                    b"referenced",
                    referenced.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
                update_digest_sequence(
                    hasher,
                    b"stylesheet",
                    stylesheet.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
            })
        }

        assert_ne!(digest(&["a"], &["b"], &[]), digest(&[], &["a"], &["b"]));
    }
}
