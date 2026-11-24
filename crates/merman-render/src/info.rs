use crate::model::{Bounds, InfoDiagramLayout};
use crate::svg::PreparedCommonCss;
use crate::text::{TextMeasurer, TextStyle};
use crate::theme::{Color, FamilyTheme};
use merman_model::baseline::PINNED_MERMAID_BASELINE_VERSION;
use merman_model::presentation::PresentationConfig;

const INFO_BASELINE_WIDTH_PX: f64 = 400.0;
pub(crate) const INFO_BASELINE_HEIGHT_PX: f64 = 100.0;
const INFO_VERSION_FONT_SIZE_PX: f64 = 32.0;
const INFO_VERSION_BASELINE_X_PX: f64 = 100.0;
const INFO_VERSION_BASELINE_Y_PX: f64 = 40.0;

#[derive(Debug)]
pub(crate) enum InfoAppearance {
    Mermaid(InfoClassicStyle),
    Native(InfoNativeStyle),
}

#[derive(Debug)]
pub(crate) struct InfoVersionGeometry {
    pub(crate) font_size_px: f64,
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl InfoVersionGeometry {
    fn prepare(
        font_family: &str,
        font_size_px: f64,
        measurer: &dyn TextMeasurer,
    ) -> (InfoDiagramLayout, Self) {
        let version = format!("v{PINNED_MERMAID_BASELINE_VERSION}");
        let style = TextStyle {
            font_family: Some(font_family.to_owned()),
            font_size: font_size_px,
            ..TextStyle::default()
        };
        let (left, right) = measurer.measure_svg_text_bbox_x(&version, &style);
        let text_height = measurer
            .measure_svg_raw_text_bbox_height_px(&version, &style)
            .max(0.0);
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

#[derive(Debug)]
pub(crate) struct InfoClassicStyle {
    pub(crate) css: PreparedCommonCss,
    pub(crate) version: InfoVersionGeometry,
}

impl InfoClassicStyle {
    pub(crate) fn resolve(
        config: &PresentationConfig,
        measurer: &dyn TextMeasurer,
    ) -> (InfoDiagramLayout, Self) {
        let font_family = crate::config::config_font_family_css(config);
        let css = PreparedCommonCss::new(config.as_value(), Some(&font_family));
        let (layout, version) =
            InfoVersionGeometry::prepare(&font_family, INFO_VERSION_FONT_SIZE_PX, measurer);
        (layout, Self { css, version })
    }
}

#[derive(Debug)]
pub(crate) struct InfoNativeStyle {
    pub(crate) font_family: String,
    pub(crate) text_color: Color,
    pub(crate) version: InfoVersionGeometry,
}

impl InfoNativeStyle {
    pub(crate) fn resolve(
        theme: &FamilyTheme,
        measurer: &dyn TextMeasurer,
    ) -> (InfoDiagramLayout, Self) {
        let typography = &theme.typography;
        let font_family = crate::theme::font_family_css(typography.stack(typography.code_family));
        let (layout, version) =
            InfoVersionGeometry::prepare(&font_family, typography.base_size_px * 2.0, measurer);
        (
            layout,
            Self {
                font_family,
                text_color: theme.roles.text_primary,
                version,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn info_layout_records_upstream_configured_canvas_size() {
        let (layout, style) = InfoClassicStyle::resolve(
            &PresentationConfig::default(),
            &DeterministicTextMeasurer::default(),
        );
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
