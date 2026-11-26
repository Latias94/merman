use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{
    Result, config::ConfigLookup, mermaid_style::font_family_css, text::TextStyle,
};
use merman_theme_contract::v2::{Appearance, Color, DataPalette, FamilyTheme, Paint};

use super::{
    PieDiagramLayout,
    layout::{PieLayoutPaint, layout_pie},
};

#[derive(Debug)]
pub struct PieNativeStyle {
    canvas: Paint,
    pub data: DataPalette,
    pub slice_gap_px: f64,
    pub slice_opacity: f64,
    pub title_color: Color,
    pub legend_color: Color,
    pub title: TextStyle,
    pub label: TextStyle,
    pub content_width: f64,
    pub slice_ordinals: Vec<usize>,
}

impl PieNativeStyle {
    /// Binds the native paints and measures with the host's client-rect width and title extents.
    pub fn prepare(
        theme: &FamilyTheme,
        model: &PieDiagramModel,
        diagram_title: Option<&str>,
        effective_config: &impl ConfigLookup,
        measure_width: &dyn Fn(&str, &TextStyle) -> f64,
        vertical_extents: fn(&TextStyle) -> (f64, f64),
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
                vertical_extents,
            },
            measure_width,
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

    pub fn uses_pattern(&self, ordinal: usize) -> bool {
        self.data.pattern_encoding && !ordinal.is_multiple_of(6)
    }

    /// Selects the percentage-label ink using the host canvas and the native export backdrop.
    pub fn percentage_ink(
        &self,
        background: Option<Color>,
        appearance_and_backdrop: Option<(Appearance, Option<Color>)>,
    ) -> impl Fn(usize) -> Color + '_ {
        let host_canvas = background.map(Paint::Solid);
        let export_ink = appearance_and_backdrop.and_then(|(_, backdrop)| {
            self.data.fill_inks_for_export(
                host_canvas.as_ref().unwrap_or(&self.canvas),
                self.slice_opacity,
                backdrop?,
            )
        });
        move |ordinal| match (&export_ink, background, appearance_and_backdrop) {
            (Some(inks), _, _) => inks[ordinal % inks.len()],
            (None, Some(canvas), Some((appearance, _))) => {
                self.data
                    .fill_ink_for_canvas(ordinal, canvas, self.slice_opacity, appearance)
            }
            _ => self.data.fill_ink(ordinal),
        }
    }
}
