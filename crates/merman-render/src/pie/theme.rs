use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::PieMermaidStyle;

#[cfg(test)]
use crate::Result;
use crate::config::{ConfigLookup, config_font_family_css};
use crate::svg::PreparedCommonCss;

#[derive(Debug)]
pub(crate) struct PieThemePlan {
    pub(super) style: PieMermaidStyle,
    pub(super) common: PreparedCommonCss,
}

impl PieThemePlan {
    pub(crate) fn baseline(model: &PieDiagramModel, effective_config: &impl ConfigLookup) -> Self {
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

    pub(crate) fn mermaid_style(&self) -> &PieMermaidStyle {
        &self.style
    }

    pub(crate) fn common_css(&self) -> &PreparedCommonCss {
        &self.common
    }

    #[cfg(test)]
    pub(crate) fn content_width(&self) -> Result<f64> {
        self.style.content_width()
    }
}
