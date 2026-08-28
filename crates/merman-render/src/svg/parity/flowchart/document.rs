use std::fmt::Write as _;

use super::super::SvgDiagramId;
use super::super::root_svg;
use super::super::util::escape_xml_into;

const MINIMUM_PAINT_INSET_PX: f64 = 1.0;

pub(super) struct FlowchartSvgDocumentRequest<'a> {
    pub family_kind: crate::family::RenderFamilyKind,
    pub diagram_id: SvgDiagramId<'a>,
    pub diagram_type: &'a str,
    pub model: &'a crate::flowchart::FlowchartModel,
    pub use_max_width: bool,
    pub diagram_padding: f64,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
}

pub(super) struct FlowchartSvgDocument<'a> {
    diagram_id: SvgDiagramId<'a>,
    diagram_type: &'a str,
    use_max_width: bool,
    root_viewport: root_svg::RootViewportContext<'a>,
    root_spec: root_svg::RootViewportSpec,
    acc_title: Option<&'a str>,
    acc_descr: Option<&'a str>,
    aria_labelledby: Option<String>,
    aria_describedby: Option<String>,
}

pub(super) fn prepare_flowchart_svg_document(
    request: FlowchartSvgDocumentRequest<'_>,
) -> FlowchartSvgDocument<'_> {
    let root_bounds = flowchart_root_bounds(
        request.bbox_min_x,
        request.bbox_min_y,
        request.bbox_max_x,
        request.bbox_max_y,
        request.diagram_padding,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, request.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));
    let root_viewport = root_svg::RootViewportContext::new(request.family_kind, request.diagram_id);

    let acc_title = request
        .model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let acc_descr = request
        .model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{}", request.diagram_id));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{}", request.diagram_id));

    FlowchartSvgDocument {
        diagram_id: request.diagram_id,
        diagram_type: request.diagram_type,
        use_max_width: request.use_max_width,
        root_viewport,
        root_spec,
        acc_title,
        acc_descr,
        aria_labelledby,
        aria_describedby,
    }
}

fn flowchart_root_bounds(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    diagram_padding: f64,
) -> root_svg::DiagramBounds {
    // Flowchart paint can cross geometry extrema by roughly one CSS pixel because of the default
    // stroke and rasterization. Supplement finite user padding below that threshold so the total
    // inset is at least one pixel, while leaving invalid non-finite values to root validation.
    let paint_guard = if diagram_padding.is_finite() {
        (MINIMUM_PAINT_INSET_PX - diagram_padding.max(0.0)).max(0.0)
    } else {
        0.0
    };
    root_svg::DiagramBounds::from_extents(
        min_x - paint_guard,
        min_y - paint_guard,
        max_x + paint_guard,
        max_y + paint_guard,
        diagram_padding,
    )
}

impl FlowchartSvgDocument<'_> {
    pub(super) fn push_root_open(&self, out: &mut String) -> crate::Result<root_svg::RootDocument> {
        let mut root_chrome = root_svg::RootChrome::new(self.diagram_id, self.diagram_type);
        root_chrome.class = Some("flowchart");
        root_chrome.aria_labelledby = self.aria_labelledby.as_deref();
        root_chrome.aria_describedby = self.aria_describedby.as_deref();
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

    pub(super) fn push_accessibility_metadata(&self, out: &mut String) {
        if let Some(title) = self.acc_title {
            let _ = write!(out, r#"<title id="chart-title-{}">"#, self.diagram_id);
            escape_xml_into(out, title);
            out.push_str("</title>");
        }
        if let Some(descr) = self.acc_descr {
            let _ = write!(out, r#"<desc id="chart-desc-{}">"#, self.diagram_id);
            escape_xml_into(out, descr);
            out.push_str("</desc>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_padding_keeps_a_family_local_minimum_paint_inset() {
        let guarded = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 0.0);
        assert_eq!(guarded.min_x, 9.0);
        assert_eq!(guarded.min_y, 19.0);
        assert_eq!(guarded.width, 102.0);
        assert_eq!(guarded.height, 202.0);

        let fractional = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 0.25);
        assert_eq!(fractional.min_x, 9.0);
        assert_eq!(fractional.min_y, 19.0);
        assert_eq!(fractional.width, 102.0);
        assert_eq!(fractional.height, 202.0);

        let padded = flowchart_root_bounds(10.0, 20.0, 110.0, 220.0, 8.0);
        assert_eq!(padded.min_x, 2.0);
        assert_eq!(padded.min_y, 12.0);
        assert_eq!(padded.width, 116.0);
        assert_eq!(padded.height, 216.0);
    }
}
