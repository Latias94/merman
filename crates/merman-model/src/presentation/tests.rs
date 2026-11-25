use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use crate::resources::{InputResourceLimitId, InputResourcePolicy, ResourceProfile};
use crate::{MermaidConfig, OperationControl};

use super::*;

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
    assert!(config.explicit_config_owns_path("themeVariables.primaryColor"));
    assert!(!config.explicit_config_owns_path("themeVariables.mainBkg"));
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
fn equal_config_values_keep_distinct_authored_paths() {
    let values = json!({"themeVariables": {"primaryColor": "#123456"}});
    let caller = authored(values.clone());
    let framework = frozen(FrozenPresentationConfigParts::new(
        MermaidConfig::from_value(values),
    ))
    .unwrap();
    assert_eq!(caller.as_value(), framework.as_value());
    assert_ne!(caller, framework);
    assert!(caller.explicit_config_owns_path("themeVariables.primaryColor"));
    assert!(!framework.explicit_config_owns_path("themeVariables.primaryColor"));
}

#[test]
fn frozen_roundtrip_preserves_exact_paths_and_shared_storage() {
    let paths = Arc::new(
        ["", "a.", "a..b", "a/b", "é.葉"]
            .into_iter()
            .map(Arc::from)
            .collect(),
    );
    let config = frozen(
        FrozenPresentationConfigParts::new(MermaidConfig::empty_object())
            .with_explicit_config_paths(Arc::clone(&paths)),
    )
    .unwrap();
    let roundtrip = frozen(config.frozen_parts()).unwrap();
    assert_eq!(config, roundtrip);
    assert!(Arc::ptr_eq(
        &paths,
        roundtrip.shared_explicit_config_paths()
    ));
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
fn shared_construction_count_preserves_first_failure_with_a_tighter_budget() {
    let json = json!({"early": "太长", "middle": [{"nested": [1]}], "later": [0, 1, 2, 3]});
    let config = MermaidConfig::from_value(json.clone());
    let paths = Arc::new(BTreeSet::from([Arc::from("middle.nested")]));
    let trusted = InputResourcePolicy::default();
    let freeze = |values, policy: &InputResourcePolicy| {
        PresentationConfig::try_from_frozen_parts_controlled(
            FrozenPresentationConfigParts::new(values)
                .with_explicit_config_paths(Arc::clone(&paths)),
            policy,
            &OperationControl::new(),
        )
        .unwrap()
    };
    assert!(freeze(config.clone(), &trusted).is_ok());
    assert!(config.construction_complexity().get().is_some());

    for (limit, boundaries) in [
        (InputResourceLimitId::MaxModelItems, [3, 11]),
        (InputResourceLimitId::MaxModelTextBytes, [6, 28]),
        (InputResourceLimitId::MaxModelNestingDepth, [1, 2]),
    ] {
        for value in boundaries {
            let policy = trusted.with_limit(limit, value).unwrap();
            let cold = freeze(MermaidConfig::from_value(json.clone()), &policy);
            let warm = freeze(config.clone(), &policy);
            assert_eq!(warm.unwrap_err(), cold.unwrap_err());
        }
    }
}

#[test]
fn layered_construction_preserves_first_failure_and_exact_limits() {
    let base = MermaidConfig::from_value(json!({
        "early": "中文",
        "flowchart": {"padding": 8, "label": "ok"},
        "later": [0, 1, 2],
    }));
    let changes = json!({"flowchart": {"label": "a longer label"}, "custom": {"x": [[42]]}});
    let mut layered = base.clone().with_shared_sections();
    let mut flat = base.clone();
    layered.deep_merge(&changes);
    flat.deep_merge(&changes);

    let freeze = |values, policy: &InputResourcePolicy| {
        PresentationConfig::try_from_frozen_parts_controlled(
            FrozenPresentationConfigParts::new(values),
            policy,
            &OperationControl::new(),
        )
        .unwrap()
    };
    for limit in [
        InputResourceLimitId::MaxModelItems,
        InputResourceLimitId::MaxModelTextBytes,
        InputResourceLimitId::MaxModelNestingDepth,
    ] {
        for ceiling in 1..=80 {
            let policy = InputResourcePolicy::default()
                .with_limit(limit, ceiling)
                .unwrap();
            let layered = freeze(layered.clone(), &policy);
            let flat = freeze(flat.clone(), &policy);
            assert_eq!(layered.err(), flat.err(), "{limit:?} at {ceiling}");
        }
    }
}

#[test]
fn cached_construction_count_is_invalidated_by_value_mutation() {
    let mut original = MermaidConfig::from_value(json!({"a": "b"}));
    let policy = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxModelTextBytes, 3)
        .unwrap();
    let freeze = |values, control: &OperationControl| {
        PresentationConfig::try_from_frozen_parts_controlled(
            FrozenPresentationConfigParts::new(values),
            &policy,
            control,
        )
    };
    assert!(
        freeze(original.clone(), &OperationControl::new())
            .unwrap()
            .is_ok()
    );
    let control = OperationControl::new();
    control.cancel();
    assert!(freeze(original.clone(), &control).is_err());

    let mut updated = original.clone();
    updated.set_value("a", json!("long"));
    assert!(updated.construction_complexity().get().is_none());
    assert!(original.construction_complexity().get().is_some());
    assert!(freeze(updated, &OperationControl::new()).unwrap().is_err());

    original.as_value_mut()["a"] = json!("long");
    assert!(original.construction_complexity().get().is_none());
    assert!(freeze(original, &OperationControl::new()).unwrap().is_err());
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
