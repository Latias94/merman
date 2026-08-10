use merman_core::MermaidConfig;

use super::DiagramThemeSpec;

/// Compiles only the explicitly requested Mermaid compatibility lane.
///
/// Typed canvas, typography, semantic rules, palettes, and effects are consumed by the selected
/// render family. Projecting any of them into global Mermaid configuration would create a second
/// cascade and allow one family's roles to affect unrelated renderers.
pub(super) fn compile(spec: &DiagramThemeSpec) -> MermaidConfig {
    spec.mermaid().to_mermaid_config()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, MermaidThemeCompatibility, ThemePreset, ThemeRule,
        ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };

    #[test]
    fn preset_compatibility_is_explicit_and_contains_no_typed_projection() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::EditorDark)
            .expect("editor dark theme");
        let config = theme.spec().mermaid().to_mermaid_config();

        assert_eq!(config.get_str("theme"), Some("base"));
        assert_eq!(config.get_bool("darkMode"), Some(true));
        assert_eq!(config.get_bool("themeVariables.darkMode"), Some(true));
        assert_eq!(config.get_str("themeVariables.primaryColor"), None);
    }

    #[test]
    fn explicit_mermaid_compatibility_values_are_preserved() {
        let compatibility = MermaidThemeCompatibility::default()
            .with_theme("base")
            .expect("valid Mermaid theme")
            .with_dark_mode(true)
            .expect("valid Mermaid dark mode")
            .with_variable("primaryColor", "#123456")
            .expect("valid Mermaid variable");
        let projected = compile(&DiagramThemeSpec::new().with_mermaid_compatibility(compatibility));

        assert_eq!(projected.get_str("theme"), Some("base"));
        assert_eq!(projected.get_bool("darkMode"), Some(true));
        assert_eq!(
            projected.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
    }

    #[test]
    fn semantic_rules_never_enter_the_global_mermaid_config() {
        let fill = CanvasPaint::solid("#2563eb").expect("valid fill");
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(fill),
            )));

        assert_eq!(compile(&spec).as_value(), &serde_json::json!({}));
    }
}
