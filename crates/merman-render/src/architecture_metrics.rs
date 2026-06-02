use crate::model::Bounds;
use crate::text::{TextMeasurer, TextStyle};

pub(crate) const ARCHITECTURE_LAYOUT_CANVAS_LABEL_WIDTH_SCALE: f64 = 1.055;
pub(crate) const ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE: f64 = 1.01;
pub(crate) const ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_THRESHOLD_PX: f64 = 200.0;
pub(crate) const ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX: f64 = 18.0;
pub(crate) const ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX: f64 = 200.0;
pub(crate) const ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX: f64 = 2.5;

#[derive(Debug, Clone)]
pub(crate) struct ArchitectureServiceBoundsEstimate {
    // Actual emitted icon bounds used when grouped service labels should not affect root getBBox.
    pub(crate) emitted_icon_bounds: Bounds,
    // Approximation of Mermaid's final SVG getBBox() for top-level services.
    pub(crate) svg_root_bounds: Bounds,
    // Approximation of the child bounds that Cytoscape compounds use for group sizing.
    pub(crate) cytoscape_group_child_bounds: Bounds,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureCytoscapeCanvasLabelMetrics {
    pub(crate) width: f64,
    pub(crate) half_width: f64,
    pub(crate) applied_scale: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ArchitectureNodeBBoxExtras {
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) top: f64,
    pub(crate) bottom: f64,
}

pub(crate) fn architecture_cytoscape_canvas_label_metrics(
    label: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> ArchitectureCytoscapeCanvasLabelMetrics {
    let m = measurer.measure(label, style);
    let width = m.width.max(0.0);
    let scale = architecture_layout_canvas_label_width_scale(width);
    let half_width = (width * scale) / 2.0;
    let half_width = (half_width * 2.0).round() / 2.0;
    ArchitectureCytoscapeCanvasLabelMetrics {
        width: m.width,
        half_width,
        applied_scale: scale,
    }
}

pub(crate) fn architecture_layout_canvas_label_width_scale(width_px: f64) -> f64 {
    if width_px >= ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_THRESHOLD_PX {
        ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE
    } else {
        ARCHITECTURE_LAYOUT_CANVAS_LABEL_WIDTH_SCALE
    }
}

pub(crate) fn architecture_create_text_bbox_height_px(font_size_px: f64, line_count: usize) -> f64 {
    let font_size_px = font_size_px.max(1.0);
    let extra_lines = line_count.max(1).saturating_sub(1) as f64;
    font_size_px * ((19.0 / 16.0) + extra_lines * 1.1)
}

pub(crate) fn architecture_create_text_root_label_extra_bottom_px(
    font_size_px: f64,
    line_count: usize,
) -> f64 {
    let font_size_px = font_size_px.max(1.0);
    let extra_lines = line_count.max(1).saturating_sub(1) as f64;
    font_size_px * ((24.1875 / 16.0) + extra_lines * 1.1)
}

pub(crate) fn architecture_create_text_bbox_y_range_px(
    font_size_px: f64,
    line_count: usize,
) -> (f64, f64) {
    let height = architecture_create_text_bbox_height_px(font_size_px, line_count);
    let max_y = architecture_create_text_root_label_extra_bottom_px(font_size_px, line_count);
    (max_y - height, max_y)
}

pub(crate) fn architecture_create_text_compound_label_extra_bottom_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) + 1.0
}

pub(crate) fn architecture_svg_group_bbox_padding_px(padding_px: f64) -> f64 {
    padding_px.max(0.0) + ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX
}

pub(crate) fn architecture_measure_cytoscape_node_bbox_extras(
    title: Option<&str>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    icon_size: f64,
    font_size_px: f64,
) -> ArchitectureNodeBBoxExtras {
    let border = 1.0;
    let half_icon = icon_size / 2.0;

    let mut half_w = half_icon + border;
    let mut bottom = border;

    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        let label_metrics = architecture_cytoscape_canvas_label_metrics(title, measurer, style);
        let label_half = label_metrics.half_width;
        half_w = half_w.max(label_half + border);
        half_w = (half_w * 2.0).round() / 2.0;
        bottom = border + (font_size_px + 1.0).max(0.0);

        if std::env::var("MERMAN_ARCH_DEBUG_CY_BBOX").ok().as_deref() == Some("1") {
            eprintln!(
                "[arch-cy-bbox] title={:?} width={:.6} label_half={:.6} scale={:.6} half_w={:.6} extras_lr={:.6} bottom={:.6}",
                title,
                label_metrics.width,
                label_half,
                label_metrics.applied_scale,
                half_w,
                (half_w - half_icon).max(0.0),
                bottom,
            );
        }
    }

    let extra_lr = (half_w - half_icon).max(0.0);
    ArchitectureNodeBBoxExtras {
        left: extra_lr,
        right: extra_lr,
        top: border,
        bottom,
    }
}

pub(crate) fn architecture_node_bbox_extras_to_manatee(
    extras: ArchitectureNodeBBoxExtras,
) -> manatee::BoundsExtras {
    manatee::BoundsExtras {
        left: extras.left,
        right: extras.right,
        top: extras.top,
        bottom: extras.bottom,
    }
}

pub(crate) fn architecture_estimate_service_bounds<TLine>(
    x: f64,
    y: f64,
    icon_size_px: f64,
    arch_font_size_px: f64,
    svg_font_size_px: f64,
    title: Option<&str>,
    text_measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    compound_text_style: &TextStyle,
    wrap_svg_words_to_lines: impl Fn(&str, f64, &dyn TextMeasurer, &TextStyle) -> Vec<TLine>,
    svg_line_plain_text: impl Fn(&TLine) -> String,
    measure_svg_text_bbox_x: impl Fn(&str, &TextStyle) -> (f64, f64),
) -> ArchitectureServiceBoundsEstimate
where
    TLine: std::fmt::Debug,
{
    let emitted_icon_bounds = Bounds {
        min_x: x,
        min_y: y,
        max_x: x + icon_size_px,
        max_y: y + icon_size_px,
    };
    let mut svg_root_bounds = emitted_icon_bounds.clone();
    let mut cytoscape_group_child_bounds = emitted_icon_bounds.clone();
    let debug_service = std::env::var("MERMAN_ARCH_DEBUG_SERVICE_BOUNDS")
        .ok()
        .filter(|value| !value.is_empty());

    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        let lines = wrap_svg_words_to_lines(title, icon_size_px * 1.5, text_measurer, text_style);
        let mut bbox_left_root = 0.0f64;
        let mut bbox_right_root = 0.0f64;
        for line in &lines {
            let s = svg_line_plain_text(line);
            let (l, r) = measure_svg_text_bbox_x(s.as_str(), text_style);
            bbox_left_root = bbox_left_root.max(l);
            bbox_right_root = bbox_right_root.max(r);
        }
        let line_count_root = lines.len().max(1);
        let label_extra_bottom_root =
            architecture_create_text_root_label_extra_bottom_px(svg_font_size_px, line_count_root);

        let metrics =
            architecture_cytoscape_canvas_label_metrics(title, text_measurer, compound_text_style);
        let compound_half_width = metrics.half_width;
        let label_extra_bottom_compound =
            architecture_create_text_compound_label_extra_bottom_px(arch_font_size_px);

        let cx = x + icon_size_px / 2.0;
        let text_left_root = cx - bbox_left_root;
        let text_right_root = cx + bbox_right_root;
        let text_bottom_root = y + icon_size_px + label_extra_bottom_root;

        let text_left_compound = cx - compound_half_width;
        let text_right_compound = cx + compound_half_width;
        let text_bottom_compound = y + icon_size_px + label_extra_bottom_compound;

        svg_root_bounds = Bounds {
            min_x: svg_root_bounds.min_x.min(text_left_root),
            min_y: svg_root_bounds.min_y,
            max_x: svg_root_bounds.max_x.max(text_right_root),
            max_y: svg_root_bounds.max_y.max(text_bottom_root),
        };
        cytoscape_group_child_bounds = Bounds {
            min_x: cytoscape_group_child_bounds.min_x.min(text_left_compound),
            min_y: cytoscape_group_child_bounds.min_y,
            max_x: cytoscape_group_child_bounds.max_x.max(text_right_compound),
            max_y: cytoscape_group_child_bounds.max_y.max(text_bottom_compound),
        };

        if debug_service.as_deref() == Some(title) {
            eprintln!(
                "[arch-service-bounds] title={:?} svg_lines={:?} root_lr=({}, {}) root_bottom={} canvas_half={} group_child_bottom={} emitted_icon_bounds=({}, {})-({}, {}) group_child_bounds=({}, {})-({}, {}) svg_root_bounds=({}, {})-({}, {})",
                title,
                lines,
                bbox_left_root,
                bbox_right_root,
                label_extra_bottom_root,
                metrics.half_width,
                label_extra_bottom_compound,
                emitted_icon_bounds.min_x,
                emitted_icon_bounds.min_y,
                emitted_icon_bounds.max_x,
                emitted_icon_bounds.max_y,
                cytoscape_group_child_bounds.min_x,
                cytoscape_group_child_bounds.min_y,
                cytoscape_group_child_bounds.max_x,
                cytoscape_group_child_bounds.max_y,
                svg_root_bounds.min_x,
                svg_root_bounds.min_y,
                svg_root_bounds.max_x,
                svg_root_bounds.max_y,
            );
        }
    }

    ArchitectureServiceBoundsEstimate {
        emitted_icon_bounds,
        svg_root_bounds,
        cytoscape_group_child_bounds,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn architecture_text_constants_match_mermaid() {
        assert!((super::architecture_create_text_bbox_height_px(16.0, 2) - 36.6).abs() < 1e-9);
        assert_eq!(
            super::architecture_create_text_bbox_y_range_px(16.0, 1),
            (5.1875, 24.1875)
        );
        assert!((super::architecture_create_text_bbox_y_range_px(16.0, 2).0 - 5.1875).abs() < 1e-9);
        assert!(
            (super::architecture_create_text_bbox_y_range_px(16.0, 2).1 - 41.7875).abs() < 1e-9
        );
        assert_eq!(
            super::architecture_create_text_compound_label_extra_bottom_px(16.0),
            17.0
        );
        assert_eq!(
            super::architecture_create_text_compound_label_extra_bottom_px(12.0),
            13.0
        );
        assert_eq!(
            super::architecture_create_text_root_label_extra_bottom_px(16.0, 1),
            24.1875
        );
        assert_eq!(super::ARCHITECTURE_LAYOUT_CANVAS_LABEL_WIDTH_SCALE, 1.055);
        assert_eq!(
            super::ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE,
            1.01
        );
        assert_eq!(
            super::ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_THRESHOLD_PX,
            200.0
        );
        assert_eq!(super::ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX, 18.0);
        assert_eq!(super::ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, 200.0);
        assert_eq!(super::ARCHITECTURE_SVG_GROUP_BBOX_EXTRA_PADDING_PX, 2.5);
    }

    #[test]
    fn architecture_node_bbox_extras_convert_to_manatee_bounds_extras() {
        let extras = super::ArchitectureNodeBBoxExtras {
            left: 1.5,
            right: 2.5,
            top: 3.5,
            bottom: 4.5,
        };
        let mapped = super::architecture_node_bbox_extras_to_manatee(extras);
        assert_eq!(mapped.left, 1.5);
        assert_eq!(mapped.right, 2.5);
        assert_eq!(mapped.top, 3.5);
        assert_eq!(mapped.bottom, 4.5);
    }

    #[test]
    fn architecture_canvas_label_metrics_report_applied_scale() {
        let style = crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let metrics = super::architecture_cytoscape_canvas_label_metrics(
            "This is a deliberately long architecture label probe",
            &measurer,
            &style,
        );
        assert!(metrics.width > 0.0);
        assert!(
            metrics.applied_scale == super::ARCHITECTURE_LAYOUT_CANVAS_LABEL_WIDTH_SCALE
                || metrics.applied_scale
                    == super::ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE
        );
    }

    #[test]
    fn architecture_canvas_label_scale_switches_at_long_label_threshold() {
        assert_eq!(
            super::architecture_layout_canvas_label_width_scale(199.999),
            super::ARCHITECTURE_LAYOUT_CANVAS_LABEL_WIDTH_SCALE
        );
        assert_eq!(
            super::architecture_layout_canvas_label_width_scale(200.0),
            super::ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE
        );
        assert_eq!(
            super::architecture_layout_canvas_label_width_scale(320.0),
            super::ARCHITECTURE_LAYOUT_CANVAS_LONG_LABEL_WIDTH_SCALE
        );
    }

    #[test]
    fn architecture_svg_group_bbox_padding_adds_headless_cytoscape_extra() {
        assert_eq!(super::architecture_svg_group_bbox_padding_px(0.0), 2.5);
        assert_eq!(super::architecture_svg_group_bbox_padding_px(12.0), 14.5);
        assert_eq!(super::architecture_svg_group_bbox_padding_px(-7.0), 2.5);
    }
}
