use std::collections::BTreeMap;

use merman_model::families::requirement::RequirementDiagramModel;
use merman_model::presentation::PresentationConfig;

use merman_render_core::resources::OperationWorkMeter;
use merman_theme_contract::v2::{Color, FamilyTheme};

mod node_visual;

#[derive(Debug)]
pub struct RequirementPaintThemePlan {
    node_visuals: BTreeMap<String, node_visual::RequirementNodeVisual>,
    css: super::RequirementCssBinding,
}

impl RequirementPaintThemePlan {
    pub fn resolve(
        native: Option<(&FamilyTheme, Option<Color>)>,
        config: &PresentationConfig,
        model: &RequirementDiagramModel,
        work: &OperationWorkMeter,
    ) -> merman_render_core::Result<Self> {
        let css = native.map_or_else(
            || super::RequirementCssBinding::resolve(config.as_value(), None, None),
            |(theme, canvas)| super::RequirementCssBinding::resolve_native(theme, config, canvas),
        );
        Self {
            css,
            node_visuals: BTreeMap::new(),
        }
        .prepare_node_visuals(model, config, work)
    }

    pub fn css(&self) -> &super::RequirementCssBinding {
        &self.css
    }

    pub(crate) fn node_visual(&self, id: &str) -> Option<&node_visual::RequirementNodeVisual> {
        self.node_visuals.get(id)
    }
}
