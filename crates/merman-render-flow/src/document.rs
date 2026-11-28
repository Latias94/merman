//! Balanced Mermaid parity SVG documents for the Flow families.

use std::fmt::Write as _;

use merman_render_core::{
    Bounds, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{self, RootDocument, RootViewportContext, RootViewportSpec, RootedSvg},
    svg_css::{CssSelectorDiagramId, PreparedLookDefs, SvgId},
    svg_output::{BoundedSvgOutput, SvgOutput},
    theme_svg::NEO_SHADOW_OFFSET_PX,
    xml::{escape_attr_display, escape_xml, escape_xml_into},
};

use crate::stylesheet::{FlowchartStylesheetRequest, title_css_class, write_flowchart_stylesheet};

mod ids;
mod ownership;
pub use ids::{FlowchartDocumentId, FlowchartDocumentScope};
pub use ownership::{FlowchartDocumentIdRequest, FlowchartDocumentIds};

pub struct FlowchartSvgDocumentRequest<'a> {
    pub root_viewport: RootViewportContext<'a>,
    pub document_scope: &'a FlowchartDocumentScope,
    pub uses_elk_adapter_dom: bool,
    pub layout_fallback: Option<&'static str>,
    pub use_max_width: bool,
    pub diagram_padding: f64,
    pub source_paint_bounds: Option<Bounds>,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
    pub acc_title: Option<&'a str>,
    pub acc_descr: Option<&'a str>,
    pub diagram_title: Option<&'a str>,
    pub title_anchor_x: f64,
    pub title_top_margin: f64,
}

pub struct FlowchartSvgDocument<'a> {
    use_max_width: bool,
    root_viewport: RootViewportContext<'a>,
    root_spec: RootViewportSpec,
    document_scope: &'a FlowchartDocumentScope,
    uses_elk_adapter_dom: bool,
    layout_fallback: Option<&'static str>,
    acc_title: Option<&'a str>,
    acc_descr: Option<&'a str>,
    diagram_title: Option<&'a str>,
    title_anchor_x: f64,
    title_top_margin: f64,
}

pub fn prepare_flowchart_svg_document(
    request: FlowchartSvgDocumentRequest<'_>,
) -> FlowchartSvgDocument<'_> {
    let mut root_bounds = flowchart_root_bounds(
        request.bbox_min_x,
        request.bbox_min_y,
        request.bbox_max_x,
        request.bbox_max_y,
        request.diagram_padding,
    );
    if let Some(paint) = request.source_paint_bounds {
        root_bounds = root_svg::DiagramBounds::from_extents(
            root_bounds.min_x.min(paint.min_x),
            root_bounds.min_y.min(paint.min_y),
            (root_bounds.min_x + root_bounds.width).max(paint.max_x),
            (root_bounds.min_y + root_bounds.height).max(paint.max_y),
            0.0,
        );
    }
    let root_spec = RootViewportSpec::mermaid(root_bounds, request.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));
    FlowchartSvgDocument {
        use_max_width: request.use_max_width,
        root_viewport: request.root_viewport,
        root_spec,
        document_scope: request.document_scope,
        uses_elk_adapter_dom: request.uses_elk_adapter_dom,
        layout_fallback: request.layout_fallback,
        acc_title: request.acc_title.filter(|s| !s.is_empty()),
        acc_descr: request.acc_descr.filter(|s| !s.is_empty()),
        diagram_title: request.diagram_title,
        title_anchor_x: request.title_anchor_x,
        title_top_margin: request.title_top_margin,
    }
}

fn flowchart_root_bounds(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    diagram_padding: f64,
) -> root_svg::DiagramBounds {
    root_svg::DiagramBounds::from_extents(min_x, min_y, max_x, max_y, diagram_padding)
}

/// The closed document and its sink stay together until the caller stops its render timer.
pub struct WrittenFlowchartSvg<'work> {
    output: BoundedSvgOutput<'work>,
    root_document: RootDocument,
}

impl WrittenFlowchartSvg<'_> {
    pub fn finish(self) -> Result<RootedSvg> {
        self.root_document.complete(self.output.finish()?)
    }
}

impl FlowchartSvgDocument<'_> {
    /// Writes the root and its document-level children on a single operation-owned sink.
    ///
    /// `emit_graph_children` appends complete children of the graph group and reports whether
    /// Step animation marks were emitted. The document owns the group and orders the look filters
    /// around it according to the selected DOM.
    pub fn write<'work>(
        self,
        work_meter: &'work OperationWorkMeter,
        stylesheet: FlowchartStylesheetRequest<'_, FlowchartDocumentId<'_>>,
        emit_graph_children: impl FnOnce(&mut BoundedSvgOutput<'work>) -> Result<bool>,
    ) -> Result<WrittenFlowchartSvg<'work>> {
        let mut out = BoundedSvgOutput::new(work_meter);
        let diagram_id = stylesheet.diagram_id;
        let diagram_type = stylesheet.diagram_type;
        let gradient_id = stylesheet.gradient_id;
        let look_defs = &stylesheet.theme.compatibility.look_defs;
        let root_document = self.push_root_open(&mut out, diagram_id, diagram_type)?;
        self.push_accessibility_metadata(&mut out);
        let deferred_palette = write_flowchart_stylesheet(&mut out, work_meter, stylesheet)?;

        if !self.uses_elk_adapter_dom {
            self.push_shadow_defs(&mut out, look_defs);
            out.checkpoint()?;
        }
        out.push_str("<g>");
        let has_step_animation = emit_graph_children(&mut out)?;
        out.push_str("</g>");
        if self.uses_elk_adapter_dom {
            // ELK writes the look filters as a sibling after the marker/root wrapper.
            self.push_shadow_defs(&mut out, look_defs);
        }
        out.checkpoint()?;

        push_flowchart_gradient(&mut out, gradient_id, look_defs);
        out.checkpoint()?;
        if let Some(title) = self.diagram_title {
            let title_x = self.title_anchor_x;
            let title_y = -self.title_top_margin;
            let _ = write!(
                &mut out,
                r#"<text text-anchor="middle" x="{}" y="{}" class="{}">{}</text>"#,
                fmt(title_x),
                fmt(title_y),
                title_css_class(diagram_type),
                escape_xml(title)
            );
        }
        if has_step_animation {
            write_step_style(&mut out, diagram_id);
        }
        out.push_str("</svg>\n");
        out.checkpoint()?;

        if let Some(palette) = deferred_palette {
            palette.insert(&mut out)?;
        }
        Ok(WrittenFlowchartSvg {
            output: out,
            root_document,
        })
    }

    fn push_shadow_defs(&self, out: &mut impl SvgOutput, prepared: &PreparedLookDefs) {
        let flood_color = prepared.flood_color();
        let offset = NEO_SHADOW_OFFSET_PX;
        let _ = write!(
            out,
            r#"<defs><filter id="{}" height="130%" width="130%"><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
            self.document_scope.drop_shadow(),
            flood_color,
            self.document_scope.drop_shadow_small(),
            flood_color
        );
    }

    fn push_root_open(
        &self,
        out: &mut impl SvgOutput,
        diagram_id: SvgId<'_>,
        diagram_type: &str,
    ) -> Result<RootDocument> {
        // RootChrome borrows complete attribute values. Materialize these two document-level
        // IDs after the bounded sink exists. Other IDs stay streaming views.
        let aria_labelledby = self
            .acc_title
            .map(|_| self.document_scope.accessibility_title().to_string());
        let aria_describedby = self
            .acc_descr
            .map(|_| self.document_scope.accessibility_description().to_string());
        let mut root_chrome = root_svg::RootChrome::new(diagram_id, diagram_type);
        root_chrome.class = Some(if diagram_type == "agentflow" {
            "agentflow"
        } else {
            "flowchart"
        });
        root_chrome.aria_labelledby = aria_labelledby.as_deref();
        root_chrome.aria_describedby = aria_describedby.as_deref();
        let fallback = self
            .layout_fallback
            .map(|backend| ("data-layout-fallback", backend));
        root_chrome.extra_attrs = fallback.as_slice();
        root_chrome.dom.trailing_newline = false;
        if !self.use_max_width {
            root_chrome.dom.style_viewbox_order =
                root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
            root_chrome.dom.fixed_height_placement =
                root_svg::SvgRootFixedHeightPlacement::AfterClass;
            root_chrome.dom.fixed_style_placement =
                root_svg::RootStylePlacement::AfterRoleDescription;
        }
        self.root_viewport
            .write_open(out, self.root_spec, root_chrome)
    }

    fn push_accessibility_metadata(&self, out: &mut impl SvgOutput) {
        if let Some(title) = self.acc_title {
            let _ = write!(
                out,
                r#"<title id="{}">"#,
                self.document_scope.accessibility_title()
            );
            escape_xml_into(out, title);
            out.push_str("</title>");
        }
        if let Some(descr) = self.acc_descr {
            let _ = write!(
                out,
                r#"<desc id="{}">"#,
                self.document_scope.accessibility_description()
            );
            escape_xml_into(out, descr);
            out.push_str("</desc>");
        }
    }
}

fn push_flowchart_gradient(
    out: &mut impl SvgOutput,
    gradient_id: FlowchartDocumentId<'_>,
    prepared: &PreparedLookDefs,
) {
    let Some((gradient_start, gradient_stop)) = prepared.gradient() else {
        return;
    };

    let _ = write!(
        out,
        r#"<linearGradient id="{}" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        gradient_id,
        escape_attr_display(gradient_start),
        escape_attr_display(gradient_stop)
    );
}

fn write_step_style(out: &mut impl SvgOutput, diagram_id: SvgId<'_>) {
    #[cfg(feature = "benchmark-stages")]
    let _stage =
        merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::CssEmit);
    let scope = CssSelectorDiagramId(diagram_id);
    let _ = write!(
        out,
        r#"<style class="edge-step-style">@media (prefers-reduced-motion: reduce) {{ #{scope} .edge-step-dot, #{scope} .edge-step-highlight {{ display: none; }} }}</style>"#,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_padding_is_applied_without_a_family_guard() {
        let zero = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 0.0);
        assert_eq!(zero.min_x, 10.0);
        assert_eq!(zero.min_y, 20.0);
        assert_eq!(zero.width, 100.0);
        assert_eq!(zero.height, 200.0);

        let fractional = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 0.25);
        assert_eq!(fractional.min_x, 9.75);
        assert_eq!(fractional.min_y, 19.75);
        assert_eq!(fractional.width, 100.5);
        assert_eq!(fractional.height, 200.5);

        let padded = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 8.0);
        assert_eq!(padded.min_x, 2.0);
        assert_eq!(padded.min_y, 12.0);
        assert_eq!(padded.width, 116.0);
        assert_eq!(padded.height, 216.0);
    }
}
