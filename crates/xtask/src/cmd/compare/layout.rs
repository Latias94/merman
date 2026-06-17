//! Common layout options used by SVG compare commands.

pub(crate) fn svg_compare_site_config() -> merman::MermaidConfig {
    merman::MermaidConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    }))
}

pub(crate) fn svg_compare_site_config_with(overrides: serde_json::Value) -> merman::MermaidConfig {
    let mut config = svg_compare_site_config();
    config.deep_merge(&overrides);
    config
}

pub(crate) fn svg_compare_engine() -> merman::Engine {
    merman::Engine::new().with_site_config(svg_compare_site_config())
}

pub(crate) fn svg_compare_engine_with_site_config(overrides: serde_json::Value) -> merman::Engine {
    merman::Engine::new().with_site_config(svg_compare_site_config_with(overrides))
}

pub(crate) fn svg_compare_layout_opts() -> merman_render::LayoutOptions {
    merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::new(
            merman_render::text::VendoredFontMetricsTextMeasurer::default(),
        ),
        use_manatee_layout: true,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_compare_engine_keeps_legacy_init_theme_variables_for_baselines() {
        let engine = svg_compare_engine();
        let meta = engine
            .parse_metadata_sync(
                r##"%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#123456"}}}%%
flowchart TD
  A --> B
"##,
                merman::ParseOptions::default(),
            )
            .expect("parse succeeds")
            .expect("diagram detected");

        assert_eq!(
            meta.effective_config.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
    }
}
