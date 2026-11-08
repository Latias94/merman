use merman_theme_contract::{
    ThemeAppearance, ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1,
    ThemeMermaidAppearanceMetadataV1, ThemeMermaidDesignMetadataV1, ThemeRecipeV1,
};

use super::{
    DiagramTheme, DiagramThemeCompiler, MermaidThemeId, ThemeDesignCompileError, ThemePreset,
};

const MERMAID_DESIGN_PAIRS: &[(MermaidThemeId, MermaidThemeId, &str)] = &[
    (MermaidThemeId::Neo, MermaidThemeId::NeoDark, "Neo"),
    (MermaidThemeId::Redux, MermaidThemeId::ReduxDark, "Redux"),
    (
        MermaidThemeId::ReduxColor,
        MermaidThemeId::ReduxDarkColor,
        "Redux Color",
    ),
];

/// Describes literal Mermaid themes with only explicitly declared appearance pairs grouped.
///
/// This read-only projection requires no recipe compiler. Default and Dark remain independent;
/// the Mermaid `null` sentinel is not a registry theme and is not a design.
pub fn describe_mermaid_theme_designs() -> Vec<ThemeMermaidDesignMetadataV1> {
    let mut designs: Vec<ThemeMermaidDesignMetadataV1> = Vec::new();
    for &theme in MermaidThemeId::ALL {
        let pair = MERMAID_DESIGN_PAIRS
            .iter()
            .find(|&&(light, dark, _)| theme == light || theme == dark);
        let (canonical, display_name) = if let Some(&(light, _, display_name)) = pair {
            (light, display_name)
        } else {
            let display_name = match theme {
                MermaidThemeId::Default => "Default",
                MermaidThemeId::Base => "Base",
                MermaidThemeId::Dark => "Dark",
                MermaidThemeId::Forest => "Forest",
                MermaidThemeId::Neutral => "Neutral",
                _ => theme.as_str(),
            };
            (theme, display_name)
        };
        let entry = ThemeMermaidAppearanceMetadataV1 {
            appearance: mermaid_appearance(theme),
            theme_id: theme.as_str().to_owned(),
        };
        if let Some(design) = designs
            .iter_mut()
            .find(|design| design.id == canonical.as_str())
        {
            design.appearances.push(entry);
        } else {
            designs.push(ThemeMermaidDesignMetadataV1 {
                id: canonical.as_str().to_owned(),
                display_name: display_name.to_owned(),
                default_appearance: mermaid_appearance(canonical),
                appearances: vec![entry],
            });
        }
    }
    designs
}

fn mermaid_appearance(theme: MermaidThemeId) -> ThemeAppearance {
    if theme.is_dark() {
        ThemeAppearance::Dark
    } else {
        ThemeAppearance::Light
    }
}

impl DiagramThemeCompiler {
    /// Compiles a canonical design or an appearance-pinned concrete preset.
    ///
    /// Every declared appearance passes the same document admission before selection. A concrete
    /// preset declares only its own appearance, while a paired design declares both.
    pub fn compile_preset_selection(
        &self,
        id: &str,
        appearance: Option<ThemeAppearance>,
    ) -> Result<DiagramTheme, ThemeDesignCompileError> {
        if let Ok(preset) = ThemePreset::from_id(id) {
            let pinned = if preset.is_dark() {
                ThemeAppearance::Dark
            } else {
                ThemeAppearance::Light
            };
            if appearance.is_some_and(|appearance| appearance != pinned) {
                return Err(selection_error(
                    "/appearance",
                    "concrete-preset-appearance",
                    format!("preset `{id}` is fixed to its `{pinned}` appearance"),
                ));
            }
        }
        let recipe = preset_document(id)?;
        self.compile_document(recipe, appearance)
    }

    /// Exports a complete editable recipe for a canonical design or concrete appearance.
    ///
    /// A canonical paired design retains both appearances. A concrete preset retains its one
    /// declared appearance. Every declared branch passes the caller's compiler policy.
    pub fn export_preset_document(
        &self,
        id: &str,
    ) -> Result<ThemeRecipeV1, ThemeDesignCompileError> {
        let recipe = preset_document(id)?;
        self.inspect_document(&recipe)?;
        Ok(recipe)
    }
}

fn preset_document(id: &str) -> Result<ThemeRecipeV1, ThemeDesignCompileError> {
    super::presets::build_preset_document(id).ok_or_else(|| {
        selection_error(
            "/preset",
            "preset-id",
            format!("unknown diagram theme preset `{id}`"),
        )
    })
}

fn selection_error(path: &str, domain: &str, message: String) -> ThemeDesignCompileError {
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::invalid_token_value(path, domain, message),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::diagram_theme::theme_preset_descriptors;

    fn materialization_error(error: ThemeDesignCompileError) -> ThemeMaterializationErrorV1 {
        let ThemeDesignCompileError::Materialization(error) = error else {
            panic!("expected a selection diagnostic")
        };
        error
    }

    #[test]
    fn explicit_appearance_cannot_change_a_concrete_pin() {
        let compiler = DiagramThemeCompiler::new();
        for concrete in theme_preset_descriptors() {
            let conflicting = if concrete.is_dark() {
                ThemeAppearance::Light
            } else {
                ThemeAppearance::Dark
            };
            let error = materialization_error(
                compiler
                    .compile_preset_selection(concrete.id(), Some(conflicting))
                    .unwrap_err(),
            );
            assert_eq!(error.diagnostic().path(), "/appearance");
            assert!(error.to_string().contains(concrete.id()));
            assert!(error.to_string().contains("fixed"));
        }
    }

    #[test]
    fn preset_selection_and_document_export_require_exact_catalog_ids() {
        let compiler = DiagramThemeCompiler::new();
        for id in ["Editor", "editor-light-dark", "one", "future-preset", ""] {
            let error =
                materialization_error(compiler.compile_preset_selection(id, None).unwrap_err());
            assert_eq!(error.diagnostic().path(), "/preset");
            assert!(compiler.export_preset_document(id).is_err());
        }
        for concrete in ["editor-light", "editor-dark", "gruvbox-light", "ayu-dark"] {
            assert!(compiler.compile_preset_selection(concrete, None).is_ok());
            let recipe = compiler.export_preset_document(concrete).unwrap();
            let json = serde_json::to_value(recipe).unwrap();
            assert_eq!(json["schema_version"], 1);
            assert_eq!(json["design"]["appearances"].as_object().unwrap().len(), 1);
        }
    }

    #[test]
    fn official_design_discovery_preserves_every_literal_theme_once() {
        let designs = describe_mermaid_theme_designs();
        assert_eq!(designs.len(), 8);
        let mut actual = BTreeSet::new();
        for design in &designs {
            assert!(
                design
                    .appearances
                    .iter()
                    .any(|appearance| appearance.appearance == design.default_appearance)
            );
            let mut appearances = BTreeSet::new();
            for appearance in &design.appearances {
                assert!(appearances.insert(appearance.appearance));
                assert!(actual.insert(appearance.theme_id.as_str()));
                let theme = MermaidThemeId::parse(&appearance.theme_id).unwrap();
                assert_eq!(appearance.appearance, mermaid_appearance(theme));
            }
        }
        assert_eq!(
            actual,
            MermaidThemeId::ALL
                .iter()
                .map(|theme| theme.as_str())
                .collect()
        );
        assert!(!actual.contains("null"));
        assert_eq!(
            designs
                .iter()
                .filter(|design| design.appearances.len() == 2)
                .map(|design| {
                    (
                        design.id.as_str(),
                        design
                            .appearances
                            .iter()
                            .map(|appearance| appearance.theme_id.as_str())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("neo", vec!["neo", "neo-dark"]),
                ("redux", vec!["redux", "redux-dark"]),
                ("redux-color", vec!["redux-color", "redux-dark-color"]),
            ]
        );
    }

    #[test]
    fn original_default_and_dark_remain_independent_single_appearance_designs() {
        let designs = describe_mermaid_theme_designs();
        for (id, appearance) in [
            ("default", ThemeAppearance::Light),
            ("base", ThemeAppearance::Light),
            ("dark", ThemeAppearance::Dark),
            ("forest", ThemeAppearance::Light),
            ("neutral", ThemeAppearance::Light),
        ] {
            let design = designs.iter().find(|design| design.id == id).unwrap();
            assert_eq!(design.default_appearance, appearance);
            assert_eq!(
                design.appearances,
                [ThemeMermaidAppearanceMetadataV1 {
                    appearance,
                    theme_id: id.to_owned(),
                }]
            );
        }
    }
}
