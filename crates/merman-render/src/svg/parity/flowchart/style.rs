//! Flowchart style compilation helpers.

use super::*;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartCompiledStyles {
    pub(super) node_style: String,
    pub(super) label_style: String,
    pub(super) label_div_decls: Vec<(String, String)>,
    pub(super) fill: Option<String>,
    fill_source: Option<SourceFacetDeclaration>,
    pub(super) stroke: Option<String>,
    stroke_source: Option<SourceFacetDeclaration>,
    pub(super) stroke_width: Option<String>,
    stroke_width_source: Option<SourceFacetDeclaration>,
    radius_sources: Vec<SourceFacetDeclaration>,
    pub(super) stroke_dasharray: Option<String>,
    stroke_dasharray_source: Option<SourceFacetDeclaration>,
    font_stack_source: Option<SourceFacetDeclaration>,
    font_size_source: Option<SourceFacetDeclaration>,
    shape_sources: Vec<PendingSourceDeclaration>,
    generated_shape_sources: Vec<PendingSourceDeclaration>,
    label_sources: Vec<PendingSourceDeclaration>,
}

#[derive(Debug, Clone)]
enum PendingSourceProvenance {
    AssignedClass {
        class_id: String,
        assignment_ordinal: usize,
        declaration_ordinal: usize,
    },
    Inline {
        declaration_ordinal: usize,
    },
}

impl PendingSourceProvenance {
    fn bind(
        &self,
        owner_id: &str,
        channel: crate::diagram_theme::SourceStyleChannel,
    ) -> crate::diagram_theme::SourceStyleProvenance {
        match self {
            Self::AssignedClass {
                class_id,
                assignment_ordinal,
                declaration_ordinal,
            } => crate::diagram_theme::SourceStyleProvenance::assigned_class(
                owner_id,
                class_id,
                channel,
                *assignment_ordinal,
                *declaration_ordinal,
            ),
            Self::Inline {
                declaration_ordinal,
            } => crate::diagram_theme::SourceStyleProvenance::inline(
                owner_id,
                channel,
                *declaration_ordinal,
            ),
        }
    }

    fn bind_for_emission(
        &self,
        owner_id: &str,
        channel: crate::diagram_theme::SourceStyleChannel,
        generated_class_css: bool,
    ) -> crate::diagram_theme::SourceStyleProvenance {
        if generated_class_css {
            if let Self::AssignedClass {
                class_id,
                declaration_ordinal,
                ..
            } = self
            {
                return crate::diagram_theme::SourceStyleProvenance::generated_class_css(
                    class_id,
                    channel,
                    *declaration_ordinal,
                );
            }
        }
        self.bind(owner_id, channel)
    }

    const fn is_inline(&self) -> bool {
        matches!(self, Self::Inline { .. })
    }

    fn class_id(&self) -> Option<&str> {
        match self {
            Self::AssignedClass { class_id, .. } => Some(class_id),
            Self::Inline { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
struct SourceFacetDeclaration {
    prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
    provenance: PendingSourceProvenance,
    admitted: bool,
}

impl SourceFacetDeclaration {
    fn status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        crate::flowchart::FlowchartSourceFacetStatus::from_parts(true, self.admitted)
    }

    fn evidence(
        &self,
        owner_id: &str,
        emission: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeSourceFacetEmission,
    ) -> (
        crate::flowchart::FlowchartSourceFacetStatus,
        Option<crate::diagram_theme::SourceStyleResidual>,
    ) {
        let emitted = emission.reach().is_verified();
        let reason = if !self.admitted {
            Some(crate::diagram_theme::SourceStyleResidualReason::InvalidValue)
        } else if !emitted {
            Some(crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface)
        } else {
            None
        };
        let provenance = self.provenance.bind_for_emission(
            owner_id,
            if emission.generated_class_css() {
                crate::diagram_theme::SourceStyleChannel::Stylesheet
            } else {
                crate::diagram_theme::SourceStyleChannel::Shape
            },
            emission.generated_class_css(),
        );
        let declaration = self.prepared.bind(provenance);
        let residual = reason.map(|reason| {
            crate::diagram_theme::SourceStyleResidual::from_declaration(&declaration, reason)
        });
        let status = if emitted {
            self.status()
        } else {
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        };
        (status, residual)
    }
}

#[derive(Debug, Clone)]
struct PendingSourceDeclaration {
    prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
    provenance: PendingSourceProvenance,
}

impl PendingSourceDeclaration {
    fn bind(
        &self,
        owner_id: &str,
        channel: crate::diagram_theme::SourceStyleChannel,
        generated_class_css: bool,
    ) -> crate::diagram_theme::SourceStyleDeclaration {
        self.prepared.bind(self.provenance.bind_for_emission(
            owner_id,
            channel,
            generated_class_css,
        ))
    }
}

pub(in crate::svg::parity::flowchart) struct FlowchartNodeSourceEvidence {
    pub(in crate::svg::parity::flowchart) fill: crate::flowchart::FlowchartSourceFacetStatus,
    pub(in crate::svg::parity::flowchart) stroke: crate::flowchart::FlowchartSourceFacetStatus,
    pub(in crate::svg::parity::flowchart) stroke_width:
        crate::flowchart::FlowchartSourceFacetStatus,
    pub(in crate::svg::parity::flowchart) stroke_dasharray:
        crate::flowchart::FlowchartSourceFacetStatus,
    pub(in crate::svg::parity::flowchart) radius: crate::flowchart::FlowchartSourceFacetStatus,
    pub(in crate::svg::parity::flowchart) residuals: Vec<crate::diagram_theme::SourceStyleResidual>,
}

impl FlowchartCompiledStyles {
    pub(super) fn source_fill_status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.fill_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn source_stroke_status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.stroke_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn source_stroke_width_status(
        &self,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.stroke_width_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn admitted_stroke_width_value(&self) -> Option<f32> {
        self.stroke_width
            .as_deref()
            .and_then(admitted_flowchart_source_stroke_width)
    }

    pub(super) fn source_stroke_dasharray_status(
        &self,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.stroke_dasharray_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn source_radius_status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.radius_sources.iter().fold(
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            |status, source| merge_source_facet_status(status, source.status()),
        )
    }

    pub(super) fn source_font_stack_status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.font_stack_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn emitted_source_font_stack_status(
        &self,
        receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        if receipt
            .prepared_typography_reach()
            .is_some_and(|reach| reach.is_verified())
        {
            self.source_font_stack_status()
        } else {
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        }
    }

    pub(super) fn source_font_size_status(&self) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.font_size_source
            .as_ref()
            .map(SourceFacetDeclaration::status)
            .unwrap_or(crate::flowchart::FlowchartSourceFacetStatus::Absent)
    }

    pub(super) fn emitted_source_font_size_status(
        &self,
        receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        if receipt
            .prepared_typography_reach()
            .is_some_and(|reach| reach.is_verified())
        {
            self.source_font_size_status()
        } else {
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        }
    }

    pub(super) fn shape_source_evidence(
        &self,
        owner_id: &str,
        receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
        wrapper_has_class: impl Fn(&str) -> bool,
    ) -> FlowchartNodeSourceEvidence {
        let (fill, fill_residual) = source_paint_evidence(
            self.fill_source.as_ref(),
            generated_source_winner(&self.generated_shape_sources, "fill", &wrapper_has_class),
            owner_id,
            receipt,
            true,
            &wrapper_has_class,
        );
        let (stroke, stroke_residual) = source_paint_evidence(
            self.stroke_source.as_ref(),
            generated_source_winner(&self.generated_shape_sources, "stroke", &wrapper_has_class),
            owner_id,
            receipt,
            false,
            &wrapper_has_class,
        );
        let (stroke_width, stroke_width_residual) = source_stroke_width_evidence(
            self.stroke_width_source.as_ref(),
            generated_source_winner(
                &self.generated_shape_sources,
                "stroke-width",
                &wrapper_has_class,
            ),
            owner_id,
            receipt,
            &wrapper_has_class,
        );
        let (stroke_dasharray, stroke_dasharray_residual) = source_stroke_dasharray_evidence(
            self.stroke_dasharray_source.as_ref(),
            generated_source_winner(
                &self.generated_shape_sources,
                "stroke-dasharray",
                &wrapper_has_class,
            ),
            owner_id,
            receipt,
            &wrapper_has_class,
        );
        let (radius, radius_residuals) = source_radius_evidence(
            &self.radius_sources,
            &self.generated_shape_sources,
            owner_id,
            receipt,
            &wrapper_has_class,
        );
        let mut residuals = fill_residual
            .into_iter()
            .chain(stroke_residual)
            .chain(stroke_width_residual)
            .chain(stroke_dasharray_residual)
            .chain(radius_residuals)
            .collect::<Vec<_>>();

        for direct_source in &self.shape_sources {
            let property = direct_source.prepared.property();
            if matches!(
                property,
                "fill" | "stroke" | "stroke-width" | "stroke-dasharray" | "rx" | "ry"
            ) || property.starts_with("--")
            {
                continue;
            }
            let generated_source = generated_source_winner(
                &self.generated_shape_sources,
                property,
                &wrapper_has_class,
            );
            let (source, emission) =
                emitted_shape_source(direct_source, generated_source, receipt, &wrapper_has_class);
            let declaration = source.bind(
                owner_id,
                if emission.generated_class_css() {
                    crate::diagram_theme::SourceStyleChannel::Stylesheet
                } else {
                    crate::diagram_theme::SourceStyleChannel::Shape
                },
                emission.generated_class_css(),
            );
            if let Some(reason) = shape_source_residual_reason(&declaration, emission.reach()) {
                residuals.push(crate::diagram_theme::SourceStyleResidual::from_declaration(
                    &declaration,
                    reason,
                ));
            }
        }

        FlowchartNodeSourceEvidence {
            fill,
            stroke,
            stroke_width,
            stroke_dasharray,
            radius,
            residuals,
        }
    }

    pub(super) fn label_source_residuals(
        &self,
        owner_id: &str,
        receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.emitted_label_source_residuals_with_prepared_typography(
            owner_id,
            receipt.typography_verified(),
            receipt.prepared_typography_reach(),
        )
    }

    pub(super) fn emitted_shape_source_residuals(
        &self,
        owner_id: &str,
        source_style_verified: bool,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.shape_sources
            .iter()
            .filter(|source| !source.prepared.property().starts_with("--"))
            .filter_map(|source| {
                let declaration = source.bind(
                    owner_id,
                    crate::diagram_theme::SourceStyleChannel::Shape,
                    false,
                );
                emitted_shape_source_residual_reason(&declaration, source_style_verified).map(
                    |reason| {
                        crate::diagram_theme::SourceStyleResidual::from_declaration(
                            &declaration,
                            reason,
                        )
                    },
                )
            })
            .collect()
    }

    pub(super) fn emitted_label_source_residuals(
        &self,
        owner_id: &str,
        typography_verified: bool,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.emitted_label_source_residuals_with_prepared_typography(
            owner_id,
            typography_verified,
            Some(if typography_verified {
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeFacetReach::Verified
            } else {
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeFacetReach::Unverified
            }),
        )
    }

    fn emitted_label_source_residuals_with_prepared_typography(
        &self,
        owner_id: &str,
        typography_verified: bool,
        prepared_typography_reach: Option<
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeFacetReach,
        >,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.label_sources
            .iter()
            .filter_map(|source| {
                let declaration = source.bind(
                    owner_id,
                    crate::diagram_theme::SourceStyleChannel::Label,
                    false,
                );
                let verified = if matches!(declaration.property(), "font-family" | "font-size") {
                    let reach = prepared_typography_reach?;
                    reach.is_verified()
                } else {
                    typography_verified
                };
                label_source_residual_reason(&declaration, verified).map(|reason| {
                    crate::diagram_theme::SourceStyleResidual::from_declaration(
                        &declaration,
                        reason,
                    )
                })
            })
            .collect()
    }

    pub(super) fn emitted_html_label_source_residuals(
        &self,
        owner_id: &str,
        typography_verified: bool,
        sanitized_xhtml: &str,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        let mut residuals = self.emitted_label_source_residuals(owner_id, typography_verified);
        residuals.extend(sanitized_xhtml_source_residuals(owner_id, sanitized_xhtml));
        residuals
    }
}

fn sanitized_xhtml_source_residuals(
    owner_id: &str,
    sanitized_xhtml: &str,
) -> Vec<crate::diagram_theme::SourceStyleResidual> {
    if sanitized_xhtml.is_empty()
        || (!contains_ascii_case_insensitive(sanitized_xhtml, b"style")
            && !contains_ascii_case_insensitive(sanitized_xhtml, b"class"))
    {
        return Vec::new();
    }

    let mut reader = quick_xml::Reader::from_str(sanitized_xhtml);
    reader.config_mut().enable_all_checks(true);
    let mut residuals = Vec::new();
    let mut declaration_ordinal = 0usize;
    let mut depth = 0usize;

    loop {
        let decoder = reader.decoder();
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if inspect_sanitized_xhtml_element(
                    owner_id,
                    &element,
                    decoder,
                    &mut declaration_ordinal,
                    &mut residuals,
                )
                .is_err()
                {
                    residuals.push(invalid_sanitized_xhtml_residual(
                        owner_id,
                        declaration_ordinal,
                    ));
                    break;
                }
                depth = depth.saturating_add(1);
            }
            Ok(Event::Empty(element)) => {
                if inspect_sanitized_xhtml_element(
                    owner_id,
                    &element,
                    decoder,
                    &mut declaration_ordinal,
                    &mut residuals,
                )
                .is_err()
                {
                    residuals.push(invalid_sanitized_xhtml_residual(
                        owner_id,
                        declaration_ordinal,
                    ));
                    break;
                }
            }
            Ok(Event::End(_)) => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    residuals.push(invalid_sanitized_xhtml_residual(
                        owner_id,
                        declaration_ordinal,
                    ));
                    break;
                };
                depth = next_depth;
            }
            Ok(Event::Eof) => {
                if depth != 0 {
                    residuals.push(invalid_sanitized_xhtml_residual(
                        owner_id,
                        declaration_ordinal,
                    ));
                }
                break;
            }
            Ok(_) => {}
            Err(_) => {
                residuals.push(invalid_sanitized_xhtml_residual(
                    owner_id,
                    declaration_ordinal,
                ));
                break;
            }
        }
    }

    residuals
}

fn inspect_sanitized_xhtml_element(
    owner_id: &str,
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    declaration_ordinal: &mut usize,
    residuals: &mut Vec<crate::diagram_theme::SourceStyleResidual>,
) -> std::result::Result<(), ()> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        let name = attribute.key.local_name();
        let is_style = name.as_ref().eq_ignore_ascii_case(b"style");
        let is_class = name.as_ref().eq_ignore_ascii_case(b"class");
        if !is_style && !is_class {
            continue;
        }
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|_| ())?;
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        if is_style {
            for raw in crate::flowchart::flowchart_split_mermaid_style_decls(value) {
                let provenance = crate::diagram_theme::SourceStyleProvenance::label_style(
                    owner_id,
                    crate::diagram_theme::SourceStyleChannel::Label,
                    *declaration_ordinal,
                );
                *declaration_ordinal = declaration_ordinal.saturating_add(1);
                match crate::diagram_theme::SourceStyleDeclaration::parse(raw, provenance) {
                    Ok(declaration) => {
                        if let Some(reason) = label_source_residual_reason(&declaration, false) {
                            residuals.push(
                                crate::diagram_theme::SourceStyleResidual::from_declaration(
                                    &declaration,
                                    reason,
                                ),
                            );
                        }
                    }
                    Err(residual) => residuals.push(residual),
                }
            }
            continue;
        }

        let provenance = crate::diagram_theme::SourceStyleProvenance::label_style(
            owner_id,
            crate::diagram_theme::SourceStyleChannel::Label,
            *declaration_ordinal,
        );
        *declaration_ordinal = declaration_ordinal.saturating_add(1);
        let raw = synthetic_class_declaration(value);
        match crate::diagram_theme::SourceStyleDeclaration::parse(&raw, provenance) {
            Ok(declaration) => {
                residuals.push(crate::diagram_theme::SourceStyleResidual::from_declaration(
                    &declaration,
                    crate::diagram_theme::SourceStyleResidualReason::UnsupportedProperty,
                ))
            }
            Err(residual) => residuals.push(residual),
        }
    }
    Ok(())
}

fn invalid_sanitized_xhtml_residual(
    owner_id: &str,
    declaration_ordinal: usize,
) -> crate::diagram_theme::SourceStyleResidual {
    crate::diagram_theme::SourceStyleResidual::invalid(
        "sanitized-xhtml-fragment",
        crate::diagram_theme::SourceStyleProvenance::label_style(
            owner_id,
            crate::diagram_theme::SourceStyleChannel::Label,
            declaration_ordinal,
        ),
    )
}

fn synthetic_class_declaration(value: &str) -> String {
    let mut raw = String::with_capacity("class:\"\"".len() + value.len());
    raw.push_str("class:\"");
    for ch in value.chars() {
        match ch {
            '\\' => raw.push_str("\\\\"),
            '"' => raw.push_str("\\\""),
            '\n' | '\r' | '\0' => {
                let _ = write!(raw, "\\{:x} ", ch as u32);
            }
            _ => raw.push(ch),
        }
    }
    raw.push('"');
    raw
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &[u8]) -> bool {
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

pub(super) fn raw_emitted_edge_path_residuals(
    owner_id: &str,
    styles_a: &[String],
    styles_b: &[String],
    declaration_ordinal_base: usize,
) -> Vec<crate::diagram_theme::SourceStyleResidual> {
    let mut residuals = Vec::new();
    let mut declaration_ordinal = declaration_ordinal_base;
    for style in styles_a.iter().chain(styles_b.iter()) {
        let declarations =
            crate::flowchart::flowchart_split_mermaid_style_decls(style).collect::<Vec<_>>();
        for raw in declarations {
            let provenance = crate::diagram_theme::SourceStyleProvenance::inline(
                owner_id,
                crate::diagram_theme::SourceStyleChannel::Shape,
                declaration_ordinal,
            );
            declaration_ordinal += 1;
            if let Err(residual) =
                crate::diagram_theme::SourceStyleDeclaration::parse(raw, provenance)
            {
                residuals.push(residual);
            }
        }
    }
    residuals
}

fn emitted_shape_source<'a>(
    direct_source: &'a PendingSourceDeclaration,
    generated_source: Option<&'a PendingSourceDeclaration>,
    receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> (
    &'a PendingSourceDeclaration,
    crate::svg::parity::flowchart::render::node::emission::FlowchartNodeSourceFacetEmission,
) {
    let direct_emission = receipt.source_shape_emission(
        direct_source.provenance.is_inline(),
        direct_source
            .provenance
            .class_id()
            .is_some_and(wrapper_has_class),
    );
    if direct_source.provenance.is_inline()
        || !receipt.generated_selector_overrides_direct()
        || generated_source.is_none()
    {
        return (direct_source, direct_emission);
    }

    let generated_source = generated_source.expect("generated source checked above");
    (generated_source, receipt.source_shape_emission(false, true))
}

fn source_paint_evidence(
    direct_source: Option<&SourceFacetDeclaration>,
    generated_source: Option<&PendingSourceDeclaration>,
    owner_id: &str,
    receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
    fill: bool,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> (
    crate::flowchart::FlowchartSourceFacetStatus,
    Option<crate::diagram_theme::SourceStyleResidual>,
) {
    let Some(direct_source) = direct_source else {
        return (crate::flowchart::FlowchartSourceFacetStatus::Absent, None);
    };

    let direct_inline = direct_source.provenance.is_inline();
    let direct_wrapper_has_class = direct_source
        .provenance
        .class_id()
        .is_some_and(wrapper_has_class);
    let direct_emission = if fill {
        receipt.source_fill_emission(
            direct_source.admitted,
            direct_inline,
            direct_wrapper_has_class,
        )
    } else {
        receipt.source_stroke_emission(
            direct_source.admitted,
            direct_inline,
            direct_wrapper_has_class,
        )
    };
    if direct_inline
        || (direct_emission.reach().is_verified()
            && (!receipt.generated_selector_overrides_direct()
                || generated_source.is_some_and(|generated| {
                    generated.prepared.property() == direct_source.prepared.property()
                        && generated.prepared.value() == direct_source.prepared.value()
                })))
        || generated_source.is_none()
    {
        return direct_source.evidence(owner_id, direct_emission);
    }

    let generated_source = generated_source.expect("generated source checked above");
    let generated_paint = SourceFacetDeclaration {
        prepared: Arc::clone(&generated_source.prepared),
        provenance: generated_source.provenance.clone(),
        admitted: admitted_flowchart_source_paint(generated_source.prepared.value()),
    };
    let generated_emission = if fill {
        receipt.source_fill_emission(generated_paint.admitted, false, true)
    } else {
        receipt.source_stroke_emission(generated_paint.admitted, false, true)
    };
    generated_paint.evidence(owner_id, generated_emission)
}

fn source_stroke_width_evidence(
    direct_source: Option<&SourceFacetDeclaration>,
    generated_source: Option<&PendingSourceDeclaration>,
    owner_id: &str,
    receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> (
    crate::flowchart::FlowchartSourceFacetStatus,
    Option<crate::diagram_theme::SourceStyleResidual>,
) {
    let Some(direct_source) = direct_source else {
        return (crate::flowchart::FlowchartSourceFacetStatus::Absent, None);
    };

    let direct_inline = direct_source.provenance.is_inline();
    let direct_wrapper_has_class = direct_source
        .provenance
        .class_id()
        .is_some_and(wrapper_has_class);
    let direct_emission = receipt.source_stroke_width_emission(
        direct_source.admitted,
        direct_inline,
        direct_wrapper_has_class,
    );
    if direct_inline
        || (direct_emission.reach().is_verified()
            && (!receipt.stroke_width_selector_overrides_direct()
                || generated_source.is_some_and(|generated| {
                    generated.prepared.property() == direct_source.prepared.property()
                        && generated.prepared.value() == direct_source.prepared.value()
                })))
        || generated_source.is_none()
    {
        return direct_source.evidence(owner_id, direct_emission);
    }

    let generated_source = generated_source.expect("generated source checked above");
    let generated_width = SourceFacetDeclaration {
        prepared: Arc::clone(&generated_source.prepared),
        provenance: generated_source.provenance.clone(),
        admitted: admitted_flowchart_source_stroke_width(generated_source.prepared.value())
            .is_some(),
    };
    generated_width.evidence(
        owner_id,
        receipt.source_stroke_width_emission(generated_width.admitted, false, true),
    )
}

fn source_stroke_dasharray_evidence(
    direct_source: Option<&SourceFacetDeclaration>,
    generated_source: Option<&PendingSourceDeclaration>,
    owner_id: &str,
    receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> (
    crate::flowchart::FlowchartSourceFacetStatus,
    Option<crate::diagram_theme::SourceStyleResidual>,
) {
    let Some(direct_source) = direct_source else {
        return (crate::flowchart::FlowchartSourceFacetStatus::Absent, None);
    };

    let direct_inline = direct_source.provenance.is_inline();
    let direct_wrapper_has_class = direct_source
        .provenance
        .class_id()
        .is_some_and(wrapper_has_class);
    let direct_emission = receipt.source_stroke_dasharray_emission(
        direct_source.admitted,
        direct_inline,
        direct_wrapper_has_class,
    );
    if direct_inline
        || (direct_emission.reach().is_verified()
            && (!receipt.stroke_dasharray_selector_overrides_direct()
                || generated_source.is_some_and(|generated| {
                    generated.prepared.property() == direct_source.prepared.property()
                        && generated.prepared.value() == direct_source.prepared.value()
                })))
        || generated_source.is_none()
    {
        return direct_source.evidence(owner_id, direct_emission);
    }

    let generated_source = generated_source.expect("generated source checked above");
    let generated_dasharray = SourceFacetDeclaration {
        prepared: Arc::clone(&generated_source.prepared),
        provenance: generated_source.provenance.clone(),
        admitted: valid_stroke_dasharray(generated_source.prepared.value()),
    };
    generated_dasharray.evidence(
        owner_id,
        receipt.source_stroke_dasharray_emission(generated_dasharray.admitted, false, true),
    )
}

fn source_radius_evidence(
    direct_sources: &[SourceFacetDeclaration],
    generated_sources: &[PendingSourceDeclaration],
    owner_id: &str,
    receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> (
    crate::flowchart::FlowchartSourceFacetStatus,
    Vec<crate::diagram_theme::SourceStyleResidual>,
) {
    let mut status = crate::flowchart::FlowchartSourceFacetStatus::Absent;
    let mut residuals = Vec::new();
    for direct_source in direct_sources {
        let generated_source = generated_source_winner(
            generated_sources,
            direct_source.prepared.property(),
            wrapper_has_class,
        );
        let direct_inline = direct_source.provenance.is_inline();
        let direct_wrapper_has_class = direct_source
            .provenance
            .class_id()
            .is_some_and(wrapper_has_class);
        let direct_emission = receipt.source_radius_emission(
            direct_source.admitted,
            direct_inline,
            direct_wrapper_has_class,
        );
        let (component_status, residual) = if direct_inline
            || (direct_emission.reach().is_verified()
                && (!receipt.radius_selector_overrides_direct()
                    || generated_source.is_some_and(|generated| {
                        generated.prepared.property() == direct_source.prepared.property()
                            && generated.prepared.value() == direct_source.prepared.value()
                    })))
            || generated_source.is_none()
        {
            direct_source.evidence(owner_id, direct_emission)
        } else {
            let generated_source = generated_source.expect("generated source checked above");
            let generated_radius = SourceFacetDeclaration {
                prepared: Arc::clone(&generated_source.prepared),
                provenance: generated_source.provenance.clone(),
                admitted: admitted_flowchart_source_radius(generated_source.prepared.value()),
            };
            generated_radius.evidence(
                owner_id,
                receipt.source_radius_emission(generated_radius.admitted, false, true),
            )
        };
        status = merge_source_facet_status(status, component_status);
        residuals.extend(residual);
    }
    (status, residuals)
}

fn merge_source_facet_status(
    left: crate::flowchart::FlowchartSourceFacetStatus,
    right: crate::flowchart::FlowchartSourceFacetStatus,
) -> crate::flowchart::FlowchartSourceFacetStatus {
    use crate::flowchart::FlowchartSourceFacetStatus::{Absent, Admitted, Unverified};
    match (left, right) {
        (Unverified, _) | (_, Unverified) => Unverified,
        (Admitted, _) | (_, Admitted) => Admitted,
        (Absent, Absent) => Absent,
    }
}

fn generated_source_winner<'a>(
    sources: &'a [PendingSourceDeclaration],
    property: &str,
    wrapper_has_class: &impl Fn(&str) -> bool,
) -> Option<&'a PendingSourceDeclaration> {
    sources.iter().rev().find(|source| {
        source.prepared.property() == property
            && source.provenance.class_id().is_some_and(wrapper_has_class)
    })
}

fn shape_source_residual_reason(
    declaration: &crate::diagram_theme::SourceStyleDeclaration,
    reach: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeFacetReach,
) -> Option<crate::diagram_theme::SourceStyleResidualReason> {
    emitted_shape_source_residual_reason(declaration, reach.is_verified())
}

pub(super) fn emitted_shape_source_residual_reason(
    declaration: &crate::diagram_theme::SourceStyleDeclaration,
    source_style_verified: bool,
) -> Option<crate::diagram_theme::SourceStyleResidualReason> {
    use crate::diagram_theme::SourceStyleResidualReason;

    let valid = match declaration.property() {
        "fill" | "stroke" => admitted_flowchart_source_paint(declaration.value()),
        "stroke-width" => admitted_flowchart_source_stroke_width(declaration.value()).is_some(),
        "rx" | "ry" => admitted_flowchart_source_radius(declaration.value()),
        "stroke-dasharray" => valid_stroke_dasharray(declaration.value()),
        "opacity" | "fill-opacity" | "stroke-opacity" => declaration
            .value()
            .trim()
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && (0.0..=1.0).contains(&value)),
        "stroke-linecap" => matches!(
            declaration.value().trim().to_ascii_lowercase().as_str(),
            "butt" | "round" | "square"
        ),
        "stroke-linejoin" => matches!(
            declaration.value().trim().to_ascii_lowercase().as_str(),
            "arcs" | "bevel" | "miter" | "miter-clip" | "round"
        ),
        _ => return Some(SourceStyleResidualReason::UnsupportedProperty),
    };
    if !valid {
        Some(SourceStyleResidualReason::InvalidValue)
    } else if !source_style_verified {
        Some(SourceStyleResidualReason::UnsupportedSurface)
    } else {
        None
    }
}

fn label_source_residual_reason(
    declaration: &crate::diagram_theme::SourceStyleDeclaration,
    typography_verified: bool,
) -> Option<crate::diagram_theme::SourceStyleResidualReason> {
    use crate::diagram_theme::SourceStyleResidualReason;

    if declaration.property().starts_with("--") {
        return None;
    }
    if !portable_source_value(declaration.value()) {
        return Some(SourceStyleResidualReason::InvalidValue);
    }

    let valid = match declaration.property() {
        "font-family" => crate::mermaid_style::is_safe_css_font_family_value(declaration.value()),
        "font-size" => declaration
            .resolve_font_size_px(crate::mermaid_style::CssFontSizeContext::uniform(16.0))
            .is_some(),
        "font-weight" => {
            crate::mermaid_style::is_supported_css_font_weight_value(declaration.value())
        }
        "font-style" => {
            crate::mermaid_style::is_supported_css_font_style_value(declaration.value())
        }
        "color" => crate::mermaid_style::is_supported_css_color_value(declaration.value()),
        "text-decoration" => {
            crate::mermaid_style::is_supported_css_text_decoration_value(declaration.value())
        }
        "text-align" => {
            crate::mermaid_style::is_supported_css_text_align_value(declaration.value())
        }
        "line-height" => {
            crate::mermaid_style::is_supported_css_line_height_value(declaration.value())
        }
        "letter-spacing" | "word-spacing" => {
            crate::mermaid_style::is_supported_css_spacing_value(declaration.value())
        }
        "text-transform" | "text-shadow" | "text-overflow" | "white-space" | "word-wrap"
        | "word-break" | "overflow-wrap" | "hyphens" => {
            return Some(SourceStyleResidualReason::UnsupportedProperty);
        }
        _ => return Some(SourceStyleResidualReason::UnsupportedProperty),
    };
    if !valid {
        Some(SourceStyleResidualReason::InvalidValue)
    } else if !typography_verified {
        Some(SourceStyleResidualReason::UnsupportedSurface)
    } else {
        None
    }
}

fn portable_source_value(value: &str) -> bool {
    let value = value.trim();
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "currentcolor" | "inherit" | "initial" | "revert" | "revert-layer" | "unset"
    ) && crate::mermaid_style::is_resource_free_css_value(value)
}

fn valid_stroke_dasharray(value: &str) -> bool {
    use cssparser::{BasicParseErrorKind, Parser, ParserInput, Token};

    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return true;
    }
    if !portable_source_value(value) {
        return false;
    }

    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let mut count = 0usize;
    let mut separator_seen = true;
    let mut comma_pending = false;
    loop {
        let token = match parser.next_including_whitespace() {
            Ok(token) => token,
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => break,
            Err(_) => return false,
        };
        match token {
            Token::WhiteSpace(_) => {
                if count != 0 {
                    separator_seen = true;
                }
            }
            Token::Comma => {
                if count == 0 || comma_pending {
                    return false;
                }
                separator_seen = true;
                comma_pending = true;
            }
            Token::Number { value, .. }
            | Token::Percentage {
                unit_value: value, ..
            } if value.is_finite() && *value >= 0.0 => {
                if count != 0 && !separator_seen {
                    return false;
                }
                count += 1;
                separator_seen = false;
                comma_pending = false;
            }
            Token::Dimension { value, unit, .. }
                if value.is_finite()
                    && *value >= 0.0
                    && matches!(
                        unit.to_ascii_lowercase().as_str(),
                        "px" | "em" | "rem" | "cm" | "mm" | "q" | "in" | "pc" | "pt"
                    ) =>
            {
                if count != 0 && !separator_seen {
                    return false;
                }
                count += 1;
                separator_seen = false;
                comma_pending = false;
            }
            _ => return false,
        }
    }

    count != 0 && !comma_pending
}

pub(in crate::svg::parity) fn flowchart_compile_styles(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
) -> FlowchartCompiledStyles {
    // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
    // - preserve insertion order of the first occurrence of a key
    // - later occurrences override values, without changing order
    struct OrderedDeclaration {
        prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
        provenance: PendingSourceProvenance,
    }

    #[derive(Default)]
    struct SemanticMap {
        order: Vec<OrderedDeclaration>,
        idx: FxHashMap<String, usize>,
    }
    impl SemanticMap {
        fn set(
            &mut self,
            prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
            provenance: PendingSourceProvenance,
        ) {
            let property = prepared.property().to_string();
            if let Some(&index) = self.idx.get(&property) {
                self.order[index].prepared = prepared;
                self.order[index].provenance = provenance;
                return;
            }
            self.idx.insert(property.clone(), self.order.len());
            self.order.push(OrderedDeclaration {
                prepared,
                provenance,
            });
        }
    }

    #[derive(Default)]
    struct EmissionMap {
        order: Vec<Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>>,
        idx: FxHashMap<String, usize>,
    }

    impl EmissionMap {
        fn set(&mut self, prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>) {
            let property = prepared.property_css().to_string();
            if let Some(&index) = self.idx.get(&property) {
                self.order[index] = prepared;
                return;
            }
            self.idx.insert(property, self.order.len());
            self.order.push(prepared);
        }
    }

    let mut semantic = SemanticMap::default();
    let mut emission = EmissionMap::default();

    let mut declaration_ordinal = 0;
    for (assignment_ordinal, c) in classes.iter().enumerate() {
        let Some(decls) = class_defs.get(c) else {
            continue;
        };
        for d in decls {
            for d in crate::flowchart::flowchart_split_mermaid_style_decls(d) {
                let ordinal = declaration_ordinal;
                declaration_ordinal += 1;
                let Some(declaration) =
                    crate::diagram_theme::PreparedSourceStyleDeclaration::parse(d)
                else {
                    continue;
                };
                let prepared = Arc::new(declaration);
                emission.set(Arc::clone(&prepared));
                semantic.set(
                    prepared,
                    PendingSourceProvenance::AssignedClass {
                        class_id: c.clone(),
                        assignment_ordinal,
                        declaration_ordinal: ordinal,
                    },
                );
            }
        }
    }

    for d in inline_styles_a.iter().chain(inline_styles_b.iter()) {
        for d in crate::flowchart::flowchart_split_mermaid_style_decls(d) {
            let ordinal = declaration_ordinal;
            declaration_ordinal += 1;
            let Some(declaration) = crate::diagram_theme::PreparedSourceStyleDeclaration::parse(d)
            else {
                continue;
            };
            let prepared = Arc::new(declaration);
            emission.set(Arc::clone(&prepared));
            semantic.set(
                prepared,
                PendingSourceProvenance::Inline {
                    declaration_ordinal: ordinal,
                },
            );
        }
    }

    let mut node_style = String::new();
    let mut label_style = String::new();

    let mut label_div_decls: Vec<(String, String)> = Vec::new();

    let mut fill: Option<String> = None;
    let mut fill_source = None;
    let mut stroke: Option<String> = None;
    let mut stroke_source = None;
    let mut stroke_width: Option<String> = None;
    let mut stroke_width_source = None;
    let mut radius_sources = Vec::new();
    let mut stroke_dasharray: Option<String> = None;
    let mut stroke_dasharray_source = None;
    let mut font_stack_source = None;
    let mut font_size_source = None;
    let mut shape_sources = Vec::new();
    let mut generated_shape_sources = Vec::new();
    let mut label_sources = Vec::new();

    let mut css_classes = classes
        .iter()
        .filter_map(|class_id| {
            class_defs
                .get_index_of(class_id)
                .map(|index| (index, class_id))
        })
        .collect::<Vec<_>>();
    css_classes.sort_unstable_by_key(|(index, _)| *index);
    css_classes.dedup_by_key(|(index, _)| *index);
    let mut css_declaration_ordinal = 0;
    for (assignment_ordinal, class_id) in css_classes {
        let Some(declarations) = class_defs.get(class_id) else {
            continue;
        };
        for raw in declarations {
            for raw in crate::flowchart::flowchart_split_mermaid_style_decls(raw) {
                let declaration_ordinal = css_declaration_ordinal;
                css_declaration_ordinal += 1;
                let Some(prepared) =
                    crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw)
                else {
                    continue;
                };
                if crate::flowchart::flowchart_is_source_spelled_label_style_key(
                    prepared.property_css(),
                ) {
                    continue;
                }
                generated_shape_sources.push(PendingSourceDeclaration {
                    prepared: Arc::new(prepared),
                    provenance: PendingSourceProvenance::AssignedClass {
                        class_id: class_id.clone(),
                        assignment_ordinal,
                        declaration_ordinal,
                    },
                });
            }
        }
    }

    for declaration in &emission.order {
        let property_css = declaration.property_css();
        let v = declaration.source_value();
        if crate::flowchart::flowchart_is_source_spelled_label_style_key(property_css) {
            if !label_style.is_empty() {
                label_style.push(';');
            }
            let _ = write!(&mut label_style, "{property_css}:{v} !important");
            label_div_decls.push((property_css.to_string(), v.to_string()));
        } else {
            if !node_style.is_empty() {
                node_style.push(';');
            }
            let _ = write!(&mut node_style, "{property_css}:{v} !important");
        }
    }

    for declaration in &semantic.order {
        let k = declaration.prepared.property();
        let v = declaration.prepared.value();
        let source_label_style = crate::flowchart::flowchart_is_source_spelled_label_style_key(
            declaration.prepared.property_css(),
        );
        if source_label_style {
            label_sources.push(PendingSourceDeclaration {
                prepared: Arc::clone(&declaration.prepared),
                provenance: declaration.provenance.clone(),
            });
        } else {
            shape_sources.push(PendingSourceDeclaration {
                prepared: Arc::clone(&declaration.prepared),
                provenance: declaration.provenance.clone(),
            });
        }
        match k {
            "fill" => {
                let admitted = admitted_flowchart_source_paint(v);
                fill = Some(v.to_string());
                fill_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted,
                });
            }
            "stroke" => {
                let admitted = admitted_flowchart_source_paint(v);
                stroke = Some(v.to_string());
                stroke_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted,
                });
            }
            "stroke-width" => {
                stroke_width = Some(v.to_string());
                stroke_width_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted: admitted_flowchart_source_stroke_width(v).is_some(),
                });
            }
            "rx" | "ry" => radius_sources.push(SourceFacetDeclaration {
                prepared: Arc::clone(&declaration.prepared),
                provenance: declaration.provenance.clone(),
                admitted: admitted_flowchart_source_radius(v),
            }),
            "stroke-dasharray" => {
                stroke_dasharray = Some(v.to_string());
                stroke_dasharray_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted: valid_stroke_dasharray(v),
                });
            }
            "font-family" if source_label_style => {
                font_stack_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted: crate::text::parse_css_font_stack(v).is_some(),
                });
            }
            "font-size" if source_label_style => {
                font_size_source = Some(SourceFacetDeclaration {
                    prepared: Arc::clone(&declaration.prepared),
                    provenance: declaration.provenance.clone(),
                    admitted: declaration
                        .prepared
                        .resolve_font_size_px(crate::mermaid_style::CssFontSizeContext::uniform(
                            16.0,
                        ))
                        .is_some(),
                });
            }
            _ => {}
        }
    }

    FlowchartCompiledStyles {
        node_style,
        label_style,
        label_div_decls,
        fill,
        fill_source,
        stroke,
        stroke_source,
        stroke_width,
        stroke_width_source,
        radius_sources,
        stroke_dasharray,
        stroke_dasharray_source,
        font_stack_source,
        font_size_source,
        shape_sources,
        generated_shape_sources,
        label_sources,
    }
}

fn admitted_flowchart_source_paint(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("none")
        || merman_core::theme_color::ThemeColor::parse(value.trim()).is_ok()
}

fn admitted_flowchart_source_stroke_width(value: &str) -> Option<f32> {
    let value = crate::mermaid_style::parse_svg_number_or_px(value)?;
    (value <= f32::MAX as f64).then_some(value as f32)
}

fn admitted_flowchart_source_radius(value: &str) -> bool {
    crate::mermaid_style::parse_svg_number_or_px(value).is_some_and(|value| value >= 0.0)
}

pub(in crate::svg::parity) fn flowchart_compile_node_styles(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
) -> FlowchartCompiledStyles {
    let effective_classes =
        crate::flowchart::flowchart_effective_node_class_names(class_defs, classes)
            .into_iter()
            .map(|class| class.to_string())
            .collect::<Vec<_>>();
    flowchart_compile_styles(
        class_defs,
        &effective_classes,
        inline_styles_a,
        inline_styles_b,
    )
}

pub(in crate::svg::parity) fn flowchart_label_div_style_prefix(
    styles: &FlowchartCompiledStyles,
    color_as_rgb: bool,
) -> String {
    fn div_style_survives_mermaid_overrides(key: &str) -> bool {
        !matches!(key, "line-height" | "text-align" | "white-space")
    }

    let mut out = String::new();
    for (key, value) in &styles.label_div_decls {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() || !div_style_survives_mermaid_overrides(key) {
            continue;
        }
        if key == "color" {
            if color_as_rgb {
                let color = super::super::util::cssom_color_value(value);
                let _ = write!(&mut out, "color: {color} !important; ");
            } else {
                let _ = write!(
                    &mut out,
                    "color: {} !important; ",
                    value.to_ascii_lowercase()
                );
            }
        } else {
            let _ = write!(&mut out, "{key}: {value} !important; ");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_style(value: &str) -> FlowchartCompiledStyles {
        FlowchartCompiledStyles {
            node_style: String::new(),
            label_style: String::new(),
            label_div_decls: vec![("color".to_string(), value.to_string())],
            fill: None,
            fill_source: None,
            stroke: None,
            stroke_source: None,
            stroke_width: None,
            stroke_width_source: None,
            radius_sources: Vec::new(),
            stroke_dasharray: None,
            stroke_dasharray_source: None,
            font_stack_source: None,
            font_size_source: None,
            shape_sources: Vec::new(),
            generated_shape_sources: Vec::new(),
            label_sources: Vec::new(),
        }
    }

    #[test]
    fn flowchart_html_label_color_uses_the_shared_cssom_boundary() {
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("#12345680"), true),
            "color: rgba(18, 52, 86, 0.502) !important; "
        );
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("hsl(210 50% 40%)"), true),
            "color: rgb(51, 102, 153) !important; "
        );
        assert_eq!(
            flowchart_label_div_style_prefix(&color_style("var(--LabelColor)"), true),
            "color: var(--LabelColor) !important; "
        );
    }

    #[test]
    fn flowchart_shape_style_preserves_source_keys_with_a_canonical_semantic_winner() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"FILL:#ef4444".to_string(), r"stroke:#111827".to_string()],
            &[r"f\69ll:#22c55e".to_string()],
        );

        assert_eq!(styles.fill.as_deref(), Some("#22c55e"));
        assert_eq!(
            styles.node_style,
            r"FILL:#ef4444 !important;stroke:#111827 !important;f\69ll:#22c55e !important"
        );
    }

    #[test]
    fn source_spelling_controls_label_typography_ownership() {
        let class_defs =
            IndexMap::from([("local".to_string(), vec![r"f\6f nt-size:22px".to_string()])]);
        let styles = flowchart_compile_styles(&class_defs, &["local".to_string()], &[], &[]);

        assert_eq!(styles.node_style, r"f\6f nt-size:22px !important");
        assert!(styles.label_style.is_empty());
        assert_eq!(
            styles.source_font_size_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert!(styles.label_sources.is_empty());
        assert_eq!(styles.shape_sources.len(), 1);
        assert_eq!(styles.generated_shape_sources.len(), 1);
    }

    #[test]
    fn unadmitted_source_paint_is_preserved_but_not_treated_as_portable() {
        for value in ["red junk", "var(--paint)", "inherit", "currentColor"] {
            let styles =
                flowchart_compile_styles(&IndexMap::new(), &[], &[format!("fill:{value}")], &[]);

            assert_eq!(styles.fill.as_deref(), Some(value));
            assert_eq!(
                styles.source_fill_status(),
                crate::flowchart::FlowchartSourceFacetStatus::Unverified
            );
            assert!(
                styles
                    .node_style
                    .contains(&format!("fill:{value} !important"))
            );
            let evidence = styles.shape_source_evidence(
                "A",
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
                |_| false,
            );
            assert_eq!(evidence.residuals.len(), 1);
            assert_eq!(evidence.residuals[0].property(), Some("fill"));
            assert_eq!(evidence.residuals[0].provenance().owner_id(), "A");
            assert_eq!(
                evidence.residuals[0].provenance().origin(),
                crate::diagram_theme::SourceStyleOrigin::InlineStyle
            );
            assert_eq!(
                evidence.residuals[0].reason(),
                crate::diagram_theme::SourceStyleResidualReason::InvalidValue
            );
        }
    }

    #[test]
    fn escaped_custom_property_keeps_its_css_spelling() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"--brand\:accent:#22c55e".to_string()],
            &[],
        );

        assert!(
            styles
                .node_style
                .contains(r"--brand\:accent:#22c55e !important")
        );
        assert!(!styles.node_style.contains("--brand:accent:"));
    }

    #[test]
    fn generated_shape_evidence_uses_class_definition_order() {
        let mut class_defs = IndexMap::new();
        class_defs.insert("z".to_string(), vec!["opacity:0.5".to_string()]);
        class_defs.insert("a".to_string(), vec!["opacity:var(--alpha)".to_string()]);
        let styles =
            flowchart_compile_styles(&class_defs, &["a".to_string(), "z".to_string()], &[], &[]);

        let evidence = styles.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::unverified(),
            |_| true,
        );

        assert_eq!(evidence.residuals.len(), 1);
        assert_eq!(evidence.residuals[0].provenance().owner_id(), "a");
        assert_eq!(evidence.residuals[0].property(), Some("opacity"));
        assert_eq!(
            evidence.residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::InvalidValue
        );
        assert_eq!(
            evidence.residuals[0].provenance().origin(),
            crate::diagram_theme::SourceStyleOrigin::GeneratedClassCss
        );
    }

    #[test]
    fn shape_source_residuals_cover_supported_and_unsupported_properties() {
        for (declaration, expected_reason) in [
            (
                "filter:blur(2px)",
                Some(crate::diagram_theme::SourceStyleResidualReason::UnsupportedProperty),
            ),
            (
                "transform:translateX(1px)",
                Some(crate::diagram_theme::SourceStyleResidualReason::UnsupportedProperty),
            ),
            (
                "opacity:2",
                Some(crate::diagram_theme::SourceStyleResidualReason::InvalidValue),
            ),
            (
                "stroke-dasharray:banana",
                Some(crate::diagram_theme::SourceStyleResidualReason::InvalidValue),
            ),
            ("stroke-width:2px", None),
            ("stroke-dasharray:4 2px,3%", None),
            ("opacity:0.5", None),
        ] {
            let styles =
                flowchart_compile_styles(&IndexMap::new(), &[], &[declaration.to_string()], &[]);
            let evidence = styles.shape_source_evidence(
                "A",
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
                |_| false,
            );

            assert_eq!(
                evidence.residuals.first().map(|residual| residual.reason()),
                expected_reason,
                "declaration={declaration} residuals={:?}",
                evidence.residuals
            );
        }
    }

    #[test]
    fn source_stroke_width_admission_is_bound_to_the_writer_receipt() {
        let verified = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["stroke-width:2.5px".to_string()],
            &[],
        );
        assert_eq!(
            verified.source_stroke_width_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert_eq!(verified.admitted_stroke_width_value(), Some(2.5));
        let evidence = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
            |_| false,
        );
        assert_eq!(
            evidence.stroke_width,
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(evidence.residuals.is_empty());

        let unsupported_surface = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::unverified(),
            |_| false,
        );
        assert_eq!(
            unsupported_surface.stroke_width,
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert_eq!(unsupported_surface.residuals.len(), 1);
        assert_eq!(
            unsupported_surface.residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        );

        let dynamic = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["stroke-width:var(--width)".to_string()],
            &[],
        );
        assert_eq!(
            dynamic.source_stroke_width_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Unverified
        );
        assert_eq!(dynamic.admitted_stroke_width_value(), None);
        let evidence = dynamic.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
            |_| false,
        );
        assert_eq!(evidence.residuals.len(), 1);
        assert_eq!(evidence.residuals[0].property(), Some("stroke-width"));
        assert_eq!(
            evidence.residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::InvalidValue
        );
    }

    #[test]
    fn source_stroke_dasharray_admission_is_bound_to_the_writer_receipt() {
        let verified = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["stroke-dasharray:4 2px 3%".to_string()],
            &[],
        );
        assert_eq!(
            verified.source_stroke_dasharray_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        let evidence = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
            |_| false,
        );
        assert_eq!(
            evidence.stroke_dasharray,
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(evidence.residuals.is_empty());

        let unsupported_surface = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::unverified(),
            |_| false,
        );
        assert_eq!(
            unsupported_surface.stroke_dasharray,
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert_eq!(unsupported_surface.residuals.len(), 1);
        assert_eq!(
            unsupported_surface.residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        );

        let dynamic = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["stroke-dasharray:var(--dash)".to_string()],
            &[],
        );
        assert_eq!(
            dynamic.source_stroke_dasharray_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Unverified
        );
        let evidence = dynamic.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
            |_| false,
        );
        assert_eq!(evidence.residuals.len(), 1);
        assert_eq!(evidence.residuals[0].property(), Some("stroke-dasharray"));
        assert_eq!(
            evidence.residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::InvalidValue
        );
    }

    #[test]
    fn source_radius_admission_is_bound_to_the_process_writer_receipt() {
        let verified = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["rx:4px".to_string(), "ry:6".to_string()],
            &[],
        );
        assert_eq!(
            verified.source_radius_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(verified.node_style.contains("rx:4px !important"));
        assert!(verified.node_style.contains("ry:6 !important"));

        let evidence = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
            |_| false,
        );
        assert_eq!(
            evidence.radius,
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(evidence.residuals.is_empty());

        let unsupported_surface = verified.shape_source_evidence(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::unverified(),
            |_| false,
        );
        assert_eq!(
            unsupported_surface.radius,
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert_eq!(unsupported_surface.residuals.len(), 2);
        assert!(unsupported_surface.residuals.iter().all(|residual| {
            residual.reason() == crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        }));

        for declaration in ["rx:var(--radius)", "ry:-1px"] {
            let dynamic =
                flowchart_compile_styles(&IndexMap::new(), &[], &[declaration.to_string()], &[]);
            assert_eq!(
                dynamic.source_radius_status(),
                crate::flowchart::FlowchartSourceFacetStatus::Unverified
            );
            let evidence = dynamic.shape_source_evidence(
                "A",
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::classic_process(false),
                |_| false,
            );
            assert_eq!(
                evidence.radius,
                crate::flowchart::FlowchartSourceFacetStatus::Unverified
            );
            assert_eq!(evidence.residuals.len(), 1);
            assert_eq!(
                evidence.residuals[0].reason(),
                crate::diagram_theme::SourceStyleResidualReason::InvalidValue
            );
        }
    }

    #[test]
    fn label_source_residuals_reject_unmodeled_css_without_rejecting_admitted_fonts() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[
                "font-family:Arial".to_string(),
                "text-shadow:0 1px 2px #000".to_string(),
                "white-space:nowrap".to_string(),
            ],
            &[],
        );

        let residuals = styles.label_source_residuals(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(true, true),
        );
        assert_eq!(residuals.len(), 2);
        assert!(residuals.iter().all(|residual| {
            residual.reason()
                == crate::diagram_theme::SourceStyleResidualReason::UnsupportedProperty
        }));
        assert!(
            residuals
                .iter()
                .all(|residual| residual.property() != Some("font-family"))
        );
    }

    #[test]
    fn label_source_residuals_validate_supported_property_grammars() {
        for declaration in [
            "font-weight:banana",
            "font-weight:inherit",
            "font-style:sideways",
            "color:banana",
            "color:currentColor",
            "text-decoration:banana",
            "text-align:banana",
            "line-height:banana",
            "letter-spacing:banana",
            "word-spacing:banana",
        ] {
            let styles =
                flowchart_compile_styles(&IndexMap::new(), &[], &[declaration.to_string()], &[]);
            let residuals = styles.label_source_residuals(
                "A",
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified(),
            );
            assert_eq!(residuals.len(), 1, "declaration={declaration}");
            assert_eq!(
                residuals[0].reason(),
                crate::diagram_theme::SourceStyleResidualReason::InvalidValue,
                "declaration={declaration}"
            );
        }

        for declaration in [
            "font-weight:700",
            "font-style:italic",
            "color:#123456",
            "text-decoration:underline",
            "text-align:center",
            "line-height:1.5",
            "letter-spacing:-0.5px",
            "word-spacing:normal",
        ] {
            let styles =
                flowchart_compile_styles(&IndexMap::new(), &[], &[declaration.to_string()], &[]);
            assert!(
                styles
                    .label_source_residuals(
                        "A",
                        crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified(),
                    )
                    .is_empty(),
                "declaration={declaration}"
            );
        }
    }

    #[test]
    fn admitted_label_typography_requires_a_verified_emission_receipt() {
        let styles =
            flowchart_compile_styles(&IndexMap::new(), &[], &["font-size:28px".to_string()], &[]);

        let residuals = styles.label_source_residuals(
            "A",
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::unverified()
                .with_prepared_typography_reach(true, false),
        );
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].property(), Some("font-size"));
        assert_eq!(
            residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        );
    }

    #[test]
    fn admitted_font_family_requires_a_consumed_prepared_writer_receipt() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &["font-family:Excalifont".to_string()],
            &[],
        );
        let unverified =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(true, false);

        assert_eq!(
            styles.emitted_source_font_stack_status(unverified),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        let residuals = styles.label_source_residuals("A", unverified);
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].property(), Some("font-family"));
        assert_eq!(
            residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        );

        let verified =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(true, true);
        assert_eq!(
            styles.emitted_source_font_stack_status(verified),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(styles.label_source_residuals("A", verified).is_empty());

        let not_applicable =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(false, false);
        assert_eq!(
            styles.emitted_source_font_stack_status(not_applicable),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert!(
            styles
                .label_source_residuals("A", not_applicable)
                .is_empty()
        );
    }

    #[test]
    fn admitted_font_size_requires_a_consumed_prepared_writer_receipt() {
        let styles =
            flowchart_compile_styles(&IndexMap::new(), &[], &["font-size:26px".to_string()], &[]);
        let unverified =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(true, false);

        assert_eq!(
            styles.emitted_source_font_size_status(unverified),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        let residuals = styles.label_source_residuals("A", unverified);
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].property(), Some("font-size"));
        assert_eq!(
            residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::UnsupportedSurface
        );

        let verified =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(true, true);
        assert_eq!(
            styles.emitted_source_font_size_status(verified),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(styles.label_source_residuals("A", verified).is_empty());

        let not_applicable =
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
                .with_prepared_typography_reach(false, false);
        assert_eq!(
            styles.emitted_source_font_size_status(not_applicable),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert!(
            styles
                .label_source_residuals("A", not_applicable)
                .is_empty()
        );
    }
}
