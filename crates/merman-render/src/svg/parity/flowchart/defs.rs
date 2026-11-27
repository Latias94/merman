//! Flowchart SVG defs and marker emission.

use std::fmt;

use indexmap::IndexMap;
use merman_theme_contract::v2::MarkerStyle;
use rustc_hash::FxHashMap;

use merman_render_flow::markers::{
    FlowchartMarkerVariantKey, FlowchartMarkerVariantMode, base_marker_count, casing_marker_filled,
    flowchart_marker_shape_spec, push_base_markers, push_colored_marker,
    write_flowchart_marker_id_xml,
};

use super::super::util::escape_xml_display;
use super::edge_fill::FlowchartEdgeFillPlan;
use super::{
    FlowchartHierarchyPlan, FlowchartMarkerBase, FlowchartRenderCtx,
    flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
};

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
pub(super) fn compact_classic_point_end(
    color: merman_render_core::paint_scene::SceneColor,
) -> merman_render_core::paint_scene::SceneMarker<'static> {
    use merman_render_core::paint_scene::{
        SceneBox, SceneMarker, SceneMarkerOrient, SceneMarkerUnits, ScenePaint, ScenePathCommand,
        SceneStroke,
    };

    use merman_render_flow::markers::CLASSIC_POINT_END_VIEW_BOX;

    const WIDTH: f64 = CLASSIC_POINT_END_VIEW_BOX[0];
    const HEIGHT: f64 = CLASSIC_POINT_END_VIEW_BOX[1];
    const COMMANDS: [ScenePathCommand; 4] = [
        ScenePathCommand::MoveTo([0.0, 0.0]),
        ScenePathCommand::LineTo([WIDTH, HEIGHT / 2.0]),
        ScenePathCommand::LineTo([0.0, HEIGHT]),
        ScenePathCommand::Close,
    ];
    let spec = flowchart_marker_shape_spec(FlowchartMarkerBase::PointEnd, false);
    SceneMarker {
        view_box: Some(SceneBox {
            x: 0.0,
            y: 0.0,
            width: spec.viewport.view_box[0],
            height: spec.viewport.view_box[1],
        }),
        reference: spec.viewport.reference,
        width: spec.viewport.size[0],
        height: spec.viewport.size[1],
        units: SceneMarkerUnits::UserSpaceOnUse,
        orient: SceneMarkerOrient::Auto,
        commands: &COMMANDS,
        paint: ScenePaint {
            fill: Some(color),
            stroke: Some(SceneStroke {
                color,
                width: 1.0,
                dash: None,
            }),
        },
    }
}

#[cfg(any(feature = "layout-shoal", test))]
pub(super) fn measure_marker(
    base: Option<FlowchartMarkerBase>,
    setback: Option<f64>,
) -> crate::graph_layout::Marker {
    let Some(base) = base else {
        return Default::default();
    };
    let classic = flowchart_marker_shape_spec(base, false).viewport;
    let margin = flowchart_marker_shape_spec(base, base.supports_margin()).viewport;
    crate::graph_layout::Marker {
        trim: setback.unwrap_or_default(),
        depth: classic.size[0]
            .max(margin.size[0])
            .max(classic.rotation_radius())
            .max(margin.rotation_radius()),
        width: classic.size[1].max(margin.size[1]),
    }
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

#[derive(Debug, Clone, Copy)]
enum FlowchartMarkerColor<'a> {
    Source(&'a str),
    Adaptive(&'a str),
    Casing(&'a str),
}

#[derive(Debug)]
struct PreparedFlowchartMarkerColor<'a> {
    raw: &'a str,
    id: String,
    mode: FlowchartMarkerVariantMode,
}

impl<'a> PreparedFlowchartMarkerColor<'a> {
    fn prepare(
        color: FlowchartMarkerColor<'a>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let (raw, mode) = match color {
            FlowchartMarkerColor::Source(raw) => (raw, FlowchartMarkerVariantMode::Source),
            FlowchartMarkerColor::Adaptive(raw) => (raw, FlowchartMarkerVariantMode::AdaptiveInk),
            FlowchartMarkerColor::Casing(raw) => (raw, FlowchartMarkerVariantMode::Casing),
        };
        let normalization_work = 1usize
            .checked_add(raw.len().div_ceil(64))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.charge(normalization_work)?;
        let mut id = marker_color_id(raw);
        match mode {
            FlowchartMarkerVariantMode::Source => {}
            // Mermaid source colors normalize to alphanumerics and underscores only.
            FlowchartMarkerVariantMode::AdaptiveInk => id.insert_str(0, "shoal-"),
            FlowchartMarkerVariantMode::Casing => id.insert_str(0, "shoal-casing-"),
        }
        Ok(Self { raw, id, mode })
    }
}

#[derive(Debug)]
pub(in crate::svg::parity::flowchart) struct FlowchartMarkerEmissionPlan {
    #[cfg(test)]
    edges: FxHashMap<String, Vec<FlowchartEdgeMarkerReferences>>,
    edge_occurrences: FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgeMarkerReferences>,
    casing_edge_occurrences:
        FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgeMarkerReferences>,
    variants: IndexMap<FlowchartMarkerVariantKey, String>,
    marker_style: MarkerStyle,
    base_marker_bytes: usize,
    bpmn_markers: bool,
    extra_marker_bytes: usize,
    edge_marker_attribute_bytes: usize,
    serialization_input_work: usize,
}

impl FlowchartMarkerEmissionPlan {
    fn new() -> Self {
        Self {
            #[cfg(test)]
            edges: FxHashMap::default(),
            edge_occurrences: FxHashMap::default(),
            casing_edge_occurrences: FxHashMap::default(),
            variants: IndexMap::new(),
            marker_style: MarkerStyle::Classic,
            base_marker_bytes: 0,
            bpmn_markers: false,
            extra_marker_bytes: 0,
            edge_marker_attribute_bytes: 0,
            serialization_input_work: 0,
        }
    }

    pub(in crate::svg::parity::flowchart) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy_plan: &FlowchartHierarchyPlan<'_>,
        fill_plan: &FlowchartEdgeFillPlan,
    ) -> crate::Result<Self> {
        let look = ctx.compatibility.look;
        let hand_drawn = look.is_hand_drawn();
        let neo = look.is_neo();
        let mut plan = Self::new();
        if let Some(native) = ctx.native {
            plan.marker_style = native.form.marker_style;
        }
        for &edge in hierarchy_plan.ordered_edges() {
            Self::bind_path_sources_for(ctx, hierarchy_plan, edge.key)?;
            let edge_styles = ctx.edge_style_plan.edge_for(edge.key)?;
            let marker_color = edge_styles
                .edge_marker_color(hand_drawn)
                .map(FlowchartMarkerColor::Source)
                .or_else(|| fill_plan.ink(edge.key).map(FlowchartMarkerColor::Adaptive));
            let margin = neo && !ctx.edge_style_plan.animation_for(edge.key)?.is_active();
            plan.register_edge_with_identity(
                edge.key,
                edge.edge,
                marker_color,
                margin,
                ctx.document_ids().marker_scope(),
                ctx.diagram_type,
                ctx.work_meter,
            )?;
            if let Some(casing_color) = fill_plan.casing_color(edge.key) {
                let hex = casing_color.to_string();
                plan.register_casing_edge(
                    edge.key,
                    edge.edge,
                    &hex,
                    margin,
                    ctx.document_ids().marker_scope(),
                    ctx.diagram_type,
                    ctx.work_meter,
                )?;
            }
        }
        plan.finalize_svg_budget(
            ctx.document_ids().marker_scope(),
            ctx.diagram_type,
            ctx.security_level_loose,
            ctx.work_meter,
        )?;
        Ok(plan)
    }

    pub(super) fn bind_path_sources_for(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<()> {
        ctx.edge_style_plan.bind_path_sources_for(
            key,
            ctx.compatibility.html_labels,
            ctx.uses_elk_adapter_dom || ctx.swimlane_direction.is_some(),
            hierarchy.edge_root(key)?.is_some(),
            ctx.work_meter,
        )
    }

    fn variant(&self, index: usize) -> (&FlowchartMarkerVariantKey, &str) {
        let (key, raw_color) = self
            .variants
            .get_index(index)
            .expect("prepared Flowchart marker variant index");
        (key, raw_color.as_str())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Marker registration carries the edge identity, independent marker facets, and document scope to one checked emission boundary."
    )]
    fn register_edge_with_identity(
        &mut self,
        key: crate::flowchart::FlowchartEdgeKey,
        edge: &crate::flowchart::FlowEdge,
        marker_color: Option<FlowchartMarkerColor<'_>>,
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
                .map(|raw| PreparedFlowchartMarkerColor::prepare(raw, work_meter))
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

    #[allow(
        clippy::too_many_arguments,
        reason = "Casing marker registration parallels edge marker registration at one checked emission boundary."
    )]
    fn register_casing_edge(
        &mut self,
        key: crate::flowchart::FlowchartEdgeKey,
        edge: &crate::flowchart::FlowEdge,
        casing_color: &str,
        margin: bool,
        diagram_id: &str,
        diagram_type: &str,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        work_meter.charge(1)?;
        let start_base = flowchart_edge_marker_start_base(edge);
        let end_base = flowchart_edge_marker_end_base(edge);
        if start_base.is_none() && end_base.is_none() {
            return Ok(());
        }
        let prepared_color = PreparedFlowchartMarkerColor::prepare(
            FlowchartMarkerColor::Casing(casing_color),
            work_meter,
        )?;
        let start = start_base
            .map(|base| self.register_reference(base, Some(&prepared_color), margin, work_meter))
            .transpose()?;
        let end = end_base
            .map(|base| self.register_reference(base, Some(&prepared_color), margin, work_meter))
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
        self.casing_edge_occurrences.insert(key, references);
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
            marker_color.map(FlowchartMarkerColor::Source),
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
        self.bpmn_markers |= !base.supports_margin();
        let margin = margin && base.supports_margin();
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
            mode: marker_color.mode,
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
        let base_marker_count = base_marker_count(diagram_type, self.bpmn_markers);
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
        push_base_markers(
            &mut base_counter,
            diagram_id,
            diagram_type,
            self.marker_style,
            self.bpmn_markers,
        );
        let base_marker_bytes = base_counter.finish(work_meter)?;

        let extra_marker_input_bytes = self.variants.iter().try_fold(
            0usize,
            |bytes, (key, raw_color)| -> crate::Result<usize> {
                let diagram_type_bytes = diagram_type
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                let color_scan_count = key.color_scan_count(self.marker_style);
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

    pub(in crate::svg::parity::flowchart) fn push_casing_marker_attributes_for(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        diagram_id: &str,
        diagram_type: &str,
        key: crate::flowchart::FlowchartEdgeKey,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let Some(references) = self.casing_edge_occurrences.get(&key) else {
            return Ok(());
        };
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
                self.marker_style,
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
        push_base_markers(
            out,
            self.diagram_id,
            self.diagram_type,
            self.marker_plan.marker_style,
            self.marker_plan.bpmn_markers,
        );
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

fn push_casing_marker(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    key: &FlowchartMarkerVariantKey,
    color: Option<&str>,
    marker_style: MarkerStyle,
) {
    use crate::number_format::canonical_number as number;

    let spec = flowchart_marker_shape_spec(key.base, key.margin);
    let [width, height] = spec.viewport.view_box;
    let [reference_x, reference_y] = spec.viewport.reference;
    let [marker_width, marker_height] = spec.viewport.size;
    let scale = (marker_width / width).min(marker_height / height);
    let custom_arrow = matches!(
        key.base,
        FlowchartMarkerBase::PointStart | FlowchartMarkerBase::PointEnd
    )
    .then(|| crate::theme::marker_svg::arrow_path(marker_style))
    .flatten();
    let base_stroke_vb = if custom_arrow.is_some() {
        1.0
    } else {
        match (key.base, key.margin) {
            (
                FlowchartMarkerBase::PointStart
                | FlowchartMarkerBase::PointEnd
                | FlowchartMarkerBase::CircleStart
                | FlowchartMarkerBase::CircleEnd,
                true,
            ) => 0.0,
            (
                FlowchartMarkerBase::PointStart
                | FlowchartMarkerBase::PointEnd
                | FlowchartMarkerBase::CircleStart
                | FlowchartMarkerBase::CircleEnd,
                false,
            ) => 1.0,
            (FlowchartMarkerBase::CrossStart | FlowchartMarkerBase::CrossEnd, false) => 2.0,
            (FlowchartMarkerBase::CrossStart | FlowchartMarkerBase::CrossEnd, true) => 2.5,
            (
                FlowchartMarkerBase::OpenArrowStart
                | FlowchartMarkerBase::OpenArrowEnd
                | FlowchartMarkerBase::HollowCircleStart
                | FlowchartMarkerBase::HollowCircleEnd,
                _,
            ) => 1.4,
        }
    };
    let casing_stroke_vb = base_stroke_vb + super::edge_fill::CASING_WIDTH_ADD / scale;
    let pad = (casing_stroke_vb / 2.0 + 1.0).ceil();
    let padded_w = width + 2.0 * pad;
    let padded_h = height + 2.0 * pad;

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
        r#"" class="merman-edge-casing-marker" viewBox="0 0 {} {}" refX="{}" refY="{}" markerUnits="userSpaceOnUse" markerWidth="{}" markerHeight="{}" orient="auto">"#,
        number(padded_w),
        number(padded_h),
        number(reference_x + pad),
        number(reference_y + pad),
        number(padded_w * scale),
        number(padded_h * scale),
    );

    if let Some(path) = custom_arrow {
        let start = matches!(key.base, FlowchartMarkerBase::PointStart);
        let (translate, sx) = if start { (width, -width) } else { (0.0, width) };
        let _ = write!(
            out,
            r#"<path d="{path}" transform="matrix({} 0 0 {} {} {})""#,
            number(sx / 10.0),
            number(height / 10.0),
            number(translate + pad),
            number(pad),
        );
    } else {
        let geom = match (key.base, key.margin) {
            (FlowchartMarkerBase::PointEnd, false) => r#"<path d="M 0 0 L 10 5 L 0 10 z""#,
            (FlowchartMarkerBase::PointStart, false) => r#"<path d="M 0 5 L 10 10 L 10 0 z""#,
            (FlowchartMarkerBase::PointEnd, true) => r#"<path d="M 0 0 L 11.5 7 L 0 14 z""#,
            (FlowchartMarkerBase::PointStart, true) => r#"<polygon points="0,7 11.5,14 11.5,0""#,
            (FlowchartMarkerBase::CircleEnd | FlowchartMarkerBase::CircleStart, _) => {
                r#"<circle cx="5" cy="5" r="5""#
            }
            (FlowchartMarkerBase::CrossEnd | FlowchartMarkerBase::CrossStart, false) => {
                r#"<path d="M 1,1 l 9,9 M 10,1 l -9,9""#
            }
            (FlowchartMarkerBase::CrossEnd | FlowchartMarkerBase::CrossStart, true) => {
                r#"<path d="M 1,1 L 14,14 M 1,14 L 14,1""#
            }
            (FlowchartMarkerBase::OpenArrowEnd, _) => r#"<path d="M 1 1 L 11 6 L 1 11""#,
            (FlowchartMarkerBase::OpenArrowStart, _) => r#"<path d="M 11 1 L 1 6 L 11 11""#,
            (FlowchartMarkerBase::HollowCircleStart | FlowchartMarkerBase::HollowCircleEnd, _) => {
                r#"<circle cx="5" cy="5" r="3.6""#
            }
        };
        let _ = write!(
            out,
            r#"{geom} transform="translate({} {})""#,
            number(pad),
            number(pad)
        );
    }

    let filled = casing_marker_filled(key.base, marker_style);
    if let Some(color) = color {
        let color = escape_xml_display(color);
        if filled {
            let _ = write!(out, r#" fill="{color}" stroke="{color}""#);
        } else {
            let _ = write!(out, r#" fill="none" stroke="{color}""#);
        }
    } else if !filled {
        let _ = out.write_str(r#" fill="none""#);
    }
    let _ = write!(
        out,
        r#" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/></marker>"#,
        number(casing_stroke_vb),
    );
}

fn push_extra_marker(
    out: &mut impl fmt::Write,
    diagram_id: &str,
    diagram_type: &str,
    key: &FlowchartMarkerVariantKey,
    raw_color: &str,
    security_level_loose: bool,
    marker_style: MarkerStyle,
) {
    let raw_color = raw_color.trim_end_matches(';');
    let normalized_color;
    let color = if security_level_loose {
        Some(raw_color)
    } else {
        normalized_color =
            merman_markup::sanitize::dompurify_normalize_dom_uri_attribute(raw_color);
        normalized_color.as_deref()
    };
    if key.mode == FlowchartMarkerVariantMode::Casing {
        push_casing_marker(out, diagram_id, diagram_type, key, color, marker_style);
        return;
    }
    push_colored_marker(out, diagram_id, diagram_type, key, color, marker_style);
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

    #[test]
    fn bpmn_markers_keep_their_source_viewports_no_fill_and_no_neo_margin_variant() {
        let work = meter();
        let color =
            PreparedFlowchartMarkerColor::prepare(FlowchartMarkerColor::Source("#2468ac"), &work)
                .unwrap();
        let mut plan = FlowchartMarkerEmissionPlan::new();
        for base in [
            FlowchartMarkerBase::OpenArrowEnd,
            FlowchartMarkerBase::OpenArrowStart,
            FlowchartMarkerBase::HollowCircleStart,
            FlowchartMarkerBase::HollowCircleEnd,
        ] {
            let reference = plan
                .register_reference(base, Some(&color), true, &work)
                .unwrap();
            assert!(!reference.margin);
            let (key, raw_color) = plan.variant(reference.variant_index.unwrap());
            let mut svg = String::from("<svg>");
            push_extra_marker(
                &mut svg,
                "diagram",
                "flowchart-v2",
                key,
                raw_color,
                false,
                MarkerStyle::Classic,
            );
            svg.push_str("</svg>");
            let document = crate::xml_dom::Document::parse(&svg).unwrap();
            let marker = document
                .descendants()
                .find(|node| node.has_tag_name("marker"))
                .unwrap();
            assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
            assert!(!marker.attribute("id").unwrap().contains("-margin"));
            let shape = marker.children().find(|node| node.is_element()).unwrap();
            assert_eq!(shape.attribute("stroke"), Some("#2468ac"));
            assert_eq!(shape.attribute("fill"), None);
            assert!(shape.attribute("style").unwrap().contains("fill: none"));
            if shape.has_tag_name("circle") {
                assert_eq!(shape.attribute("r"), Some("3.6"));
                assert_eq!(marker.attribute("markerWidth"), Some("10"));
            } else {
                assert_eq!(marker.attribute("markerWidth"), Some("12"));
                assert!(
                    shape
                        .attribute("style")
                        .unwrap()
                        .contains("stroke-linejoin: round")
                );
            }
        }
    }

    #[test]
    fn bpmn_marker_references_defs_and_budget_share_one_prepared_plan() {
        let work = meter();
        let mut plan = FlowchartMarkerEmissionPlan::new();
        for (id, edge_type) in [
            ("open", "arrow_open_head"),
            ("hollow", "arrow_hollow_circle"),
        ] {
            plan.register_edge(&edge(id, edge_type), Some("#2468ac"), &work)
                .unwrap();
        }
        plan.finalize_svg_budget("diagram", "flowchart-v2", false, &work)
            .unwrap();
        let mut output = String::new();
        let defs = FlowchartDefs {
            diagram_id: "diagram",
            diagram_type: "flowchart-v2",
            marker_plan: &plan,
            security_level_loose: false,
            work_meter: &work,
        };
        defs.push_base_markers(&mut output).unwrap();
        defs.push_extra_markers(&mut output).unwrap();
        assert_eq!(
            output.len(),
            plan.base_marker_bytes + plan.extra_marker_bytes
        );
        assert!(output.contains("diagram_flowchart-v2-openArrowStart"));
        assert!(output.contains("diagram_flowchart-v2-hollowCircleEnd"));
        let mut attributes = String::new();
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "open",
            0,
            &work,
        )
        .unwrap();
        assert!(attributes.contains("openArrowEnd__2468ac"));
        attributes.clear();
        plan.push_edge_marker_attributes(
            &mut attributes,
            "diagram",
            "flowchart-v2",
            "hollow",
            0,
            &work,
        )
        .unwrap();
        assert!(attributes.contains("hollowCircleEnd__2468ac"));
        let limited = meter_with_limits(
            plan.base_marker_bytes + plan.extra_marker_bytes + plan.edge_marker_attribute_bytes - 1,
            usize::MAX,
        );
        let mut rejected = FlowchartMarkerEmissionPlan::new();
        for (id, edge_type) in [
            ("open", "arrow_open_head"),
            ("hollow", "arrow_hollow_circle"),
        ] {
            rejected
                .register_edge(&edge(id, edge_type), Some("#2468ac"), &limited)
                .unwrap();
        }
        assert!(
            rejected
                .finalize_svg_budget("diagram", "flowchart-v2", false, &limited)
                .is_err()
        );
    }

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
            step: None,
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
        push_base_markers(
            &mut defs,
            "diagram",
            "flowchart-v2",
            MarkerStyle::Classic,
            false,
        );
        let svg = format!("<svg>{defs}</svg>");
        let doc = crate::xml_dom::Document::parse(&svg).unwrap();
        for (base, margin) in [
            FlowchartMarkerBase::PointEnd,
            FlowchartMarkerBase::PointStart,
            FlowchartMarkerBase::CircleEnd,
            FlowchartMarkerBase::CircleStart,
            FlowchartMarkerBase::CrossEnd,
            FlowchartMarkerBase::CrossStart,
        ]
        .into_iter()
        .flat_map(|base| [false, true].map(|margin| (base, margin)))
        {
            let spec = flowchart_marker_shape_spec(base, margin);
            let id = format!(
                "diagram_flowchart-v2-{}{}",
                base.id_suffix(),
                if margin { "-margin" } else { "" },
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
    fn base_marker_defs_match_mermaid_11_16_1_bytes() {
        let mut defs = String::new();
        push_base_markers(
            &mut defs,
            "diagram",
            "flowchart-v2",
            MarkerStyle::Classic,
            false,
        );

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
        push_base_markers(
            &mut base_defs,
            "diagram",
            "flowchart-v2",
            MarkerStyle::Classic,
            false,
        );

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
                mode: FlowchartMarkerVariantMode::Source,
            };
            let mut actual = String::new();
            push_extra_marker(
                &mut actual,
                "diagram",
                "flowchart-v2",
                &key,
                "#2468ac",
                false,
                MarkerStyle::Classic,
            );

            assert_eq!(
                marker_chunk(&actual, &colored_id),
                expected,
                "base={base:?}, margin={margin}"
            );
        }
    }

    #[test]
    fn colored_marker_plan_defines_every_referenced_base_in_first_use_order() {
        let circle = edge("circle", "double_arrow_circle");
        let cross = edge("cross", "double_arrow_cross");
        let point = edge("point", "double_arrow_point");
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut plan = FlowchartMarkerEmissionPlan::new();
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
        let mut exact_plan = FlowchartMarkerEmissionPlan::new();
        exact_plan
            .register_edge(&edge, Some(&color), &exact_meter)
            .expect("normalization, reference, and variant work must be charged once each");
        assert_eq!(exact_meter.used(), exact_work);
    }

    #[test]
    fn agentflow_hierarchy_definitions_are_included_in_the_svg_budget() {
        let diagram_id = "diagram<&";
        let mut emitted = String::new();
        push_base_markers(
            &mut emitted,
            diagram_id,
            "agentflow",
            MarkerStyle::Classic,
            false,
        );
        let exact_meter = meter_with_limits(emitted.len(), usize::MAX);
        let mut exact_plan = FlowchartMarkerEmissionPlan::new();
        exact_plan
            .finalize_svg_budget(diagram_id, "agentflow", false, &exact_meter)
            .unwrap();
        assert_eq!(exact_plan.base_marker_bytes, emitted.len());
        assert_eq!(exact_meter.projected_svg_bytes(), emitted.len());
        let short_meter = meter_with_limits(emitted.len() - 1, usize::MAX);
        let mut short_plan = FlowchartMarkerEmissionPlan::new();
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
        let mut probe = FlowchartMarkerEmissionPlan::new();
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
        let mut exact = FlowchartMarkerEmissionPlan::new();
        exact
            .register_edge(&edge, Some("#ef4444"), &exact_meter)
            .unwrap();
        exact
            .finalize_svg_budget("diagram", "flowchart-v2", false, &exact_meter)
            .unwrap();
        assert_eq!(exact_meter.used(), work);
        assert_eq!(exact_meter.projected_svg_bytes(), projected);

        let short_svg_meter = meter_with_limits(projected - 1, usize::MAX);
        let mut short_svg = FlowchartMarkerEmissionPlan::new();
        short_svg
            .register_edge(&edge, Some("#ef4444"), &short_svg_meter)
            .unwrap();
        let error = short_svg
            .finalize_svg_budget("diagram", "flowchart-v2", false, &short_svg_meter)
            .expect_err("marker bytes above the ceiling must fail before emission");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert_eq!(short_svg_meter.projected_svg_bytes(), 0);

        let short_work_meter = meter_with_limits(usize::MAX, work - 1);
        let mut short_work = FlowchartMarkerEmissionPlan::new();
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
        let mut probe = FlowchartMarkerEmissionPlan::new();
        probe
            .register_edge(&edge, Some("#22c55e"), &probe_meter)
            .unwrap();
        probe
            .finalize_svg_budget("diagram", "flowchart-v2", false, &probe_meter)
            .unwrap();
        let projected = probe.base_marker_bytes + probe.extra_marker_bytes;
        let prefix = "x".repeat(probe.edge_marker_attribute_bytes + 1);

        let exact_meter = meter_with_limits(prefix.len() + projected, usize::MAX);
        let mut exact_plan = FlowchartMarkerEmissionPlan::new();
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
        let mut short_plan = FlowchartMarkerEmissionPlan::new();
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
        let mut probe = FlowchartMarkerEmissionPlan::new();
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
        let mut exact_plan = FlowchartMarkerEmissionPlan::new();
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
        let mut short_plan = FlowchartMarkerEmissionPlan::new();
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
            ("  #333  ", false, Some("#333")),
            ("stroke:#123456", false, None),
            ("st\nroke:#333", false, None),
            ("javascript:alert(1)", false, None),
            ("var(--edge)", false, Some("var(--edge)")),
            ("rgb(12,34,56)", false, Some("rgb(12,34,56)")),
            ("stroke:DarkGray", true, Some("stroke:DarkGray")),
            ("stroke:#123456", true, Some("stroke:#123456")),
        ] {
            let work = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let edge = super::tests::edge("edge", "arrow_point");
            let mut plan = FlowchartMarkerEmissionPlan::new();
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
            let document = crate::xml_dom::Document::parse(&out).unwrap();
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
