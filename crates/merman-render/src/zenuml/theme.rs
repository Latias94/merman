use super::native_theme::ZenumlNativeStyle;

/// Prepared source compatibility values shared by ZenUML measurement and paint.
#[derive(Debug, Default)]
pub(crate) struct ZenumlSourceStyle {
    pub(crate) typography: super::ZenumlTypography,
    pub(crate) native: Option<Box<ZenumlNativeStyle>>,
    colors: std::collections::BTreeMap<&'static str, String>,
    transparent_frame: bool,
}

impl ZenumlSourceStyle {
    pub(crate) fn for_native(
        theme: &crate::theme::FamilyTheme,
        background: Option<crate::theme::Color>,
    ) -> Self {
        Self {
            typography: super::ZenumlTypography::from_native(theme),
            native: Some(Box::new(ZenumlNativeStyle::resolve(theme, background))),
            ..Self::default()
        }
    }

    pub(crate) fn new(config: &serde_json::Value) -> Self {
        let Some(config) = crate::config::value_at(config, &["themeVariables", "zenuml"])
            .and_then(serde_json::Value::as_object)
        else {
            return Self::default();
        };
        let configured = |key: &str| {
            config
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let colors = [
            "backgroundColor",
            "titleTextColor",
            "messageTextColor",
            "messageLineColor",
            "lifelineColor",
            "sequenceNumberTextColor",
            "fragmentBorderColor",
            "borderColor",
            "textColor",
            "lineColor",
            "actorBackgroundColor",
            "actorBorderColor",
            "actorTextColor",
            "noteBackgroundColor",
            "noteBorderColor",
            "noteTextColor",
            "groupBackgroundColor",
            "groupBorderColor",
            "groupTextColor",
            "activationBackgroundColor",
            "activationBorderColor",
            "mutedSurfaceColor",
            "fragmentTextColor",
            "subtleTextColor",
        ]
        .into_iter()
        .filter_map(|key| {
            let color = configured(key).or_else(|| {
                let inherited: &[&str] = match key {
                    "actorBackgroundColor"
                    | "noteBackgroundColor"
                    | "groupBackgroundColor"
                    | "mutedSurfaceColor" => &["surfaceColor"],
                    "titleTextColor"
                    | "messageTextColor"
                    | "sequenceNumberTextColor"
                    | "actorTextColor"
                    | "noteTextColor"
                    | "groupTextColor"
                    | "subtleTextColor"
                    | "fragmentTextColor" => &["textColor"],
                    "messageLineColor" | "lifelineColor" => &["lineColor"],
                    "fragmentBorderColor"
                    | "actorBorderColor"
                    | "noteBorderColor"
                    | "groupBorderColor"
                    | "activationBorderColor" => &["borderColor"],
                    "activationBackgroundColor" => &["mutedSurfaceColor", "surfaceColor"],
                    _ => &[],
                };
                inherited.iter().find_map(|key| configured(key))
            });
            color.map(|value| (key, value))
        })
        .collect();
        Self {
            typography: super::ZenumlTypography::from_config(config),
            colors,
            ..Self::default()
        }
    }

    pub(crate) fn color<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.configured_color(key).unwrap_or(fallback)
    }

    pub(crate) fn transparent_frame(&self) -> bool {
        self.transparent_frame
    }

    pub(crate) fn configured_color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(String::as_str)
    }

    pub(crate) fn with_canvas_replacement(mut self) -> Self {
        self.transparent_frame = true;
        self
    }

    pub(crate) fn uses_default_css(&self) -> bool {
        !self.transparent_frame && self.colors.is_empty() && self.typography.uses_default_css()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn source_colors_inherit_within_zenuml_and_keep_explicit_terminal_overrides() {
        let source = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"zenuml": {
                "textColor": "#123456", "surfaceColor": "#abcdef", "borderColor": "#765432"
            }}
        }));
        for key in [
            "actorTextColor",
            "noteTextColor",
            "groupTextColor",
            "fragmentTextColor",
            "subtleTextColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#123456"), "{key}");
        }
        for key in [
            "actorBackgroundColor",
            "noteBackgroundColor",
            "groupBackgroundColor",
            "mutedSurfaceColor",
            "activationBackgroundColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#abcdef"), "{key}");
        }
        for key in [
            "actorBorderColor",
            "noteBorderColor",
            "groupBorderColor",
            "activationBorderColor",
        ] {
            assert_eq!(source.configured_color(key), Some("#765432"), "{key}");
        }
        let overridden = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"zenuml": {
                "textColor": "#123456", "surfaceColor": "#abcdef", "borderColor": "#765432",
                "actorTextColor": "#111111", "noteTextColor": "#222222", "groupTextColor": "#333333",
                "actorBackgroundColor": "#444444", "actorBorderColor": "#555555", "mutedSurfaceColor": "#666666"
            }}
        }));
        for (key, color) in [
            ("actorTextColor", "#111111"),
            ("noteTextColor", "#222222"),
            ("groupTextColor", "#333333"),
            ("actorBackgroundColor", "#444444"),
            ("actorBorderColor", "#555555"),
            ("activationBackgroundColor", "#666666"),
        ] {
            assert_eq!(overridden.configured_color(key), Some(color), "{key}");
        }
        let unrelated = super::ZenumlSourceStyle::new(&serde_json::json!({
            "themeVariables": {"textColor": "#999999", "surfaceColor": "#999999", "borderColor": "#999999"}
        }));
        assert_eq!(unrelated.configured_color("actorTextColor"), None);
        assert_eq!(unrelated.configured_color("actorBackgroundColor"), None);
        assert_eq!(unrelated.configured_color("actorBorderColor"), None);

        for (muted, activation, expected) in [
            (serde_json::json!(""), serde_json::Value::Null, ""),
            (serde_json::json!(false), serde_json::Value::Null, "#abcdef"),
            (
                serde_json::json!("#123456"),
                serde_json::json!("#654321"),
                "#654321",
            ),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "themeVariables": {"zenuml": {
                    "surfaceColor": "#abcdef", "mutedSurfaceColor": muted,
                    "activationBackgroundColor": activation,
                    "actorBackgroundColor": "", "noteBackgroundColor": 7
                }}
            }));
            assert_eq!(
                source.configured_color("activationBackgroundColor"),
                Some(expected)
            );
            assert_eq!(source.configured_color("actorBackgroundColor"), Some(""));
            assert_eq!(
                source.configured_color("noteBackgroundColor"),
                Some("#abcdef")
            );
        }
    }

    #[test]
    fn source_style_defaults_without_a_zenuml_object_and_keeps_namespace_isolation() {
        for namespace in [
            serde_json::Value::Null,
            serde_json::json!(false),
            serde_json::json!(7),
            serde_json::json!("Inter"),
            serde_json::json!([{"fontFamily": "Inter", "textColor": "red"}]),
            serde_json::json!({}),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "fontFamily": "Root Font", "fontSize": 40,
                "themeVariables": {
                    "fontFamily": "Global Font", "fontSize": 32,
                    "textColor": "red", "surfaceColor": "blue", "zenuml": namespace
                }
            }));
            assert_eq!(source.typography.css_family(), "Helvetica,Verdana,serif");
            assert_eq!(source.typography.message_style().font_size, 16.0);
            assert_eq!(source.configured_color("actorTextColor"), None);
            assert_eq!(source.configured_color("actorBackgroundColor"), None);
        }
    }

    #[test]
    fn source_typography_keeps_authored_empty_family_and_existing_size_parsing() {
        for (family, size, expected_family, expected_size) in [
            (
                serde_json::json!("Inter, sans-serif"),
                serde_json::json!("24px"),
                Some("Inter, sans-serif"),
                21.0,
            ),
            (serde_json::json!(""), serde_json::json!(16), Some(""), 14.0),
            (serde_json::json!([]), serde_json::json!("0px"), None, 16.0),
        ] {
            let source = super::ZenumlSourceStyle::new(&serde_json::json!({
                "themeVariables": {"zenuml": {"fontFamily": family, "fontSize": size}}
            }));
            assert_eq!(source.typography.font_family.as_deref(), expected_family);
            assert_eq!(source.typography.message_style().font_size, expected_size);
        }
    }
}
