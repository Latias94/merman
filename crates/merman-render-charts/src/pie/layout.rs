use merman_model::families::pie::{PieDiagramModel, PieSection};
use merman_render_core::{Bounds, Error, Result, config::ConfigLookup, text::TextStyle};
use merman_theme_contract::v2::DataPalette;

use super::{
    PIE_LEGEND_RECT_SIZE_PX, PIE_LEGEND_SPACING_PX, PIE_MARGIN_PX, PIE_TITLE_BASELINE_Y_PX,
    PieConfigView, PieDiagramLayout, PieLegendItemLayout, PieLegendPosition, PieMermaidStyle,
    PieSliceLayout,
};

fn visible_pie_sections(model: &PieDiagramModel) -> (f64, Vec<(usize, &PieSection)>) {
    let total: f64 = model
        .sections
        .iter()
        .filter(|section| section.value.is_finite() && section.value >= 0.0)
        .map(|section| section.value)
        .sum();
    let sections = if total.is_finite() && total > 0.0 {
        model
            .sections
            .iter()
            .enumerate()
            .filter(|(_, section)| section.value.is_finite() && section.value > 0.0)
            .filter(|(_, section)| (section.value / total) * 100.0 >= 1.0)
            .collect()
    } else {
        Vec::new()
    };
    (total, sections)
}

pub fn pie_legend_text(label: &str, value: f64, show_data: bool) -> String {
    if show_data {
        format!("{label} [{}]", ryu_js::Buffer::new().format(value))
    } else {
        label.to_owned()
    }
}

pub fn native_legend_baseline_y((ascent, descent): (f64, f64)) -> f64 {
    (PIE_LEGEND_RECT_SIZE_PX + ascent - descent) / 2.0
}

fn polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    // Mermaid pie charts use a "12 o'clock is zero" convention with y increasing downwards.
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

/// Lays out a classic Pie using the same font family as its stylesheet.
/// The host measures a single SVG text node's bounding client rect width.
pub fn layout_mermaid_pie(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &impl ConfigLookup,
    style: &PieMermaidStyle,
    font_family: &str,
    measure_width: &dyn Fn(&str, &TextStyle) -> f64,
) -> Result<PieDiagramLayout> {
    let geometry = layout_pie(
        model,
        diagram_title,
        effective_config,
        PieLayoutPaint::Mermaid { style, font_family },
        measure_width,
    )?;
    style.prepare_content_width(geometry.content_width)?;
    Ok(geometry.layout)
}

#[derive(Clone, Copy)]
pub(super) enum PieLayoutPaint<'a> {
    Mermaid {
        style: &'a PieMermaidStyle,
        font_family: &'a str,
    },
    Native {
        data: &'a DataPalette,
        title: &'a TextStyle,
        label: &'a TextStyle,
        vertical_extents: fn(&TextStyle) -> (f64, f64),
    },
}

impl PieLayoutPaint<'_> {
    fn fill(self, ordinal: usize, label: &str) -> Result<String> {
        match self {
            Self::Mermaid { style, .. } => {
                style
                    .fill_for(label)
                    .map(str::to_owned)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("Pie theme plan is missing slice label `{label}`"),
                    })
            }
            Self::Native { data, .. } => Ok(data.fill(ordinal).to_string()),
        }
    }
}

pub(super) struct PieLayoutGeometry {
    pub(super) layout: PieDiagramLayout,
    pub(super) content_width: f64,
    pub(super) slice_ordinals: Vec<usize>,
}

pub(super) fn layout_pie(
    model: &PieDiagramModel,
    diagram_title: Option<&str>,
    effective_config: &impl ConfigLookup,
    paint: PieLayoutPaint<'_>,
    measure_width: &dyn Fn(&str, &TextStyle) -> f64,
) -> Result<PieLayoutGeometry> {
    // Mermaid@11.16 `packages/mermaid/src/diagrams/pie/pieRenderer.ts` constants.
    let legend_rect_size = PIE_LEGEND_RECT_SIZE_PX;
    let legend_spacing = PIE_LEGEND_SPACING_PX;
    let center: f64 = 225.0;
    let radius: f64 = 185.0;

    let (mermaid_legend_style, mermaid_title_style);
    let (outer_radius, legend_style, title_style, vertical_extents) = match paint {
        PieLayoutPaint::Mermaid { style, font_family } => {
            let css = style.css_binding();
            mermaid_legend_style = css.legend_measurement_style(font_family);
            mermaid_title_style = css.title_measurement_style(font_family);
            (
                radius + css.outer_stroke_width_number / 2.0,
                &mermaid_legend_style,
                &mermaid_title_style,
                None,
            )
        }
        PieLayoutPaint::Native {
            title,
            label,
            vertical_extents,
            ..
        } => (radius, label, title, Some(vertical_extents)),
    };
    let cfg = PieConfigView::new(effective_config).layout_settings();
    let title = model.title.as_deref().or(diagram_title);
    let label_radius = radius.max(0.0) * cfg.text_position;
    let mut legend_step_y = legend_rect_size + legend_spacing;
    if let Some(vertical_extents) = vertical_extents {
        let (ascent, descent) = vertical_extents(legend_style);
        legend_step_y = legend_step_y.max((ascent + descent).ceil() + legend_spacing);
    }
    let legend_position = cfg.legend_position;
    let total_legend_height = (model.sections.len() as f64) * legend_step_y;
    let centered_legend_start_y = -(legend_step_y * (model.sections.len().max(1) as f64)) / 2.0;
    let legend_start_y = match legend_position {
        PieLegendPosition::Top => -radius,
        PieLegendPosition::Bottom => radius + legend_step_y,
        _ => centered_legend_start_y,
    };

    let (total, pie_sections) = visible_pie_sections(model);

    // D3 normalizes angles over the visible sectors in input order. The displayed percentages
    // retain the original total, including values that fall below Mermaid's 1% threshold.
    let mut slices = Vec::new();
    let mut slice_ordinals = Vec::new();
    let pie_total: f64 = pie_sections.iter().map(|(_, section)| section.value).sum();
    let is_full_circle = pie_sections.len() == 1;
    if pie_total.is_finite() && pie_total > 0.0 {
        let mut start_angle = 0.0;
        for (ordinal, section) in pie_sections {
            let end_angle = if is_full_circle {
                std::f64::consts::TAU
            } else {
                start_angle + (section.value / pie_total) * std::f64::consts::TAU
            };
            let (text_x, text_y) = polar_xy(label_radius, (start_angle + end_angle) / 2.0);
            slices.push(PieSliceLayout {
                label: section.label.clone(),
                value: section.value,
                start_angle,
                end_angle,
                is_full_circle,
                percent: (100.0 * (section.value / total)).round() as i64,
                text_x,
                text_y,
                fill: paint.fill(ordinal, &section.label)?,
            });
            if vertical_extents.is_some() {
                slice_ordinals.push(ordinal);
            }
            start_angle = end_angle;
        }
    }

    // The legend retains the original category order, including categories without a sector.
    let mut legend_items = Vec::with_capacity(model.sections.len());
    for (i, sec) in model.sections.iter().enumerate() {
        let y = legend_start_y + (i as f64) * legend_step_y;
        let fill = paint.fill(i, &sec.label)?;
        legend_items.push(PieLegendItemLayout {
            label: sec.label.clone(),
            value: sec.value,
            fill,
            y,
        });
    }

    let mut max_legend_width: f64 = 0.0;
    for sec in &model.sections {
        let label = pie_legend_text(&sec.label, sec.value, model.show_data);
        let trimmed = label.trim_end();
        // Mermaid 11.16 measures pie legend text via a single SVG `<text>` node's
        // `getBoundingClientRect().width`.
        let w = if trimmed.is_empty() {
            0.0
        } else {
            let width = measure_width(trimmed, legend_style);
            if legend_style.font_size == 0.0 {
                0.0
            } else {
                width
            }
        };
        max_legend_width = max_legend_width.max(w);
    }

    let title_width = title
        .map(|title| {
            let width = measure_width(title, title_style);
            if title_style.font_size == 0.0 {
                0.0
            } else {
                width
            }
        })
        .unwrap_or(0.0);

    let base_w: f64 = center * 2.0;
    let legend_extra_width = legend_rect_size + legend_spacing + max_legend_width;
    let centered_legend_x = -max_legend_width / 2.0 - (legend_rect_size + legend_spacing);

    let padded_chart_width = base_w + PIE_MARGIN_PX;
    let chart_and_legend_width = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom | PieLegendPosition::Center => {
            padded_chart_width
        }
        PieLegendPosition::Left => padded_chart_width + legend_extra_width,
        PieLegendPosition::Right => {
            if model.sections.is_empty() {
                f64::NEG_INFINITY
            } else {
                padded_chart_width + legend_extra_width
            }
        }
    };
    let height = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom => {
            (base_w + total_legend_height).max(1.0)
        }
        PieLegendPosition::Left | PieLegendPosition::Center | PieLegendPosition::Right => {
            base_w.max(1.0)
        }
    };

    let legend_x = match legend_position {
        PieLegendPosition::Top | PieLegendPosition::Bottom | PieLegendPosition::Center => {
            centered_legend_x
        }
        PieLegendPosition::Left => -radius - (legend_rect_size + legend_spacing),
        PieLegendPosition::Right => 12.0 * legend_rect_size,
    };

    let title_left = base_w / 2.0 - title_width / 2.0;
    let title_right = base_w / 2.0 + title_width / 2.0;
    let mut bounds = Bounds {
        min_x: title_left.min(0.0),
        min_y: 0.0,
        max_x: chart_and_legend_width.max(title_right),
        max_y: height,
    };
    if model.sections.is_empty() {
        bounds.max_x = bounds.max_x.max(base_w);
    }
    if let Some(vertical_extents) = vertical_extents {
        if title.is_some_and(|title| !title.is_empty()) {
            let (ascent, _) = vertical_extents(title_style);
            bounds.min_y = bounds.min_y.min(center + PIE_TITLE_BASELINE_Y_PX - ascent);
        }
        if let Some(last) = legend_items.last() {
            let (ascent, descent) = vertical_extents(legend_style);
            let text_baseline = native_legend_baseline_y((ascent, descent));
            let left = center + legend_x;
            bounds.min_x = bounds.min_x.min(left);
            bounds.max_x = bounds.max_x.max(left + legend_extra_width);
            bounds.min_y = bounds
                .min_y
                .min(center + legend_start_y + (text_baseline - ascent).min(0.0));
            bounds.max_y = bounds
                .max_y
                .max(center + last.y + (text_baseline + descent).max(legend_rect_size));
        }
    }

    Ok(PieLayoutGeometry {
        layout: PieDiagramLayout {
            bounds: Some(bounds),
            title: title.map(str::to_owned),
            center_x: center,
            center_y: center,
            radius,
            outer_radius,
            legend_x,
            legend_start_y,
            legend_step_y,
            slices,
            legend_items,
        },
        content_width: chart_and_legend_width,
        slice_ordinals,
    })
}
