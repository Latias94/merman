use std::ops::Deref;

use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::{PieMermaidCss, PieMermaidStyle};

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

    pub(crate) fn css_binding(&self) -> PieCssBinding<'_> {
        PieCssBinding {
            common: &self.common,
            values: self.style.css_binding(),
        }
    }

    pub(crate) fn content_width(&self) -> Result<f64> {
        self.style.content_width()
    }
}

/// Joins the host's shared stylesheet with the chart's resolved CSS values for emission.
pub(crate) struct PieCssBinding<'a> {
    pub(crate) common: &'a PreparedCommonCss,
    values: &'a PieMermaidCss,
}

impl Deref for PieCssBinding<'_> {
    type Target = PieMermaidCss;

    fn deref(&self) -> &Self::Target {
        self.values
    }
}
