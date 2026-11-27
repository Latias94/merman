use merman_model::presentation::PresentationConfig;

use merman_render_core::config::{config_css_number_or_string, config_font_family_css};
use merman_theme_contract::v2::FamilyTheme;

use super::{EventModelingCssBinding, EventModelingNativeStyle};

/// Style shared by Event Modeling layout and SVG emission.
#[derive(Debug)]
pub(crate) struct EventModelingStyle {
    css_binding: EventModelingCssBinding,
    native: Option<EventModelingNativeStyle>,
    font_family_css: String,
    font_size_css: Option<String>,
}

impl EventModelingStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        let values = config.as_value();
        let mut css_binding = EventModelingCssBinding::resolve(values);
        let native = native.map(|theme| EventModelingNativeStyle::resolve(theme, &mut css_binding));
        Self {
            css_binding,
            native,
            font_family_css: config_font_family_css(values),
            font_size_css: config_css_number_or_string(values, &["themeVariables", "fontSize"]),
        }
    }

    pub(crate) fn css_binding(&self) -> &EventModelingCssBinding {
        &self.css_binding
    }

    pub(crate) fn native(&self) -> Option<&EventModelingNativeStyle> {
        self.native.as_ref()
    }

    pub(crate) fn write_stylesheet(
        &self,
        out: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
    ) {
        if let Some(native) = self.native() {
            native.write_stylesheet(out, diagram_id);
            return;
        }
        let fill = &self.css_binding.text_color;
        let font = &self.font_family_css;
        let _ = write!(
            out,
            "#{diagram_id} {{ fill: {fill}; color: {fill}; }}#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {font};"
        );
        if let Some(font_size) = &self.font_size_css {
            let _ = write!(out, " font-size: {font_size};");
        }
        let _ = write!(out, " }}#{diagram_id} .em-relation {{ fill: none; }}");
    }
}
