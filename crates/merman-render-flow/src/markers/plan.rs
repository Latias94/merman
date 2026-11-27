//! Operation-local marker references, admission, and definition emission.

use std::fmt;

use indexmap::IndexMap;
use merman_model::families::flowchart::FlowEdge;
use merman_render_core::{Error, Result, resources::OperationWorkMeter, svg_output::SvgOutput};
use merman_theme_contract::v2::{Color, MarkerStyle};
use rustc_hash::FxHashMap;

#[cfg(test)]
use super::flowchart_marker_shape_spec;
use super::{
    FlowchartMarkerBase, FlowchartMarkerVariantKey, FlowchartMarkerVariantMode, base_marker_count,
    flowchart_edge_marker_end_base, flowchart_edge_marker_start_base, push_base_markers,
    push_colored_marker, write_flowchart_marker_id_xml,
};
use crate::document::FlowchartDocumentScope;
use crate::projection::{FlowchartEdgeKey, FlowchartRenderEdgeRef};

#[derive(Debug, Clone, Copy)]
struct FlowchartMarkerReference {
    base: FlowchartMarkerBase,
    margin: bool,
    variant_index: Option<usize>,
}

#[derive(Debug)]
struct FlowchartEdgeMarkerReferences {
    start: Option<FlowchartMarkerReference>,
    end: Option<FlowchartMarkerReference>,
    serialized_bytes: usize,
}

#[derive(Debug, Clone, Copy)]
enum FlowchartMarkerColor<'a> {
    Source(&'a str),
    Adaptive(&'a str),
}

#[derive(Debug)]
struct PreparedFlowchartMarkerColor<'a> {
    raw: &'a str,
    id: String,
    mode: FlowchartMarkerVariantMode,
}

impl<'a> PreparedFlowchartMarkerColor<'a> {
    fn prepare(color: FlowchartMarkerColor<'a>, work_meter: &OperationWorkMeter) -> Result<Self> {
        let (raw, mode) = match color {
            FlowchartMarkerColor::Source(raw) => (raw, FlowchartMarkerVariantMode::Source),
            FlowchartMarkerColor::Adaptive(raw) => (raw, FlowchartMarkerVariantMode::AdaptiveInk),
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
        }
        Ok(Self { raw, id, mode })
    }
}

#[derive(Clone, Copy)]
pub struct FlowchartMarkerSurface {
    pub clip: usize,
    pub color: Color,
}

/// Resolved marker inputs for one rendered occurrence, in graph emission order.
pub struct FlowchartEdgeMarkers<'a> {
    pub edge: FlowchartRenderEdgeRef<'a>,
    pub source_color: Option<&'a str>,
    pub adaptive_ink: Option<&'static str>,
    pub surfaces: &'a [FlowchartMarkerSurface],
    pub margin: bool,
}

/// Occurrence references and marker definitions admitted for one document and operation.
#[derive(Debug)]
pub struct FlowchartMarkerEmissionPlan<'a> {
    diagram_id: &'a str,
    diagram_type: &'a str,
    security_level_loose: bool,
    work_meter: &'a OperationWorkMeter,
    edge_occurrences: FxHashMap<FlowchartEdgeKey, FlowchartEdgeMarkerReferences>,
    surface_occurrences: FxHashMap<(FlowchartEdgeKey, usize), FlowchartEdgeMarkerReferences>,
    variants: IndexMap<FlowchartMarkerVariantKey, String>,
    marker_style: MarkerStyle,
    base_marker_bytes: usize,
    bpmn_markers: bool,
    extra_marker_bytes: usize,
    edge_marker_attribute_bytes: usize,
    serialization_input_work: usize,
}

impl<'a> FlowchartMarkerEmissionPlan<'a> {
    fn new(
        diagram_id: &'a str,
        diagram_type: &'a str,
        security_level_loose: bool,
        marker_style: MarkerStyle,
        work_meter: &'a OperationWorkMeter,
    ) -> Self {
        Self {
            diagram_id,
            diagram_type,
            security_level_loose,
            work_meter,
            edge_occurrences: FxHashMap::default(),
            surface_occurrences: FxHashMap::default(),
            variants: IndexMap::new(),
            marker_style,
            base_marker_bytes: 0,
            bpmn_markers: false,
            extra_marker_bytes: 0,
            edge_marker_attribute_bytes: 0,
            serialization_input_work: 0,
        }
    }

    /// Consume each resolved occurrence immediately so its admission precedes the next lookup.
    pub fn prepare<'edge>(
        document_scope: &'a FlowchartDocumentScope,
        diagram_type: &'a str,
        security_level_loose: bool,
        marker_style: MarkerStyle,
        work_meter: &'a OperationWorkMeter,
        edges: impl IntoIterator<Item = Result<FlowchartEdgeMarkers<'edge>>>,
    ) -> Result<Self> {
        let mut plan = Self::new(
            document_scope.as_str(),
            diagram_type,
            security_level_loose,
            marker_style,
            work_meter,
        );
        for inputs in edges {
            let FlowchartEdgeMarkers {
                edge,
                source_color,
                adaptive_ink,
                surfaces,
                margin,
            } = inputs?;
            let color = adaptive_ink
                .map(FlowchartMarkerColor::Adaptive)
                .or_else(|| source_color.map(FlowchartMarkerColor::Source));
            plan.register_edge_with_identity(edge.key, edge.edge, color, margin)?;
            for surface in surfaces {
                let hex = surface.color.to_string();
                let references = plan.prepare_references(
                    edge.edge,
                    Some(FlowchartMarkerColor::Adaptive(&hex)),
                    margin,
                )?;
                plan.surface_occurrences
                    .insert((edge.key, surface.clip), references);
            }
        }
        plan.finalize_svg_budget()?;
        Ok(plan)
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
        key: FlowchartEdgeKey,
        edge: &FlowEdge,
        marker_color: Option<FlowchartMarkerColor<'_>>,
        margin: bool,
    ) -> Result<()> {
        let references = self.prepare_references(edge, marker_color, margin)?;
        if self.edge_occurrences.insert(key, references).is_some() {
            return Err(Error::InvalidModel {
                message: format!(
                    "duplicate prepared Flowchart marker occurrence {}",
                    key.semantic_index()
                ),
            });
        }
        Ok(())
    }

    fn prepare_references(
        &mut self,
        edge: &FlowEdge,
        marker_color: Option<FlowchartMarkerColor<'_>>,
        margin: bool,
    ) -> Result<FlowchartEdgeMarkerReferences> {
        let work_meter = self.work_meter;
        let diagram_id = self.diagram_id;
        let diagram_type = self.diagram_type;
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
        Ok(references)
    }

    #[cfg(test)]
    fn register_edge(&mut self, edge: &FlowEdge, marker_color: Option<&str>) -> Result<()> {
        let key = FlowchartEdgeKey::new(self.edge_occurrences.len());
        self.register_edge_with_identity(
            key,
            edge,
            marker_color.map(FlowchartMarkerColor::Source),
            false,
        )
    }

    fn register_reference(
        &mut self,
        base: FlowchartMarkerBase,
        marker_color: Option<&PreparedFlowchartMarkerColor<'_>>,
        margin: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<FlowchartMarkerReference> {
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

    fn finalize_svg_budget(&mut self) -> Result<()> {
        let work_meter = self.work_meter;
        let diagram_id = self.diagram_id;
        let diagram_type = self.diagram_type;
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

        let extra_marker_input_bytes =
            self.variants
                .iter()
                .try_fold(0usize, |bytes, (key, raw_color)| -> Result<usize> {
                    let diagram_type_bytes = diagram_type
                        .len()
                        .checked_mul(2)
                        .ok_or_else(|| work_meter.arithmetic_overflow())?;
                    let color_scan_count = key.color_scan_count();
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
                })?;
        self.preflight_serialization_input(extra_marker_input_bytes.div_ceil(64), work_meter)?;
        let mut extra_counter = SvgByteCounter::default();
        self.write_extra_markers(&mut extra_counter);
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
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let pending = self
            .serialization_input_work
            .checked_add(additional)
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.preflight(pending)?;
        self.serialization_input_work = pending;
        Ok(())
    }

    pub fn push_edge_marker_attributes_for(
        &self,
        out: &mut impl SvgOutput,
        key: FlowchartEdgeKey,
    ) -> Result<()> {
        let references = self
            .edge_occurrences
            .get(&key)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart marker occurrence {}",
                    key.semantic_index()
                ),
            })?;
        self.append_marker_attributes(out, references)
    }

    pub(crate) fn push_surface_marker_attributes_for(
        &self,
        out: &mut impl SvgOutput,
        key: (FlowchartEdgeKey, usize),
    ) -> Result<()> {
        let references = self
            .surface_occurrences
            .get(&key)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Flowchart surface marker {key:?}"),
            })?;
        self.append_marker_attributes(out, references)
    }

    fn append_marker_attributes(
        &self,
        out: &mut impl SvgOutput,
        references: &FlowchartEdgeMarkerReferences,
    ) -> Result<()> {
        self.work_meter
            .check_svg_append(out.len(), references.serialized_bytes)?;
        let before = out.len();
        self.push_prepared_edge_marker_attributes(
            out,
            self.diagram_id,
            self.diagram_type,
            references,
        );
        out.checkpoint()?;
        debug_assert_eq!(out.len() - before, references.serialized_bytes);
        Ok(())
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

    fn write_extra_markers(&self, out: &mut impl fmt::Write) {
        for (key, raw_color) in &self.variants {
            push_extra_marker(
                out,
                self.diagram_id,
                self.diagram_type,
                key,
                raw_color,
                self.security_level_loose,
                self.marker_style,
            );
        }
    }

    pub(crate) fn push_base_markers(&self, out: &mut impl SvgOutput) -> Result<()> {
        self.work_meter
            .check_svg_append(out.len(), self.base_marker_bytes)?;
        let before = out.len();
        push_base_markers(
            out,
            self.diagram_id,
            self.diagram_type,
            self.marker_style,
            self.bpmn_markers,
        );
        out.checkpoint()?;
        debug_assert_eq!(out.len() - before, self.base_marker_bytes);
        Ok(())
    }

    pub(crate) fn push_extra_markers(&self, out: &mut impl SvgOutput) -> Result<()> {
        self.work_meter
            .check_svg_append(out.len(), self.extra_marker_bytes)?;
        let before = out.len();
        self.write_extra_markers(out);
        out.checkpoint()?;
        debug_assert_eq!(out.len() - before, self.extra_marker_bytes);
        Ok(())
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
    fn finish(self, work_meter: &OperationWorkMeter) -> Result<usize> {
        if self.overflowed {
            work_meter.check_svg_append(usize::MAX, 1)?;
            unreachable!("overflowing SVG count must be rejected")
        }
        Ok(self.bytes)
    }
}

#[cfg(test)]
mod tests;
