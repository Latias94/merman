//! Flowchart style compilation helpers.

use super::*;
use cssparser::{BasicParseErrorKind, Delimiter, Parser, ParserInput, Token};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartCompiledStyles {
    edge_class_declarations: Vec<Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>>,
    edge_animation_active: Option<bool>,
    edge_class_shape_sources: Vec<PendingSourceDeclaration>,
    pub(super) node_style: String,
    pub(super) label_style: String,
    pub(super) label_div_decls: Vec<(String, String)>,
    pub(super) fill: Option<String>,
    fill_source: Option<SourceFacetDeclaration>,
    pub(super) stroke: Option<String>,
    stroke_source: Option<SourceFacetDeclaration>,
    inline_stroke_source: Option<SourceFacetDeclaration>,
    pub(super) stroke_width: Option<String>,
    stroke_width_source: Option<SourceFacetDeclaration>,
    radius_sources: Vec<SourceFacetDeclaration>,
    pub(super) stroke_dasharray: Option<String>,
    stroke_dasharray_source: Option<SourceFacetDeclaration>,
    inline_stroke_dasharray_status: crate::flowchart::FlowchartSourceFacetStatus,
    font_stack_source: Option<SourceFacetDeclaration>,
    font_size_source: Option<SourceFacetDeclaration>,
    shape_sources: Vec<PendingSourceDeclaration>,
    inline_shape_sources: Vec<PendingSourceDeclaration>,
    generated_shape_sources: Vec<PendingSourceDeclaration>,
    label_sources: Vec<PendingSourceDeclaration>,
    label_foreground_status: crate::flowchart::FlowchartSourceFacetStatus,
    invalid_sources: Vec<PendingInvalidSourceDeclaration>,
}

#[derive(Debug, Clone)]
enum PendingSourceProvenance {
    AssignedClass {
        class_id: Arc<str>,
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
                Arc::clone(class_id),
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
        if generated_class_css
            && let Self::AssignedClass {
                class_id,
                declaration_ordinal,
                ..
            } = self
        {
            return crate::diagram_theme::SourceStyleProvenance::generated_class_css(
                Arc::clone(class_id),
                channel,
                *declaration_ordinal,
            );
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

    fn terminal_foreground_provenance(
        &self,
    ) -> crate::flowchart::FlowchartTerminalForegroundProvenance {
        match self {
            Self::AssignedClass { .. } => {
                crate::flowchart::FlowchartTerminalForegroundProvenance::AssignedClass
            }
            Self::Inline { .. } => {
                crate::flowchart::FlowchartTerminalForegroundProvenance::InlineStyle
            }
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

#[derive(Debug, Clone)]
struct PendingInvalidSourceDeclaration {
    raw: Arc<str>,
    provenance: PendingSourceProvenance,
    channel: crate::diagram_theme::SourceStyleChannel,
    label_foreground: bool,
}

impl PendingInvalidSourceDeclaration {
    fn bind(&self, owner_id: &str) -> crate::diagram_theme::SourceStyleResidual {
        crate::diagram_theme::SourceStyleResidual::invalid_prepared(
            Arc::clone(&self.raw),
            self.provenance.bind(owner_id, self.channel),
        )
    }
}

#[derive(Debug, Clone)]
enum PreparedClassDeclaration {
    Valid(Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>),
    Invalid {
        raw: Arc<str>,
        channel: crate::diagram_theme::SourceStyleChannel,
        label_foreground: bool,
    },
}

#[derive(Debug, Clone, Default)]
pub(in crate::svg::parity::flowchart) struct FlowchartPreparedClassStyles {
    classes: IndexMap<Arc<str>, Vec<PreparedClassDeclaration>>,
    #[cfg(test)]
    parsed_declaration_count: usize,
}

impl FlowchartPreparedClassStyles {
    pub(in crate::svg::parity::flowchart) fn prepare<'a>(
        class_defs: &IndexMap<String, Vec<String>>,
        class_ids: impl IntoIterator<Item = &'a str>,
        work_meter: Option<&crate::resources::OperationWorkMeter>,
    ) -> std::result::Result<Self, crate::resources::OperationWorkError> {
        let mut selected = FxHashSet::default();
        for class_id in class_ids {
            charge_style_work(work_meter, 1)?;
            selected.insert(class_id);
        }

        let mut classes = IndexMap::new();
        #[cfg(test)]
        let mut parsed_declaration_count = 0usize;
        for (class_id, declarations) in class_defs {
            if !selected.contains(class_id.as_str()) {
                continue;
            }
            let mut prepared = Vec::new();
            for raw_group in declarations {
                charge_style_scan(work_meter, raw_group.len())?;
                for raw in crate::flowchart::flowchart_split_mermaid_style_decls(raw_group) {
                    charge_style_work(work_meter, 1)?;
                    #[cfg(test)]
                    {
                        parsed_declaration_count = parsed_declaration_count.saturating_add(1);
                    }
                    let declaration =
                        crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw)
                            .map(Arc::new)
                            .map(PreparedClassDeclaration::Valid)
                            .unwrap_or_else(|| {
                                let (channel, label_foreground) =
                                    classify_invalid_source_style(raw);
                                PreparedClassDeclaration::Invalid {
                                    raw: Arc::from(raw.trim()),
                                    channel,
                                    label_foreground,
                                }
                            });
                    prepared.push(declaration);
                }
            }
            classes.insert(Arc::from(class_id.as_str()), prepared);
        }
        Ok(Self {
            classes,
            #[cfg(test)]
            parsed_declaration_count,
        })
    }

    fn get_index_of(&self, class_id: &str) -> Option<usize> {
        self.classes.get_index_of(class_id)
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) const fn parsed_declaration_count(&self) -> usize {
        self.parsed_declaration_count
    }
}

fn charge_style_scan(
    work_meter: Option<&crate::resources::OperationWorkMeter>,
    bytes: usize,
) -> std::result::Result<(), crate::resources::OperationWorkError> {
    let chunks = bytes.saturating_add(63) / 64;
    charge_style_work(work_meter, chunks.saturating_add(1))
}

fn charge_style_work(
    work_meter: Option<&crate::resources::OperationWorkMeter>,
    units: usize,
) -> std::result::Result<(), crate::resources::OperationWorkError> {
    if let Some(work_meter) = work_meter {
        work_meter.charge(units)?;
    }
    Ok(())
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
    fn invalid_source_residuals(
        &self,
        owner_id: &str,
        channel: crate::diagram_theme::SourceStyleChannel,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        self.invalid_sources
            .iter()
            .filter(|source| source.channel == channel)
            .map(|source| source.bind(owner_id))
            .collect()
    }

    pub(super) fn emitted_edge_class_declarations(
        &self,
    ) -> &[Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>] {
        &self.edge_class_declarations
    }

    pub(super) const fn edge_animation_active(&self) -> Option<bool> {
        self.edge_animation_active
    }

    pub(super) fn edge_marker_color(&self, hand_drawn: bool) -> Option<&str> {
        let source = if hand_drawn {
            self.inline_stroke_source.as_ref()?
        } else {
            self.stroke_source.as_ref()?
        };
        Some(if hand_drawn {
            source.prepared.raw()
        } else {
            marker_source_value(&source.prepared)
        })
    }

    pub(super) fn projected_edge_source_style_bytes(
        &self,
        default_edge_style: &[String],
        edge_style: &[String],
        hand_drawn: bool,
    ) -> Option<usize> {
        let joined_inline = projected_joined_style_bytes(default_edge_style, edge_style)?;
        if hand_drawn {
            return Some(joined_inline);
        }

        let class_bytes = self.edge_class_declarations.iter().enumerate().try_fold(
            0usize,
            |bytes, (index, declaration)| {
                let declaration_bytes = usize::from(index != 0)
                    .checked_add(super::super::util::escaped_xml_len(
                        declaration.property_css(),
                    ))?
                    .checked_add(1)?
                    .checked_add(super::super::util::escaped_xml_len(
                        declaration.source_value(),
                    ))?;
                bytes.checked_add(declaration_bytes)
            },
        )?;
        let class_separator = usize::from(!self.edge_class_declarations.is_empty());
        let inline_bytes = if default_edge_style.is_empty() && edge_style.is_empty() {
            1
        } else {
            joined_inline.checked_mul(2)?.checked_add(3)?
        };
        class_bytes
            .checked_add(class_separator)?
            .checked_add(inline_bytes)
    }

    /// Drops node-only compiled payload while preserving every path, marker, label, and evidence
    /// input consumed by Flowchart edge rendering.
    pub(in crate::svg::parity::flowchart) fn into_edge_artifact(
        mut self,
        hand_drawn: bool,
    ) -> Self {
        self.node_style = String::new();
        self.fill = None;
        self.fill_source = None;
        self.stroke = None;
        self.stroke_width = None;
        self.stroke_width_source = None;
        self.radius_sources = Vec::new();
        self.stroke_dasharray = None;
        self.generated_shape_sources = Vec::new();

        if hand_drawn {
            // Hand-drawn edges emit inline shape declarations directly and report assigned-class
            // declarations as unsupported. The semantic winner list supplies the latter evidence.
            self.edge_class_shape_sources = Vec::new();
        }
        self
    }

    /// Swimlane edge-label nodes consume label typography plus shape residuals, but never edge
    /// path declarations or node shape facets.
    pub(in crate::svg::parity::flowchart) fn into_swimlane_edge_label_artifact(mut self) -> Self {
        self.edge_class_declarations = Vec::new();
        self.edge_class_shape_sources = Vec::new();
        self.node_style = String::new();
        self.fill = None;
        self.fill_source = None;
        self.stroke = None;
        self.stroke_source = None;
        self.inline_stroke_source = None;
        self.stroke_width = None;
        self.stroke_width_source = None;
        self.radius_sources = Vec::new();
        self.stroke_dasharray = None;
        self.stroke_dasharray_source = None;
        self.inline_stroke_dasharray_status = crate::flowchart::FlowchartSourceFacetStatus::Absent;
        self.inline_shape_sources = Vec::new();
        self.generated_shape_sources = Vec::new();
        self
    }

    pub(super) fn effective_edge_label_text_style<'a>(
        &self,
        base: &'a crate::text::TextStyle,
    ) -> std::borrow::Cow<'a, crate::text::TextStyle> {
        self.effective_edge_label_text_style_with_provenance(base)
            .style
    }

    pub(super) fn effective_edge_label_text_style_with_provenance<'a>(
        &self,
        base: &'a crate::text::TextStyle,
    ) -> crate::flowchart::FlowchartTextStyleResolution<'a> {
        let mut style = std::borrow::Cow::Borrowed(base);
        let mut prepared_text_overrides =
            crate::text::PreparedTextCssTypographyOverrides::default();
        let mut terminal_foreground = None;
        for declaration in &self.label_sources {
            crate::flowchart::flowchart_apply_text_style_decl(
                &mut style,
                declaration.prepared.property(),
                declaration.prepared.value(),
            );
            prepared_text_overrides.observe_declaration(
                declaration.prepared.property(),
                declaration.prepared.value(),
            );
            if declaration.prepared.property().trim() == "color" {
                terminal_foreground = Some(crate::flowchart::FlowchartTerminalForeground::new(
                    declaration.prepared.value(),
                    declaration.provenance.terminal_foreground_provenance(),
                ));
            }
        }
        crate::flowchart::FlowchartTextStyleResolution {
            style,
            prepared_text_overrides,
            terminal_foreground,
        }
    }

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

    pub(super) fn emitted_edge_source_stroke_status(
        &self,
        hand_drawn: bool,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        let source = if hand_drawn {
            self.inline_stroke_source.as_ref()
        } else {
            self.stroke_source.as_ref()
        };
        source
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

    pub(super) fn emitted_edge_source_stroke_dasharray_status(
        &self,
        hand_drawn: bool,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        if !hand_drawn {
            return self.source_stroke_dasharray_status();
        }
        self.inline_stroke_dasharray_status
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

    pub(super) fn source_label_foreground_status(
        &self,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.label_foreground_status
    }

    pub(super) fn emitted_source_label_foreground_status(
        &self,
        receipt: crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        if receipt
            .label_fill_reach()
            .is_some_and(|reach| reach.is_verified())
        {
            self.source_label_foreground_status()
        } else {
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        }
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
        let mut residuals = self
            .invalid_source_residuals(owner_id, crate::diagram_theme::SourceStyleChannel::Shape);
        residuals.extend(
            fill_residual
                .into_iter()
                .chain(stroke_residual)
                .chain(stroke_width_residual)
                .chain(stroke_dasharray_residual)
                .chain(radius_residuals),
        );

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
        let receipt = if source_style_verified {
            crate::flowchart::FlowchartShapeFacetEmissionReceipt::all()
        } else {
            crate::flowchart::FlowchartShapeFacetEmissionReceipt::none()
        };
        self.emitted_shape_source_residuals_with_receipt(owner_id, receipt)
    }

    pub(super) fn emitted_shape_source_residuals_with_receipt(
        &self,
        owner_id: &str,
        receipt: crate::flowchart::FlowchartShapeFacetEmissionReceipt,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        let mut residuals = self
            .invalid_source_residuals(owner_id, crate::diagram_theme::SourceStyleChannel::Shape);
        residuals.extend(
            self.shape_sources
                .iter()
                .filter(|source| !source.prepared.property().starts_with("--"))
                .filter_map(|source| {
                    let declaration = source.bind(
                        owner_id,
                        crate::diagram_theme::SourceStyleChannel::Shape,
                        false,
                    );
                    let source_style_verified = receipt.verifies(source.prepared.property());
                    emitted_shape_source_residual_reason(&declaration, source_style_verified).map(
                        |reason| {
                            crate::diagram_theme::SourceStyleResidual::from_declaration(
                                &declaration,
                                reason,
                            )
                        },
                    )
                }),
        );
        residuals
    }

    pub(super) fn emitted_edge_source_residuals(
        &self,
        owner_id: &str,
        hand_drawn: bool,
    ) -> Vec<crate::diagram_theme::SourceStyleResidual> {
        let mut residuals = self
            .invalid_source_residuals(owner_id, crate::diagram_theme::SourceStyleChannel::Shape);
        let collect_residual = |source: &PendingSourceDeclaration, source_style_verified| {
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
        };
        if hand_drawn {
            residuals.extend(
                self.shape_sources
                    .iter()
                    .filter(|source| !source.provenance.is_inline())
                    .filter(|source| !source.prepared.property().starts_with("--"))
                    .filter_map(|source| collect_residual(source, false)),
            );
            residuals.extend(
                self.inline_shape_sources
                    .iter()
                    .filter(|source| !source.prepared.property().starts_with("--"))
                    .filter_map(|source| collect_residual(source, true)),
            );
        } else {
            residuals.extend(
                self.edge_class_shape_sources
                    .iter()
                    .chain(&self.inline_shape_sources)
                    .filter(|source| !source.prepared.property().starts_with("--"))
                    .filter_map(|source| collect_residual(source, true)),
            );
        }
        residuals
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
        let mut residuals = self
            .invalid_source_residuals(owner_id, crate::diagram_theme::SourceStyleChannel::Label);
        residuals.extend(self.label_sources.iter().filter_map(|source| {
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
                crate::diagram_theme::SourceStyleResidual::from_declaration(&declaration, reason)
            })
        }));
        residuals
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

fn marker_source_value(declaration: &crate::diagram_theme::PreparedSourceStyleDeclaration) -> &str {
    let Some((_, source_value)) = declaration.raw().split_once(':') else {
        return declaration.value();
    };
    let source_value = source_value.trim_end_matches(';');
    if !declaration.important() {
        return source_value;
    }

    let value = declaration.value();
    source_value
        .find(value)
        .map(|offset| &source_value[..offset + value.len()])
        .unwrap_or(value)
}

fn flowchart_edge_animation_declaration_active(
    declaration: &crate::diagram_theme::PreparedSourceStyleDeclaration,
) -> Option<bool> {
    match declaration.property() {
        "animation" => parse_flowchart_animation_shorthand(declaration.value()),
        "animation-name" => parse_flowchart_animation_name(declaration.value()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartAnimationNameComponent {
    Active,
    Inactive,
    CssWide,
}

const FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS: &[&str] = &["var", "env"];
const FLOWCHART_ANIMATION_FUNCTIONS: &[&str] = &[
    "linear",
    "cubic-bezier",
    "steps",
    "calc",
    "min",
    "max",
    "clamp",
    "scroll",
    "view",
];
const FLOWCHART_ANIMATION_SHORTHAND_KEYWORDS: &[&str] = &[
    "none",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "linear",
    "step-start",
    "step-end",
    "infinite",
    "normal",
    "reverse",
    "alternate",
    "alternate-reverse",
    "forwards",
    "backwards",
    "both",
    "running",
    "paused",
    "auto",
];

fn flowchart_animation_identifier_matches(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

fn parse_flowchart_animation_name(value: &str) -> Option<bool> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let names = parser
        .parse_comma_separated(parse_flowchart_animation_name_component)
        .ok()?;
    if names.len() != 1 && names.contains(&FlowchartAnimationNameComponent::CssWide) {
        return None;
    }
    Some(names.contains(&FlowchartAnimationNameComponent::Active))
}

fn parse_flowchart_animation_name_component<'i, 't>(
    parser: &mut Parser<'i, 't>,
) -> std::result::Result<FlowchartAnimationNameComponent, cssparser::ParseError<'i, ()>> {
    let token = parser.next()?.clone();
    let component = match token {
        Token::Ident(name) if name.eq_ignore_ascii_case("none") => {
            FlowchartAnimationNameComponent::Inactive
        }
        Token::Ident(name) if crate::diagram_theme::is_css_wide_keyword(name.as_ref()) => {
            FlowchartAnimationNameComponent::CssWide
        }
        Token::Ident(_) | Token::QuotedString(_) => FlowchartAnimationNameComponent::Active,
        Token::Function(name)
            if flowchart_animation_identifier_matches(
                name.as_ref(),
                FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS,
            ) =>
        {
            consume_flowchart_animation_function(parser)?;
            FlowchartAnimationNameComponent::Active
        }
        _ => return Err(parser.new_custom_error(())),
    };
    parser.expect_exhausted()?;
    Ok(component)
}

fn parse_flowchart_animation_shorthand(value: &str) -> Option<bool> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_comma_separated(parse_flowchart_single_animation)
        .ok()
        .map(|animations| animations.into_iter().any(std::convert::identity))
}

fn parse_flowchart_single_animation<'i, 't>(
    parser: &mut Parser<'i, 't>,
) -> std::result::Result<bool, cssparser::ParseError<'i, ()>> {
    let mut active_name = false;
    let mut name_seen = false;
    let mut component_count = 0usize;
    let mut css_wide = false;

    loop {
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => break,
            Err(error) => return Err(error.into()),
        };
        if matches!(token, Token::WhiteSpace(_)) {
            continue;
        }
        component_count = component_count.saturating_add(1);
        match token {
            Token::Ident(name) if crate::diagram_theme::is_css_wide_keyword(name.as_ref()) => {
                css_wide = true;
            }
            Token::Ident(name) if flowchart_animation_shorthand_keyword(name.as_ref()) => {}
            Token::Ident(_) | Token::QuotedString(_) => {
                if name_seen {
                    return Err(parser.new_custom_error(()));
                }
                name_seen = true;
                active_name = true;
            }
            Token::Dimension { unit, .. }
                if unit.eq_ignore_ascii_case("s") || unit.eq_ignore_ascii_case("ms") => {}
            Token::Number { value, .. } if value.is_finite() && value >= 0.0 => {}
            Token::Function(name) => {
                let dynamic = flowchart_animation_identifier_matches(
                    name.as_ref(),
                    FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS,
                );
                let known = dynamic
                    || flowchart_animation_identifier_matches(
                        name.as_ref(),
                        FLOWCHART_ANIMATION_FUNCTIONS,
                    );
                if !known {
                    return Err(parser.new_custom_error(()));
                }
                consume_flowchart_animation_function(parser)?;
                if dynamic {
                    active_name = true;
                }
            }
            _ => return Err(parser.new_custom_error(())),
        }
    }

    if component_count == 0 || (css_wide && component_count != 1) {
        return Err(parser.new_custom_error(()));
    }
    Ok(active_name)
}

fn consume_flowchart_animation_function<'i, 't>(
    parser: &mut Parser<'i, 't>,
) -> std::result::Result<(), cssparser::ParseError<'i, ()>> {
    parser.parse_nested_block(|nested| {
        while nested.next_including_whitespace().is_ok() {}
        Ok(())
    })
}

fn flowchart_animation_shorthand_keyword(value: &str) -> bool {
    flowchart_animation_identifier_matches(value, FLOWCHART_ANIMATION_SHORTHAND_KEYWORDS)
}

fn projected_joined_style_bytes(a: &[String], b: &[String]) -> Option<usize> {
    a.iter()
        .chain(b)
        .enumerate()
        .try_fold(0usize, |bytes, (index, part)| {
            bytes.checked_add(
                usize::from(index != 0).checked_add(super::super::util::escaped_xml_len(part))?,
            )
        })
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

pub(super) fn sanitized_xhtml_typography_statuses(
    sanitized_xhtml: &str,
) -> (
    crate::flowchart::FlowchartSourceFacetStatus,
    crate::flowchart::FlowchartSourceFacetStatus,
) {
    use crate::flowchart::FlowchartSourceFacetStatus::{Absent, Admitted, Unverified};

    if sanitized_xhtml.is_empty()
        || (!contains_ascii_case_insensitive(sanitized_xhtml, b"style")
            && !contains_ascii_case_insensitive(sanitized_xhtml, b"class"))
    {
        return (Absent, Absent);
    }

    let mut reader = quick_xml::Reader::from_str(sanitized_xhtml);
    reader.config_mut().enable_all_checks(true);
    let mut font_stack = Absent;
    let mut font_size = Absent;

    loop {
        let decoder = reader.decoder();
        let element = match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => element,
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(_) => return (Unverified, Unverified),
        };

        let mut renderer_math_wrapper = false;
        for attribute in element.attributes() {
            let Ok(attribute) = attribute else {
                return (Unverified, Unverified);
            };
            let name = attribute.key.local_name();
            renderer_math_wrapper |= matches!(
                name.as_ref(),
                b"data-merman-prepared-math-native" | b"data-merman-prepared-math-occurrence"
            );
        }

        for attribute in element.attributes() {
            let Ok(attribute) = attribute else {
                return (Unverified, Unverified);
            };
            let name = attribute.key.local_name();
            let is_style = name.as_ref().eq_ignore_ascii_case(b"style");
            let is_class = name.as_ref().eq_ignore_ascii_case(b"class");
            if (!is_style && !is_class) || renderer_math_wrapper {
                continue;
            }
            let Ok(value) =
                attribute.decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            else {
                return (Unverified, Unverified);
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if is_class {
                font_stack = font_stack.merge(Unverified);
                font_size = font_size.merge(Unverified);
                continue;
            }

            let mut input = ParserInput::new(value);
            let mut parser = Parser::new(&mut input);
            while !parser.is_exhausted() {
                let start = parser.position();
                let property = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
                    let property = declaration.expect_ident_cloned()?.to_ascii_lowercase();
                    declaration.expect_colon()?;
                    while declaration.next_including_whitespace().is_ok() {}
                    Ok::<_, cssparser::ParseError<'_, ()>>(property)
                });
                let raw = parser
                    .slice(start..parser.position())
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                let (affects_stack, affects_size) = match property.as_deref().ok() {
                    Some("font-family") => (true, false),
                    Some("font-size") => (false, true),
                    Some("font") | Some("all") => (true, true),
                    _ => continue,
                };
                let provenance = crate::diagram_theme::SourceStyleProvenance::label_style(
                    "sanitized-xhtml-fragment",
                    crate::diagram_theme::SourceStyleChannel::Label,
                    0,
                );
                let status =
                    match crate::diagram_theme::SourceStyleDeclaration::parse(raw, provenance) {
                        Ok(declaration)
                            if matches!(declaration.property(), "font-family" | "font-size")
                                && label_source_residual_reason(&declaration, true).is_none() =>
                        {
                            Admitted
                        }
                        Ok(_) | Err(_) => Unverified,
                    };
                if affects_stack {
                    font_stack = font_stack.merge(status);
                }
                if affects_size {
                    font_size = font_size.merge(status);
                }
            }
        }
    }

    (font_stack, font_size)
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
        let prefer_direct = direct_inline
            || (direct_emission.reach().is_verified()
                && (!receipt.radius_selector_overrides_direct()
                    || generated_source.is_some_and(|generated| {
                        generated.prepared.property() == direct_source.prepared.property()
                            && generated.prepared.value() == direct_source.prepared.value()
                    })));
        let (component_status, residual) = match generated_source {
            Some(generated_source) if !prefer_direct => {
                let generated_radius = SourceFacetDeclaration {
                    prepared: Arc::clone(&generated_source.prepared),
                    provenance: generated_source.provenance.clone(),
                    admitted: admitted_flowchart_source_radius(generated_source.prepared.value()),
                };
                generated_radius.evidence(
                    owner_id,
                    receipt.source_radius_emission(generated_radius.admitted, false, true),
                )
            }
            _ => direct_source.evidence(owner_id, direct_emission),
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
    sources
        .iter()
        .filter(|source| {
            source.prepared.property() == property
                && source.provenance.class_id().is_some_and(wrapper_has_class)
        })
        .fold(None, |winner, candidate| match winner {
            Some(winner) if winner.prepared.important() && !candidate.prepared.important() => {
                Some(winner)
            }
            _ => Some(candidate),
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
    let prepared_classes =
        FlowchartPreparedClassStyles::prepare(class_defs, classes.iter().map(String::as_str), None)
            .expect("unmetered source style preparation cannot fail");
    flowchart_compile_prepared_styles(
        &prepared_classes,
        classes,
        inline_styles_a,
        inline_styles_b,
        None,
    )
    .expect("unmetered source style compilation cannot fail")
}

pub(in crate::svg::parity::flowchart) fn flowchart_compile_prepared_styles(
    class_styles: &FlowchartPreparedClassStyles,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
    work_meter: Option<&crate::resources::OperationWorkMeter>,
) -> std::result::Result<FlowchartCompiledStyles, crate::resources::OperationWorkError> {
    // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
    // - preserve insertion order of the first occurrence of a key
    // - later declarations of equal priority override values without changing order
    // - an earlier `!important` declaration remains the winner over later normal declarations
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
        fn get(&self, property: &str) -> Option<&OrderedDeclaration> {
            self.idx
                .get(property)
                .and_then(|index| self.order.get(*index))
        }

        fn set(
            &mut self,
            prepared: Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
            provenance: PendingSourceProvenance,
        ) {
            let property = prepared.property().to_string();
            if let Some(&index) = self.idx.get(&property) {
                if self.order[index].prepared.important() && !prepared.important() {
                    return;
                }
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
                if self.order[index].important() && !prepared.important() {
                    return;
                }
                self.order[index] = prepared;
                return;
            }
            self.idx.insert(property, self.order.len());
            self.order.push(prepared);
        }
    }

    #[derive(Default)]
    struct AnimationNameCascade {
        winner: Option<(bool, bool)>,
    }

    impl AnimationNameCascade {
        fn observe(
            &mut self,
            prepared: &crate::diagram_theme::PreparedSourceStyleDeclaration,
            work_meter: Option<&crate::resources::OperationWorkMeter>,
        ) -> std::result::Result<(), crate::resources::OperationWorkError> {
            if !matches!(prepared.property(), "animation" | "animation-name") {
                return Ok(());
            }
            charge_style_scan(work_meter, prepared.value().len())?;
            let Some(active) = flowchart_edge_animation_declaration_active(prepared) else {
                return Ok(());
            };
            if self
                .winner
                .is_some_and(|(_, important)| important && !prepared.important())
            {
                return Ok(());
            }
            self.winner = Some((active, prepared.important()));
            Ok(())
        }

        fn active(&self) -> Option<bool> {
            self.winner.map(|(active, _)| active)
        }
    }

    let mut semantic = SemanticMap::default();
    let mut inline_semantic = SemanticMap::default();
    let mut inline_declarations = Vec::new();
    let mut emission = EmissionMap::default();
    let mut animation_name = AnimationNameCascade::default();
    let mut edge_class_declarations = Vec::new();
    let mut edge_class_shape_sources = Vec::new();
    let mut invalid_sources = Vec::new();

    let mut declaration_ordinal = 0usize;
    for (assignment_ordinal, c) in classes.iter().enumerate() {
        let Some((class_id, declarations)) = class_styles.classes.get_key_value(c.as_str()) else {
            continue;
        };
        charge_style_work(work_meter, declarations.len())?;
        for declaration in declarations {
            let ordinal = declaration_ordinal;
            declaration_ordinal = declaration_ordinal.saturating_add(1);
            let provenance = PendingSourceProvenance::AssignedClass {
                class_id: Arc::clone(class_id),
                assignment_ordinal,
                declaration_ordinal: ordinal,
            };
            let prepared = match declaration {
                PreparedClassDeclaration::Valid(prepared) => Arc::clone(prepared),
                PreparedClassDeclaration::Invalid {
                    raw,
                    channel,
                    label_foreground,
                } => {
                    invalid_sources.push(PendingInvalidSourceDeclaration {
                        raw: Arc::clone(raw),
                        channel: *channel,
                        label_foreground: *label_foreground,
                        provenance,
                    });
                    continue;
                }
            };
            if !crate::flowchart::flowchart_is_source_spelled_label_style_key(
                prepared.property_css(),
            ) {
                edge_class_declarations.push(Arc::clone(&prepared));
                edge_class_shape_sources.push(PendingSourceDeclaration {
                    prepared: Arc::clone(&prepared),
                    provenance: provenance.clone(),
                });
            }
            animation_name.observe(&prepared, work_meter)?;
            emission.set(Arc::clone(&prepared));
            semantic.set(prepared, provenance);
        }
    }

    for raw_group in inline_styles_a.iter().chain(inline_styles_b.iter()) {
        charge_style_scan(work_meter, raw_group.len())?;
        for raw in crate::flowchart::flowchart_split_mermaid_style_decls(raw_group) {
            charge_style_work(work_meter, 1)?;
            let ordinal = declaration_ordinal;
            declaration_ordinal = declaration_ordinal.saturating_add(1);
            let provenance = PendingSourceProvenance::Inline {
                declaration_ordinal: ordinal,
            };
            let Some(declaration) =
                crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw)
            else {
                let (channel, label_foreground) = classify_invalid_source_style(raw);
                invalid_sources.push(PendingInvalidSourceDeclaration {
                    raw: Arc::from(raw.trim()),
                    channel,
                    label_foreground,
                    provenance,
                });
                continue;
            };
            let prepared = Arc::new(declaration);
            inline_declarations.push(OrderedDeclaration {
                prepared: Arc::clone(&prepared),
                provenance: provenance.clone(),
            });
            animation_name.observe(&prepared, work_meter)?;
            emission.set(Arc::clone(&prepared));
            inline_semantic.set(Arc::clone(&prepared), provenance.clone());
            semantic.set(prepared, provenance);
        }
    }

    let mut node_style = String::new();
    let mut label_style = String::new();

    let mut label_div_decls: Vec<(String, String)> = Vec::new();

    let mut fill: Option<String> = None;
    let mut fill_source = None;
    let mut stroke: Option<String> = None;
    let mut stroke_source = None;
    let mut inline_stroke_source = None;
    let mut stroke_width: Option<String> = None;
    let mut stroke_width_source = None;
    let mut radius_sources = Vec::new();
    let mut stroke_dasharray: Option<String> = None;
    let mut stroke_dasharray_source = None;
    let mut inline_stroke_dasharray_status = crate::flowchart::FlowchartSourceFacetStatus::Absent;
    let mut font_stack_source = None;
    let mut font_size_source = None;
    let mut shape_sources = Vec::new();
    let mut inline_shape_sources = Vec::new();
    let mut generated_shape_sources = Vec::new();
    let mut label_sources = Vec::new();

    let mut css_classes = classes
        .iter()
        .filter_map(|class_id| {
            class_styles
                .get_index_of(class_id)
                .map(|index| (index, class_id))
        })
        .collect::<Vec<_>>();
    css_classes.sort_unstable_by_key(|(index, _)| *index);
    css_classes.dedup_by_key(|(index, _)| *index);
    let mut css_declaration_ordinal = 0usize;
    for (assignment_ordinal, class_id) in css_classes {
        let Some((prepared_class_id, declarations)) =
            class_styles.classes.get_key_value(class_id.as_str())
        else {
            continue;
        };
        for declaration in declarations {
            let declaration_ordinal = css_declaration_ordinal;
            css_declaration_ordinal = css_declaration_ordinal.saturating_add(1);
            let PreparedClassDeclaration::Valid(prepared) = declaration else {
                continue;
            };
            if crate::flowchart::flowchart_is_source_spelled_label_style_key(
                prepared.property_css(),
            ) {
                continue;
            }
            generated_shape_sources.push(PendingSourceDeclaration {
                prepared: Arc::clone(prepared),
                provenance: PendingSourceProvenance::AssignedClass {
                    class_id: Arc::clone(prepared_class_id),
                    assignment_ordinal,
                    declaration_ordinal,
                },
            });
        }
    }

    for declaration in &emission.order {
        let property_css = declaration.property_css();
        let v = declaration.value();
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

    let label_foreground_status = label_sources
        .iter()
        .filter(|source| source.prepared.property() == "color")
        .map(|source| {
            crate::flowchart::FlowchartSourceFacetStatus::from_parts(
                true,
                crate::mermaid_style::is_supported_css_color_value(source.prepared.value()),
            )
        })
        .chain(
            invalid_sources
                .iter()
                .filter(|source| source.label_foreground)
                .map(|_| crate::flowchart::FlowchartSourceFacetStatus::Unverified),
        )
        .fold(
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            crate::flowchart::FlowchartSourceFacetStatus::merge,
        );

    if let Some(declaration) = inline_semantic.get("stroke") {
        inline_stroke_source = Some(SourceFacetDeclaration {
            prepared: Arc::clone(&declaration.prepared),
            provenance: declaration.provenance.clone(),
            admitted: admitted_flowchart_source_paint(declaration.prepared.value()),
        });
    }
    if let Some(declaration) = inline_semantic.get("stroke-dasharray") {
        inline_stroke_dasharray_status = crate::flowchart::FlowchartSourceFacetStatus::from_parts(
            true,
            valid_stroke_dasharray(declaration.prepared.value()),
        );
    }
    inline_shape_sources.extend(
        inline_declarations
            .iter()
            .filter(|declaration| {
                !crate::flowchart::flowchart_is_source_spelled_label_style_key(
                    declaration.prepared.property_css(),
                )
            })
            .map(|declaration| PendingSourceDeclaration {
                prepared: Arc::clone(&declaration.prepared),
                provenance: declaration.provenance.clone(),
            }),
    );
    Ok(FlowchartCompiledStyles {
        edge_class_declarations,
        edge_animation_active: animation_name.active(),
        edge_class_shape_sources,
        node_style,
        label_style,
        label_div_decls,
        fill,
        fill_source,
        stroke,
        stroke_source,
        inline_stroke_source,
        stroke_width,
        stroke_width_source,
        radius_sources,
        stroke_dasharray,
        stroke_dasharray_source,
        inline_stroke_dasharray_status,
        font_stack_source,
        font_size_source,
        shape_sources,
        inline_shape_sources,
        generated_shape_sources,
        label_sources,
        label_foreground_status,
        invalid_sources,
    })
}

fn classify_invalid_source_style(raw: &str) -> (crate::diagram_theme::SourceStyleChannel, bool) {
    let property = raw
        .trim()
        .split_once(':')
        .map(|(property, _)| property.trim())
        .unwrap_or_default();
    let channel = if crate::flowchart::flowchart_is_source_spelled_label_style_key(property) {
        crate::diagram_theme::SourceStyleChannel::Label
    } else {
        crate::diagram_theme::SourceStyleChannel::Shape
    };
    let label_foreground = channel == crate::diagram_theme::SourceStyleChannel::Label
        && property.eq_ignore_ascii_case("color");
    (channel, label_foreground)
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

    #[test]
    fn sanitized_html_typography_distinguishes_user_overrides_from_prepared_math_wrappers() {
        use crate::flowchart::FlowchartSourceFacetStatus::{Absent, Admitted, Unverified};

        assert_eq!(
            sanitized_xhtml_typography_statuses(
                r#"<span style="font-family:Arial;font-size:24px">label</span>"#,
            ),
            (Admitted, Admitted),
        );
        assert_eq!(
            sanitized_xhtml_typography_statuses(r#"<span class="custom">label</span>"#),
            (Unverified, Unverified),
        );
        assert_eq!(
            sanitized_xhtml_typography_statuses(
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" style="font-family:Typed;font-size:18px">x²</span>"#,
            ),
            (Absent, Absent),
        );
        assert_eq!(
            sanitized_xhtml_typography_statuses(
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1"><span style="font-size:31px">user</span></span>"#,
            ),
            (Absent, Admitted),
        );
    }

    fn color_style(value: &str) -> FlowchartCompiledStyles {
        FlowchartCompiledStyles {
            edge_class_declarations: Vec::new(),
            edge_animation_active: None,
            edge_class_shape_sources: Vec::new(),
            node_style: String::new(),
            label_style: String::new(),
            label_div_decls: vec![("color".to_string(), value.to_string())],
            fill: None,
            fill_source: None,
            stroke: None,
            stroke_source: None,
            inline_stroke_source: None,
            stroke_width: None,
            stroke_width_source: None,
            radius_sources: Vec::new(),
            stroke_dasharray: None,
            stroke_dasharray_source: None,
            inline_stroke_dasharray_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
            font_stack_source: None,
            font_size_source: None,
            shape_sources: Vec::new(),
            inline_shape_sources: Vec::new(),
            generated_shape_sources: Vec::new(),
            label_sources: Vec::new(),
            label_foreground_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
            invalid_sources: Vec::new(),
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
    fn animation_keyword_and_function_tables_are_ascii_case_insensitive() {
        assert!(flowchart_animation_shorthand_keyword("EaSe-In"));
        assert_eq!(
            parse_flowchart_animation_shorthand("2s LiNeAr(0, 1) BoTh"),
            Some(false)
        );
        assert_eq!(
            parse_flowchart_animation_shorthand("VaR(--animation-name) 2s EaSe-In"),
            Some(true)
        );
        assert_eq!(
            parse_flowchart_animation_name("EnV(--animation-name)"),
            Some(true)
        );
    }

    #[test]
    fn edge_artifacts_drop_node_only_payload_without_losing_edge_evidence() {
        let class_defs = IndexMap::from([(
            "edge-source".to_string(),
            vec![
                "fill:#f8fafc,stroke:#2563eb,stroke-width:3px,rx:4px".to_string(),
                "color:#111827,font-size:18px,filter:url(#shadow)".to_string(),
            ],
        )]);
        let compiled = flowchart_compile_styles(
            &class_defs,
            &["edge-source".to_string()],
            &["stroke:#16a34a,stroke-dasharray:4 2".to_string()],
            &["invalid-source".to_string()],
        );

        assert!(!compiled.node_style.is_empty());
        assert!(compiled.fill_source.is_some());
        assert!(compiled.stroke_width_source.is_some());
        assert!(!compiled.radius_sources.is_empty());
        assert!(!compiled.generated_shape_sources.is_empty());

        let ordinary = compiled.clone().into_edge_artifact(false);
        assert!(ordinary.node_style.is_empty());
        assert!(ordinary.fill.is_none());
        assert!(ordinary.fill_source.is_none());
        assert!(ordinary.stroke.is_none());
        assert!(ordinary.stroke_width.is_none());
        assert!(ordinary.stroke_width_source.is_none());
        assert!(ordinary.radius_sources.is_empty());
        assert!(ordinary.stroke_dasharray.is_none());
        assert!(ordinary.stroke_dasharray_source.is_some());
        assert_eq!(
            ordinary.inline_stroke_dasharray_status,
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert!(ordinary.generated_shape_sources.is_empty());
        assert!(!ordinary.shape_sources.is_empty());
        assert!(!ordinary.edge_class_declarations.is_empty());
        assert!(!ordinary.edge_class_shape_sources.is_empty());
        assert!(!ordinary.inline_shape_sources.is_empty());
        assert!(ordinary.stroke_source.is_some());
        assert!(ordinary.inline_stroke_source.is_some());
        assert!(!ordinary.label_sources.is_empty());
        assert!(!ordinary.invalid_sources.is_empty());

        let hand_drawn = compiled.clone().into_edge_artifact(true);
        assert!(hand_drawn.edge_class_shape_sources.is_empty());
        assert!(!hand_drawn.shape_sources.is_empty());
        assert!(!hand_drawn.inline_shape_sources.is_empty());
        assert!(hand_drawn.inline_stroke_source.is_some());
        assert!(hand_drawn.stroke_dasharray_source.is_some());
        assert_eq!(
            hand_drawn.inline_stroke_dasharray_status,
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );

        let swimlane_label = compiled.into_swimlane_edge_label_artifact();
        assert!(swimlane_label.edge_class_declarations.is_empty());
        assert!(swimlane_label.edge_class_shape_sources.is_empty());
        assert!(swimlane_label.node_style.is_empty());
        assert!(swimlane_label.stroke_source.is_none());
        assert!(swimlane_label.inline_shape_sources.is_empty());
        assert!(!swimlane_label.shape_sources.is_empty());
        assert!(!swimlane_label.label_sources.is_empty());
    }

    #[test]
    fn hand_drawn_edge_dasharray_precedence_uses_only_inline_sources() {
        let class_defs = IndexMap::from([(
            "edge-source".to_string(),
            vec!["stroke-dasharray:8 3".to_string()],
        )]);
        let class_only =
            flowchart_compile_styles(&class_defs, &["edge-source".to_string()], &[], &[])
                .into_edge_artifact(true);
        assert_eq!(
            class_only.emitted_edge_source_stroke_dasharray_status(true),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
        assert_eq!(
            class_only.emitted_edge_source_stroke_dasharray_status(false),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );

        let inline = flowchart_compile_styles(
            &class_defs,
            &["edge-source".to_string()],
            &["stroke-dasharray:6 2".to_string()],
            &[],
        )
        .into_edge_artifact(true);
        assert_eq!(
            inline.emitted_edge_source_stroke_dasharray_status(true),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
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
    fn edge_class_emission_reuses_compiled_source_declarations() {
        let class_defs = IndexMap::from([(
            "edge-source".to_string(),
            vec![
                "stroke:#2563eb,animation:dash 2s linear".to_string(),
                "color:#f8fafc".to_string(),
            ],
        )]);
        let styles = flowchart_compile_styles(
            &class_defs,
            &["edge-source".to_string()],
            &["stroke-width:3px".to_string()],
            &[],
        );

        assert_eq!(
            styles
                .emitted_edge_class_declarations()
                .iter()
                .map(|declaration| (declaration.property_css(), declaration.source_value()))
                .collect::<Vec<_>>(),
            vec![("stroke", "#2563eb"), ("animation", "dash 2s linear")]
        );
        assert_eq!(
            styles.source_stroke_status(),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
        assert_eq!(styles.stroke.as_deref(), Some("#2563eb"));
    }

    #[test]
    fn invalid_assigned_class_declarations_keep_element_provenance() {
        let class_defs = IndexMap::from([(
            "unsafe".to_string(),
            vec!["font-size:".to_string(), "filter:url(#alpha)".to_string()],
        )]);
        let styles = flowchart_compile_styles(&class_defs, &["unsafe".to_string()], &[], &[]);

        let shape_residuals = styles.emitted_edge_source_residuals("edge", false);
        assert_eq!(shape_residuals.len(), 1);
        assert_eq!(shape_residuals[0].raw(), "filter:url(#alpha)");
        assert_eq!(shape_residuals[0].property(), None);
        assert_eq!(shape_residuals[0].provenance().owner_id(), "edge");
        assert_eq!(shape_residuals[0].provenance().class_id(), Some("unsafe"));
        assert_eq!(
            shape_residuals[0].provenance().origin(),
            crate::diagram_theme::SourceStyleOrigin::AssignedClass
        );
        assert_eq!(
            shape_residuals[0].provenance().channel(),
            crate::diagram_theme::SourceStyleChannel::Shape
        );
        assert_eq!(
            shape_residuals[0].provenance().assignment_ordinal(),
            Some(0)
        );
        assert_eq!(shape_residuals[0].provenance().declaration_ordinal(), 1);
        assert_eq!(
            shape_residuals[0].reason(),
            crate::diagram_theme::SourceStyleResidualReason::InvalidDeclaration
        );

        let label_residuals = styles.emitted_label_source_residuals("edge", false);
        assert_eq!(label_residuals.len(), 1);
        assert_eq!(label_residuals[0].raw(), "font-size:");
        assert_eq!(
            label_residuals[0].provenance().channel(),
            crate::diagram_theme::SourceStyleChannel::Label
        );
        assert_eq!(label_residuals[0].provenance().declaration_ordinal(), 0);
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
