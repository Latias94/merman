use std::collections::BTreeMap;

use merman_theme_contract::{
    ThemeAppearance, ThemeAppearanceOverridesV1, ThemeDesignTokenValueV1, ThemeDesignV1,
    ThemePresetAppearanceMetadataV1, ThemePresetFamilyDesignV1, ThemePresetMetadataV1,
    ThemePresetQualifiedCellV1, ThemeRecipeV1,
};

use super::{
    DiagramThemeCompiler, ThemePreset, catalog, theme_preset_descriptors,
    theme_preset_unavailability_reason,
};

struct PresetDesign {
    id: &'static str,
    display_name: &'static str,
    default_appearance: ThemeAppearance,
    appearances: &'static [(ThemeAppearance, ThemePreset)],
}

const PRESET_DESIGNS: &[PresetDesign] = &[
    PresetDesign {
        id: "editor",
        display_name: "Editor",
        default_appearance: ThemeAppearance::Light,
        appearances: &[
            (ThemeAppearance::Light, ThemePreset::EditorLight),
            (ThemeAppearance::Dark, ThemePreset::EditorDark),
        ],
    },
    PresetDesign {
        id: "one-dark",
        display_name: "One Dark",
        default_appearance: ThemeAppearance::Dark,
        appearances: &[(ThemeAppearance::Dark, ThemePreset::OneDark)],
    },
    PresetDesign {
        id: "gruvbox",
        display_name: "Gruvbox",
        default_appearance: ThemeAppearance::Light,
        appearances: &[
            (ThemeAppearance::Light, ThemePreset::GruvboxLight),
            (ThemeAppearance::Dark, ThemePreset::GruvboxDark),
        ],
    },
    PresetDesign {
        id: "ayu",
        display_name: "Ayu",
        default_appearance: ThemeAppearance::Light,
        appearances: &[
            (ThemeAppearance::Light, ThemePreset::AyuLight),
            (ThemeAppearance::Dark, ThemePreset::AyuDark),
        ],
    },
    PresetDesign {
        id: "brutalist",
        display_name: "Brutalist",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::Brutalist)],
    },
    PresetDesign {
        id: "spotless",
        display_name: "Spotless",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::Spotless)],
    },
    PresetDesign {
        id: "cyberpunk",
        display_name: "Cyberpunk",
        default_appearance: ThemeAppearance::Dark,
        appearances: &[(ThemeAppearance::Dark, ThemePreset::Cyberpunk)],
    },
    PresetDesign {
        id: "ghibli",
        display_name: "Ghibli",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::Ghibli)],
    },
    PresetDesign {
        id: "paper-cut",
        display_name: "Paper Cut",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::PaperCut)],
    },
    PresetDesign {
        id: "carbon-copy",
        display_name: "Carbon Copy",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::CarbonCopy)],
    },
    PresetDesign {
        id: "pop-art",
        display_name: "Pop Art",
        default_appearance: ThemeAppearance::Light,
        appearances: &[(ThemeAppearance::Light, ThemePreset::PopArt)],
    },
];

impl PresetDesign {
    fn base_entry(&self) -> &'static catalog::PresetCatalogEntry {
        catalog::entry_for_preset(self.appearances[0].1)
    }

    fn document(&self) -> ThemeRecipeV1 {
        let (base_appearance, base_preset) = self.appearances[0];
        if self.appearances.len() == 1 {
            return concrete_document(base_preset);
        }
        let mut design = catalog::build_design(base_preset);
        let mut appearances = BTreeMap::new();
        appearances.insert(base_appearance, take_palette_appearance(&mut design));
        for &(appearance, preset) in &self.appearances[1..] {
            let mut concrete = catalog::build_design(preset);
            let overrides = take_palette_appearance(&mut concrete);
            assert_eq!(
                concrete, design,
                "paired reading presets must share their declared role template"
            );
            appearances.insert(appearance, overrides);
        }
        design.appearances = Some(appearances);
        design.default_appearance = Some(self.default_appearance);
        ThemeRecipeV1::Design { design }
    }

    fn describe(&self, compiler: &DiagramThemeCompiler) -> ThemePresetMetadataV1 {
        let availability_reason_ids = compiler
            .inspect_document(&self.document())
            .err()
            .map(|error| vec![theme_preset_unavailability_reason(&error).to_owned()])
            .unwrap_or_default();
        let available = availability_reason_ids.is_empty();
        let base = self.base_entry();
        ThemePresetMetadataV1 {
            id: self.id.to_owned(),
            display_name: self.display_name.to_owned(),
            supported_appearances: self
                .appearances
                .iter()
                .map(|&(appearance, _)| appearance)
                .collect(),
            default_appearance: self.default_appearance,
            appearances: self
                .appearances
                .iter()
                .map(|&(appearance, preset)| {
                    let entry = catalog::entry_for_preset(preset);
                    ThemePresetAppearanceMetadataV1 {
                        appearance,
                        concrete_preset_id: preset.id().to_owned(),
                        available,
                        availability_reason_ids: availability_reason_ids.clone(),
                        family_designs: entry
                            .family_designs()
                            .iter()
                            .map(|(family_id, treatment)| ThemePresetFamilyDesignV1 {
                                family_id: (*family_id).to_owned(),
                                treatment: (*treatment).to_owned(),
                            })
                            .collect(),
                        preview_colors: entry.preview_colors(),
                        application_notes: entry.application_notes(),
                        qualified_cells: entry
                            .qualified_cells()
                            .iter()
                            .map(ThemePresetQualifiedCellV1::from)
                            .collect(),
                    }
                })
                .collect(),
            maturity: base.maturity().to_owned(),
            available,
            availability_reason_ids,
            license_expression: base.license_expression().to_owned(),
            required_attribution: base.required_attribution().map(str::to_owned),
            export_kind: base.export_kind().to_owned(),
        }
    }
}

fn concrete_document(preset: ThemePreset) -> ThemeRecipeV1 {
    let mut design = catalog::build_design(preset);
    let appearance = if preset.is_dark() {
        ThemeAppearance::Dark
    } else {
        ThemeAppearance::Light
    };
    design.appearances.get_or_insert_with(|| {
        BTreeMap::from([(appearance, ThemeAppearanceOverridesV1::default())])
    });
    design.default_appearance.get_or_insert(appearance);
    ThemeRecipeV1::Design { design }
}

fn take_palette_appearance(design: &mut ThemeDesignV1) -> ThemeAppearanceOverridesV1 {
    let (tokens, shared_tokens) = std::mem::take(&mut design.tokens)
        .into_iter()
        .partition(|(_, token)| matches!(&token.value, ThemeDesignTokenValueV1::Color(_)));
    design.tokens = shared_tokens;
    ThemeAppearanceOverridesV1 {
        tokens,
        mermaid: design.mermaid.take(),
        ..Default::default()
    }
}

/// A compiler-independent preset candidate for help, completion, and exact-ID discovery.
///
/// The list includes canonical designs and concrete IDs. Use the preset catalog for
/// appearance relationships and effective-policy availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePresetSelectionDescriptor {
    id: &'static str,
    display_name: &'static str,
    maturity: &'static str,
}

impl ThemePresetSelectionDescriptor {
    /// Returns an accepted canonical or concrete selection ID.
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Returns the design name, or the appearance-specific name for a concrete choice.
    pub const fn display_name(self) -> &'static str {
        self.display_name
    }

    /// Returns the open catalog maturity classification.
    pub const fn maturity(self) -> &'static str {
        self.maturity
    }
}

/// Lists canonical designs and concrete choices without compiling any recipe.
pub fn theme_preset_selection_descriptors() -> Vec<ThemePresetSelectionDescriptor> {
    PRESET_DESIGNS
        .iter()
        .map(|design| ThemePresetSelectionDescriptor {
            id: design.id,
            display_name: design.display_name,
            maturity: design.base_entry().maturity(),
        })
        .chain(
            theme_preset_descriptors()
                .iter()
                .filter(|concrete| {
                    !PRESET_DESIGNS
                        .iter()
                        .any(|design| design.id == concrete.id())
                })
                .map(|concrete| ThemePresetSelectionDescriptor {
                    id: concrete.id(),
                    display_name: concrete.display_name(),
                    maturity: concrete.maturity(),
                }),
        )
        .collect()
}

/// Describes one card per canonical design under complete-document compiler admission.
pub fn describe_theme_presets(compiler: &DiagramThemeCompiler) -> Vec<ThemePresetMetadataV1> {
    PRESET_DESIGNS
        .iter()
        .map(|design| design.describe(compiler))
        .collect()
}

pub(super) fn describe_preset_design(
    compiler: &DiagramThemeCompiler,
    preset: ThemePreset,
) -> ThemePresetMetadataV1 {
    PRESET_DESIGNS
        .iter()
        .find(|design| {
            design
                .appearances
                .iter()
                .any(|&(_, concrete)| concrete == preset)
        })
        .expect("every concrete preset must belong to a canonical design")
        .describe(compiler)
}

pub(in crate::diagram_theme) fn build_preset_document(id: &str) -> Option<ThemeRecipeV1> {
    PRESET_DESIGNS
        .iter()
        .find(|design| design.id == id)
        .map(PresetDesign::document)
        .or_else(|| ThemePreset::from_id(id).ok().map(concrete_document))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::diagram_theme::{ThemeResourceLimitId, ThemeResourcePolicy};

    #[test]
    fn canonical_catalog_groups_only_declared_pairs_and_retains_single_appearances() {
        let compiler = DiagramThemeCompiler::new();
        let catalog = describe_theme_presets(&compiler);
        assert_eq!(
            catalog
                .iter()
                .map(|entry| (entry.id.as_str(), entry.supported_appearances.len()))
                .collect::<Vec<_>>(),
            [
                ("editor", 2),
                ("one-dark", 1),
                ("gruvbox", 2),
                ("ayu", 2),
                ("brutalist", 1),
                ("spotless", 1),
                ("cyberpunk", 1),
                ("ghibli", 1),
                ("paper-cut", 1),
                ("carbon-copy", 1),
                ("pop-art", 1),
            ]
        );
        for entry in catalog {
            assert!(entry.available, "{}", entry.id);
            assert!(entry.availability_reason_ids.is_empty());
            assert_eq!(entry.maturity, "alpha");
            assert_eq!(entry.export_kind, "design");
            assert!(
                entry
                    .supported_appearances
                    .contains(&entry.default_appearance)
            );
            assert_eq!(
                entry
                    .appearances
                    .iter()
                    .map(|appearance| appearance.appearance)
                    .collect::<Vec<_>>(),
                entry.supported_appearances
            );
            for appearance in entry.appearances {
                assert!(appearance.available);
                assert!(appearance.availability_reason_ids.is_empty());
                assert!(appearance.qualified_cells.is_empty());
            }
        }
    }

    #[test]
    fn completion_candidates_come_from_canonical_and_concrete_catalogs_without_duplicates() {
        let candidates = theme_preset_selection_descriptors();
        let ids = candidates
            .iter()
            .map(|candidate| candidate.id())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), candidates.len());
        for design in PRESET_DESIGNS {
            assert!(ids.contains(design.id));
            let descriptor = candidates
                .iter()
                .find(|candidate| candidate.id() == design.id)
                .unwrap();
            assert_eq!(descriptor.display_name(), design.display_name);
            assert_eq!(descriptor.maturity(), design.base_entry().maturity());
        }
        for concrete in theme_preset_descriptors() {
            assert!(ids.contains(concrete.id()));
        }
    }

    #[test]
    fn each_concrete_id_identifies_one_appearance_and_exports_a_classified_document() {
        let compiler = DiagramThemeCompiler::new();
        let catalog = describe_theme_presets(&compiler);
        let mut concrete_ids = BTreeSet::new();
        for group in &catalog {
            for metadata in &group.appearances {
                assert!(concrete_ids.insert(metadata.concrete_preset_id.as_str()));
                let preset = ThemePreset::from_id(&metadata.concrete_preset_id).unwrap();
                let original = compiler
                    .compile_recipe(ThemeRecipeV1::Design {
                        design: catalog::build_design(preset),
                    })
                    .unwrap();
                let document = compiler
                    .export_preset_document(&metadata.concrete_preset_id)
                    .unwrap();
                let json = serde_json::to_value(&document).unwrap();
                assert_eq!(json["schema_version"], 1);
                assert_eq!(json["design"]["appearances"].as_object().unwrap().len(), 1);
                assert_eq!(
                    json["design"]["default_appearance"],
                    metadata.appearance.as_str()
                );
                for appearance in [None, Some(metadata.appearance)] {
                    let selected = compiler
                        .compile_preset_selection(&metadata.concrete_preset_id, appearance)
                        .unwrap();
                    assert_eq!(selected.recipe_fingerprint(), original.recipe_fingerprint());
                    assert_eq!(selected.selected_appearance(), Some(metadata.appearance));
                }
                let descriptor = theme_preset_descriptors()
                    .iter()
                    .find(|descriptor| descriptor.preset() == preset)
                    .unwrap();
                assert_eq!(descriptor.describe(&compiler), *group);
            }
        }
        assert_eq!(
            concrete_ids,
            theme_preset_descriptors()
                .iter()
                .map(|descriptor| descriptor.id())
                .collect()
        );
    }

    #[test]
    fn complete_group_exports_round_trip_every_original_concrete_recipe() {
        let compiler = DiagramThemeCompiler::new();
        for design in PRESET_DESIGNS {
            let recipe = compiler.export_preset_document(design.id).unwrap();
            let json = serde_json::to_value(&recipe).unwrap();
            assert_eq!(json["schema_version"], 1);
            assert_eq!(
                json["design"]["default_appearance"],
                design.default_appearance.as_str()
            );
            assert_eq!(
                json["design"]["appearances"].as_object().unwrap().len(),
                design.appearances.len()
            );
            let imported: ThemeRecipeV1 = serde_json::from_value(json).unwrap();
            assert_eq!(imported, recipe);
            for &(appearance, concrete) in design.appearances {
                let selected = compiler
                    .compile_document(imported.clone(), Some(appearance))
                    .unwrap();
                let original = compiler
                    .compile_recipe(ThemeRecipeV1::Design {
                        design: catalog::build_design(concrete),
                    })
                    .unwrap();
                assert_eq!(
                    selected.recipe_fingerprint(),
                    original.recipe_fingerprint(),
                    "{} {appearance}",
                    design.id
                );
                assert_eq!(selected.selected_appearance(), Some(appearance));
                let selection = compiler
                    .compile_preset_selection(design.id, Some(appearance))
                    .unwrap();
                assert_eq!(
                    selection.recipe_fingerprint(),
                    selected.recipe_fingerprint()
                );
            }
            let default = compiler.compile_document(imported, None).unwrap();
            assert_eq!(
                default.selected_appearance(),
                Some(design.default_appearance)
            );
            assert_eq!(
                compiler
                    .compile_preset_selection(design.id, None)
                    .unwrap()
                    .recipe_fingerprint(),
                default.recipe_fingerprint()
            );
        }
    }

    #[test]
    fn paired_exports_share_the_role_template_and_keep_palette_values_local() {
        let compiler = DiagramThemeCompiler::new();
        for design in PRESET_DESIGNS
            .iter()
            .filter(|design| design.appearances.len() == 2)
        {
            let ThemeRecipeV1::Design { design: mut recipe } =
                compiler.export_preset_document(design.id).unwrap()
            else {
                unreachable!()
            };
            let shared_font = recipe.tokens.get_mut("body-font").unwrap();
            shared_font.value =
                ThemeDesignTokenValueV1::FontStack(super::super::design::literal(vec![
                    "monospace".to_owned(),
                ]));
            let light = recipe
                .appearances
                .as_mut()
                .unwrap()
                .get_mut(&ThemeAppearance::Light)
                .unwrap();
            light.tokens.get_mut("canvas").unwrap().value =
                ThemeDesignTokenValueV1::Color(super::super::design::literal("#fefefe".to_owned()));
            assert!(recipe.tokens.contains_key("series"));
            assert!(!recipe.tokens.contains_key("canvas"));
            assert!(recipe.mermaid.is_none());
            for &(appearance, concrete) in design.appearances {
                let overrides = &recipe.appearances.as_ref().unwrap()[&appearance];
                assert!(overrides.tokens.contains_key("canvas"));
                assert!(!overrides.tokens.contains_key("body-font"));
                assert!(!overrides.tokens.contains_key("series"));
                assert!(overrides.styles.is_empty());
                assert!(overrides.rules.is_empty());
                assert!(overrides.palettes.is_empty());
                let mut expected = catalog::build_design(concrete);
                expected.tokens.get_mut("body-font").unwrap().value =
                    recipe.tokens["body-font"].value.clone();
                if appearance == ThemeAppearance::Light {
                    expected.tokens.get_mut("canvas").unwrap().value =
                        overrides.tokens["canvas"].value.clone();
                }
                assert_eq!(
                    compiler
                        .compile_document(
                            ThemeRecipeV1::Design {
                                design: recipe.clone(),
                            },
                            Some(appearance),
                        )
                        .unwrap()
                        .recipe_fingerprint(),
                    compiler
                        .compile_recipe(merman_theme_contract::ThemeRecipeV1::Design {
                            design: expected,
                        })
                        .unwrap()
                        .recipe_fingerprint()
                );
            }
        }
    }

    #[test]
    fn appearance_discovery_keeps_each_concrete_designs_swatches_and_guidance() {
        let compiler = DiagramThemeCompiler::new();
        for (grouped, design) in describe_theme_presets(&compiler).iter().zip(PRESET_DESIGNS) {
            for (metadata, &(appearance, concrete)) in
                grouped.appearances.iter().zip(design.appearances)
            {
                let expected = catalog::entry_for_preset(concrete);
                assert_eq!(metadata.appearance, appearance);
                assert_eq!(metadata.concrete_preset_id, concrete.id());
                assert_eq!(metadata.preview_colors, expected.preview_colors());
                assert_eq!(metadata.application_notes, expected.application_notes());
                assert_eq!(
                    metadata.family_designs,
                    expected
                        .family_designs()
                        .iter()
                        .map(|&(family_id, treatment)| ThemePresetFamilyDesignV1 {
                            family_id: family_id.to_owned(),
                            treatment: treatment.to_owned(),
                        })
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    metadata.qualified_cells,
                    expected
                        .qualified_cells()
                        .iter()
                        .map(ThemePresetQualifiedCellV1::from)
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn rejected_pair_is_unavailable_for_every_declared_appearance_and_export() {
        let limited = DiagramThemeCompiler::new().with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
                .unwrap(),
        );
        for appearance in [
            None,
            Some(ThemeAppearance::Light),
            Some(ThemeAppearance::Dark),
        ] {
            assert!(
                limited
                    .compile_preset_selection("editor", appearance)
                    .is_err()
            );
        }
        assert!(limited.export_preset_document("editor").is_err());
        let grouped = describe_theme_presets(&limited)
            .into_iter()
            .find(|entry| entry.id == "editor")
            .unwrap();
        assert!(!grouped.available);
        assert_eq!(
            grouped.availability_reason_ids,
            ["theme-preset.resource-policy-rejected"]
        );
        for appearance in grouped.appearances {
            assert!(!appearance.available);
            assert_eq!(
                appearance.availability_reason_ids,
                grouped.availability_reason_ids
            );
        }
    }

    #[test]
    fn whole_document_policy_rejection_keeps_declared_appearance_facts() {
        let unrestricted = describe_theme_presets(&DiagramThemeCompiler::new());
        let limited = DiagramThemeCompiler::new().with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
                .unwrap(),
        );
        for (expected, rejected) in unrestricted
            .into_iter()
            .zip(describe_theme_presets(&limited))
        {
            let mut expected = expected;
            expected.available = false;
            expected.availability_reason_ids =
                vec!["theme-preset.resource-policy-rejected".to_owned()];
            for appearance in &mut expected.appearances {
                appearance.available = false;
                appearance.availability_reason_ids = expected.availability_reason_ids.clone();
            }
            assert_eq!(rejected, expected);
        }
    }
}
