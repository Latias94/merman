use super::super::root_svg;
use super::super::util::{escape_attr_into, escape_xml_into};
use crate::environment::RootViewportOverridePolicy;

pub(super) struct FlowchartSvgDocumentRequest<'a> {
    pub diagram_id: &'a str,
    pub diagram_type: &'a str,
    pub model: &'a crate::flowchart::FlowchartModel,
    pub use_max_width: bool,
    pub root_viewport_override_policy: RootViewportOverridePolicy,
    pub diagram_padding: f64,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
}

pub(super) struct FlowchartSvgDocument<'a> {
    diagram_id: &'a str,
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
    // Chromium's `getBBox()` values frequently land on an `f32` lattice. Mermaid then computes the
    // root viewport in JS double space:
    // - viewBox.x/y = bbox.x/y - padding
    // - viewBox.w/h = bbox.width/height + 2*padding
    //
    // Mirror that by quantizing the content bounds to `f32` first, then applying padding in `f64`.
    let bbox_min_x_f32 = request.bbox_min_x as f32;
    let bbox_min_y_f32 = request.bbox_min_y as f32;
    let bbox_max_x_f32 = request.bbox_max_x as f32;
    let bbox_max_y_f32 = request.bbox_max_y as f32;
    let bbox_has_area = (bbox_max_x_f32 - bbox_min_x_f32).abs() >= 1e-6
        || (bbox_max_y_f32 - bbox_min_y_f32).abs() >= 1e-6;
    let bbox_w_f32 = if bbox_has_area {
        (bbox_max_x_f32 - bbox_min_x_f32).max(1.0)
    } else {
        0.0
    };
    let bbox_h_f32 = if bbox_has_area {
        (bbox_max_y_f32 - bbox_min_y_f32).max(1.0)
    } else {
        0.0
    };

    let vb_min_x = (bbox_min_x_f32 as f64) - request.diagram_padding;
    let vb_min_y = (bbox_min_y_f32 as f64) - request.diagram_padding;
    let vb_w = ((bbox_w_f32 as f64) + request.diagram_padding * 2.0).max(1.0);
    let vb_h = ((bbox_h_f32 as f64) + request.diagram_padding * 2.0).max(1.0);

    let root_bounds = root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, request.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(vb_w));
    let root_viewport = root_svg::RootViewportContext::new(
        crate::family::RenderFamilyKind::Flowchart,
        request.diagram_id,
        request.root_viewport_override_policy,
    );

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

impl FlowchartSvgDocument<'_> {
    pub(super) fn push_root_open(&self, out: &mut String) -> crate::Result<()> {
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
        if let (Some(id), Some(title)) = (self.aria_labelledby.as_deref(), self.acc_title) {
            out.push_str(r#"<title id=""#);
            escape_attr_into(out, id);
            out.push_str(r#"">"#);
            escape_xml_into(out, title);
            out.push_str("</title>");
        }
        if let (Some(id), Some(descr)) = (self.aria_describedby.as_deref(), self.acc_descr) {
            out.push_str(r#"<desc id=""#);
            escape_attr_into(out, id);
            out.push_str(r#"">"#);
            escape_xml_into(out, descr);
            out.push_str("</desc>");
        }
    }
}
