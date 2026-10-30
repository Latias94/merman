use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use crate::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};
use crate::{MermaidConfig, OperationControl};

use super::*;

fn recipe(values: Value) -> ThemeCompatibilityRecipe {
    ThemeCompatibilityRecipe::try_new([0x5a; 32], MermaidConfig::from_value(values)).unwrap()
}

fn field(id: &str, kind: ThemeCompatibilityFieldKind, paths: &[&str]) -> ThemeCompatibilityField {
    ThemeCompatibilityField::try_new_controlled(
        Arc::from(id),
        kind,
        paths.iter().map(|path| Arc::from(*path)).collect(),
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

fn frozen(
    parts: FrozenPresentationConfigParts,
) -> Result<PresentationConfig, PresentationConfigError> {
    PresentationConfig::try_from_frozen_parts_controlled(
        parts,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
}

fn authored(values: Value) -> PresentationConfig {
    PresentationConfig::from_config_controlled(
        MermaidConfig::from_value(values),
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn ordinary_authored_config_owns_only_declared_values_without_source_inferences() {
    let empty = authored(json!({}));
    assert!(!empty.explicit_config_owns_path(""));
    assert!(!empty.explicit_config_owns_path("missing"));
    assert_eq!(empty, PresentationConfig::empty_object());

    let config = authored(json!({
        "theme": "dark",
        "flowchart": {"nodeSpacing": 50},
        "themeVariables": {"primaryColor": "#123456"},
        "empty": {},
        "array": [],
        "null": null,
    }));
    assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
    assert!(config.explicit_config_owns_path("flowchart"));
    assert!(!config.explicit_config_owns_path("flowchart.rankSpacing"));
    assert!(config.explicit_config_owns_path("empty"));
    assert!(config.explicit_config_owns_path("array"));
    assert!(config.explicit_config_owns_path("null"));
    assert!(config.config_paint_overrides_typed_default("themeVariables.primaryColor"));
    assert!(!config.config_paint_overrides_typed_default("themeVariables.mainBkg"));
    assert!(config.theme_compatibility().recipe().is_none());
    assert_eq!(config.theme_compatibility().mermaid_residual_count(), 0);
    assert_eq!(config.family_paint_inputs(), None);
    assert_eq!(
        config.palette_paint_authority(),
        PalettePaintAuthority::PropertyInputs
    );
}

#[test]
fn immutable_config_clones_do_not_publish_mutations_from_a_raw_config_clone() {
    let original = authored(json!({"themeVariables": {"primaryColor": "#123456"}}));
    let frozen_clone = original.clone();
    let mut raw_clone = original.as_config().clone();
    raw_clone.set_value("themeVariables.primaryColor", json!("#abcdef"));
    assert_eq!(original, frozen_clone);
    assert_eq!(
        original.get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        raw_clone.get_str("themeVariables.primaryColor"),
        Some("#abcdef")
    );
}

#[test]
fn equal_config_values_keep_distinct_ownership_and_recipe_declarations() {
    let values = json!({"themeVariables": {"primaryColor": "#123456"}});
    let caller = authored(values.clone());
    let framework = frozen(FrozenPresentationConfigParts::new(
        MermaidConfig::from_value(values.clone()),
    ))
    .unwrap();
    let residual = field(
        "mermaid.themeVariables.primaryColor",
        ThemeCompatibilityFieldKind::Variable,
        &["themeVariables.primaryColor"],
    );
    let compatible = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(values.clone()))
            .with_theme_compatibility(Some(recipe(values)), Arc::from([residual])),
    )
    .unwrap();
    assert_eq!(caller.as_value(), framework.as_value());
    assert_eq!(framework.as_value(), compatible.as_value());
    assert_ne!(caller, framework);
    assert_ne!(framework, compatible);
    assert_ne!(caller, compatible);
    assert!(caller.explicit_config_owns_path("themeVariables.primaryColor"));
    assert!(!framework.config_path_overrides_typed_default("themeVariables.primaryColor"));
    assert!(compatible.config_path_overrides_typed_default("themeVariables.primaryColor"));
    assert!(!compatible.explicit_config_owns_path("themeVariables.primaryColor"));
}

#[test]
fn frozen_fields_accept_surviving_derivations_after_the_input_is_shadowed() {
    let residual = field(
        "mermaid.themeVariables.primaryColor",
        ThemeCompatibilityFieldKind::Variable,
        &["themeVariables.derivedColor"],
    );
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": "#999999", "derivedColor": "#567890"}
        })))
        .with_theme_compatibility(
            Some(recipe(
                json!({"themeVariables": {"primaryColor": "#123456"}}),
            )),
            Arc::from([residual]),
        ),
    )
    .unwrap();
    assert!(config.config_path_overrides_typed_default("themeVariables.derivedColor"));
    assert!(!config.config_path_overrides_typed_default("themeVariables.primaryColor"));
    assert!(
        !config
            .theme_compatibility()
            .mermaid_residual_path_survives("themeVariables.primaryColor")
    );
}

#[test]
fn separate_conceptual_fields_can_share_a_derived_assignment() {
    let fields = [
        field(
            "mermaid.themeVariables.primaryColor",
            ThemeCompatibilityFieldKind::Variable,
            &["themeVariables.sharedColor"],
        ),
        field(
            "mermaid.themeVariables.secondaryColor",
            ThemeCompatibilityFieldKind::Variable,
            &["themeVariables.sharedColor"],
        ),
    ];
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "themeVariables": {"sharedColor": "#abcdef"}
        })))
        .with_theme_compatibility(
            Some(recipe(json!({
                "themeVariables": {"primaryColor": "#123456", "secondaryColor": "#654321"}
            }))),
            Arc::from(fields),
        ),
    )
    .unwrap();
    assert_eq!(config.theme_compatibility().mermaid_residual_count(), 2);
}

#[test]
fn frozen_fields_preserve_nested_theme_object_dependencies() {
    let residual = field(
        "mermaid.themeVariables.background",
        ThemeCompatibilityFieldKind::Variable,
        &["themeVariables.xyChart.backgroundColor"],
    );
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "themeVariables": {"xyChart": {"backgroundColor": "#123456", "titleColor": "#abcdef"}}
        })))
        .with_theme_compatibility(
            Some(recipe(json!({"themeVariables": {"background": "#123456"}}))),
            Arc::from([residual]),
        ),
    )
    .unwrap();
    assert!(config.config_path_overrides_typed_default("themeVariables.xyChart.backgroundColor"));
    assert!(!config.config_path_overrides_typed_default("themeVariables.xyChart.titleColor"));
    assert!(!config.config_path_overrides_typed_default("themeVariables.background"));
}

#[test]
fn frozen_constructor_checks_recipe_associations_and_duplicate_declarations() {
    let residual = field(
        "mermaid.themeVariables.primaryColor",
        ThemeCompatibilityFieldKind::Variable,
        &["themeVariables.mainBkg"],
    );
    let values = MermaidConfig::from_value(json!({"themeVariables": {"mainBkg": "#abcdef"}}));
    let parts = FrozenPresentationConfigParts::new(values.clone())
        .with_theme_compatibility(None, Arc::from([residual.clone()]));
    assert_eq!(
        frozen(parts).unwrap_err(),
        PresentationConfigError::MissingThemeRecipe
    );

    let parts = FrozenPresentationConfigParts::new(values.clone()).with_theme_compatibility(
        Some(recipe(json!({"themeVariables": {"other": "#123456"}}))),
        Arc::from([residual.clone()]),
    );
    assert!(matches!(
        frozen(parts),
        Err(PresentationConfigError::FieldAbsentFromRecipe { .. })
    ));

    let bound_recipe = recipe(json!({"themeVariables": {"primaryColor": "#123456"}}));
    let parts = FrozenPresentationConfigParts::new(values).with_theme_compatibility(
        Some(bound_recipe.clone()),
        Arc::from([residual.clone(), residual.clone()]),
    );
    assert!(matches!(
        frozen(parts),
        Err(PresentationConfigError::DuplicateThemeField { .. })
    ));

    let parts = FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
        .with_theme_compatibility(Some(bound_recipe), Arc::from([residual]));
    assert!(matches!(
        frozen(parts),
        Err(PresentationConfigError::MissingAssignmentValue { .. })
    ));
}

#[test]
fn field_constructor_rejects_wrong_identity_empty_or_nonphysical_paths() {
    for (id, kind, paths) in [
        (
            "mermaid.darkMode",
            ThemeCompatibilityFieldKind::Theme,
            vec!["theme"],
        ),
        (
            "mermaid.themeVariables.darkMode",
            ThemeCompatibilityFieldKind::Variable,
            vec!["themeVariables.darkMode"],
        ),
        ("mermaid.theme", ThemeCompatibilityFieldKind::Theme, vec![]),
        (
            "mermaid.theme",
            ThemeCompatibilityFieldKind::Theme,
            vec!["theme", "theme"],
        ),
        (
            "mermaid.theme",
            ThemeCompatibilityFieldKind::Theme,
            vec![""],
        ),
        (
            "mermaid.theme",
            ThemeCompatibilityFieldKind::Theme,
            vec!["themeVariables"],
        ),
        (
            "mermaid.theme",
            ThemeCompatibilityFieldKind::Theme,
            vec!["themeVariables..color"],
        ),
        (
            "mermaid.theme",
            ThemeCompatibilityFieldKind::Theme,
            vec!["themeVariables. color "],
        ),
    ] {
        let result = ThemeCompatibilityField::try_new_controlled(
            Arc::from(id),
            kind,
            paths.iter().map(|path| Arc::from(*path)).collect(),
            &InputResourcePolicy::default(),
            &OperationControl::new(),
        )
        .unwrap();
        assert!(result.is_err(), "id={id}, kind={kind:?}, paths={paths:?}");
    }
}

#[test]
fn dark_mode_remains_one_concept_with_one_or_both_mirrored_paths() {
    for paths in [
        vec!["darkMode"],
        vec!["themeVariables.darkMode"],
        vec!["darkMode", "themeVariables.darkMode"],
    ] {
        let config = frozen(
            FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
                "darkMode": true, "themeVariables": {"darkMode": true}
            })))
            .with_theme_compatibility(
                Some(recipe(json!({"darkMode": true}))),
                Arc::from([field(
                    "mermaid.darkMode",
                    ThemeCompatibilityFieldKind::DarkMode,
                    &paths,
                )]),
            ),
        )
        .unwrap();
        let evidence = config.theme_compatibility();
        assert!(evidence.mermaid_dark_mode_field_survives());
        assert_eq!(evidence.mermaid_residual_count(), 1);
        assert_eq!(
            evidence
                .mermaid_fields()
                .next()
                .unwrap()
                .surviving_assignment_paths()
                .collect::<Vec<_>>(),
            paths
        );
    }
}

#[test]
fn reconciliation_requires_all_paths_and_conflicting_events_fail_closed() {
    use ThemeCompatibilityConsumptionDisposition::{NotReadByFamily, ReplacedByTypedSurface};

    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "darkMode": true, "themeVariables": {"darkMode": true}
        })))
        .with_theme_compatibility(
            Some(recipe(json!({"darkMode": true}))),
            Arc::from([field(
                "mermaid.darkMode",
                ThemeCompatibilityFieldKind::DarkMode,
                &["darkMode", "themeVariables.darkMode"],
            )]),
        ),
    )
    .unwrap();
    let evidence = config.theme_compatibility();
    let dark_mode = evidence.mermaid_fields().next().unwrap();
    assert!(
        dark_mode
            .consume_assignment_path("themeVariables", NotReadByFamily)
            .is_none()
    );
    let root = dark_mode
        .consume_assignment_path("darkMode", NotReadByFamily)
        .unwrap();
    let mirrored = dark_mode
        .consume_assignment_path("themeVariables.darkMode", ReplacedByTypedSurface)
        .unwrap();
    let conflict = dark_mode
        .consume_assignment_path("darkMode", ReplacedByTypedSurface)
        .unwrap();
    let partial = evidence.reconcile_mermaid_consumptions([&root, &root]);
    assert_eq!(partial.remaining_field_count(), 1);
    assert_eq!(partial.consumed_field_count(), 0);
    let covered = evidence.reconcile_mermaid_consumptions([&root, &mirrored, &root]);
    assert_eq!(covered.remaining_field_count(), 0);
    assert_eq!(covered.consumed_field_count(), 1);
    for events in [
        vec![&root, &mirrored, &conflict, &root],
        vec![&conflict, &root, &mirrored, &root],
    ] {
        let conflict = evidence.reconcile_mermaid_consumptions(events);
        assert_eq!(conflict.remaining_field_count(), 1);
        assert_eq!(conflict.consumed_field_count(), 0);
    }
    let all = dark_mode.consume_all(NotReadByFamily).collect::<Vec<_>>();
    assert_eq!(
        evidence
            .reconcile_mermaid_consumptions(&all)
            .consumed_field_count(),
        1
    );
}

#[test]
fn frozen_roundtrip_preserves_exact_paths_shared_storage_and_empty_mask() {
    let paths: Arc<BTreeSet<Arc<str>>> = Arc::new(
        ["", "a.", "a..b", "a/b", "é.葉"]
            .into_iter()
            .map(Arc::from)
            .collect(),
    );
    let fields = Arc::from([field(
        "mermaid.darkMode",
        ThemeCompatibilityFieldKind::DarkMode,
        &["themeVariables.darkMode", "darkMode"],
    )]);
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "darkMode": true, "themeVariables": {"darkMode": true}
        })))
        .with_explicit_config_paths(Arc::clone(&paths))
        .with_theme_compatibility(Some(recipe(json!({"darkMode": true}))), fields)
        .with_palette_paint_authority(PalettePaintAuthority::SelectedPalette)
        .with_family_paint_inputs(Some(FamilyPaintInputs::GitGraph(
            GitGraphPaintInputs::default(),
        ))),
    )
    .unwrap();
    let roundtrip = frozen(config.frozen_parts()).unwrap();
    assert_eq!(config, roundtrip);
    assert!(Arc::ptr_eq(
        &paths,
        roundtrip.shared_explicit_config_paths()
    ));
    assert!(Arc::ptr_eq(
        config.theme_compatibility().shared_fields(),
        roundtrip.theme_compatibility().shared_fields()
    ));
    assert_eq!(
        roundtrip
            .gitgraph_paint_inputs()
            .map(GitGraphPaintInputs::owned_bits),
        Some(0)
    );
    assert_eq!(roundtrip.er_paint_inputs(), None);
    let absent = frozen(config.frozen_parts().with_family_paint_inputs(None)).unwrap();
    assert_ne!(config, absent);
    assert_eq!(absent.gitgraph_paint_inputs(), None);
}

#[test]
fn paint_masks_reject_unknown_bits_and_keep_the_pinned_input_order() {
    let gitgraph = GitGraphPaintInputs::default()
        .with_owned(GitGraphPaintInput::PrimaryColor)
        .with_owned(GitGraphPaintInput::TagBorder);
    assert_eq!(gitgraph.owned_bits(), 0b10_0001);
    assert_eq!(gitgraph.available_sources(), 0b01_1110);
    assert!(gitgraph.is_owned(GitGraphPaintInput::TagBorder));
    assert!(!gitgraph.is_owned(GitGraphPaintInput::MainBackground));
    assert!(GitGraphPaintInputs::try_from_owned_bits(0b100_0000).is_err());
    let er = ErPaintInputs::default()
        .with_owned(ErPaintInput::Text)
        .with_owned(ErPaintInput::EvenRow);
    assert_eq!(er.owned_bits(), 0b1_0001);
    assert!(er.is_owned(ErPaintInput::EvenRow));
    assert!(ErPaintInputs::try_from_owned_bits(0b10_0000).is_err());
    assert_eq!(
        FamilyPaintInputs::GitGraph(gitgraph).family_id(),
        crate::family::DiagramFamilyId::GIT_GRAPH
    );
    assert_eq!(FamilyPaintInputs::Er(er).render_model_kind(), "er");
    let parts = FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
        .with_family_paint_inputs(Some(FamilyPaintInputs::Er(ErPaintInputs::default())));
    assert_eq!(
        frozen(parts).unwrap_err(),
        PresentationConfigError::MissingFamilyPaintRecipe
    );
}

#[test]
fn palette_authority_only_claims_present_paints() {
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(json!({
            "themeVariables": {"primaryColor": "#123456"}, "fontSize": 16
        })))
        .with_palette_paint_authority(PalettePaintAuthority::SelectedPalette),
    )
    .unwrap();
    assert!(config.config_paint_overrides_typed_default("themeVariables.primaryColor"));
    assert!(!config.config_path_overrides_typed_default("themeVariables.primaryColor"));
    assert!(!config.config_paint_overrides_typed_default("themeVariables.mainBkg"));
    assert!(!config.config_paint_overrides_typed_default("fontSize"));
}

#[test]
fn exact_dotted_path_ownership_keeps_empty_segments_and_unicode_boundaries() {
    let paths = [
        "", "a", "a.", "a.b", "a..b", "a-b", "a/b", "a.b.c", "ab", "é.葉", "é",
    ];
    for owner in paths {
        let config = frozen(
            FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
                .with_explicit_config_paths(Arc::new(BTreeSet::from([Arc::from(owner)]))),
        )
        .unwrap();
        for query in paths {
            let overlaps = owner.is_empty()
                || query.is_empty()
                || owner == query
                || owner
                    .strip_prefix(query)
                    .is_some_and(|suffix| suffix.starts_with('.'))
                || query
                    .strip_prefix(owner)
                    .is_some_and(|suffix| suffix.starts_with('.'));
            assert_eq!(
                config.explicit_config_owns_path(query),
                overlaps,
                "owner={owner:?}, query={query:?}"
            );
        }
    }
    let config = authored(json!({"a": {"": {"b": true}}, "literal.dot": true, "é": {"葉": 1}}));
    assert_eq!(
        config.explicit_config_paths().collect::<Vec<_>>(),
        ["a..b", "literal.dot", "é.葉"]
    );
    assert!(!config.explicit_config_owns_path("a.b"));
    assert!(!config.explicit_config_owns_path("literal-dotted"));
}

#[test]
fn bounded_construction_rejects_wide_values_before_traversing_their_children() {
    let policy = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxModelItems, 4)
        .unwrap();
    let values = MermaidConfig::from_value(Value::Array(vec![Value::Null; 1_024]));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    let result = PresentationConfig::try_from_frozen_parts_controlled(
        FrozenPresentationConfigParts::new(values),
        &policy,
        &control,
    )
    .unwrap();
    assert!(
        matches!(result, Err(PresentationConfigError::ResourceLimit(error))
        if error.limit == "max_model_items" && error.actual == 1_025)
    );
}

#[test]
fn bounded_construction_checks_cancellation_and_all_retained_declarations() {
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert!(
        PresentationConfig::from_config_controlled(
            MermaidConfig::empty_object(),
            &InputResourcePolicy::default(),
            &cancelled,
        )
        .is_err()
    );
    assert!(
        PresentationConfig::try_from_frozen_parts_controlled(
            FrozenPresentationConfigParts::new(MermaidConfig::empty_object()),
            &InputResourcePolicy::default(),
            &cancelled,
        )
        .is_err()
    );
    let policy = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxModelTextBytes, 8)
        .unwrap();
    let parts = FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
        .with_explicit_config_paths(Arc::new(BTreeSet::from([Arc::from("long.declared.path")])));
    let result = PresentationConfig::try_from_frozen_parts_controlled(
        parts,
        &policy,
        &OperationControl::new(),
    )
    .unwrap();
    assert!(
        matches!(result, Err(PresentationConfigError::ResourceLimit(error)) if error.limit == "max_model_text_bytes")
    );
    let parts = FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
        .with_theme_compatibility(
            Some(recipe(json!({"theme": "long-theme-name"}))),
            Arc::from([]),
        );
    let result = PresentationConfig::try_from_frozen_parts_controlled(
        parts,
        &policy,
        &OperationControl::new(),
    )
    .unwrap();
    assert!(
        matches!(result, Err(PresentationConfigError::ResourceLimit(error)) if error.limit == "max_model_text_bytes")
    );
}

#[test]
fn deep_frozen_values_keep_iterative_clone_equality_and_drop() {
    let mut value = json!("leaf");
    for _ in 0..2_048 {
        value = Value::Object(Map::from_iter([("node".to_string(), value)]));
    }
    let config = PresentationConfig::try_from_frozen_parts_controlled(
        FrozenPresentationConfigParts::new(MermaidConfig::from_value(value)),
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let cloned = config.clone();
    assert_eq!(config, cloned);
    let json_clone = config
        .clone_value_bounded_controlled(usize::MAX, usize::MAX, &OperationControl::new())
        .unwrap()
        .unwrap();
    drop(config);
    drop(cloned);
    crate::json::drop_value_nonrecursive(json_clone);
}
