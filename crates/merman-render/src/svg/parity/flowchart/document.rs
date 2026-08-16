use super::super::root_svg;
use super::super::util::escape_xml_into;
use super::document_ids::FlowchartDocumentIds;

pub(super) struct FlowchartSvgDocumentRequest<'a> {
    pub family_id: crate::DiagramFamilyId,
    pub diagram_id: &'a str,
    pub diagram_type: &'a str,
    pub model: &'a crate::flowchart::FlowchartModel,
    pub document_ids: &'a FlowchartDocumentIds<'a>,
    pub use_max_width: bool,
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
    document_ids: &'a FlowchartDocumentIds<'a>,
    acc_title: Option<&'a str>,
    acc_descr: Option<&'a str>,
}

pub(super) fn prepare_flowchart_svg_document(
    request: FlowchartSvgDocumentRequest<'_>,
) -> FlowchartSvgDocument<'_> {
    let root_bounds = root_svg::DiagramBounds::from_extents(
        request.bbox_min_x,
        request.bbox_min_y,
        request.bbox_max_x,
        request.bbox_max_y,
        request.diagram_padding,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, request.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));
    let root_viewport = root_svg::RootViewportContext::new(request.family_id, request.diagram_id);

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
    FlowchartSvgDocument {
        diagram_id: request.diagram_id,
        diagram_type: request.diagram_type,
        use_max_width: request.use_max_width,
        root_viewport,
        root_spec,
        document_ids: request.document_ids,
        acc_title,
        acc_descr,
    }
}

impl FlowchartSvgDocument<'_> {
    pub(super) fn push_root_open(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
    ) -> crate::Result<root_svg::RootDocument> {
        // RootChrome currently borrows complete attribute values. Materialize at most these two
        // document-level ids after the bounded sink exists; per-owner ids stay streaming views.
        let aria_labelledby = self
            .acc_title
            .map(|_| self.document_ids.accessibility_title().to_string());
        let aria_describedby = self
            .acc_descr
            .map(|_| self.document_ids.accessibility_description().to_string());
        let mut root_chrome = root_svg::RootChrome::new(self.diagram_id, self.diagram_type);
        root_chrome.class = Some("flowchart");
        root_chrome.aria_labelledby = aria_labelledby.as_deref();
        root_chrome.aria_describedby = aria_describedby.as_deref();
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

    pub(super) fn push_accessibility_metadata(&self, out: &mut impl crate::svg::parity::SvgOutput) {
        if let Some(title) = self.acc_title {
            let _ = write!(
                out,
                r#"<title id="{}">"#,
                self.document_ids.accessibility_title()
            );
            escape_xml_into(out, title);
            out.push_str("</title>");
        }
        if let Some(descr) = self.acc_descr {
            let _ = write!(
                out,
                r#"<desc id="{}">"#,
                self.document_ids.accessibility_description()
            );
            escape_xml_into(out, descr);
            out.push_str("</desc>");
        }
    }
}
