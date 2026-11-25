use merman_model::presentation::PresentationConfig;
use serde_json::Value;

use super::{WardleyDiagramLayout, WardleyTextRole};

/// The four configurable SVG font-size expressions in the pinned Wardley renderer.
#[derive(Debug)]
pub(super) struct WardleyFontSizes {
    title: WardleyFontSize,
    axis: WardleyFontSize,
    stage: WardleyFontSize,
    label: WardleyFontSize,
    native_base: Option<WardleyFontSize>,
}

#[derive(Debug)]
pub(crate) struct WardleyFontSize {
    pub(crate) css: String,
    pub(crate) used_px: Option<f64>,
}

impl WardleyFontSize {
    pub(crate) fn numeric(size: f64) -> Self {
        if !size.is_finite() {
            return Self::css("NaN".to_owned());
        }
        Self::css(crate::number_format::canonical_number(size).to_string())
    }

    fn css(css: String) -> Self {
        use crate::mermaid_style::{
            css_font_size_is_zero, resolve_explicit_css_absolute_font_size_px,
        };

        let invalid_attribute = {
            let mut parser = cssparser::Parser::new(&css);
            let mut invalid = false;
            while let Ok(token) = parser.next() {
                if matches!(
                    token,
                    cssparser::Token::Semicolon | cssparser::Token::Delim('!')
                ) {
                    invalid = true;
                    break;
                }
            }
            invalid
        };
        if invalid_attribute {
            return Self { css, used_px: None };
        }
        // SVG attributes accept unitless numbers, but not declaration priority. Reuse the
        // existing absolute-length parser; relative, invalid and host-owned sizes stay unknown.
        let used_px = if css_font_size_is_zero(&css) {
            Some(0.0)
        } else {
            let mut parser = cssparser::Parser::new(&css);
            let coefficient = match parser.next() {
                Ok(cssparser::Token::Number { value, .. }) => {
                    let value = *value;
                    parser.is_exhausted().then_some(value)
                }
                Ok(cssparser::Token::Dimension { value, .. }) => Some(*value),
                _ => None,
            };
            coefficient
                .filter(|value| *value > 0.0 && value.is_normal())
                .and_then(|_| {
                    css.trim()
                        .parse::<f64>()
                        .ok()
                        .or_else(|| resolve_explicit_css_absolute_font_size_px(&css))
                })
                .filter(|size| *size > 0.0 && (*size as f32).is_normal())
        };
        Self { css, used_px }
    }
}

impl WardleyFontSizes {
    pub(super) fn from_config(config: &Value) -> Self {
        let config = &config["wardley-beta"];
        let axis = config.get("axisFontSize").filter(|value| !value.is_null());
        let label = config.get("labelFontSize").filter(|value| !value.is_null());
        let axis_number = axis.map_or(Some(12.0), crate::config::json_f64);
        let direct = |value: Option<&Value>, fallback| {
            value.map_or_else(
                || WardleyFontSize::numeric(fallback),
                |value| match value {
                    Value::Number(number) => {
                        WardleyFontSize::numeric(number.as_f64().unwrap_or(f64::NAN))
                    }
                    Value::String(css) => WardleyFontSize::css(css.clone()),
                    _ => WardleyFontSize::css("NaN".to_owned()),
                },
            )
        };
        // Only these two upstream expressions perform arithmetic. A CSS length string does
        // not become its numeric prefix before multiplication or subtraction.
        let arithmetic = |value: Option<f64>| {
            value.map_or_else(
                || WardleyFontSize::css("NaN".to_owned()),
                WardleyFontSize::numeric,
            )
        };
        Self {
            title: arithmetic(axis_number.map(|size| size * 1.05)),
            axis: direct(axis, 12.0),
            stage: arithmetic(axis_number.map(|size| size - 2.0)),
            label: direct(label, 10.0),
            native_base: None,
        }
    }

    pub(super) fn apply_native(&mut self, config: &PresentationConfig, base_size: f64) {
        if !config.explicit_config_owns_path("wardley-beta.axisFontSize") {
            self.title = WardleyFontSize::numeric(base_size * 1.05);
            self.axis = WardleyFontSize::numeric(base_size);
            self.stage = WardleyFontSize::numeric((base_size - 2.0).max(0.0));
        }
        if !config.explicit_config_owns_path("wardley-beta.labelFontSize") {
            self.label = WardleyFontSize::numeric(base_size);
        }
        self.native_base = Some(WardleyFontSize::numeric(base_size));
    }

    pub(super) fn role(&self, role: WardleyTextRole) -> Option<&WardleyFontSize> {
        match role {
            WardleyTextRole::Title => Some(&self.title),
            WardleyTextRole::AxisLabel => Some(&self.axis),
            WardleyTextRole::StageLabel => Some(&self.stage),
            WardleyTextRole::NodeLabel
            | WardleyTextRole::AnchorLabel
            | WardleyTextRole::EvolvedLabel
            | WardleyTextRole::LinkLabel => Some(&self.label),
            WardleyTextRole::AnnotationPoint
            | WardleyTextRole::AnnotationBoxLine
            | WardleyTextRole::Note
            | WardleyTextRole::Accelerator
            | WardleyTextRole::Deaccelerator => self.native_base.as_ref(),
        }
    }

    pub(super) fn apply_known_sizes(&self, layout: &mut WardleyDiagramLayout) {
        if let Some(size) = self.title.used_px
            && let Some(title) = &mut layout.title
        {
            title.font_size = size;
        }
        if let Some(size) = self.axis.used_px {
            layout.axes.x_label.font_size = size;
            layout.axes.y_label.font_size = size;
        }
        if let Some(size) = self.stage.used_px {
            for stage in &mut layout.stages {
                stage.label.font_size = size;
            }
        }
        if let Some(size) = self.label.used_px {
            for node in &mut layout.nodes {
                node.label_layout.font_size = size;
            }
            for link in &mut layout.links {
                if let Some(label) = &mut link.label {
                    label.font_size = size;
                }
            }
        }
        if let Some(size) = self.native_base.as_ref().and_then(|font| font.used_px) {
            for annotation in &mut layout.annotations {
                for point in &mut annotation.points {
                    point.label.font_size = size;
                }
            }
            for note in &mut layout.notes {
                note.text.font_size = size;
            }
            for arrow in layout
                .accelerators
                .iter_mut()
                .chain(&mut layout.deaccelerators)
            {
                arrow.label.font_size = size;
            }
        }
    }
}
