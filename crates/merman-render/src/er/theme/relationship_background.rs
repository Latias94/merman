use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::ResolvedDiagramTheme;
use crate::family::RelationshipLabelBackgroundPlan;

pub(super) fn resolve(
    theme: Option<&ResolvedDiagramTheme>,
    config: &PresentationConfig,
) -> RelationshipLabelBackgroundPlan {
    // Match ErCssBinding's effective suppliers, rather than treating all related
    // compatibility properties as owners of every background layer.
    let er_background = matches!(
        config.get_str("theme"),
        Some("redux-color" | "redux-dark-color")
    ) && config.as_value()["themeVariables"]["erEdgeLabelBackground"]
        .is_string();
    RelationshipLabelBackgroundPlan::new(
        theme,
        config.config_paint_overrides_typed_default(if er_background {
            "themeVariables.erEdgeLabelBackground"
        } else {
            "themeVariables.tertiaryColor"
        }),
        config.config_paint_overrides_typed_default(if er_background {
            "themeVariables.erEdgeLabelBackground"
        } else {
            "themeVariables.edgeLabelBackground"
        }),
    )
}
