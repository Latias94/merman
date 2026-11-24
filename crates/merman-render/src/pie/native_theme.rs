use merman_model::families::pie::PieDiagramModel;
use merman_theme_contract::v2::{DataPalette, Paint};

use crate::model::PieDiagramLayout;
use crate::text::{TextMeasurer, TextStyle};
use crate::theme::{Color, FamilyTheme, font_family_css};
use crate::{DiagramFamilyId, Result};

use super::{PieLayoutPaint, layout_pie};

#[derive(Debug)]
pub(crate) struct PieNativeStyle {
    pub(crate) canvas: Paint,
    pub(crate) data: DataPalette,
    pub(crate) slice_gap_px: f64,
    pub(crate) slice_opacity: f64,
    pub(crate) title_color: Color,
    pub(crate) legend_color: Color,
    pub(crate) title: TextStyle,
    pub(crate) label: TextStyle,
    pub(crate) content_width: f64,
    pub(crate) slice_ordinals: Vec<usize>,
}

impl PieNativeStyle {
    pub(crate) fn prepare(
        theme: &FamilyTheme,
        model: &PieDiagramModel,
        diagram_title: Option<&str>,
        effective_config: &serde_json::Value,
        measurer: &dyn TextMeasurer,
    ) -> Result<(PieDiagramLayout, Self)> {
        let typography = &theme.typography;
        let title = TextStyle {
            font_family: Some(font_family_css(typography.stack(typography.title_family))),
            font_size: typography.base_size_px * 1.5,
            font_weight: Some(typography.title_weight.to_string()),
            font_style: None,
        };
        let label = TextStyle {
            font_family: Some(font_family_css(typography.stack(typography.body_family))),
            font_size: typography.base_size_px,
            font_weight: Some(typography.label_weight.to_string()),
            font_style: None,
        };
        let geometry = layout_pie(
            model,
            diagram_title,
            effective_config,
            PieLayoutPaint::Native {
                data: &theme.data,
                title: &title,
                label: &label,
            },
            measurer,
        )?;
        Ok((
            geometry.layout,
            Self {
                canvas: theme.roles.canvas.clone(),
                data: theme.data.clone(),
                slice_gap_px: theme.form.chart.pie_gap_px,
                slice_opacity: theme.form.chart.pie_opacity,
                title_color: theme.roles.text_title,
                legend_color: theme.roles.text_primary,
                title,
                label,
                content_width: geometry.content_width,
                slice_ordinals: geometry.slice_ordinals,
            },
        ))
    }
}
