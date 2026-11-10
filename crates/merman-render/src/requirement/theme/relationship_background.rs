use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::ResolvedDiagramTheme;
use crate::family::RelationshipLabelBackgroundPlan;

pub(super) fn resolve(
    theme: Option<&ResolvedDiagramTheme>,
    config: &PresentationConfig,
) -> RelationshipLabelBackgroundPlan {
    // RequirementCssBinding uses the family-specific value only for labelBkg.
    // relationLabelBackground remains the independent reqLabelBox CSS owner.
    let family_background =
        config.as_value()["themeVariables"]["requirementEdgeLabelBackground"].is_string();
    RelationshipLabelBackgroundPlan::new(
        theme,
        config.config_paint_overrides_typed_default(if family_background {
            "themeVariables.requirementEdgeLabelBackground"
        } else {
            "themeVariables.edgeLabelBackground"
        }),
        config.config_paint_overrides_typed_default("themeVariables.edgeLabelBackground"),
    )
}
