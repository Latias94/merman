use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{
    config::{ConfigLookup, config_font_family_css},
    svg_css::PreparedCommonCss,
};

#[cfg(test)]
use merman_render_core::Result;

use super::PieMermaidStyle;

#[derive(Debug)]
pub struct PieThemePlan {
    style: PieMermaidStyle,
    common: PreparedCommonCss,
}

impl PieThemePlan {
    pub(in crate::pie) fn baseline(
        model: &PieDiagramModel,
        effective_config: &impl ConfigLookup,
    ) -> Self {
        let style = PieMermaidStyle::baseline(model, effective_config);
        let inherited_font_size = style.css_binding().inherited_font_size_px();
        let common = PreparedCommonCss::with_resolved_typography(
            effective_config,
            config_font_family_css(effective_config),
            format!("{inherited_font_size}px"),
        )
        .with_configured_root_font_family(effective_config);
        Self { style, common }
    }

    pub(in crate::pie) fn mermaid_style(&self) -> &PieMermaidStyle {
        &self.style
    }

    pub(in crate::pie) fn common_css(&self) -> &PreparedCommonCss {
        &self.common
    }

    #[cfg(test)]
    pub(in crate::pie) fn content_width(&self) -> Result<f64> {
        self.style.content_width()
    }
}
