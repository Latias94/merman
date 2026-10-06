//! Flowchart SVG defs and marker emission.

use std::fmt;
use std::sync::Mutex;

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::util::{escape_xml, escape_xml_display};
use super::{
    FlowchartHierarchyPlan, FlowchartMarkerBase, FlowchartRenderCtx, flowchart_config_diagram_look,
    flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
};

const AGENTFLOW_HIERARCHY_MARKERS: [(&str, &str); 2] = [
    (
        "hierarchyEnd",
        r#" viewBox="0 0 12 10" refX="10" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="10" orient="auto"><path d="M 0 0 L 6 5 L 0 10 M 4 0 L 10 5 L 4 10" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0; fill: none;"/></marker>"#,
    ),
    (
        "hierarchyStart",
        r#" viewBox="0 0 12 10" refX="2" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="10" orient="auto"><path d="M 12 0 L 6 5 L 12 10 M 8 0 L 2 5 L 8 10" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0; fill: none;"/></marker>"#,
    ),
];

#[derive(Debug, Clone, Copy)]
enum FlowchartMarkerPaint {
    None,
    Stroke,
    StrokeAndFill,
}

impl FlowchartMarkerPaint {
    fn color_scan_count(self) -> usize {
        match self {
            Self::None | Self::Stroke => 1,
            Self::StrokeAndFill => 2,
        }
    }

    fn source_owned_channels(self, source_admitted: bool, config_owned: bool) -> (bool, bool) {
        // These flags prove source/config precedence, never visible stroke pixels. Neo margin
        // shapes can own a stroke declaration while their fixed stroke width remains zero.
        match self {
            Self::None => (config_owned, config_owned),
            Self::Stroke => (config_owned, source_admitted),
            Self::StrokeAndFill => (source_admitted, source_admitted),
        }
    }

    fn push(self, out: &mut impl fmt::Write, color: &str) {
        match self {
            Self::None => {}
            Self::Stroke => {
                let _ = write!(out, r#" stroke="{}""#, escape_xml_display(color));
            }
            Self::StrokeAndFill => {
                let color = escape_xml_display(color);
                let _ = write!(out, r#" stroke="{color}" fill="{color}""#);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartMarkerShapeSpec {
    base: FlowchartMarkerBase,
    margin: bool,
    marker_class: &'static str,
    shape: &'static str,
    paint: FlowchartMarkerPaint,
    viewport: FlowchartMarkerViewport,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartMarkerViewport {
    view_box: [f64; 2],
    reference: [f64; 2],
    size: [f64; 2],
}

impl FlowchartMarkerViewport {
    fn rotation_radius(self) -> f64 {
        // All built-in markers use a zero-origin viewBox and the SVG default
        // xMidYMid meet mapping. The default hidden overflow clips to this viewport.
        let scale = (self.size[0] / self.view_box[0]).min(self.size[1] / self.view_box[1]);
        let reference_x =
            self.reference[0] * scale + (self.size[0] - self.view_box[0] * scale) / 2.0;
        let reference_y =
            self.reference[1] * scale + (self.size[1] - self.view_box[1] * scale) / 2.0;
        // The farthest viewport corner bounds the marker under any path tangent rotation.
        reference_x
            .abs()
            .max((self.size[0] - reference_x).abs())
            .hypot(reference_y.abs().max((self.size[1] - reference_y).abs()))
    }
}

const FLOWCHART_MARKER_SHAPE_SPECS: [FlowchartMarkerShapeSpec; 12] = [
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointEnd,
        margin: false,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [5.0, 5.0],
            size: [8.0, 8.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointStart,
        margin: false,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [4.5, 5.0],
            size: [8.0, 8.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointEnd,
        margin: true,
        marker_class: "marker",
        shape: r#" viewBox="0 0 11.5 14" refX="11.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="10.5" markerHeight="14" orient="auto"><path d="M 0 0 L 11.5 7 L 0 14 z" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::StrokeAndFill,
        viewport: FlowchartMarkerViewport {
            view_box: [11.5, 14.0],
            reference: [11.5, 7.0],
            size: [10.5, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::PointStart,
        margin: true,
        marker_class: "marker",
        shape: r#" viewBox="0 0 11.5 14" refX="1" refY="7" markerUnits="userSpaceOnUse" markerWidth="11.5" markerHeight="14" orient="auto"><polygon points="0,7 11.5,14 11.5,0" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::None,
        viewport: FlowchartMarkerViewport {
            view_box: [11.5, 14.0],
            reference: [1.0, 7.0],
            size: [11.5, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleEnd,
        margin: false,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [11.0, 5.0],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleStart,
        margin: false,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [-1.0, 5.0],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleEnd,
        margin: true,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refY="5" refX="12.25" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [12.25, 5.0],
            size: [14.0, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CircleStart,
        margin: true,
        marker_class: "marker",
        shape: r#" viewBox="0 0 10 10" refX="-2" refY="5" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [10.0, 10.0],
            reference: [-2.0, 5.0],
            size: [14.0, 14.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossEnd,
        margin: false,
        marker_class: "marker cross",
        shape: r#" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [11.0, 11.0],
            reference: [12.0, 5.2],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossStart,
        margin: false,
        marker_class: "marker cross",
        shape: r#" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [11.0, 11.0],
            reference: [-1.0, 5.2],
            size: [11.0, 11.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossEnd,
        margin: true,
        marker_class: "marker cross",
        shape: r#" viewBox="0 0 15 15" refX="17.7" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [15.0, 15.0],
            reference: [17.7, 7.5],
            size: [12.0, 12.0],
        },
    },
    FlowchartMarkerShapeSpec {
        base: FlowchartMarkerBase::CrossStart,
        margin: true,
        marker_class: "marker cross",
        shape: r#" viewBox="0 0 15 15" refX="-3.5" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5; stroke-dasharray: 1, 0;""#,
        paint: FlowchartMarkerPaint::Stroke,
        viewport: FlowchartMarkerViewport {
            view_box: [15.0, 15.0],
            reference: [-3.5, 7.5],
            size: [12.0, 12.0],
        },
    },
];

fn flowchart_marker_shape_spec(
    base: FlowchartMarkerBase,
    margin: bool,
) -> &'static FlowchartMarkerShapeSpec {
    FLOWCHART_MARKER_SHAPE_SPECS
        .iter()
        .find(|spec| spec.base == base && spec.margin == margin)
        .expect("Flowchart marker shape descriptor")
}

#[derive(Debug, Clone, Copy)]
struct FlowchartMarkerReference {
    base: FlowchartMarkerBase,
    margin: bool,
    variant_index: Option<usize>,
}

#[derive(Debug, Default, Clone)]
struct FlowchartEdgeMarkerReferences {
    start: Option<FlowchartMarkerReference>,
    end: Option<FlowchartMarkerReference>,
    serialized_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FlowchartMarkerVariantKey {
    base: FlowchartMarkerBase,
    margin: bool,
    color_id: String,
    typed_edge: bool,
}

impl FlowchartMarkerVariantKey {
    fn paint(&self) -> FlowchartMarkerPaint {
        if self.typed_edge {
            match self.base {
                FlowchartMarkerBase::PointStart
                | FlowchartMarkerBase::PointEnd
                | FlowchartMarkerBase::CircleStart
                | FlowchartMarkerBase::CircleEnd => FlowchartMarkerPaint::StrokeAndFill,
                _ => FlowchartMarkerPaint::Stroke,
            }
        } else {
            flowchart_marker_shape_spec(self.base, self.margin).paint
        }
    }
}

#[derive(Debug)]
struct PreparedFlowchartMarkerColor<'a> {
    raw: &'a str,
    id: String,
    typed_edge: bool,
}

impl<'a> PreparedFlowchartMarkerColor<'a> {
    fn prepare(
        raw: &'a str,
        typed_edge: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let normalization_work = 1usize
            .checked_add(raw.len().div_ceil(64))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.charge(normalization_work)?;
        let id = marker_color_id(raw);
        // Source marker ids contain only ASCII alphanumerics and underscores. The hyphen keeps
        // typed edge paint separate even when source and theme colors normalize identically.
        let id = if typed_edge && !id.is_empty() {
            format!("theme-{id}")
        } else {
            id
        };
        Ok(Self {
            raw,
            id,
            typed_edge,
        })
    }
}

pub(in crate::svg::parity::flowchart) struct FlowchartMarkerPathCheckpoint<'a> {
    pub key: crate::flowchart::FlowchartEdgeKey,
    pub has_geometry: bool,
    pub attributes: Option<&'a str>,
}

struct MarkerAttributeMatcher<'a> {
    remaining: &'a str,
    matches: bool,
}

impl fmt::Write for MarkerAttributeMatcher<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if let Some(rest) = self.remaining.strip_prefix(value) {
            self.remaining = rest;
        } else {
            self.matches = false;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct FlowchartMarkerTerminalReceipt {
    seen_edges: FxHashSet<crate::flowchart::FlowchartEdgeKey>,
    fill_owners: Vec<bool>,
    stroke_owners: Vec<bool>,
    valid: bool,
}

impl Default for FlowchartMarkerTerminalReceipt {
    fn default() -> Self {
        Self {
            seen_edges: FxHashSet::default(),
            fill_owners: Vec::new(),
            stroke_owners: Vec::new(),
            valid: true,
        }
    }
}

#[derive(Debug)]
pub(in crate::svg::parity::flowchart) struct FlowchartMarkerEmissionPlan {
    #[cfg(test)]
    edges: FxHashMap<String, Vec<FlowchartEdgeMarkerReferences>>,
    edge_occurrences: FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgeMarkerReferences>,
    terminal: Option<Mutex<FlowchartMarkerTerminalReceipt>>,
    variants: IndexMap<FlowchartMarkerVariantKey, String>,
    hand_drawn: bool,
    base_marker_bytes: usize,
    extra_marker_bytes: usize,
    edge_marker_attribute_bytes: usize,
    serialization_input_work: usize,
}

impl FlowchartMarkerEmissionPlan {
    fn new(hand_drawn: bool) -> Self {
        Self {
            #[cfg(test)]
            edges: FxHashMap::default(),
            edge_occurrences: FxHashMap::default(),
            terminal: None,
            variants: IndexMap::new(),
            hand_drawn,
            base_marker_bytes: 0,
            extra_marker_bytes: 0,
            edge_marker_attribute_bytes: 0,
            serialization_input_work: 0,
        }
    }

    pub(in crate::svg::parity::flowchart) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy_plan: &FlowchartHierarchyPlan<'_>,
    ) -> crate::Result<Self> {
        let look = flowchart_config_diagram_look(ctx.config);
        let hand_drawn = look.is_hand_drawn();
        let neo = look.is_neo();
        let mut plan = Self::new(hand_drawn);
        if let Some(theme) = ctx.resolved_theme {
            ctx.work_meter
                .charge(theme.family_mechanism_routes().len())?;
            if theme.family_mechanism_routes().iter().any(|route| {
                use crate::diagram_theme::{FamilyThemeMechanism, ThemeTarget};
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Marker,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Marker
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Marker,
                        ..
                    }
                )
            }) {
                plan.terminal = Some(Mutex::new(FlowchartMarkerTerminalReceipt::default()));
            }
        }
        for &edge in hierarchy_plan.ordered_edges() {
            let edge_styles = ctx.edge_style_plan.edge_for(edge.key)?;
            let stroke_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                edge_styles.emitted_edge_source_stroke_status(hand_drawn),
                ctx.edge_stroke_config_override,
            );
            let typed_stroke = ctx.edge_theme.stroke_value(stroke_precedence, true);
            let marker_color = typed_stroke.or_else(|| edge_styles.edge_marker_color(hand_drawn));
            let margin = neo && !ctx.edge_style_plan.animation_for(edge.key)?.is_active();
            plan.register_edge_with_identity(
                edge.key,
                edge.edge,
                marker_color,
                typed_stroke.is_some(),
                margin,
                ctx.document_ids.marker_scope(),
                ctx.diagram_type,
                ctx.work_meter,
            )?;
        }
        plan.finalize_svg_budget(
            ctx.document_ids.marker_scope(),
            ctx.diagram_type,
            ctx.security_level_loose,
            ctx.work_meter,
        )?;
        Ok(plan)
    }

    /// Called only after the complete edge path and its prepared references reach a checkpoint.
    pub(in crate::svg::parity::flowchart) fn record_edge_checkpoint(
        &self,
        checkpoint: FlowchartMarkerPathCheckpoint<'_>,
        source: crate::flowchart::FlowchartSourceFacetStatus,
        config_owned: bool,
        diagram_id: &str,
        diagram_type: &str,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let Some(terminal) = &self.terminal else {
            return Ok(());
        };
        let key = checkpoint.key;
        let references =
            self.edge_occurrences
                .get(&key)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!(
                        "missing prepared Flowchart marker occurrence {}",
                        key.semantic_index()
                    ),
                })?;
        let count = usize::from(references.start.is_some()) + usize::from(references.end.is_some());
        if count == 0 {
            if checkpoint
                .attributes
                .is_some_and(|attributes| !attributes.is_empty())
            {
                work_meter.charge(1)?;
                terminal
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .valid = false;
            }
            return Ok(());
        }
        work_meter.charge(1 + count + references.serialized_bytes.div_ceil(64))?;
        let mut actual = MarkerAttributeMatcher {
            remaining: checkpoint.attributes.unwrap_or_default(),
            matches: checkpoint.attributes.is_some(),
        };
        self.push_prepared_edge_marker_attributes(
            &mut actual,
            diagram_id,
            diagram_type,
            references,
        );
        let references_match = actual.matches && actual.remaining.is_empty();
        let mut terminal = terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let first_checkpoint = terminal.seen_edges.insert(key);
        terminal.valid &= checkpoint.has_geometry && references_match && first_checkpoint;
        if !terminal.valid {
            return Ok(());
        }
        for reference in [references.start, references.end].into_iter().flatten() {
            let paint = if !self.hand_drawn && reference.variant_index.is_some() {
                flowchart_marker_shape_spec(reference.base, reference.margin).paint
            } else {
                FlowchartMarkerPaint::None
            };
            let (fill, stroke) = paint.source_owned_channels(
                source == crate::flowchart::FlowchartSourceFacetStatus::Admitted,
                config_owned,
            );
            terminal.fill_owners.push(fill);
            terminal.stroke_owners.push(stroke);
        }
        Ok(())
    }

    pub(in crate::svg::parity::flowchart) fn finish_theme_evidence(
        &self,
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Option<crate::family::FamilyThemeEvidence>> {
        use crate::diagram_theme::{ThemeTarget, ThemeVariant};
        use crate::family::{
            TerminalVariantDomain, UnsupportedTerminalDomain,
            reconcile_unsupported_terminal_domains,
        };
        let (Some(terminal), Some(theme)) = (&self.terminal, theme) else {
            return Ok(None);
        };
        let terminal = terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work_meter.charge(self.edge_occurrences.len() + terminal.fill_owners.len())?;
        let mut evidence = crate::family::FamilyThemeEvidence::from_theme(Some(theme));
        if !terminal.valid
            || self.edge_occurrences.iter().any(|(key, references)| {
                (references.start.is_some() || references.end.is_some())
                    && !terminal.seen_edges.contains(key)
            })
        {
            // Expected references without a completed path are incomplete, not terminal-less.
            return Ok(Some(evidence));
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Marker,
                TerminalVariantDomain::uniform(terminal.fill_owners.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&terminal.fill_owners)
            .with_source_owned_stroke(&terminal.stroke_owners)],
            work_meter,
        )?;
        Ok(Some(evidence))
    }

    fn variant(&self, index: usize) -> (&FlowchartMarkerVariantKey, &str) {
        let (key, raw_color) = self
            .variants
            .get_index(index)
            .expect("prepared Flowchart marker variant index");
        (key, raw_color.as_str())
    }

    fn register_edge_with_identity(
        &mut self,
        key: crate::flowchart::FlowchartEdgeKey,
        edge: &crate::flowchart::FlowEdge,
        marker_color: Option<&str>,
        typed_edge: bool,
        margin: bool,
        diagram_id: &str,
        diagram_type: &str,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        work_meter.charge(1)?;
        let start_base = flowchart_edge_marker_start_base(edge);
        let end_base = flowchart_edge_marker_end_base(edge);
        let prepared_color = if start_base.is_some() || end_base.is_some() {
            marker_color
                .map(|raw| PreparedFlowchartMarkerColor::prepare(raw, typed_edge, work_meter))
                .transpose()?
        } else {
            None
        };
        let start = start_base
            .map(|base| self.register_reference(base, prepared_color.as_ref(), margin, work_meter))
            .transpose()?;
        let end = end_base
            .map(|base| self.register_reference(base, prepared_color.as_ref(), margin, work_meter))
            .transpose()?;
        let mut references = FlowchartEdgeMarkerReferences {
            start,
            end,
            serialized_bytes: 0,
        };
        let marker_id_input_bytes = [references.start, references.end]
            .into_iter()
            .flatten()
            .try_fold(0usize, |bytes, reference| {
                let color_id_len = reference
                    .variant_index
                    .map(|index| self.variant(index).0.color_id.len())
                    .unwrap_or_default();
                bytes
                    .checked_add(diagram_id.len())
                    .and_then(|bytes| bytes.checked_add(diagram_type.len()))
                    .and_then(|bytes| bytes.checked_add(reference.base.id_suffix().len()))
                    .and_then(|bytes| bytes.checked_add(color_id_len))
                    .ok_or_else(|| work_meter.arithmetic_overflow())
            })?;
        self.preflight_serialization_input(marker_id_input_bytes.div_ceil(64), work_meter)?;
        let mut counter = SvgByteCounter::default();
        self.push_prepared_edge_marker_attributes(
            &mut counter,
            diagram_id,
            diagram_type,
            &references,
        );
        references.serialized_bytes = counter.finish(work_meter)?;
        self.edge_marker_attribute_bytes = self
            .edge_marker_attribute_bytes
            .checked_add(references.serialized_bytes)
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        #[cfg(test)]
        self.edges
            .entry(edge.id.clone())
            .or_default()
            .push(references.clone());
        if self.edge_occurrences.insert(key, references).is_some() {
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "duplicate prepared Flowchart marker occurrence {}",
                    key.semantic_index()
                ),
            });
        }
        Ok(())
    }

    #[cfg(test)]
    fn register_edge(
        &mut self,
        edge: &crate::flowchart::FlowEdge,
        marker_color: Option<&str>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let key = crate::flowchart::FlowchartEdgeKey::new(self.edge_occurrences.len());
        self.register_edge_with_identity(
            key,
            edge,
            marker_color,
            false,
            false,
            "diagram",
            "flowchart-v2",
            work_meter,
        )
    }

    fn register_reference(
        &mut self,
        base: FlowchartMarkerBase,
        marker_color: Option<&PreparedFlowchartMarkerColor<'_>>,
        margin: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<FlowchartMarkerReference> {
        work_meter.charge(1)?;
        let Some(marker_color) = marker_color else {
            return Ok(FlowchartMarkerReference {
                base,
                margin,
                variant_index: None,
            });
        };
        if marker_color.id.is_empty() {
            return Ok(FlowchartMarkerReference {
                base,
                margin,
                variant_index: None,
            });
        }

        let key = FlowchartMarkerVariantKey {
            base,
            margin,
            color_id: marker_color.id.clone(),
            typed_edge: marker_color.typed_edge,
        };
        let variant_index = if let Some(index) = self.variants.get_index_of(&key) {
            index
        } else {
            work_meter.charge(1)?;
            let index = self.variants.len();
            let replaced = self.variants.insert(key, marker_color.raw.to_string());
            debug_assert!(replaced.is_none());
            index
        };
        Ok(FlowchartMarkerReference {
            base,
            margin,
            variant_index: Some(variant_index),
        })
    }

    fn finalize_svg_budget(
        &mut self,
        diagram_id: &str,
        diagram_type: &str,
        security_level_loose: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let base_marker_count = FLOWCHART_MARKER_SHAPE_SPECS.len()
            + if diagram_type == "agentflow" {
                AGENTFLOW_HIERARCHY_MARKERS.len()
            } else {
                0
            };
        let base_marker_input_bytes = diagram_id
            .len()
            .checked_add(
                diagram_type
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?,
            )
            .and_then(|bytes| bytes.checked_mul(base_marker_count))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        self.preflight_serialization_input(base_marker_input_bytes.div_ceil(64), work_meter)?;
        let mut base_counter = SvgByteCounter::default();
        push_base_markers(&mut base_counter, diagram_id, diagram_type);
        let base_marker_bytes = base_counter.finish(work_meter)?;

        let extra_marker_input_bytes = self.variants.iter().try_fold(
            0usize,
            |bytes, (key, raw_color)| -> crate::Result<usize> {
                let diagram_type_bytes = diagram_type
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                let color_scan_count = key.paint().color_scan_count();
                let raw_color_bytes = raw_color
                    .len()
                    .checked_mul(color_scan_count)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                bytes
                    .checked_add(diagram_id.len())
                    .and_then(|bytes| bytes.checked_add(diagram_type_bytes))
                    .and_then(|bytes| bytes.checked_add(key.base.id_suffix().len()))
                    .and_then(|bytes| bytes.checked_add(key.color_id.len()))
                    .and_then(|bytes| bytes.checked_add(raw_color_bytes))
                    .ok_or_else(|| work_meter.arithmetic_overflow().into())
            },
        )?;
        self.preflight_serialization_input(extra_marker_input_bytes.div_ceil(64), work_meter)?;
        let mut extra_counter = SvgByteCounter::default();
        self.push_extra_markers(
            &mut extra_counter,
            diagram_id,
            diagram_type,
            security_level_loose,
        );
        let extra_marker_bytes = extra_counter.finish(work_meter)?;
        let projected_svg_bytes = match base_marker_bytes
            .checked_add(extra_marker_bytes)
            .and_then(|bytes| bytes.checked_add(self.edge_marker_attribute_bytes))
        {
            Some(projected) => projected,
            None => {
                work_meter.check_svg_append(usize::MAX, 1)?;
                unreachable!("overflowing marker projection must be rejected")
            }
        };
        let serialization_work = self
            .serialization_input_work
            .checked_add(projected_svg_bytes.div_ceil(256))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;

        work_meter.charge(serialization_work)?;
        work_meter.charge_svg_bytes(projected_svg_bytes)?;
        self.base_marker_bytes = base_marker_bytes;
        self.extra_marker_bytes = extra_marker_bytes;
        Ok(())
    }

    fn preflight_serialization_input(
        &mut self,
        additional: usize,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let pending = self
            .serialization_input_work
            .checked_add(additional)
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.preflight(pending)?;
        self.serialization_input_work = pending;
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn push_edge_marker_attributes(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        diagram_id: &str,
        diagram_type: &str,
        edge_id: &str,
        occurrence: usize,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let references = self
            .edges
            .get(edge_id)
            .and_then(|references| references.get(occurrence))
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart marker references for `{edge_id}` occurrence {occurrence}"
                ),
            })?;
        work_meter.check_svg_append(out.len(), references.serialized_bytes)?;
        let before = out.len();
        self.push_prepared_edge_marker_attributes(out, diagram_id, diagram_type, references);
        debug_assert_eq!(out.len() - before, references.serialized_bytes);
        out.checkpoint()
    }

    /// Bounds the selected marker paint around either endpoint under any rotation.
    pub(in crate::svg::parity::flowchart) fn edge_paint_outset_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<f64> {
        if self.hand_drawn {
            return None;
        }
        let references = self.edge_occurrences.get(&key)?;
        Some(
            [references.start, references.end]
                .into_iter()
                .flatten()
                .map(|reference| {
                    flowchart_marker_shape_spec(reference.base, reference.margin)
                        .viewport
                        .rotation_radius()
                })
                .fold(0.0, f64::max),
        )
    }

    pub(in crate::svg::parity::flowchart) fn push_edge_marker_attributes_for(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        diagram_id: &str,
        diagram_type: &str,
        key: crate::flowchart::FlowchartEdgeKey,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let references =
            self.edge_occurrences
                .get(&key)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!(
                        "missing prepared Flowchart marker occurrence {}",
                        key.semantic_index()
                    ),
                })?;
        work_meter.check_svg_append(out.len(), references.serialized_bytes)?;
        let before = out.len();
        self.push_prepared_edge_marker_attributes(out, diagram_id, diagram_type, references);
        debug_assert_eq!(out.len() - before, references.serialized_bytes);
        out.checkpoint()
    }

    fn push_prepared_edge_marker_attributes(
        &self,
        out: &mut impl fmt::Write,
        diagram_id: &str,
        diagram_type: &str,
        references: &FlowchartEdgeMarkerReferences,
    ) {
        if let Some(reference) = references.start {
            self.push_marker_attribute(out, "start", diagram_id, diagram_type, reference);
        }
        if let Some(reference) = references.end {
            self.push_marker_attribute(out, "end", diagram_id, diagram_type, reference);
        }
    }

    fn push_marker_attribute(
        &self,
        out: &mut impl fmt::Write,
        position: &str,
        diagram_id: &str,
        diagram_type: &str,
        reference: FlowchartMarkerReference,
    ) {
        let color_id = reference
            .variant_index
            .map(|index| self.variant(index).0.color_id.as_str());
        let _ = write!(out, r#" marker-{position}="url(#"#);
        write_flowchart_marker_id_xml(
            out,
            diagram_id,
            diagram_type,
            reference.base,
            reference.margin,
            color_id,
        );
        let _ = out.write_str(r#")""#);
    }

    fn push_extra_markers(
        &self,
        out: &mut impl fmt::Write,
        diagram_id: &str,
        diagram_type: &str,
        security_level_loose: bool,
    ) {
        for (key, raw_color) in &self.variants {
            push_extra_marker(
                out,
                diagram_id,
                diagram_type,
                key,
                raw_color,
                security_level_loose,
            );
        }
    }
}

pub(in crate::svg::parity::flowchart) struct FlowchartDefs<'a> {
    diagram_id: &'a str,
    diagram_type: &'a str,
    marker_plan: &'a FlowchartMarkerEmissionPlan,
    security_level_loose: bool,
    work_meter: &'a crate::resources::OperationWorkMeter,
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_defs<'a>(
    diagram_id: &'a str,
    diagram_type: &'a str,
    ctx: &'a FlowchartRenderCtx<'_>,
    marker_plan: &'a FlowchartMarkerEmissionPlan,
) -> FlowchartDefs<'a> {
    FlowchartDefs {
        diagram_id,
        diagram_type,
        marker_plan,
        security_level_loose: ctx.security_level_loose,
        work_meter: ctx.work_meter,
    }
}

impl FlowchartDefs<'_> {
    pub(in crate::svg::parity::flowchart) fn push_base_markers(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
    ) -> crate::Result<()> {
        self.work_meter
            .check_svg_append(out.len(), self.marker_plan.base_marker_bytes)?;
        let before = out.len();
        push_base_markers(out, self.diagram_id, self.diagram_type);
        debug_assert_eq!(out.len() - before, self.marker_plan.base_marker_bytes);
        out.checkpoint()
    }

    pub(in crate::svg::parity::flowchart) fn push_extra_markers(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
    ) -> crate::Result<()> {
        self.work_meter
            .check_svg_append(out.len(), self.marker_plan.extra_marker_bytes)?;
        let before = out.len();
        self.marker_plan.push_extra_markers(
            out,
            self.diagram_id,
            self.diagram_type,
            self.security_level_loose,
        );
        debug_assert_eq!(out.len() - before, self.marker_plan.extra_marker_bytes);
        out.checkpoint()
    }
}

fn push_base_markers(out: &mut impl fmt::Write, diagram_id: &str, diagram_type: &str) {
    let id = escape_xml(diagram_id);
    let ty = escape_xml(diagram_type);
    for spec in FLOWCHART_MARKER_SHAPE_SPECS {
        let _ = write!(
            out,
            r#"<marker id="{}_{}-{}"#,
            id.as_str(),
            ty.as_str(),
            spec.base.id_suffix()
        );
        if spec.margin {
            let _ = out.write_str("-margin");
        }
        let _ = write!(out, r#"" class="{} {}""#, spec.marker_class, ty.as_str());
        let _ = out.write_str(spec.shape);
        let _ = out.write_str("/></marker>");
    }
    if diagram_type == "agentflow" {
        // Mermaid eagerly registers these after the ordinary marker set, even without
        // hierarchy edges. Keep them on the same namespace and resource-budget path.
        for (suffix, shape) in AGENTFLOW_HIERARCHY_MARKERS {
            let _ = write!(
                out,
                r#"<marker id="{id}_{ty}-{suffix}" class="marker hierarchy {ty}"{shape}"#,
            );
        }
    }
}
fn marker_color_id(color: &str) -> String {
    // Mermaid's DOM marker id coloring logic (Mermaid 12) uses:
    // `strokeColor.replace(/[^\dA-Za-z]/g, '_')`
    //
    // Important: this does not trim whitespace. As a result, values like `" orange"` (leading
    // space captured from `style="...stroke: orange;..."`) produce a leading `_` in the color id,
    // which in turn yields a `__orange` suffix in the final marker id.
    let raw = color.trim_end_matches(';');
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(raw.len());
    for code_unit in raw.encode_utf16() {
        if code_unit <= 0x7f && (code_unit as u8).is_ascii_alphanumeric() {
            out.push(char::from(code_unit as u8));
        } else {
            out.push('_');
        }
    }
    out
}

#[inline]
fn write_flowchart_marker_id_xml(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    base: FlowchartMarkerBase,
    margin: bool,
    color_id: Option<&str>,
) {
    let _ = write!(out, "{}", escape_xml_display(diagram_id));
    let _ = out.write_char('_');
    let _ = write!(out, "{}", escape_xml_display(diagram_type));
    let _ = out.write_char('-');
    let _ = out.write_str(base.id_suffix());
    if margin {
        let _ = out.write_str("-margin");
    }

    let Some(color_id) = color_id else {
        return;
    };
    let _ = out.write_char('_');
    let _ = out.write_str(color_id);
}

fn push_extra_marker(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    key: &FlowchartMarkerVariantKey,
    raw_color: &str,
    security_level_loose: bool,
) {
    let raw_color = raw_color.trim_end_matches(';');
    let color = if security_level_loose {
        Some(raw_color)
    } else if raw_color
        .trim_start()
        .strip_prefix("stroke:")
        .is_some_and(|value| !value.trim_start().starts_with('#'))
    {
        // Strict/sandbox sanitization drops raw stroke tokens without changing marker identity.
        None
    } else {
        Some(raw_color.trim())
    };
    let spec = flowchart_marker_shape_spec(key.base, key.margin);

    let _ = out.write_str(r#"<marker id=""#);
    write_flowchart_marker_id_xml(
        out,
        diagram_id,
        diagram_type,
        key.base,
        key.margin,
        Some(key.color_id.as_str()),
    );
    let _ = write!(
        out,
        r#"" class="{} {}""#,
        spec.marker_class,
        escape_xml_display(diagram_type)
    );
    let _ = out.write_str(spec.shape);
    if let Some(color) = color {
        key.paint().push(out, color);
    }
    let _ = out.write_str("/></marker>");
}

#[derive(Debug, Default)]
struct SvgByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl fmt::Write for SvgByteCounter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        match self.bytes.checked_add(text.len()) {
            Some(bytes) => self.bytes = bytes,
            None => {
                self.bytes = usize::MAX;
                self.overflowed = true;
            }
        }
        Ok(())
    }
}

impl SvgByteCounter {
    fn finish(self, work_meter: &crate::resources::OperationWorkMeter) -> crate::Result<usize> {
        if self.overflowed {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing SVG count must be rejected")
        }
        Ok(self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    fn meter() -> crate::resources::OperationWorkMeter {
        crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
    }

    fn meter_with_limits(
        svg_bytes: usize,
        work_units: usize,
    ) -> crate::resources::OperationWorkMeter {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, svg_bytes)
            .unwrap()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work_units)
            .unwrap();
        crate::resources::OperationWorkMeter::new(policy)
    }

    pub(super) fn edge(id: &str, edge_type: &str) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id: id.to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some(edge_type.to_string()),
            arrow: String::new(),
            start_marker: Default::default(),
            end_marker: Default::default(),
            is_user_defined_id: true,
            stroke: None,
            stroke_kind: Default::default(),
            visibility: Default::default(),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn marker_chunk<'a>(svg: &'a str, marker_id: &str) -> &'a str {
        let needle = format!(r#"id="{marker_id}""#);
        let start = svg.find(&needle).expect("marker id");
        let end = svg[start..].find("</marker>").expect("marker end") + start + 9;
        &svg[start..end]
    }

    #[test]
    fn marker_paint_viewports_match_emitted_marker_geometry() {
        let mut defs = String::new();
        push_base_markers(&mut defs, "diagram", "flowchart-v2");
        let svg = format!("<svg>{defs}</svg>");
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for spec in FLOWCHART_MARKER_SHAPE_SPECS {
            let id = format!(
                "diagram_flowchart-v2-{}{}",
                spec.base.id_suffix(),
                if spec.margin { "-margin" } else { "" },
            );
            let marker = doc
                .descendants()
                .find(|node| node.attribute("id") == Some(id.as_str()))
                .unwrap();
            let number = |name| marker.attribute(name).unwrap().parse::<f64>().unwrap();
            let view_box = marker
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                view_box,
                [
                    0.0,
                    0.0,
                    spec.viewport.view_box[0],
                    spec.viewport.view_box[1]
                ]
            );
            assert_eq!(spec.viewport.reference, [number("refX"), number("refY")]);
            assert_eq!(
                spec.viewport.size,
                [number("markerWidth"), number("markerHeight")]
            );
            assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
            assert_eq!(marker.attribute("orient"), Some("auto"));
            assert_eq!(marker.attribute("preserveAspectRatio"), None);
            assert_eq!(marker.attribute("overflow"), None);
            assert_eq!(marker.attribute("style"), None);
        }
    }

    #[test]
    fn edge_marker_paint_outsets_follow_selected_marker_occurrences() {
        let meter = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        for (index, (edge_type, margin, x, y)) in [
            ("double_arrow_point", false, 4.4_f64, 4.0_f64),
            ("double_arrow_circle", false, 12.1, 5.5),
            ("double_arrow_cross", false, 12.0, 5.8),
            ("double_arrow_point", true, 10.5, 7.0),
            ("double_arrow_circle", true, 17.15, 7.0),
            ("double_arrow_cross", true, 14.8, 6.0),
            ("arrow_open", false, 0.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        {
            let key = crate::flowchart::FlowchartEdgeKey::new(index);
            plan.register_edge_with_identity(
                key,
                &edge("duplicate", edge_type),
                Some("#00f2ff"),
                false,
                margin,
                "diagram",
                "flowchart-v2",
                &meter,
            )
            .unwrap();
            let radius = plan.edge_paint_outset_for(key).unwrap();
            assert!(
                (radius - x.hypot(y)).abs() < 1e-12,
                "{edge_type}/{margin}: {radius}"
            );
        }
        assert_eq!(
            plan.edge_paint_outset_for(crate::flowchart::FlowchartEdgeKey::new(99)),
            None
        );
        plan.hand_drawn = true;
        assert_eq!(
            plan.edge_paint_outset_for(crate::flowchart::FlowchartEdgeKey::new(0)),
            None
        );
    }

    #[test]
    fn base_marker_defs_match_mermaid_11_16_1_bytes() {
        let mut defs = String::new();
        push_base_markers(&mut defs, "diagram", "flowchart-v2");

        assert_eq!(
            defs,
            concat!(
                r#"<marker id="diagram_flowchart-v2-pointEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-pointStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="4.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 5 L 10 10 L 10 0 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-pointEnd-margin" class="marker flowchart-v2" viewBox="0 0 11.5 14" refX="11.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="10.5" markerHeight="14" orient="auto"><path d="M 0 0 L 11.5 7 L 0 14 z" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-pointStart-margin" class="marker flowchart-v2" viewBox="0 0 11.5 14" refX="1" refY="7" markerUnits="userSpaceOnUse" markerWidth="11.5" markerHeight="14" orient="auto"><polygon points="0,7 11.5,14 11.5,0" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-circleEnd" class="marker flowchart-v2" viewBox="0 0 10 10" refX="11" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-circleStart" class="marker flowchart-v2" viewBox="0 0 10 10" refX="-1" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-circleEnd-margin" class="marker flowchart-v2" viewBox="0 0 10 10" refY="5" refX="12.25" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-circleStart-margin" class="marker flowchart-v2" viewBox="0 0 10 10" refX="-2" refY="5" markerUnits="userSpaceOnUse" markerWidth="14" markerHeight="14" orient="auto"><circle cx="5" cy="5" r="5" class="arrowMarkerPath" style="stroke-width: 0; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-crossEnd" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="12" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-crossStart" class="marker cross flowchart-v2" viewBox="0 0 11 11" refX="-1" refY="5.2" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M 1,1 l 9,9 M 10,1 l -9,9" class="arrowMarkerPath" style="stroke-width: 2; stroke-dasharray: 1, 0;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-crossEnd-margin" class="marker cross flowchart-v2" viewBox="0 0 15 15" refX="17.7" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5;"/></marker>"#,
                r#"<marker id="diagram_flowchart-v2-crossStart-margin" class="marker cross flowchart-v2" viewBox="0 0 15 15" refX="-3.5" refY="7.5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 1,1 L 14,14 M 1,14 L 14,1" class="arrowMarkerPath" style="stroke-width: 2.5; stroke-dasharray: 1, 0;"/></marker>"#,
            )
        );
    }

    #[test]
    fn colored_marker_shapes_only_add_color_identity_and_paint_to_base_bytes() {
        let cases = [
            (FlowchartMarkerBase::PointEnd, false, true, true),
            (FlowchartMarkerBase::PointStart, false, true, true),
            (FlowchartMarkerBase::PointEnd, true, true, true),
            (FlowchartMarkerBase::PointStart, true, false, false),
            (FlowchartMarkerBase::CircleEnd, false, true, false),
            (FlowchartMarkerBase::CircleStart, false, true, false),
            (FlowchartMarkerBase::CircleEnd, true, true, false),
            (FlowchartMarkerBase::CircleStart, true, true, false),
            (FlowchartMarkerBase::CrossEnd, false, true, false),
            (FlowchartMarkerBase::CrossStart, false, true, false),
            (FlowchartMarkerBase::CrossEnd, true, true, false),
            (FlowchartMarkerBase::CrossStart, true, true, false),
        ];
        let mut base_defs = String::new();
        push_base_markers(&mut base_defs, "diagram", "flowchart-v2");

        for (base, margin, paints_surface, paints_fill) in cases {
            let base_id = format!(
                "diagram_flowchart-v2-{}{}",
                base.id_suffix(),
                if margin { "-margin" } else { "" }
            );
            let colored_id = format!("{base_id}__2468ac");
            let mut expected = marker_chunk(&base_defs, &base_id).replacen(
                &format!(r#"id="{base_id}""#),
                &format!(r#"id="{colored_id}""#),
                1,
            );
            if paints_surface {
                let paint = if paints_fill {
                    r##" stroke="#2468ac" fill="#2468ac""##
                } else {
                    r##" stroke="#2468ac""##
                };
                expected = expected.replacen("/></marker>", &format!("{paint}/></marker>"), 1);
            }

            let key = FlowchartMarkerVariantKey {
                base,
                margin,
                color_id: "_2468ac".to_string(),
                typed_edge: false,
            };
            let mut actual = String::new();
            push_extra_marker(
                &mut actual,
                "diagram",
                "flowchart-v2",
                &key,
                "#2468ac",
                false,
            );

            assert_eq!(
                marker_chunk(&actual, &colored_id),
                expected,
                "base={base:?}, margin={margin}"
            );
        }
    }

    #[test]
    fn typed_edge_markers_paint_filled_shapes_and_all_outlines() {
        for spec in FLOWCHART_MARKER_SHAPE_SPECS {
            let key = FlowchartMarkerVariantKey {
                base: spec.base,
                margin: spec.margin,
                color_id: "theme-_2468ac".to_string(),
                typed_edge: true,
            };
            let mut actual = String::new();
            push_extra_marker(
                &mut actual,
                "diagram",
                "flowchart-v2",
                &key,
                "#2468ac",
                false,
            );
            assert!(actual.contains(r##" stroke="#2468ac""##), "{actual}");
            let filled_shape = matches!(
                spec.base,
                FlowchartMarkerBase::PointStart
                    | FlowchartMarkerBase::PointEnd
                    | FlowchartMarkerBase::CircleStart
                    | FlowchartMarkerBase::CircleEnd
            );
            assert_eq!(
                actual.contains(r##" fill="#2468ac""##),
                filled_shape,
                "{actual}"
            );
        }
    }

    #[test]
    fn same_source_and_typed_color_keep_distinct_identity_and_exact_svg_budget() {
        let source_edge = edge("source", "double_arrow_point");
        let typed_edge = edge("typed", "double_arrow_point");
        let meter = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new(true);
        plan.register_edge(&source_edge, Some("#2468ac"), &meter)
            .unwrap();
        plan.register_edge_with_identity(
            crate::flowchart::FlowchartEdgeKey::new(1),
            &typed_edge,
            Some("#2468ac"),
            true,
            false,
            "diagram",
            "flowchart-v2",
            &meter,
        )
        .unwrap();
        plan.finalize_svg_budget("diagram", "flowchart-v2", false, &meter)
            .unwrap();
        assert_eq!(plan.variants.len(), 4);

        let mut output = String::new();
        push_base_markers(&mut output, "diagram", "flowchart-v2");
        plan.push_extra_markers(&mut output, "diagram", "flowchart-v2", false);
        assert_eq!(
            output.len(),
            plan.base_marker_bytes + plan.extra_marker_bytes
        );
        for suffix in ["pointStart", "pointEnd"] {
            let source = marker_chunk(&output, &format!("diagram_flowchart-v2-{suffix}__2468ac"));
            assert!(
                source.contains(r##" stroke="#2468ac" fill="#2468ac""##),
                "{source}"
            );
            let typed = marker_chunk(
                &output,
                &format!("diagram_flowchart-v2-{suffix}_theme-_2468ac"),
            );
            assert!(
                typed.contains(r##" stroke="#2468ac" fill="#2468ac""##),
                "{typed}"
            );
        }
        for edge_id in ["source", "typed"] {
            plan.push_edge_marker_attributes(
                &mut output,
                "diagram",
                "flowchart-v2",
                edge_id,
                0,
                &meter,
            )
            .unwrap();
        }
        assert_eq!(output.len(), meter.projected_svg_bytes());
        assert!(output.contains("url(#diagram_flowchart-v2-pointEnd__2468ac)"));
        assert!(output.contains("url(#diagram_flowchart-v2-pointEnd_theme-_2468ac)"));
    }

    #[test]
    fn colored_marker_plan_defines_every_referenced_base_in_first_use_order() {
        let circle = edge("circle", "double_arrow_circle");
        let cross = edge("cross", "double_arrow_cross");
        let point = edge("point", "double_arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let meter = meter();
        plan.register_edge(&circle, Some("#ef4444"), &meter)
            .unwrap();
        plan.register_edge(&cross, Some("#22c55e"), &meter).unwrap();
        plan.register_edge(&point, Some("#2563eb"), &meter).unwrap();

        assert_eq!(
            plan.variants
                .iter()
                .map(|(key, _)| key.base)
                .collect::<Vec<_>>(),
            vec![
                FlowchartMarkerBase::CircleStart,
                FlowchartMarkerBase::CircleEnd,
                FlowchartMarkerBase::CrossStart,
                FlowchartMarkerBase::CrossEnd,
                FlowchartMarkerBase::PointStart,
                FlowchartMarkerBase::PointEnd,
            ]
        );

        let mut attributes = String::new();
        for edge_id in ["circle", "cross", "point"] {
            plan.push_edge_marker_attributes(
                &mut attributes,
                "diagram",
                "flowchart-v2",
                edge_id,
                0,
                &meter,
            )
            .expect("prepared marker references");
        }
        for suffix in [
            "circleStart__ef4444",
            "circleEnd__ef4444",
            "crossStart__22c55e",
            "crossEnd__22c55e",
            "pointStart__2563eb",
            "pointEnd__2563eb",
        ] {
            assert!(
                attributes.contains(&format!("url(#diagram_flowchart-v2-{suffix})")),
                "missing reference {suffix}: {attributes}"
            );
        }

        let mut defs = String::new();
        plan.push_extra_markers(&mut defs, "diagram", "flowchart-v2", false);
        for suffix in [
            "circleStart__ef4444",
            "circleEnd__ef4444",
            "crossStart__22c55e",
            "crossEnd__22c55e",
            "pointStart__2563eb",
            "pointEnd__2563eb",
        ] {
            assert_eq!(
                defs.matches(&format!(r#"id="diagram_flowchart-v2-{suffix}""#))
                    .count(),
                1,
                "definition {suffix}: {defs}"
            );
        }

        for suffix in ["pointStart__2563eb", "pointEnd__2563eb"] {
            let marker = marker_chunk(&defs, &format!("diagram_flowchart-v2-{suffix}"));
            assert!(marker.contains(r##" stroke="#2563eb" fill="#2563eb""##));
        }
        for (suffix, color) in [
            ("circleStart__ef4444", "#ef4444"),
            ("circleEnd__ef4444", "#ef4444"),
            ("crossStart__22c55e", "#22c55e"),
            ("crossEnd__22c55e", "#22c55e"),
        ] {
            let marker = marker_chunk(&defs, &format!("diagram_flowchart-v2-{suffix}"));
            assert!(marker.contains(&format!(r#" stroke="{color}""#)));
            assert!(!marker.contains(" fill="), "stroke-only marker: {marker}");
        }
    }

    #[test]
    fn duplicate_mermaid_edge_ids_keep_occurrence_scoped_marker_references() {
        let first = edge("duplicate", "double_arrow_circle");
        let second = edge("duplicate", "double_arrow_cross");
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let meter = meter();
        plan.register_edge(&first, Some("#ef4444"), &meter).unwrap();
        plan.register_edge(&second, Some("#22c55e"), &meter)
            .unwrap();

        let mut first_attributes = String::new();
        plan.push_edge_marker_attributes_for(
            &mut first_attributes,
            "diagram",
            "flowchart-v2",
            crate::flowchart::FlowchartEdgeKey::new(0),
            &meter,
        )
        .unwrap();
        assert!(first_attributes.contains("circleStart__ef4444"));
        assert!(first_attributes.contains("circleEnd__ef4444"));

        let mut second_attributes = String::new();
        plan.push_edge_marker_attributes_for(
            &mut second_attributes,
            "diagram",
            "flowchart-v2",
            crate::flowchart::FlowchartEdgeKey::new(1),
            &meter,
        )
        .unwrap();
        assert!(second_attributes.contains("crossStart__22c55e"));
        assert!(second_attributes.contains("crossEnd__22c55e"));
    }

    #[test]
    fn colored_marker_plan_deduplicates_by_base_and_color_id_with_first_raw_color() {
        let first = edge("first", "arrow_point");
        let second = edge("second", "arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let meter = meter();
        plan.register_edge(&first, Some("#abc"), &meter).unwrap();
        plan.register_edge(&second, Some("$abc"), &meter).unwrap();

        assert_eq!(marker_color_id("#abc"), marker_color_id("$abc"));
        assert_eq!(plan.variants.len(), 1);
        assert_eq!(
            plan.variants
                .get_index(0)
                .map(|(_, raw_color)| raw_color.as_str()),
            Some("#abc")
        );

        let mut attributes = String::new();
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "first",
            0,
            &meter,
        )
        .expect("first marker");
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "second",
            0,
            &meter,
        )
        .expect("second marker");
        assert_eq!(
            attributes
                .matches("url(#diagram_flowchart-v2-pointEnd__abc)")
                .count(),
            2
        );

        let mut defs = String::new();
        plan.push_extra_markers(&mut defs, "diagram", "flowchart-v2", false);
        assert_eq!(
            defs.matches(r#"id="diagram_flowchart-v2-pointEnd__abc""#)
                .count(),
            1
        );
        assert!(defs.contains(r##" stroke="#abc" fill="#abc""##));
        assert!(!defs.contains(r#" stroke="$abc""#));
    }

    #[test]
    fn marker_color_id_matches_javascript_utf16_replacement_semantics() {
        assert_eq!(marker_color_id("var(--é)"), "var_____");
        assert_eq!(marker_color_id("var(--😀)"), "var______");
        assert_ne!(marker_color_id("var(--é)"), marker_color_id("var(--😀)"));
    }

    #[test]
    fn same_colored_bidirectional_base_is_deduplicated_without_merging_start_and_end() {
        let first = edge("first", "double_arrow_point");
        let second = edge("second", "double_arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let meter = meter();
        plan.register_edge(&first, Some("#ef4444"), &meter).unwrap();
        assert_eq!(meter.used(), 7);
        plan.register_edge(&second, Some("#ef4444"), &meter)
            .unwrap();
        assert_eq!(meter.used(), 12);

        assert_eq!(plan.variants.len(), 2);
        assert_eq!(
            plan.variants.get_index(0).map(|(key, _)| key.base),
            Some(FlowchartMarkerBase::PointStart)
        );
        assert_eq!(
            plan.variants.get_index(1).map(|(key, _)| key.base),
            Some(FlowchartMarkerBase::PointEnd)
        );
        assert!(plan.variants.keys().all(|key| key.color_id == "_ef4444"));

        let mut attributes = String::new();
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "first",
            0,
            &meter,
        )
        .unwrap();
        assert_eq!(
            attributes,
            concat!(
                r#" marker-start="url(#diagram_flowchart-v2-pointStart__ef4444)""#,
                r#" marker-end="url(#diagram_flowchart-v2-pointEnd__ef4444)""#,
            )
        );
    }

    #[test]
    fn uncolored_edges_keep_base_marker_references_and_emit_no_extra_defs() {
        let edge = edge("edge", "arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let meter = meter();
        plan.register_edge(&edge, None, &meter).unwrap();

        let mut attributes = String::new();
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "edge",
            0,
            &meter,
        )
        .expect("base marker reference");
        assert_eq!(
            attributes,
            r#" marker-end="url(#diagram_flowchart-v2-pointEnd)""#
        );

        let mut defs = String::new();
        plan.push_extra_markers(&mut defs, "diagram", "flowchart-v2", false);
        assert!(defs.is_empty());
    }

    #[test]
    fn marker_color_normalization_work_is_admitted_atomically_before_scan() {
        let edge = edge("edge", "arrow_point");
        let color = format!("#{}", "a".repeat(64));
        let normalization_work = 1 + color.len().div_ceil(64);
        assert_eq!(normalization_work, 3);

        let short_meter = meter_with_limits(usize::MAX, normalization_work);
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        let error = plan
            .register_edge(&edge, Some(&color), &short_meter)
            .expect_err("normalization must be rejected before scanning the color");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert_eq!(
            short_meter.used(),
            1,
            "only the edge admission may be charged"
        );
        assert!(plan.variants.is_empty());
        assert!(plan.edge_occurrences.is_empty());

        let exact_work = 1 + normalization_work + 1 + 1;
        let exact_meter = meter();
        let mut exact_plan = FlowchartMarkerEmissionPlan::new(false);
        exact_plan
            .register_edge(&edge, Some(&color), &exact_meter)
            .expect("normalization, reference, and variant work must be charged once each");
        assert_eq!(exact_meter.used(), exact_work);
    }

    #[test]
    fn agentflow_hierarchy_definitions_are_included_in_the_svg_budget() {
        let diagram_id = "diagram<&";
        let mut emitted = String::new();
        push_base_markers(&mut emitted, diagram_id, "agentflow");
        let exact_meter = meter_with_limits(emitted.len(), usize::MAX);
        let mut exact_plan = FlowchartMarkerEmissionPlan::new(false);
        exact_plan
            .finalize_svg_budget(diagram_id, "agentflow", false, &exact_meter)
            .unwrap();
        assert_eq!(exact_plan.base_marker_bytes, emitted.len());
        assert_eq!(exact_meter.projected_svg_bytes(), emitted.len());
        let short_meter = meter_with_limits(emitted.len() - 1, usize::MAX);
        let mut short_plan = FlowchartMarkerEmissionPlan::new(false);
        let error = short_plan
            .finalize_svg_budget(diagram_id, "agentflow", false, &short_meter)
            .expect_err("hierarchy definitions cannot escape the precharged SVG ceiling");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert_eq!(short_meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn marker_plan_precharges_exact_serialized_bytes_and_work() {
        let edge = edge("edge", "double_arrow_circle");
        let probe_meter = meter();
        let mut probe = FlowchartMarkerEmissionPlan::new(false);
        probe
            .register_edge(&edge, Some("#ef4444"), &probe_meter)
            .unwrap();
        probe
            .finalize_svg_budget("diagram", "flowchart-v2", false, &probe_meter)
            .unwrap();
        let projected =
            probe.base_marker_bytes + probe.extra_marker_bytes + probe.edge_marker_attribute_bytes;
        let work = probe_meter.used();
        assert!(projected > 0);
        assert!(work > 0);
        assert_eq!(probe_meter.projected_svg_bytes(), projected);

        let exact_meter = meter_with_limits(projected, work);
        let mut exact = FlowchartMarkerEmissionPlan::new(false);
        exact
            .register_edge(&edge, Some("#ef4444"), &exact_meter)
            .unwrap();
        exact
            .finalize_svg_budget("diagram", "flowchart-v2", false, &exact_meter)
            .unwrap();
        assert_eq!(exact_meter.used(), work);
        assert_eq!(exact_meter.projected_svg_bytes(), projected);

        let short_svg_meter = meter_with_limits(projected - 1, usize::MAX);
        let mut short_svg = FlowchartMarkerEmissionPlan::new(false);
        short_svg
            .register_edge(&edge, Some("#ef4444"), &short_svg_meter)
            .unwrap();
        let error = short_svg
            .finalize_svg_budget("diagram", "flowchart-v2", false, &short_svg_meter)
            .expect_err("marker bytes above the ceiling must fail before emission");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert_eq!(short_svg_meter.projected_svg_bytes(), 0);

        let short_work_meter = meter_with_limits(usize::MAX, work - 1);
        let mut short_work = FlowchartMarkerEmissionPlan::new(false);
        let result = short_work
            .register_edge(&edge, Some("#ef4444"), &short_work_meter)
            .and_then(|_| {
                short_work.finalize_svg_budget("diagram", "flowchart-v2", false, &short_work_meter)
            });
        assert!(matches!(
            result.expect_err("marker work above the ceiling must fail"),
            crate::Error::ResourceLimitExceeded(_)
        ));
        assert_eq!(short_work_meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn marker_defs_check_the_whole_svg_ceiling_before_each_append() {
        let edge = edge("edge", "double_arrow_cross");
        let probe_meter = meter();
        let mut probe = FlowchartMarkerEmissionPlan::new(false);
        probe
            .register_edge(&edge, Some("#22c55e"), &probe_meter)
            .unwrap();
        probe
            .finalize_svg_budget("diagram", "flowchart-v2", false, &probe_meter)
            .unwrap();
        let projected = probe.base_marker_bytes + probe.extra_marker_bytes;
        let prefix = "x".repeat(probe.edge_marker_attribute_bytes + 1);

        let exact_meter = meter_with_limits(prefix.len() + projected, usize::MAX);
        let mut exact_plan = FlowchartMarkerEmissionPlan::new(false);
        exact_plan
            .register_edge(&edge, Some("#22c55e"), &exact_meter)
            .unwrap();
        exact_plan
            .finalize_svg_budget("diagram", "flowchart-v2", false, &exact_meter)
            .unwrap();
        let exact_defs = FlowchartDefs {
            diagram_id: "diagram",
            diagram_type: "flowchart-v2",
            marker_plan: &exact_plan,
            security_level_loose: false,
            work_meter: &exact_meter,
        };
        let mut exact_out = prefix.clone();
        exact_defs.push_base_markers(&mut exact_out).unwrap();
        exact_defs.push_extra_markers(&mut exact_out).unwrap();
        assert_eq!(exact_out.len(), prefix.len() + projected);

        let short_meter = meter_with_limits(prefix.len() + projected - 1, usize::MAX);
        let mut short_plan = FlowchartMarkerEmissionPlan::new(false);
        short_plan
            .register_edge(&edge, Some("#22c55e"), &short_meter)
            .unwrap();
        short_plan
            .finalize_svg_budget("diagram", "flowchart-v2", false, &short_meter)
            .unwrap();
        let short_defs = FlowchartDefs {
            diagram_id: "diagram",
            diagram_type: "flowchart-v2",
            marker_plan: &short_plan,
            security_level_loose: false,
            work_meter: &short_meter,
        };
        let mut short_out = prefix;
        short_defs.push_base_markers(&mut short_out).unwrap();
        let before_extra = short_out.clone();
        assert!(short_defs.push_extra_markers(&mut short_out).is_err());
        assert_eq!(short_out, before_extra);
    }

    #[test]
    fn marker_attributes_check_the_whole_svg_ceiling_before_append() {
        let edge = edge("edge", "double_arrow_point");
        let probe_meter = meter();
        let mut probe = FlowchartMarkerEmissionPlan::new(false);
        probe
            .register_edge(&edge, Some("#2563eb"), &probe_meter)
            .unwrap();
        probe
            .finalize_svg_budget("diagram", "flowchart-v2", false, &probe_meter)
            .unwrap();
        let definitions = probe.base_marker_bytes + probe.extra_marker_bytes;
        let attributes = probe.edge_marker_attribute_bytes;
        let prefix = "x".repeat(definitions + 1);

        let exact_meter = meter_with_limits(prefix.len() + attributes, usize::MAX);
        let mut exact_plan = FlowchartMarkerEmissionPlan::new(false);
        exact_plan
            .register_edge(&edge, Some("#2563eb"), &exact_meter)
            .unwrap();
        exact_plan
            .finalize_svg_budget("diagram", "flowchart-v2", false, &exact_meter)
            .unwrap();
        let mut exact_out = prefix.clone();
        exact_plan
            .push_edge_marker_attributes(
                &mut exact_out,
                "diagram",
                "flowchart-v2",
                "edge",
                0,
                &exact_meter,
            )
            .unwrap();
        assert_eq!(exact_out.len(), prefix.len() + attributes);

        let short_meter = meter_with_limits(prefix.len() + attributes - 1, usize::MAX);
        let mut short_plan = FlowchartMarkerEmissionPlan::new(false);
        short_plan
            .register_edge(&edge, Some("#2563eb"), &short_meter)
            .unwrap();
        short_plan
            .finalize_svg_budget("diagram", "flowchart-v2", false, &short_meter)
            .unwrap();
        let mut short_out = prefix;
        let before = short_out.clone();
        assert!(
            short_plan
                .push_edge_marker_attributes(
                    &mut short_out,
                    "diagram",
                    "flowchart-v2",
                    "edge",
                    0,
                    &short_meter,
                )
                .is_err()
        );
        assert_eq!(short_out, before);
    }

    fn unsupported_marker_theme() -> crate::diagram_theme::ResolvedDiagramTheme {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch, ThemeTarget,
        };
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ca3579").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::FLOWCHART)
    }

    #[test]
    fn marker_missing_geometry_checkpoint_or_actual_reference_cannot_claim_source_owned_na() {
        let theme = unsupported_marker_theme();
        let work = meter();
        for case in [
            "complete",
            "omitted",
            "empty-geometry",
            "missing-attributes",
            "wrong-reference",
            "extra-attribute",
            "duplicate",
        ] {
            let mut plan = FlowchartMarkerEmissionPlan::new(false);
            plan.terminal = Some(Mutex::new(FlowchartMarkerTerminalReceipt::default()));
            plan.register_edge(&edge("edge", "arrow_point"), Some("#246801"), &work)
                .unwrap();
            let key = crate::flowchart::FlowchartEdgeKey::new(0);
            let mut attributes = String::new();
            plan.push_edge_marker_attributes_for(
                &mut attributes,
                "diagram",
                "flowchart-v2",
                key,
                &work,
            )
            .unwrap();
            let actual = match case {
                "missing-attributes" => String::new(),
                "wrong-reference" => attributes.replace("pointEnd", "circleEnd"),
                "extra-attribute" => format!("{attributes} marker-start=\"url(#unexpected)\""),
                _ => attributes,
            };
            if case != "omitted" {
                for _ in 0..if case == "duplicate" { 2 } else { 1 } {
                    plan.record_edge_checkpoint(
                        FlowchartMarkerPathCheckpoint {
                            key,
                            has_geometry: case != "empty-geometry",
                            attributes: Some(&actual),
                        },
                        crate::flowchart::FlowchartSourceFacetStatus::Admitted,
                        false,
                        "diagram",
                        "flowchart-v2",
                        &work,
                    )
                    .unwrap();
                }
            }
            let evidence = plan
                .finish_theme_evidence(Some(&theme), &work)
                .unwrap()
                .unwrap();
            assert!(evidence.applied().is_empty(), "{case}");
            assert!(evidence.residuals().is_empty(), "{case}");
            assert_eq!(
                evidence.not_applicable_mechanisms().len(),
                usize::from(case == "complete"),
                "{case}"
            );
        }
    }

    #[test]
    fn an_uncheckpointed_edge_without_marker_references_does_not_invalidate_the_marker_domain() {
        let theme = unsupported_marker_theme();
        let work = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        plan.terminal = Some(Mutex::new(FlowchartMarkerTerminalReceipt::default()));
        plan.register_edge(&edge("open", "arrow_open"), None, &work)
            .unwrap();
        plan.register_edge(&edge("marked", "arrow_point"), None, &work)
            .unwrap();
        let key = crate::flowchart::FlowchartEdgeKey::new(1);
        let mut attributes = String::new();
        plan.push_edge_marker_attributes_for(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            key,
            &work,
        )
        .unwrap();
        plan.record_edge_checkpoint(
            FlowchartMarkerPathCheckpoint {
                key,
                has_geometry: true,
                attributes: Some(&attributes),
            },
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            false,
            "diagram",
            "flowchart-v2",
            &work,
        )
        .unwrap();
        let evidence = plan
            .finish_theme_evidence(Some(&theme), &work)
            .unwrap()
            .unwrap();
        assert_eq!(evidence.residuals().len(), 1);
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn marker_checkpoint_work_rejection_does_not_advance_terminal_ownership() {
        let theme = unsupported_marker_theme();
        let work = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        plan.terminal = Some(Mutex::new(FlowchartMarkerTerminalReceipt::default()));
        plan.register_edge(&edge("edge", "double_arrow_point"), Some("#246801"), &work)
            .unwrap();
        let key = crate::flowchart::FlowchartEdgeKey::new(0);
        let mut attributes = String::new();
        plan.push_edge_marker_attributes_for(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            key,
            &work,
        )
        .unwrap();
        let limited = meter_with_limits(usize::MAX, 1);
        assert!(
            plan.record_edge_checkpoint(
                FlowchartMarkerPathCheckpoint {
                    key,
                    has_geometry: true,
                    attributes: Some(&attributes),
                },
                crate::flowchart::FlowchartSourceFacetStatus::Admitted,
                false,
                "diagram",
                "flowchart-v2",
                &limited
            )
            .is_err()
        );
        let terminal = plan.terminal.as_ref().unwrap().lock().unwrap();
        assert!(terminal.valid);
        assert!(terminal.seen_edges.is_empty());
        assert!(terminal.fill_owners.is_empty());
        assert!(terminal.stroke_owners.is_empty());
        drop(terminal);
        let evidence = plan
            .finish_theme_evidence(Some(&theme), &work)
            .unwrap()
            .unwrap();
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn unexpected_actual_marker_reference_on_an_open_edge_keeps_the_domain_incomplete() {
        let theme = unsupported_marker_theme();
        let work = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new(false);
        plan.terminal = Some(Mutex::new(FlowchartMarkerTerminalReceipt::default()));
        plan.register_edge(&edge("open", "arrow_open"), None, &work)
            .unwrap();
        plan.record_edge_checkpoint(
            FlowchartMarkerPathCheckpoint {
                key: crate::flowchart::FlowchartEdgeKey::new(0),
                has_geometry: true,
                attributes: Some(" marker-end=\"url(#unexpected)\""),
            },
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            false,
            "diagram",
            "flowchart-v2",
            &work,
        )
        .unwrap();
        let evidence = plan
            .finish_theme_evidence(Some(&theme), &work)
            .unwrap()
            .unwrap();
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }
}
#[cfg(test)]
mod paint_security_tests {
    use super::*;

    #[test]
    fn strict_mode_drops_raw_stroke_tokens_but_loose_mode_preserves_them() {
        for (raw, loose, expected) in [
            ("stroke:DarkGray", false, None),
            (" stroke:DarkGray", false, None),
            ("#333", false, Some("#333")),
            ("stroke:#123456", false, Some("stroke:#123456")),
            ("stroke:DarkGray", true, Some("stroke:DarkGray")),
        ] {
            let work = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let edge = super::tests::edge("edge", "arrow_point");
            let mut plan = FlowchartMarkerEmissionPlan::new(false);
            plan.register_edge(&edge, Some(raw), &work).unwrap();
            plan.finalize_svg_budget("diagram", "flowchart-v2", loose, &work)
                .unwrap();
            let defs = FlowchartDefs {
                diagram_id: "diagram",
                diagram_type: "flowchart-v2",
                marker_plan: &plan,
                security_level_loose: loose,
                work_meter: &work,
            };
            let mut out = String::from("<svg><defs>");
            defs.push_extra_markers(&mut out).unwrap();
            out.push_str("</defs></svg>");
            let document = roxmltree::Document::parse(&out).unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker"))
                .expect("colored marker retains its identity");
            assert_eq!(
                marker.attribute("id"),
                Some(format!("diagram_flowchart-v2-pointEnd_{}", marker_color_id(raw)).as_str()),
            );
            let shape = marker
                .children()
                .find(|node| node.is_element())
                .expect("colored marker shape");
            for attribute in ["stroke", "fill"] {
                assert_eq!(
                    shape.attribute(attribute),
                    expected,
                    "raw={raw:?}, loose={loose}, attribute={attribute}: {out}",
                );
            }
        }
    }
}
