use crate::text::{TextMeasurer, TextStyle};

pub(crate) const ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE: f64 = 1.055;
pub(crate) const ARCHITECTURE_LONG_LABEL_WIDTH_SCALE: f64 = 1.01;
pub(crate) const ARCHITECTURE_LONG_LABEL_WIDTH_THRESHOLD_PX: f64 = 200.0;
pub(crate) const ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX: f64 = 18.0;
pub(crate) const ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX: f64 = 200.0;
pub(crate) const ARCHITECTURE_COMPOUND_BBOX_EXTRA_PADDING_PX: f64 = 2.5;

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
    let scale = architecture_cytoscape_canvas_label_width_scale(width);
    let half_width = (width * scale) / 2.0;
    let half_width = (half_width * 2.0).round() / 2.0;
    ArchitectureCytoscapeCanvasLabelMetrics {
        width: m.width,
        half_width,
        applied_scale: scale,
    }
}

pub(crate) fn architecture_cytoscape_canvas_label_width_scale(width_px: f64) -> f64 {
    if width_px >= ARCHITECTURE_LONG_LABEL_WIDTH_THRESHOLD_PX {
        ARCHITECTURE_LONG_LABEL_WIDTH_SCALE
    } else {
        ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE
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

pub(crate) fn architecture_create_text_compound_label_extra_bottom_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) * (17.0 / 16.0)
}

pub(crate) fn architecture_compound_bbox_padding_px(padding_px: f64) -> f64 {
    padding_px.max(0.0) + ARCHITECTURE_COMPOUND_BBOX_EXTRA_PADDING_PX
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

#[cfg(test)]
mod tests {
    #[test]
    fn architecture_text_constants_match_mermaid() {
        assert!((super::architecture_create_text_bbox_height_px(16.0, 2) - 36.6).abs() < 1e-9);
        assert_eq!(
            super::architecture_create_text_compound_label_extra_bottom_px(16.0),
            17.0
        );
        assert_eq!(
            super::architecture_create_text_root_label_extra_bottom_px(16.0, 1),
            24.1875
        );
        assert_eq!(
            super::ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE,
            1.055
        );
        assert_eq!(super::ARCHITECTURE_LONG_LABEL_WIDTH_SCALE, 1.01);
        assert_eq!(super::ARCHITECTURE_LONG_LABEL_WIDTH_THRESHOLD_PX, 200.0);
        assert_eq!(super::ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX, 18.0);
        assert_eq!(super::ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, 200.0);
        assert_eq!(super::ARCHITECTURE_COMPOUND_BBOX_EXTRA_PADDING_PX, 2.5);
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
            metrics.applied_scale == super::ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE
                || metrics.applied_scale == super::ARCHITECTURE_LONG_LABEL_WIDTH_SCALE
        );
    }

    #[test]
    fn architecture_canvas_label_scale_switches_at_long_label_threshold() {
        assert_eq!(
            super::architecture_cytoscape_canvas_label_width_scale(199.999),
            super::ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE
        );
        assert_eq!(
            super::architecture_cytoscape_canvas_label_width_scale(200.0),
            super::ARCHITECTURE_LONG_LABEL_WIDTH_SCALE
        );
        assert_eq!(
            super::architecture_cytoscape_canvas_label_width_scale(320.0),
            super::ARCHITECTURE_LONG_LABEL_WIDTH_SCALE
        );
    }

    #[test]
    fn architecture_compound_bbox_padding_adds_mermaid_extra_padding() {
        assert_eq!(super::architecture_compound_bbox_padding_px(0.0), 2.5);
        assert_eq!(super::architecture_compound_bbox_padding_px(12.0), 14.5);
        assert_eq!(super::architecture_compound_bbox_padding_px(-7.0), 2.5);
    }
}
