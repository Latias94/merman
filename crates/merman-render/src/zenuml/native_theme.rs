use merman_theme_contract::v2::{Elevation, Form, MarkerStyle};
use rustc_hash::FxHashMap;

use crate::text::TextMeasurer;
use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

use super::{ZenumlDiagramLayout, ZenumlTypography};

pub(crate) struct ZenumlThemeAdapter;

impl FamilyThemeAdapter for ZenumlThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::ZENUML;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::CALLOUT)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::ELEVATION)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::MARKER_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct ZenumlNativeStyle {
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
    pub(crate) actor: Color,
    pub(crate) container: Color,
    pub(crate) callout: Color,
    pub(crate) emphasis: Color,
    pub(crate) border: Color,
    pub(crate) connector: Color,
    pub(crate) guide: Color,
    pub(crate) primary: Color,
    pub(crate) muted: Color,
    pub(crate) title: Color,
    pub(crate) callout_text: Color,
    pub(crate) emphasis_text: Color,
    pub(crate) number_text: Color,
    pub(crate) number_widths: FxHashMap<String, f64>,
}

impl ZenumlNativeStyle {
    pub(super) fn resolve(theme: &FamilyTheme, background: Option<Color>) -> Self {
        let roles = &theme.roles;
        let connector = roles.stroke_connector.first_color();
        let badge = connector.composite(
            background.unwrap_or_else(|| roles.canvas.first_color()),
            1.0,
        );
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        Self {
            form: theme.form,
            elevation: theme.elevation.clone(),
            actor: roles.surface_node.first_color(),
            container: roles.surface_container.first_color(),
            callout: roles.surface_callout.first_color(),
            emphasis: roles.surface_emphasis.first_color(),
            border: roles.stroke_border.first_color(),
            connector,
            guide: roles.stroke_guide.first_color(),
            primary: roles.text_primary,
            muted: roles.text_muted,
            title: roles.text_title,
            callout_text: roles.text_callout,
            emphasis_text: roles.text_on_emphasis,
            number_text: if badge.contrast(black) >= badge.contrast(white) {
                black
            } else {
                white
            },
            number_widths: FxHashMap::default(),
        }
    }

    pub(crate) fn measure_numbers(
        &mut self,
        layout: &mut ZenumlDiagramLayout,
        typography: &ZenumlTypography,
        measurer: &dyn TextMeasurer,
    ) {
        let style = typography.number_style();
        let numbers = layout
            .messages
            .iter()
            .map(|message| &message.number)
            .chain(layout.self_calls.iter().map(|call| &call.number))
            .chain(
                layout
                    .creations
                    .iter()
                    .map(|creation| &creation.message.number),
            )
            .chain(layout.returns.iter().map(|returned| &returned.number))
            .chain(layout.fragments.iter().map(|fragment| &fragment.number));
        let mut max_width: f64 = 0.0;
        for number in numbers.filter(|number| !number.is_empty()) {
            let width = *self.number_widths.entry(number.clone()).or_insert_with(|| {
                let height = style.font_size * 1.4;
                (measurer.measure(number, &style).width + style.font_size * 0.6).max(height)
            });
            max_width = max_width.max(width);
        }
        if max_width > 0.0 {
            // Numbers are anchored just to the left of the connector or fragment.
            layout.frame_border_left += max_width;
        }
    }

    pub(crate) fn filled_arrow(&self) -> Option<&'static str> {
        match self.form.marker_style {
            MarkerStyle::Classic => Some("M1 1.25 L6.15 4.5 L1 7.75 Z"),
            MarkerStyle::Filled => Some("M0.75 0.75 L6.25 4.5 L0.75 8.25 Z"),
            MarkerStyle::Stealth => Some("M0.75 0.75 L6.25 4.5 L0.75 8.25 L2.5 4.5 Z"),
            MarkerStyle::Open => None,
        }
    }
}

impl ZenumlTypography {
    pub(super) fn from_native(theme: &FamilyTheme) -> Self {
        let typography = &theme.typography;
        Self {
            font_family: Some(font_family_css(typography.stack(typography.body_family))),
            font_size: Some(typography.base_size_px),
            font_weight: Some(typography.label_weight.to_string()),
            title_family: Some(font_family_css(typography.stack(typography.title_family))),
            title_weight: Some(typography.title_weight.to_string()),
        }
    }
}
