//! Info layout, presentation binding, and SVG emission.

use merman_model::{baseline::PINNED_MERMAID_BASELINE_VERSION, presentation::PresentationConfig};
use merman_render_core::{
    Bounds,
    config::{config_font_family_css, config_theme_font_size_css_or_root_number_px_opt},
    mermaid_style::font_family_css,
    number_format::canonical_number,
    svg_css::PreparedCommonCss,
    text::TextStyle,
};
use merman_theme_contract::v2::{Color, FamilyTheme};
use serde::{Deserialize, Serialize};

mod svg;

const INFO_BASELINE_WIDTH_PX: f64 = 400.0;
pub const INFO_BASELINE_HEIGHT_PX: f64 = 100.0;
const INFO_VERSION_FONT_SIZE_PX: f64 = 32.0;
const INFO_VERSION_BASELINE_X_PX: f64 = 100.0;
const INFO_VERSION_BASELINE_Y_PX: f64 = 40.0;

type MeasureVersionBBox<'a> = dyn Fn(&str, &TextStyle) -> ((f64, f64), f64) + 'a;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoDiagramLayout {
    pub bounds: Option<Bounds>,
    pub version: String,
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The boxed Info family artifact already owns the appearance."
)]
pub enum InfoAppearance {
    Mermaid(InfoClassicStyle),
    Native(InfoNativeStyle),
}

#[derive(Debug)]
pub struct InfoClassicStyle {
    css: PreparedCommonCss,
    version: InfoVersionGeometry,
}

#[derive(Debug)]
pub struct InfoNativeStyle {
    font_family: String,
    text_color: Color,
    version: InfoVersionGeometry,
}

#[derive(Debug)]
struct InfoVersionGeometry {
    font_size_px: f64,
    x: f64,
    y: f64,
}

/// Prepares the Info layout and the style used to measure and emit its version text.
///
/// `measure_bbox` returns the left and right SVG extents, then the raw SVG bbox height.
pub fn prepare_info_diagram(
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measure_bbox: &MeasureVersionBBox<'_>,
) -> (InfoDiagramLayout, InfoAppearance) {
    if let Some(theme) = native {
        let typography = &theme.typography;
        let font_family = font_family_css(typography.stack(typography.code_family));
        let (layout, version) =
            InfoVersionGeometry::prepare(&font_family, typography.base_size_px * 2.0, measure_bbox);
        return (
            layout,
            InfoAppearance::Native(InfoNativeStyle {
                font_family,
                text_color: theme.roles.text_primary,
                version,
            }),
        );
    }

    let font_family = config_font_family_css(config);
    let effective_config = config.as_value();
    let font_size_css = config_theme_font_size_css_or_root_number_px_opt(effective_config)
        .map(|font_size| format!("{}px", canonical_number(font_size)))
        .unwrap_or_else(|| "16px".to_string());
    let css =
        PreparedCommonCss::with_resolved_typography(effective_config, font_family, font_size_css);
    let (layout, version) =
        InfoVersionGeometry::prepare(css.font_family(), INFO_VERSION_FONT_SIZE_PX, measure_bbox);
    (
        layout,
        InfoAppearance::Mermaid(InfoClassicStyle { css, version }),
    )
}

impl InfoVersionGeometry {
    fn prepare(
        font_family: &str,
        font_size_px: f64,
        measure_bbox: &MeasureVersionBBox<'_>,
    ) -> (InfoDiagramLayout, Self) {
        let version = format!("v{PINNED_MERMAID_BASELINE_VERSION}");
        let style = TextStyle {
            font_family: Some(font_family.to_owned()),
            font_size: font_size_px,
            ..TextStyle::default()
        };
        let ((left, right), text_height) = measure_bbox(&version, &style);
        let text_height = text_height.max(0.0);
        let anchor_x = INFO_VERSION_BASELINE_X_PX / INFO_BASELINE_WIDTH_PX;
        let anchor_y = INFO_VERSION_BASELINE_Y_PX / INFO_BASELINE_HEIGHT_PX;
        // Keep the upstream viewport as a minimum and preserve the anchor's relative position
        // when the selected font needs additional room.
        let width = INFO_BASELINE_WIDTH_PX
            .max(left.max(0.0) / anchor_x)
            .max(right.max(0.0) / (1.0 - anchor_x));
        let height = INFO_BASELINE_HEIGHT_PX
            .max(text_height / anchor_y)
            .max(text_height / (1.0 - anchor_y));
        let layout = InfoDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: width,
                max_y: height,
            }),
            version,
        };
        let geometry = Self {
            font_size_px,
            x: if width == INFO_BASELINE_WIDTH_PX {
                INFO_VERSION_BASELINE_X_PX
            } else {
                anchor_x * width
            },
            y: if height == INFO_BASELINE_HEIGHT_PX {
                INFO_VERSION_BASELINE_Y_PX
            } else {
                anchor_y * height
            },
        };
        (layout, geometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_layout_records_upstream_configured_canvas_size() {
        let (layout, appearance) =
            prepare_info_diagram(&PresentationConfig::default(), None, &|_, _| {
                ((64.0, 64.0), 32.0)
            });
        let InfoAppearance::Mermaid(style) = appearance else {
            panic!("expected Mermaid appearance");
        };
        assert_eq!(
            layout.bounds,
            Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 400.0,
                max_y: 100.0,
            })
        );
        assert_eq!(style.version.x, INFO_VERSION_BASELINE_X_PX);
        assert_eq!(style.version.y, INFO_VERSION_BASELINE_Y_PX);
    }
}
