use crate::text::{TextMeasurer, TextStyle};

pub(crate) const ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE: f64 = 1.055;
pub(crate) const ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX: f64 = 18.0;
pub(crate) const ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX: f64 = 200.0;
pub(crate) const ARCHITECTURE_COMPOUND_BBOX_EXTRA_PADDING_PX: f64 = 2.5;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureCytoscapeCanvasLabelMetrics {
    pub(crate) width: f64,
    pub(crate) half_width: f64,
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
    let half_width = (m.width.max(0.0) * ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE) / 2.0;
    let half_width = (half_width * 2.0).round() / 2.0;
    ArchitectureCytoscapeCanvasLabelMetrics {
        width: m.width,
        half_width,
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
                "[arch-cy-bbox] title={:?} width={:.6} label_half={:.6} half_w={:.6} extras_lr={:.6} bottom={:.6}",
                title,
                label_metrics.width,
                label_half,
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
        assert_eq!(super::ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX, 18.0);
        assert_eq!(super::ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, 200.0);
    }
}
