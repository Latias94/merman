use crate::config::{config_bool, config_f64, config_string};
use serde_json::Value;

const DEFAULT_TEXT_POSITION: f64 = 0.75;
const DEFAULT_DONUT_HOLE: f64 = 0.0;
const DEFAULT_LEGEND_POSITION: &str = "right";
const DEFAULT_USE_MAX_WIDTH: bool = true;

/// Mirror parseFontSize: numbers stay fractional; strings use parseInt(_, 10), even with CSS units.
pub(super) fn pie_outer_stroke_width(config: &Value) -> f64 {
    let value = &config["themeVariables"]["pieOuterStrokeWidth"];
    if let Some(width) = value.as_f64() {
        return width;
    }
    value
        .as_str()
        .and_then(|text| {
            let text = text.trim_start_matches(|ch: char| {
                (ch.is_whitespace() && ch != '\u{85}') || ch == '\u{feff}'
            });
            let digits = text
                .strip_prefix('-')
                .or_else(|| text.strip_prefix('+'))
                .unwrap_or(text);
            let count = digits
                .bytes()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if count == 0 {
                return None;
            }
            text[..text.len() - digits.len() + count]
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(2.0)
}

/// Resolve stylesheet sizes whose geometry does not depend on the host document.
pub(super) fn pie_font_size_px(css: &str, inherited_px: f64) -> f64 {
    pie_font_size(css, inherited_px)
        .map(|(size, _)| size)
        .unwrap_or(inherited_px)
}

/// Retain whether the emitted role size actually consumes the root font size.
pub(super) fn pie_font_size(css: &str, inherited_px: f64) -> Option<(f64, bool)> {
    let mut parser = cssparser::Parser::new(css);
    let size = match parser.next() {
        Ok(cssparser::Token::Number { value, .. }) if *value == 0.0 => Some((0.0, false)),
        Ok(cssparser::Token::Dimension { value, unit, .. }) => {
            let scale = if unit.eq_ignore_ascii_case("px") {
                Some((1.0, false))
            } else if unit.eq_ignore_ascii_case("em") {
                Some((inherited_px, true))
            } else {
                None
            };
            scale.map(|(scale, inherited)| (f64::from(*value) * scale, inherited))
        }
        Ok(cssparser::Token::Percentage { unit_value, .. }) => {
            Some((f64::from(*unit_value) * inherited_px, true))
        }
        Ok(cssparser::Token::Ident(value))
            if value.eq_ignore_ascii_case("inherit") || value.eq_ignore_ascii_case("unset") =>
        {
            Some((inherited_px, true))
        }
        _ => None,
    };
    let _ = parser.try_parse(cssparser::parse_important);
    if parser.expect_exhausted().is_err() {
        return None;
    }
    size.filter(|(size, _)| size.is_finite() && *size >= 0.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PieLegendPosition {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

pub(crate) struct PieConfigView<'a> {
    pie_config: &'a Value,
}

impl<'a> PieConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            pie_config: effective_config.get("pie").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(&self) -> PieLayoutSettings {
        PieLayoutSettings {
            text_position: self
                .pie_f64("textPosition")
                .unwrap_or(DEFAULT_TEXT_POSITION),
            legend_position: self.legend_position(),
        }
    }

    pub(crate) fn render_settings(&self) -> PieRenderSettings {
        PieRenderSettings {
            donut_hole: self.donut_hole(),
            legend_position: self.legend_position(),
            use_max_width: self
                .pie_bool("useMaxWidth")
                .unwrap_or(DEFAULT_USE_MAX_WIDTH),
        }
    }

    fn donut_hole(&self) -> f64 {
        let donut_hole = self.pie_f64("donutHole").unwrap_or(DEFAULT_DONUT_HOLE);
        if donut_hole > 0.0 && donut_hole <= 0.9 {
            donut_hole
        } else {
            DEFAULT_DONUT_HOLE
        }
    }

    fn legend_position(&self) -> PieLegendPosition {
        match self.pie_string("legendPosition").as_deref() {
            Some("top") => PieLegendPosition::Top,
            Some("bottom") => PieLegendPosition::Bottom,
            Some("left") => PieLegendPosition::Left,
            Some("center") => PieLegendPosition::Center,
            Some("right") => PieLegendPosition::Right,
            _ => match DEFAULT_LEGEND_POSITION {
                "top" => PieLegendPosition::Top,
                "bottom" => PieLegendPosition::Bottom,
                "left" => PieLegendPosition::Left,
                "center" => PieLegendPosition::Center,
                _ => PieLegendPosition::Right,
            },
        }
    }

    fn pie_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.pie_config, &[key])
    }

    fn pie_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.pie_config, &[key])
    }

    fn pie_string(&self, key: &str) -> Option<String> {
        config_string(self.pie_config, &[key])
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PieLayoutSettings {
    pub(crate) text_position: f64,
    pub(crate) legend_position: PieLegendPosition,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PieRenderSettings {
    pub(crate) donut_hole: f64,
    pub(crate) legend_position: PieLegendPosition,
    pub(crate) use_max_width: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn outer_stroke_width_matches_mermaid_parse_font_size() {
        for (value, expected) in [
            (json!(20.5), 20.5),
            (json!(0), 0.0),
            (json!("20.5px"), 20.0),
            (json!("5em"), 5.0),
            (json!("100%"), 100.0),
            (json!("  +7.5rem"), 7.0),
            (json!(".5px"), 2.0),
            (json!("calc(1px + 1em)"), 2.0),
            (json!("var(--width)"), 2.0),
            (json!(null), 2.0),
        ] {
            assert_eq!(
                pie_outer_stroke_width(&json!({"themeVariables": {
                    "pieOuterStrokeWidth": value
                }})),
                expected
            );
        }
        assert_eq!(pie_outer_stroke_width(&json!({})), 2.0);
    }

    #[test]
    fn pie_layout_settings_preserve_defaults() {
        let cfg = json!({});
        let settings = PieConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.text_position, DEFAULT_TEXT_POSITION);
        assert_eq!(settings.legend_position, PieLegendPosition::Right);
    }

    #[test]
    fn pie_layout_settings_project_configured_values() {
        let cfg = json!({
            "pie": {
                "textPosition": "0.5",
                "legendPosition": "top"
            }
        });
        let settings = PieConfigView::new(&cfg).layout_settings();

        assert_eq!(settings.text_position, 0.5);
        assert_eq!(settings.legend_position, PieLegendPosition::Top);
    }

    #[test]
    fn pie_render_settings_project_and_normalize_values() {
        let cfg = json!({
            "pie": {
                "donutHole": "0.4",
                "legendPosition": "left",
                "useMaxWidth": false
            }
        });
        let settings = PieConfigView::new(&cfg).render_settings();

        assert_eq!(settings.donut_hole, 0.4);
        assert_eq!(settings.legend_position, PieLegendPosition::Left);
        assert!(!settings.use_max_width);
    }

    #[test]
    fn pie_render_settings_reject_invalid_donut_hole() {
        for donut_hole in [0.0, -0.1, 1.0] {
            let cfg = json!({
                "pie": {
                    "donutHole": donut_hole
                }
            });
            let settings = PieConfigView::new(&cfg).render_settings();

            assert_eq!(settings.donut_hole, DEFAULT_DONUT_HOLE);
        }
    }
}
