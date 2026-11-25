use super::*;
use serde_json::json;

fn value_at_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(root, |value, key| value.as_object()?.get(key))
}

#[test]
fn executable_theme_programs_cover_typed_catalog() {
    assert!(
        THEME_PROGRAMS
            .iter()
            .map(|program| program.id)
            .eq(MermaidThemeId::ALL.iter().copied()),
        "the executable theme programs must cover the typed catalog exactly"
    );
}

#[test]
fn owned_staged_theme_materialization_needs_no_json_cow_copy() {
    let raw = SourcePresentationConfig::from_value(json!({
        "theme": "base", "themeVariables": { "primaryColor": "#123456" },
    }));
    let (materialized, work) = crate::config::measure_config_work(|| {
        materialize_selected_theme(raw, MermaidThemeId::Base).unwrap()
    });
    assert_eq!(
        materialized.get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(work.json_cow_copies, 0, "{work:?}");
    assert_eq!(work.selected_theme_materializations, 1, "{work:?}");
}

#[test]
fn owned_legacy_theme_materialization_needs_no_json_cow_copy() {
    for theme in MermaidThemeId::ALL
        .iter()
        .copied()
        .filter(|theme| ThemeProgram::resolve(*theme).staged_program().is_none())
    {
        let unrelated = "retained host data ".repeat(8_192);
        let raw = SourcePresentationConfig::from_value(json!({
            "theme": theme.as_str(),
            "themeVariables": { "primaryColor": "#123456" },
            "unrelatedHostData": unrelated,
        }));
        let (materialized, work) =
            crate::config::measure_config_work(|| materialize_selected_theme(raw, theme).unwrap());
        assert_eq!(
            materialized.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert_eq!(
            materialized.get_str("unrelatedHostData"),
            Some(unrelated.as_str())
        );
        assert_eq!(work.json_cow_copies, 0, "{theme}: {work:?}");

        assert_eq!(work.selected_theme_materializations, 1, "{theme}: {work:?}");
    }
}

#[test]
fn shared_legacy_theme_materialization_detaches_only_at_successful_commit() {
    for theme in MermaidThemeId::ALL
        .iter()
        .copied()
        .filter(|theme| ThemeProgram::resolve(*theme).staged_program().is_none())
    {
        let original = SourcePresentationConfig::from_value(json!({
            "theme": theme.as_str(),
            "themeVariables": { "primaryColor": "#123456" },
            "unrelatedHostData": "retained host data ".repeat(8_192),
        }));
        let mut working = original.clone();
        let (_, work) =
            crate::config::measure_config_work(|| apply_theme_defaults(&mut working).unwrap());
        assert_eq!(work.json_cow_copies, 1, "{theme}: {work:?}");
        assert_eq!(
            original.as_value()["themeVariables"],
            json!({"primaryColor": "#123456"})
        );
        assert_eq!(
            working.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert_eq!(
            working.as_value()["unrelatedHostData"],
            original.as_value()["unrelatedHostData"]
        );
        assert!(
            working.as_value()["themeVariables"]
                .as_object()
                .unwrap()
                .len()
                > 1
        );
    }
}

#[test]
fn source_theme_materialization_merges_array_sources_without_object_coercion() {
    let materialized = crate::MermaidEngine::new()
        .parse_metadata_sync(
            "%%{init: {\"theme\": \"base\", \"themeVariables\": [{\"primaryColor\": \"#123456\"}, {\"secondaryColor\": \"#654321\"}]}}%%\nflowchart TD\nA-->B",
        )
        .expect("materialize source theme through the public parse pipeline")
        .effective_config;

    assert_eq!(
        materialized.get_str("themeVariables.primaryColor"),
        Some("#123456")
    );
    assert_eq!(
        materialized.get_str("themeVariables.secondaryColor"),
        Some("#654321")
    );
}

#[test]
fn all_themes_preserve_generated_object_keys_after_partial_overrides() {
    let mut covered = std::collections::BTreeSet::new();
    for theme in crate::supported_themes() {
        let mut baseline = SourcePresentationConfig::from_value(json!({"theme": theme}));
        apply_theme_defaults(&mut baseline).unwrap();
        let generated = theme_variables_map(baseline.as_config());
        for (variable, value) in &generated {
            let Some(nested) = value.as_object().filter(|value| value.len() > 1) else {
                continue;
            };
            let first = nested.keys().next().unwrap();
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { (variable): { (first): "#123456" } }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let resolved = theme_variables_map(config.as_config());
            let observed = resolved.get(variable).and_then(Value::as_object).unwrap();
            assert_eq!(
                observed.get(first),
                Some(&json!("#123456")),
                "{theme}/{variable}"
            );
            for (key, expected) in nested {
                if key != first {
                    assert_eq!(
                        observed.get(key),
                        Some(expected),
                        "{theme}/{variable}/{key}"
                    );
                }
            }
            covered.insert(variable.clone());
        }
    }
    for variable in ["xyChart", "radar", "cynefin"] {
        assert!(
            covered.contains(variable),
            "missing object-valued theme variable {variable}"
        );
    }
}

#[test]
fn theme_replay_object_merge_is_shallow_and_retains_scalar_replacement() {
    let mut snapshot = ThemeStageSnapshot::from_variables(
        json!({
            "chart": { "nested": { "keep": 1, "replace": 2 }, "defaultColor": "blue" },
            "array": [1, 2], "scalar": "old", "nullable": "old"
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    snapshot.overlay(
        json!({
            "chart": { "nested": { "replace": 3 } },
            "array": [3], "scalar": "new", "nullable": null
        })
        .as_object()
        .unwrap(),
        true,
    );
    assert_eq!(
        snapshot.variables["chart"],
        json!({
            "nested": { "replace": 3 }, "defaultColor": "blue"
        })
    );
    assert_eq!(snapshot.variables["array"], json!([3]));
    assert_eq!(snapshot.variables["scalar"], json!("new"));
    assert_eq!(snapshot.variables["nullable"], Value::Null);
}

#[test]
fn generated_theme_overrides_preserve_null_replacement_and_removal() {
    let base = serde_json::json!({"unchanged": false, "deleted": 1, "nullable": "old", "object": {"old": 1}});
    let restored = restore_theme_overrides(
        base.as_object().unwrap(),
        serde_json::json!({
            "set": {"nullable": null, "object": {"new": 2}, "added": 0},
            "remove": ["deleted"]
        }),
    );
    assert_eq!(
        Value::Object(restored),
        serde_json::json!({
            "unchanged": false, "nullable": null, "object": {"new": 2}, "added": 0
        })
    );
}

#[test]
fn supported_theme_defaults_match_upstream_snapshot() {
    for &theme in MermaidThemeId::NAMES {
        let mut cfg = SourcePresentationConfig::from_value(json!({
            "theme": theme
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let actual = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();
        let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
            .resolved_without_overrides();

        assert_eq!(actual, expected, "theme {theme}");
    }
}

#[test]
fn node_text_dependencies_follow_the_selected_theme_program() {
    for &theme in MermaidThemeId::NAMES {
        let derives_node_text = matches!(
            theme,
            "base"
                | "neo"
                | "neo-dark"
                | "redux"
                | "redux-dark"
                | "redux-color"
                | "redux-dark-color"
        );
        for node_text in [None, Some(""), Some("#556677")] {
            let mut variables = json!({"primaryTextColor": "#123456", "textColor": "#fedcba"});
            if let Some(color) = node_text {
                variables["nodeTextColor"] = json!(color);
            }
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": variables,
            }));
            apply_theme_defaults(&mut config).unwrap();
            let expected = node_text.or(derives_node_text.then_some("#123456"));
            assert_eq!(
                config.get_str("themeVariables.nodeTextColor"),
                expected,
                "{theme}: node={node_text:?}"
            );
            assert_eq!(config.get_str("themeVariables.textColor"), Some("#fedcba"));
        }
    }
}

#[test]
fn architecture_color_dependencies_preserve_explicit_replay() {
    for &theme in MermaidThemeId::NAMES {
        for line_color in ["#333333", "#123456", "#777"] {
            for arrow_color in [None, Some("#556677")] {
                let mut variables = json!({"lineColor": line_color});
                if let Some(color) = arrow_color {
                    variables["archEdgeArrowColor"] = json!(color);
                }
                let mut config = SourcePresentationConfig::from_value(json!({
                    "theme": theme,
                    "themeVariables": variables,
                }));
                apply_theme_defaults(&mut config).unwrap();
                let expected_edge = match theme {
                    "base" => "#777",
                    "dark" => "lightgrey",
                    _ => line_color,
                };
                assert_eq!(
                    config.get_str("themeVariables.archEdgeColor"),
                    Some(expected_edge),
                    "{theme}: arrow={arrow_color:?}"
                );
                assert_eq!(
                    config.get_str("themeVariables.archEdgeArrowColor"),
                    Some(arrow_color.unwrap_or(expected_edge)),
                    "{theme}: arrow={arrow_color:?}"
                );
            }
        }
    }
}

#[test]
fn architecture_group_border_remains_independent_of_late_primary_border_overrides() {
    for &theme in MermaidThemeId::NAMES {
        let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
            .resolved_without_overrides()
            .get("archGroupBorderColor")
            .and_then(Value::as_str)
            .unwrap();
        for primary_border in [expected, "#123456"] {
            for group_border in [None, Some("#556677")] {
                let mut config = SourcePresentationConfig::from_value(json!({"theme": theme}));
                let mut variables = json!({"primaryBorderColor": primary_border});
                if let Some(color) = group_border {
                    variables["archGroupBorderColor"] = json!(color);
                }
                config.deep_merge(&json!({"themeVariables": variables}));
                apply_theme_defaults(&mut config).unwrap();
                assert_eq!(
                    config.get_str("themeVariables.archGroupBorderColor"),
                    Some(group_border.unwrap_or(expected)),
                    "{theme}: primary={primary_border}, group={group_border:?}"
                );
            }
        }
    }
}

#[test]
fn base_cluster_colors_drive_dependent_values() {
    for source in [
        "primaryColor",
        "tertiaryColor",
        "tertiaryBorderColor",
        "secondaryColor",
    ] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "base", "themeVariables": { (source): "#123456" }
        }));
        apply_theme_defaults(&mut config).unwrap();
        assert_eq!(
            config.get_str("themeVariables.clusterBkg"),
            config.get_str("themeVariables.tertiaryColor")
        );
        assert_eq!(
            config.get_str("themeVariables.clusterBorder"),
            config.get_str("themeVariables.tertiaryBorderColor")
        );
    }
    let mut config = SourcePresentationConfig::from_value(json!({"theme": "base"}));
    config.deep_merge(&json!({"themeVariables": {
        "tertiaryColor": "#123456", "clusterBkg": "#abcdef", "clusterBorder": "#fedcba"
    }}));
    apply_theme_defaults(&mut config).unwrap();
    assert_eq!(config.get_str("themeVariables.clusterBkg"), Some("#abcdef"));
    assert_eq!(
        config.get_str("themeVariables.clusterBorder"),
        Some("#fedcba")
    );
}

#[test]
fn variable_dependencies_follow_the_selected_theme_program() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            "default",
            "textColor",
            "#22c55e",
            &["signalColor", "signalTextColor"],
        ),
        (
            "default",
            "actorTextColor",
            "#16a34a",
            &["labelTextColor", "loopTextColor", "noteTextColor"],
        ),
        (
            "base",
            "mainBkg",
            "#0f172a",
            &["actorBkg", "labelBoxBkgColor"],
        ),
        (
            "base",
            "primaryBorderColor",
            "#38bdf8",
            &["actorBorder", "actorLineColor", "labelBoxBorderColor"],
        ),
        (
            "dark",
            "mainContrastColor",
            "#f8fafc",
            &[
                "actorTextColor",
                "signalColor",
                "signalTextColor",
                "labelTextColor",
                "loopTextColor",
            ],
        ),
        (
            "dark",
            "border1",
            "#a78bfa",
            &[
                "actorBorder",
                "actorLineColor",
                "labelBoxBorderColor",
                "activationBorderColor",
            ],
        ),
        (
            "neo",
            "primaryColor",
            "#0ea5e9",
            &["nodeBkg", "tagLabelBackground"],
        ),
        (
            "neo-dark",
            "primaryBorderColor",
            "#38bdf8",
            &["actorBorder", "actorLineColor", "labelBoxBorderColor"],
        ),
        (
            "redux",
            "mainBkg",
            "#0f172a",
            &["actorBkg", "labelBoxBkgColor", "personBkg", "stateBkg"],
        ),
        (
            "redux-color",
            "secondaryColor",
            "#22c55e",
            &[
                "edgeLabelBackground",
                "activationBkgColor",
                "commitLabelBackground",
                "relationLabelBackground",
            ],
        ),
        (
            "redux-dark",
            "primaryTextColor",
            "#f8fafc",
            &["actorTextColor", "labelTextColor", "loopTextColor"],
        ),
        (
            "redux-dark-color",
            "textColor",
            "#e2e8f0",
            &["signalTextColor", "transitionLabelColor"],
        ),
    ];

    for &(theme, source, value, targets) in cases {
        let mut config = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        config.deep_merge(&json!({
            "themeVariables": { (source): value }
        }));

        apply_theme_defaults(&mut config).unwrap();

        for target in targets {
            assert_eq!(
                config.get_str(&format!("themeVariables.{target}")),
                Some(value),
                "theme {theme} source {source} has incorrect {target} value"
            );
        }
    }
}

#[test]
fn extended_state_label_fallback_ignores_falsy_authored_background() {
    for theme in [
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        for background in [json!(false), json!(0)] {
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "primaryTextColor": "#123456" }
            }));
            config.deep_merge(&json!({ "themeVariables": { "stateBkg": background } }));
            apply_theme_defaults(&mut config).unwrap();
            assert_eq!(
                config.get_str("themeVariables.stateLabelColor"),
                Some("#123456"),
                "{theme}: {background}"
            );
            assert_eq!(
                config.as_value().pointer("/themeVariables/stateBkg"),
                Some(&background)
            );
        }
    }
}

#[test]
fn extended_theme_transforms_dependency_values() {
    for theme in [
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        let mut background = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        background.deep_merge(&json!({
            "themeVariables": { "background": "#ffffff" }
        }));
        apply_theme_defaults(&mut background).unwrap();
        for (target, expected) in [
            ("lineColor", "#000000"),
            ("transitionColor", "#000000"),
            ("sequenceNumberColor", "#ffffff"),
        ] {
            assert_eq!(
                background.get_str(&format!("themeVariables.{target}")),
                Some(expected),
                "theme {theme} has incorrect background-derived {target} value"
            );
        }

        let mut secondary = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        secondary.deep_merge(&json!({
            "themeVariables": { "secondaryColor": "#22c55e" }
        }));
        apply_theme_defaults(&mut secondary).unwrap();
        let expected_activation_border = theme_color::darken("#22c55e", 10.0).unwrap();
        assert_eq!(
            secondary.get_str("themeVariables.activationBorderColor"),
            Some(expected_activation_border.as_str()),
            "theme {theme} has incorrect activationBorderColor value"
        );
    }
}

#[test]
fn explicit_background_values_win_over_derived_values() {
    let mut derived = SourcePresentationConfig::from_value(json!({
        "theme": "base",
        "themeVariables": { "mainBkg": "#0f172a" }
    }));
    apply_theme_defaults(&mut derived).unwrap();
    assert_eq!(
        derived.get_str("themeVariables.labelBoxBkgColor"),
        Some("#0f172a")
    );

    let mut explicit_target = SourcePresentationConfig::from_value(json!({ "theme": "base" }));
    explicit_target.deep_merge(&json!({
        "themeVariables": {
            "mainBkg": "#0f172a",
            "actorBkg": "#1e293b",
            "labelBoxBkgColor": "#334155"
        }
    }));
    apply_theme_defaults(&mut explicit_target).unwrap();
    assert_eq!(
        explicit_target.get_str("themeVariables.actorBkg"),
        Some("#1e293b")
    );
    assert_eq!(
        explicit_target.get_str("themeVariables.labelBoxBkgColor"),
        Some("#334155")
    );
}

#[test]
fn unconditional_sequence_intermediates_feed_calculation_before_replay() {
    for (theme, expected_actor_line, expected_label_box_background) in [
        ("default", "#9370DB", "#ECECFF"),
        ("dark", "#ccc", "#1f2020"),
    ] {
        let mut config = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        config.deep_merge(&json!({
            "themeVariables": {
                "actorBorder": "#ec4899",
                "actorBkg": "#0f172a"
            }
        }));

        apply_theme_defaults(&mut config).unwrap();

        assert_eq!(
            config.get_str("themeVariables.actorBorder"),
            Some("#ec4899"),
            "theme {theme} must replay explicit actorBorder"
        );
        assert_eq!(
            config.get_str("themeVariables.actorBkg"),
            Some("#0f172a"),
            "theme {theme} must replay explicit actorBkg"
        );
        for (target, expected) in [
            ("actorLineColor", expected_actor_line),
            ("labelBoxBkgColor", expected_label_box_background),
        ] {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                config.get_str(&path),
                Some(expected),
                "theme {theme} must calculate {target} before explicit replay"
            );
        }
    }

    let mut forest = SourcePresentationConfig::from_value(json!({ "theme": "forest" }));
    forest.deep_merge(&json!({
        "themeVariables": { "primaryColor": "#ec4899" }
    }));
    apply_theme_defaults(&mut forest).unwrap();
    assert_eq!(forest.get_str("themeVariables.mainBkg"), Some("#cde498"));
}

#[test]
fn source_text_color_recalculates_scoped_sequence_but_preserves_unchanged_global_theme() {
    for (theme, initialized_signal, source_signal) in [
        (None, "#28253D", "#22c55e"),
        (Some("default"), "#333", "#333"),
    ] {
        let mut site = json!({
            "secure": [
                "secure", "securityLevel", "startOnLoad", "maxTextSize",
                "suppressErrorRendering", "maxEdges"
            ]
        });
        if let Some(theme) = theme {
            site["theme"] = json!(theme);
        }
        let engine = crate::MermaidEngine::new()
            .with_site_config(SourcePresentationConfig::from_value(site));
        let initialized = engine
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello\n")
            .expect("parse initialized Sequence theme");
        assert_eq!(
            initialized
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some(initialized_signal)
        );
        let metadata = engine
            .parse_metadata_sync(
                r##"%%{init: {"themeVariables": {"textColor": "#22c55e"}}}%%
sequenceDiagram
Alice->>Bob: Hello
"##,
            )
            .expect("parse Sequence init textColor");

        assert_eq!(
            metadata.config.get_str("themeVariables.textColor"),
            Some("#22c55e")
        );
        assert_eq!(
            metadata
                .effective_config
                .get_str("themeVariables.textColor"),
            Some("#22c55e")
        );
        assert_eq!(
            metadata
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some(source_signal)
        );
        assert!(
            metadata
                .effective_config
                .explicit_config_owns_path("themeVariables.textColor")
        );
        assert_eq!(metadata.config.get_str("themeVariables.signalColor"), None);
    }
}

#[test]
fn dark_extended_themes_preserve_constructor_arrowhead_after_background_override() {
    const SOURCE: &str = "flowchart LR\nA --> B\n";
    let expected_arrowhead = theme_color::invert("#333").unwrap();

    for theme in ["neo-dark", "redux-dark", "redux-dark-color"] {
        for origin in ["site", "source"] {
            let config = json!({
                "theme": theme,
                "themeVariables": {"background": "#010203"}
            });
            let (engine, source) = if origin == "site" {
                (
                    crate::MermaidEngine::new()
                        .with_site_config(SourcePresentationConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    crate::MermaidEngine::new().with_site_config(
                        SourcePresentationConfig::from_value(json!({
                            "secure": []
                        })),
                    ),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let metadata = engine.parse_metadata_sync(&source).unwrap_or_else(|error| {
                panic!("parse {origin} {theme} arrowhead snapshot config: {error}")
            });

            assert_eq!(
                metadata
                    .effective_config
                    .get_str("themeVariables.arrowheadColor"),
                Some(expected_arrowhead.as_str()),
                "{origin} {theme} must retain its constructor arrowhead snapshot"
            );
            assert_eq!(
                metadata
                    .effective_config
                    .get_str("themeVariables.lineColor"),
                Some("#fefdfc"),
                "{origin} {theme} must still recompute lineColor from explicit background"
            );
        }
    }
}

#[test]
fn venn_color_dependencies_follow_non_default_calculation_and_default_snapshots() {
    const SOURCE: &str = "venn-beta\ntitle Ownership\nset A\n";

    fn effective_config(
        theme: &str,
        theme_variables: Value,
        origin: &str,
    ) -> merman_model::presentation::PresentationConfig {
        let config = json!({
            "theme": theme,
            "themeVariables": theme_variables
        });
        let (engine, source) = if origin == "site" {
            (
                crate::MermaidEngine::new()
                    .with_site_config(SourcePresentationConfig::from_value(config)),
                SOURCE.to_string(),
            )
        } else {
            (
                crate::MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
                    json!({
                        "secure": []
                    }),
                )),
                format!("%%{{init: {config}}}%%\n{SOURCE}"),
            )
        };
        engine
            .parse_metadata_sync(&source)
            .unwrap_or_else(|error| panic!("parse {origin} {theme} Venn config: {error}"))
            .effective_config
    }

    for origin in ["site", "source"] {
        let base = effective_config(
            "base",
            json!({
                "titleColor": "#123456",
                "textColor": "#abcdef"
            }),
            origin,
        );
        for (target, expected) in [
            ("vennTitleTextColor", "#123456"),
            ("vennSetTextColor", "#abcdef"),
        ] {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                base.get_str(&path),
                Some(expected),
                "{origin} base {target}"
            );
        }

        let neutral = effective_config(
            "neutral",
            json!({
                "text": "#345678",
                "textColor": "#abcdef"
            }),
            origin,
        );
        let neutral_expectations = if origin == "site" {
            [
                ("vennTitleTextColor", "#345678"),
                ("vennSetTextColor", "#abcdef"),
            ]
        } else {
            // Mermaid's directive sanitizer does not admit the internal Neutral `text`
            // constructor variable. Source config therefore retains the `#333` title
            // snapshot, while the public `textColor` input sets Venn set text.
            [
                ("vennTitleTextColor", "#333"),
                ("vennSetTextColor", "#abcdef"),
            ]
        };
        for (target, expected) in neutral_expectations {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                neutral.get_str(&path),
                Some(expected),
                "{origin} neutral {target}"
            );
        }
        assert_eq!(
            neutral.get_str("themeVariables.text"),
            Some(if origin == "site" { "#345678" } else { "#333" }),
            "{origin} neutral directive sanitizer scope"
        );

        let explicit_targets = effective_config(
            "base",
            json!({
                "titleColor": "#123456",
                "textColor": "#abcdef",
                "vennTitleTextColor": "#fedcba",
                "vennSetTextColor": "#654321"
            }),
            origin,
        );
        assert_eq!(
            explicit_targets.get_str("themeVariables.vennTitleTextColor"),
            Some("#fedcba")
        );
        assert_eq!(
            explicit_targets.get_str("themeVariables.vennSetTextColor"),
            Some("#654321")
        );

        let default = effective_config(
            "default",
            json!({
                "titleColor": "#123456",
                "textColor": "#abcdef"
            }),
            origin,
        );
        for (target, expected) in [("vennTitleTextColor", "#333"), ("vennSetTextColor", "#333")] {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                default.get_str(&path),
                Some(expected),
                "{origin} default must retain the constructor snapshot for {target}"
            );
        }
    }
}

#[test]
fn theme_dependencies_preserve_site_and_source_values() {
    const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";
    struct Case {
        theme: &'static str,
        variables: Value,
        expected: Vec<(&'static str, String)>,
    }

    let note_border = mk_border("#abcdef", false).unwrap();
    let dark_note_background = theme_color::lighten("#123456", 16.0).unwrap();
    let dark_default_note_background = theme_color::lighten("#1f2020", 16.0).unwrap();
    let forest_actor_border = theme_color::darken("#89abcd", 20.0).unwrap();
    let neutral_actor_border = theme_color::lighten("#456789", 23.0).unwrap();
    let cases = vec![
        Case {
            theme: "base",
            variables: json!({
                "noteBkgColor": "#abcdef",
                "lineColor": "#123456",
                "textColor": "#234567",
                "mainBkg": "#345678"
            }),
            expected: vec![
                ("noteBorderColor", note_border.clone()),
                ("transitionColor", "#123456".to_string()),
                ("specialStateColor", "#123456".to_string()),
                ("transitionLabelColor", "#234567".to_string()),
                ("stateBkg", "#345678".to_string()),
                ("labelBackgroundColor", "#345678".to_string()),
            ],
        },
        Case {
            theme: "neo",
            variables: json!({ "noteBkgColor": "#abcdef" }),
            expected: vec![("noteBorderColor", note_border.clone())],
        },
        Case {
            theme: "neo-dark",
            variables: json!({ "noteBkgColor": "#abcdef" }),
            expected: vec![("noteBorderColor", note_border)],
        },
        Case {
            theme: "forest",
            variables: json!({
                "border2": "#456789",
                "actorTextColor": "#56789a",
                "lineColor": "#6789ab",
                "textColor": "#789abc",
                "mainBkg": "#89abcd"
            }),
            expected: vec![
                ("actorBorder", forest_actor_border.clone()),
                ("actorLineColor", forest_actor_border),
                ("actorBkg", "#89abcd".to_string()),
                ("labelBoxBkgColor", "#89abcd".to_string()),
                ("labelTextColor", "#56789a".to_string()),
                ("loopTextColor", "#56789a".to_string()),
                ("noteBorderColor", "#456789".to_string()),
                ("noteTextColor", "#56789a".to_string()),
                ("transitionColor", "#6789ab".to_string()),
                ("specialStateColor", "#6789ab".to_string()),
                ("transitionLabelColor", "#789abc".to_string()),
                ("stateBkg", "#89abcd".to_string()),
                ("labelBackgroundColor", "#89abcd".to_string()),
            ],
        },
        Case {
            theme: "neutral",
            variables: json!({
                "noteBkgColor": "#abcdef",
                "border1": "#456789",
                "lineColor": "#123456",
                "textColor": "#234567",
                "mainBkg": "#345678"
            }),
            expected: vec![
                ("actorBorder", neutral_actor_border.clone()),
                ("actorLineColor", neutral_actor_border.clone()),
                ("labelBoxBorderColor", neutral_actor_border),
                ("actorBkg", "#345678".to_string()),
                ("labelBoxBkgColor", "#345678".to_string()),
                ("transitionLabelColor", "#234567".to_string()),
                ("stateBkg", "#345678".to_string()),
                ("labelBackgroundColor", "#345678".to_string()),
                ("noteBorderColor", "#999".to_string()),
                ("transitionColor", "#000".to_string()),
                ("specialStateColor", "#222".to_string()),
            ],
        },
        Case {
            theme: "dark",
            variables: json!({
                "mainBkg": "#123456",
                "secondBkg": "#abcdef",
                "secondaryBorderColor": "#234567",
                "secondaryTextColor": "#345678",
                "lineColor": "#fedcba",
                "textColor": "#56789a"
            }),
            expected: vec![
                ("noteBorderColor", "#234567".to_string()),
                ("noteBkgColor", dark_note_background),
                ("noteTextColor", "#345678".to_string()),
                ("transitionLabelColor", "#56789a".to_string()),
                ("stateBkg", "#123456".to_string()),
                ("labelBackgroundColor", "#123456".to_string()),
                ("transitionColor", "lightgrey".to_string()),
                ("specialStateColor", "#f4f4f4".to_string()),
            ],
        },
        Case {
            theme: "dark",
            variables: json!({ "secondBkg": "#abcdef" }),
            expected: vec![
                ("noteBkgColor", dark_default_note_background.clone()),
                ("activationBkgColor", dark_default_note_background),
            ],
        },
        Case {
            theme: "dark",
            variables: json!({ "lineColor": "#fedcba" }),
            expected: vec![
                ("transitionColor", "lightgrey".to_string()),
                ("specialStateColor", "#f4f4f4".to_string()),
            ],
        },
        Case {
            theme: "default",
            variables: json!({
                "border2": "#456789",
                "actorTextColor": "#56789a",
                "lineColor": "#123456",
                "textColor": "#234567",
                "mainBkg": "#345678"
            }),
            expected: vec![
                ("noteBorderColor", "#456789".to_string()),
                ("noteTextColor", "#56789a".to_string()),
                ("specialStateColor", "#123456".to_string()),
                ("transitionColor", "#333333".to_string()),
                ("transitionLabelColor", "#333".to_string()),
                ("stateBkg", "#ECECFF".to_string()),
                ("labelBackgroundColor", "#ECECFF".to_string()),
            ],
        },
    ];

    for case in cases {
        for origin in ["site", "source"] {
            let config = json!({
                "theme": case.theme,
                "themeVariables": case.variables.clone()
            });
            let (engine, source) = if origin == "site" {
                (
                    crate::MermaidEngine::new()
                        .with_site_config(SourcePresentationConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    crate::MermaidEngine::new().with_site_config(
                        SourcePresentationConfig::from_value(json!({
                            "secure": []
                        })),
                    ),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let metadata = engine.parse_metadata_sync(&source).unwrap_or_else(|error| {
                panic!("parse {origin} {} dependency config: {error}", case.theme)
            });

            for (target, expected) in &case.expected {
                let path = format!("themeVariables.{target}");
                assert_eq!(
                    metadata.effective_config.get_str(&path),
                    Some(expected.as_str()),
                    "{origin} {} has incorrect {target} value",
                    case.theme
                );
            }
        }
    }
}

#[test]
fn state_terminal_dependencies_preserve_ordered_values() {
    fn config_with_explicit_theme_variables(
        theme: &str,
        theme_variables: Value,
    ) -> SourcePresentationConfig {
        let mut config = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        config.deep_merge(&json!({ "themeVariables": theme_variables }));
        config
    }

    fn assert_color(config: &SourcePresentationConfig, target: &str, expected: &str) {
        let path = format!("themeVariables.{target}");
        assert_eq!(
            config.get_str(&path),
            Some(expected),
            "incorrect {target} value"
        );
    }

    let mut default = config_with_explicit_theme_variables(
        "default",
        json!({
            "background": "#123456",
            "border1": "#234567",
            "lineColor": "#345678",
            "textColor": "#456789"
        }),
    );
    apply_theme_defaults(&mut default).unwrap();
    for (target, expected) in [
        ("nodeBorder", "#234567"),
        ("innerEndBackground", "#234567"),
        ("specialStateColor", "#345678"),
        ("titleColor", "#456789"),
    ] {
        assert_color(&default, target, expected);
    }
    assert_color(&default, "compositeBackground", "white");
    assert_color(&default, "transitionColor", "#333333");

    let mut base = config_with_explicit_theme_variables(
        "base",
        json!({
            "background": "#123456",
            "lineColor": "#234567",
            "mainBkg": "#345678",
            "primaryBorderColor": "#456789",
            "primaryTextColor": "#56789a",
            "tertiaryTextColor": "#6789ab"
        }),
    );
    apply_theme_defaults(&mut base).unwrap();
    for (target, expected) in [
        ("compositeBackground", "#123456"),
        ("specialStateColor", "#234567"),
        ("stateBkg", "#345678"),
        ("labelBackgroundColor", "#345678"),
        ("compositeTitleBackground", "#345678"),
        ("nodeBorder", "#456789"),
        ("innerEndBackground", "#456789"),
        ("stateLabelColor", "#56789a"),
        ("titleColor", "#6789ab"),
    ] {
        assert_color(&base, target, expected);
    }

    let mut explicit_state_background = config_with_explicit_theme_variables(
        "base",
        json!({
            "mainBkg": "#123456",
            "primaryTextColor": "#234567",
            "stateBkg": "#345678"
        }),
    );
    apply_theme_defaults(&mut explicit_state_background).unwrap();
    assert_color(&explicit_state_background, "stateLabelColor", "#345678");

    let mut derived_state_background =
        config_with_explicit_theme_variables("base", json!({ "mainBkg": "#123456" }));
    apply_theme_defaults(&mut derived_state_background).unwrap();
    assert_color(&derived_state_background, "stateBkg", "#123456");
    assert_color(&derived_state_background, "stateLabelColor", "#333");

    let dark_edge_label = theme_color::lighten("#234567", 25.0).unwrap();
    for (theme, variables, expected, unchanged) in [
        (
            "dark",
            json!({
                "background": "#123456",
                "labelBackground": "#234567",
                "mainBkg": "#345678",
                "primaryBorderColor": "#456789"
            }),
            vec![
                ("compositeBackground", "#123456".to_string()),
                ("edgeLabelBackground", dark_edge_label),
                ("stateBkg", "#345678".to_string()),
                ("labelBackgroundColor", "#345678".to_string()),
                ("compositeTitleBackground", "#345678".to_string()),
                ("innerEndBackground", "#456789".to_string()),
            ],
            vec![("specialStateColor", "#f4f4f4".to_string())],
        ),
        (
            "forest",
            json!({
                "background": "#123456",
                "lineColor": "#234567",
                "mainBkg": "#345678",
                "primaryBorderColor": "#456789"
            }),
            vec![
                ("compositeBackground", "#123456".to_string()),
                ("specialStateColor", "#234567".to_string()),
                ("stateBkg", "#345678".to_string()),
                ("labelBackgroundColor", "#345678".to_string()),
                ("compositeTitleBackground", "#345678".to_string()),
                ("innerEndBackground", "#456789".to_string()),
            ],
            vec![("titleColor", "#333".to_string())],
        ),
        (
            "neutral",
            json!({
                "background": "#123456",
                "mainBkg": "#234567",
                "primaryBorderColor": "#345678",
                "text": "#456789"
            }),
            vec![
                ("compositeBackground", "#123456".to_string()),
                ("stateBkg", "#234567".to_string()),
                ("labelBackgroundColor", "#234567".to_string()),
                ("compositeTitleBackground", "#234567".to_string()),
                ("innerEndBackground", "#345678".to_string()),
                ("titleColor", "#456789".to_string()),
            ],
            vec![("specialStateColor", "#222".to_string())],
        ),
    ] {
        let mut config = config_with_explicit_theme_variables(theme, variables);
        apply_theme_defaults(&mut config).unwrap();
        for (target, expected) in expected.into_iter().chain(unchanged) {
            assert_color(&config, target, &expected);
        }
    }

    for (theme, expected_composite, expected_title) in [
        ("neo", "#123456", "#345678"),
        ("neo-dark", "#123456", "#345678"),
        ("redux", "#123456", "#F9F9FB"),
        ("redux-dark", "#16141F", "#16141F"),
        ("redux-color", "#123456", "#345678"),
        ("redux-dark-color", "#123456", "#345678"),
    ] {
        let mut config = config_with_explicit_theme_variables(
            theme,
            json!({
                "background": "#123456",
                "lineColor": "#234567",
                "mainBkg": "#345678",
                "nodeBorder": "#456789",
                "primaryTextColor": "#56789a",
                "tertiaryTextColor": "#6789ab"
            }),
        );
        apply_theme_defaults(&mut config).unwrap();

        assert_color(&config, "titleColor", "#6789ab");
        assert_color(&config, "innerEndBackground", "#456789");
        assert_color(&config, "specialStateColor", "#234567");
        assert_color(&config, "stateLabelColor", "#56789a");
        assert_color(&config, "compositeBackground", expected_composite);
        assert_color(&config, "compositeTitleBackground", expected_title);
    }

    let mut default_gradient = config_with_explicit_theme_variables(
        "default",
        json!({
            "primaryBorderColor": "#123456",
            "secondaryBorderColor": "#234567"
        }),
    );
    apply_theme_defaults(&mut default_gradient).unwrap();
    assert_color(
        &default_gradient,
        "gradientStart",
        &mk_border("#ECECFF", false).unwrap(),
    );
    assert_color(
        &default_gradient,
        "gradientStop",
        &mk_border("#ffffde", false).unwrap(),
    );

    let mut base_gradient = config_with_explicit_theme_variables(
        "base",
        json!({
            "primaryBorderColor": "#123456",
            "secondaryBorderColor": "#234567"
        }),
    );
    apply_theme_defaults(&mut base_gradient).unwrap();
    assert_color(&base_gradient, "gradientStart", "#123456");
    assert_color(&base_gradient, "gradientStop", "#234567");

    for (theme, gradient_start, gradient_stop) in [
        ("dark", "#cccccc", "hsl(180, 0%, 18.3529411765%)"),
        (
            "forest",
            "hsl(78.1578947368, 18.4615384615%, 64.5098039216%)",
            "hsl(98.961038961, 60%, 74.9019607843%)",
        ),
        (
            "neutral",
            "hsl(0, 0%, 83.3333333333%)",
            "hsl(0, 0%, 88.9215686275%)",
        ),
    ] {
        let mut config = config_with_explicit_theme_variables(
            theme,
            json!({
                "primaryBorderColor": "#123456",
                "secondaryBorderColor": "#234567"
            }),
        );
        apply_theme_defaults(&mut config).unwrap();
        assert_color(&config, "gradientStart", gradient_start);
        assert_color(&config, "gradientStop", gradient_stop);
    }

    let mut base_sources = config_with_explicit_theme_variables(
        "base",
        json!({
            "background": "#123456",
            "primaryTextColor": "#234567",
            "secondaryColor": "#345678",
            "tertiaryColor": "#456789"
        }),
    );
    apply_theme_defaults(&mut base_sources).unwrap();
    let derived_line = theme_color::invert("#123456").unwrap();
    let derived_tertiary_text = theme_color::invert("#456789").unwrap();
    let derived_secondary_border = mk_border("#345678", false).unwrap();
    let derived_activation_border = theme_color::darken("#345678", 10.0).unwrap();
    let derived_sequence_number = theme_color::invert(&derived_line).unwrap();
    for (target, expected) in [
        ("lineColor", derived_line.as_str()),
        ("transitionColor", derived_line.as_str()),
        ("specialStateColor", derived_line.as_str()),
        ("sequenceNumberColor", derived_sequence_number.as_str()),
        ("textColor", "#234567"),
        ("signalTextColor", "#234567"),
        ("transitionLabelColor", "#234567"),
        ("tertiaryTextColor", derived_tertiary_text.as_str()),
        ("titleColor", derived_tertiary_text.as_str()),
        ("secondaryBorderColor", derived_secondary_border.as_str()),
        ("gradientStop", derived_secondary_border.as_str()),
        ("activationBorderColor", derived_activation_border.as_str()),
    ] {
        assert_color(&base_sources, target, expected);
    }

    for theme in [
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        let mut config = config_with_explicit_theme_variables(
            theme,
            json!({
                "primaryBorderColor": "#123456",
                "secondaryBorderColor": "#234567"
            }),
        );
        apply_theme_defaults(&mut config).unwrap();
        assert_color(&config, "gradientStart", "#0042eb");
        assert_color(&config, "gradientStop", "#eb0042");
    }
}

#[test]
fn gantt_terminal_fill_dependencies_preserve_values() {
    fn config_with_explicit_theme_variables(
        theme: &str,
        theme_variables: Value,
    ) -> SourcePresentationConfig {
        let mut config = SourcePresentationConfig::from_value(json!({ "theme": theme }));
        config.deep_merge(&json!({ "themeVariables": theme_variables }));
        config
    }

    fn assert_color(config: &SourcePresentationConfig, target: &str, expected: &str) {
        let path = format!("themeVariables.{target}");
        assert_eq!(
            config.get_str(&path),
            Some(expected),
            "incorrect {target} value"
        );
    }

    let mut base =
        config_with_explicit_theme_variables("base", json!({ "primaryColor": "#123456" }));
    apply_theme_defaults(&mut base).unwrap();
    assert_color(&base, "taskBkgColor", "#123456");
    assert_color(
        &base,
        "activeTaskBkgColor",
        &theme_color::lighten("#123456", 23.0).unwrap(),
    );

    let mut dark = config_with_explicit_theme_variables(
        "dark",
        json!({
            "mainBkg": "#123456",
            "mainContrastColor": "#abcdef"
        }),
    );
    apply_theme_defaults(&mut dark).unwrap();
    assert_color(
        &dark,
        "taskBkgColor",
        &theme_color::lighten("#123456", 23.0).unwrap(),
    );
    assert_color(&dark, "doneTaskBkgColor", "#abcdef");

    let mut forest =
        config_with_explicit_theme_variables("forest", json!({ "mainBkg": "#123456" }));
    apply_theme_defaults(&mut forest).unwrap();
    assert_color(&forest, "activeTaskBkgColor", "#123456");

    let mut neutral = config_with_explicit_theme_variables(
        "neutral",
        json!({
            "contrast": "#123456",
            "mainBkg": "#234567",
            "done": "#345678",
            "critical": "#456789"
        }),
    );
    apply_theme_defaults(&mut neutral).unwrap();
    for (target, expected) in [
        ("taskBkgColor", "#123456"),
        ("activeTaskBkgColor", "#234567"),
        ("doneTaskBkgColor", "#345678"),
        ("critBkgColor", "#456789"),
    ] {
        assert_color(&neutral, target, expected);
    }

    let active_dark = theme_color::lighten("#123456", 23.0).unwrap();
    for theme in ["neo-dark", "redux-dark", "redux-dark-color"] {
        let mut config =
            config_with_explicit_theme_variables(theme, json!({ "primaryColor": "#123456" }));
        apply_theme_defaults(&mut config).unwrap();
        assert_color(&config, "taskBkgColor", "#123456");
        assert_color(&config, "activeTaskBkgColor", &active_dark);
    }

    let pinned_light_active = theme_color::lighten("#ECECFE", 23.0).unwrap();
    for theme in ["neo", "redux", "redux-color"] {
        let mut config =
            config_with_explicit_theme_variables(theme, json!({ "primaryColor": "#123456" }));
        apply_theme_defaults(&mut config).unwrap();
        assert_color(&config, "taskBkgColor", "#ECECFE");
        assert_color(&config, "activeTaskBkgColor", &pinned_light_active);
    }

    let mut explicit_targets = config_with_explicit_theme_variables(
        "default",
        json!({
            "taskBkgColor": "#111111",
            "activeTaskBkgColor": "#222222",
            "doneTaskBkgColor": "#333333",
            "critBkgColor": "#444444"
        }),
    );
    apply_theme_defaults(&mut explicit_targets).unwrap();
    for (target, expected) in [
        ("taskBkgColor", "#111111"),
        ("activeTaskBkgColor", "#222222"),
        ("doneTaskBkgColor", "#333333"),
        ("critBkgColor", "#444444"),
    ] {
        assert_color(&explicit_targets, target, expected);
    }
}

#[test]
fn neutral_site_text_drives_unconditional_sequence_derivations() {
    let metadata = crate::MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "neutral",
            "themeVariables": { "text": "#56789a" }
        })))
        .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello\n")
        .expect("parse site-owned Neutral text");

    assert_eq!(
        metadata.effective_config.get_str("themeVariables.text"),
        Some("#56789a")
    );
    for target in [
        "actorTextColor",
        "signalColor",
        "signalTextColor",
        "labelTextColor",
        "loopTextColor",
    ] {
        let path = format!("themeVariables.{target}");
        assert_eq!(
            metadata.effective_config.get_str(&path),
            Some("#56789a"),
            "Neutral site text has incorrect derived {target} value"
        );
    }
}

#[test]
fn dark_site_main_contrast_drives_state_and_sequence_derivations() {
    let metadata = crate::MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": { "mainContrastColor": "#456789" }
        })))
        .parse_metadata_sync("stateDiagram-v2\nReady --> Done: Finish\n")
        .expect("parse site-owned Dark mainContrastColor");

    for target in [
        "lineColor",
        "actorTextColor",
        "signalColor",
        "signalTextColor",
        "labelTextColor",
        "loopTextColor",
        "transitionColor",
    ] {
        let path = format!("themeVariables.{target}");
        assert_eq!(
            metadata.effective_config.get_str(&path),
            Some("#456789"),
            "Dark site mainContrastColor has incorrect derived {target} value"
        );
    }
}

#[test]
fn dark_mode_branch_matches_generated_mermaid_oracle_for_every_theme() {
    for &theme in MermaidThemeId::NAMES {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": { "darkMode": true }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let actual = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();
        let expected =
            ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap()).resolved_dark_mode_true();
        assert_eq!(actual, expected, "theme {theme}");
    }
}

#[test]
fn generated_mermaid_oracle_locks_override_value_semantics() {
    let mut mismatches = Vec::new();
    for case in &generated_theme_oracles().oracle_cases {
        let id = case.get("id").and_then(Value::as_str).unwrap();
        let theme = case.get("theme").and_then(Value::as_str).unwrap();
        let overrides = case.get("overrides").cloned().unwrap();
        let expected_status = case.get("status").and_then(Value::as_str).unwrap();
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": overrides
        }));
        let result = apply_theme_defaults(&mut config);

        if expected_status == "error" {
            if result.is_ok() {
                mismatches.push(format!("{theme}/{id}: expected an error"));
            }
            continue;
        }
        if let Err(error) = result {
            mismatches.push(format!("{theme}/{id}: unexpected error: {error}"));
            continue;
        }

        let actual = config.as_value().get("themeVariables").unwrap();
        let selected = case.get("selected").and_then(Value::as_object).unwrap();
        for (path, expected) in selected {
            let state = expected.get("state").and_then(Value::as_str).unwrap();
            let actual_value = value_at_path(actual, path);
            match state {
                "missing" if actual_value.is_some() => mismatches.push(format!(
                    "{theme}/{id}/{path}: expected missing, found {}",
                    actual_value.unwrap()
                )),
                "value" if actual_value != expected.get("value") => mismatches.push(format!(
                    "{theme}/{id}/{path}: expected {}, found {}",
                    expected.get("value").unwrap(),
                    actual_value
                        .map(Value::to_string)
                        .unwrap_or_else(|| "missing".to_string())
                )),
                "missing" | "value" => {}
                other => {
                    mismatches.push(format!("{theme}/{id}/{path}: unknown oracle state {other}"))
                }
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "Mermaid theme oracle mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn font_only_override_preserves_upstream_derived_palette_for_public_themes() {
    for &theme in MermaidThemeId::NAMES {
        let mut cfg = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": {
                "fontFamily": "Inter, sans-serif"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let actual = cfg
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();
        let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
            .resolved_without_overrides();

        for key in [
            "cScale0",
            "cScale1",
            "cScalePeer0",
            "cScaleInv0",
            "cScaleLabel0",
        ] {
            assert_eq!(
                actual.get(key),
                expected.get(key),
                "theme {theme} has derived palette drift at {key}"
            );
        }
        for (key, expected_value) in expected {
            if key == "fontFamily" {
                continue;
            }
            assert_eq!(
                actual.get(key),
                Some(expected_value),
                "theme {theme} has a non-font snapshot drift at {key}"
            );
        }
        assert_eq!(
            actual.get("fontFamily").and_then(Value::as_str),
            Some("Inter, sans-serif"),
            "theme {theme} should replay the explicit font override"
        );
    }
}

#[test]
fn default_theme_exact_overrides_preserve_truthiness_and_typography() {
    let program = ThemeProgram::resolve(MermaidThemeId::Default);
    for (overrides, dark_mode) in [
        (
            json!({"darkMode": false, "fontFamily": "", "fontSize": 17}),
            false,
        ),
        (
            json!({"darkMode": 0, "fontFamily": 17, "fontSize": ""}),
            false,
        ),
        (
            json!({"darkMode": "", "fontFamily": null, "fontSize": null}),
            false,
        ),
        (
            json!({"darkMode": "false", "fontFamily": {"name": "Inter"}, "fontSize": ["16px"]}),
            true,
        ),
        (json!({"darkMode": []}), true),
        (json!({"darkMode": {}}), true),
    ] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "default", "themeVariables": overrides.clone(),
        }));
        apply_theme_defaults(&mut config).unwrap();

        let mut expected = if dark_mode {
            program.resolved_dark_mode_true().clone()
        } else {
            program.resolved_without_overrides().clone()
        };
        expected.extend(
            overrides
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        assert_eq!(
            config.as_value()["themeVariables"].as_object().unwrap(),
            &expected,
            "overrides={overrides}"
        );
    }
}

#[test]
fn default_theme_nonexact_invalid_colors_preserve_errors() {
    for (variable, value, expected_input) in [
        ("primaryColor", json!("not-a-color"), "not-a-color"),
        ("git7", json!("not-a-color"), "not-a-color"),
        ("quadrant1Fill", json!(17), "17"),
    ] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "default", "themeVariables": {(variable): value},
        }));
        let before = config.clone();
        assert_eq!(
            apply_theme_defaults(&mut config),
            Err(ThemeResolutionError::Color(ColorError::UnsupportedFormat {
                input: expected_input.to_string(),
            })),
            "{variable}"
        );
        assert_eq!(config.as_value(), before.as_value(), "{variable}");
    }
}

#[test]
fn base_node_border_override_controls_inferred_gradient() {
    for (overrides, expected) in [
        (json!({ "nodeBorder": "#225577" }), false),
        (
            json!({ "nodeBorder": "#225577", "useGradient": true }),
            true,
        ),
        (json!({ "mainBkg": "#ffe1ef" }), true),
        (json!({ "useGradient": false }), false),
    ] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "base", "themeVariables": overrides
        }));
        apply_theme_defaults(&mut config).unwrap();
        assert_eq!(config.as_value()["themeVariables"]["useGradient"], expected);
    }
}

#[test]
fn base_finalize_uses_node_border_presence() {
    let program = ThemeProgram::resolve(MermaidThemeId::Base);
    for border in [json!(false), json!(0), Value::Null] {
        let explicit = json!({"nodeBorder": border});
        let resolved = staged::Resolution::execute(
            staged::StagedProgram::Base,
            program.prepared_constructor(),
            explicit.as_object().unwrap(),
        )
        .unwrap();
        assert_eq!(resolved.variables["useGradient"], json!(false));
    }
}

#[test]
fn agentflow_stroke_follows_upstream_constructor_order() {
    for &theme in MermaidThemeId::NAMES {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": { "secondaryBorderColor": "#123456" }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let expected = if theme == "default" {
            json!("hsl(60, 60%, 83.5294117647%)")
        } else {
            json!("#123456")
        };
        assert_eq!(
            config.as_value()["themeVariables"]["flowContainerStroke"],
            expected,
            "{theme}"
        );
    }
}

#[test]
fn extended_theme_bands_follow_mermaid_12_background_derivations() {
    for theme in ["neo", "redux", "redux-color"] {
        for (background, expected) in [
            ("#000000", "hsl(0, 0%, 4%)"),
            ("#ffffff", "hsl(0, 0%, 96%)"),
        ] {
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme, "themeVariables": { "background": background }
            }));
            apply_theme_defaults(&mut config).unwrap();
            assert_eq!(
                config.as_value()["themeVariables"]["rectBkgColor"],
                expected,
                "{theme}"
            );
        }
    }
    for theme in ["neo-dark", "redux-dark", "redux-dark-color"] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme, "themeVariables": { "mainBkg": "#101112" }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = &config.as_value()["themeVariables"];
        assert_eq!(
            variables["secondBkg"], "hsl(210, 5.8823529412%, 22.6666666667%)",
            "{theme}"
        );
        assert_eq!(
            variables["doneTaskBkgColor"], variables["secondBkg"],
            "{theme}"
        );
    }
}

#[test]
fn redux_color_categorical_overrides_feed_pie_gantt_and_journey() {
    for theme in ["redux-color", "redux-dark-color"] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": {
                "primaryColor": "#123456", "cScale0": "#234567", "cScale1": "#345678",
                "borderColorArray": ["#456789"], "bkgColorArray": ["#567890"],
                "background": "#000000", "pie2": "#678901"
            }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = &config.as_value()["themeVariables"];
        assert_eq!(variables["pie1"], "#234567", "{theme}");
        assert_eq!(variables["pie2"], "#678901", "{theme}");
        assert_eq!(variables["sectionBkgColor"], "#234567", "{theme}");
        assert_eq!(variables["sectionBkgColor2"], "#345678", "{theme}");
        assert_eq!(variables["venn8"], "#456789", "{theme}");
        if theme == "redux-color" {
            assert_eq!(variables["fillType0"], "#567890");
            assert!(variables.get("fillType1").is_none());
        } else {
            assert_eq!(variables["fillType0"], "#701a75");
            assert_eq!(variables["altSectionBkgColor"], "#000000");
        }
    }
}

#[test]
fn redux_categorical_falsy_override_replays_after_pie_derivation() {
    for theme in ["redux-color", "redux-dark-color"] {
        for explicit_scale in [json!(false), json!(0)] {
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "cScale2": explicit_scale }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let variables = &config.as_value()["themeVariables"];
            assert_eq!(variables["cScale2"], explicit_scale, "{theme}");
            assert_eq!(variables["pie3"], "#ffb86a", "{theme}");
        }
    }
}

#[test]
fn agentflow_falsy_border_replays_after_container_stroke_derivation() {
    for (theme, expected_stroke) in [
        ("redux-color", "hsl(-120, 0%, 70%)"),
        ("redux-dark-color", "hsl(180, 0%, 18.3529411765%)"),
    ] {
        for explicit_border in [json!(false), json!(0)] {
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "secondaryBorderColor": explicit_border }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let variables = &config.as_value()["themeVariables"];
            assert_eq!(
                variables["secondaryBorderColor"], explicit_border,
                "{theme}"
            );
            assert_eq!(variables["flowContainerStroke"], expected_stroke, "{theme}");
        }
    }
}

#[test]
fn explicit_scale_override_recomputes_peer_and_inverse_from_override_stage() {
    // Oracle values from Mermaid 11.16.1 `getThemeVariables()` with the same overrides.
    let cases = [
        (
            "default",
            "hsl(240, 100%, 61.2745098039%)",
            "hsl(60, 100%, 86.2745098039%)",
        ),
        ("dark", "hsl(210, 68%, 90.3921568627%)", "#543210"),
        (
            "forest",
            "hsl(210, 68%, 45.3921568627%)",
            "hsl(30, 68%, 70.3921568627%)",
        ),
        ("neutral", "hsl(210, 68%, 70.3921568627%)", "#543210"),
        (
            "base",
            "hsl(210, 68%, 45.3921568627%)",
            "rgb(191.1000000002, 113.7500000001, 36.4)",
        ),
    ];

    for (theme, expected_peer, expected_inverse) in cases {
        let mut cfg = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": {
                "primaryColor": "#123456",
                "cScale0": "#abcdef"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();
        let actual = cfg
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        assert_eq!(
            actual.get("cScale0").and_then(Value::as_str),
            Some("#abcdef"),
            "theme {theme} must replay explicit cScale0"
        );
        assert_eq!(
            actual.get("cScalePeer0").and_then(Value::as_str),
            Some(expected_peer),
            "theme {theme} must derive cScalePeer0 from the override stage"
        );
        assert_eq!(
            actual.get("cScaleInv0").and_then(Value::as_str),
            Some(expected_inverse),
            "theme {theme} must derive cScaleInv0 from the override stage"
        );
    }
}

#[test]
fn extended_theme_scale_override_replays_after_source_ordered_derivations() {
    // Oracle: Mermaid 11.16.1 getThemeVariables({ cScale0: '#abcdef' }). Theme names do not
    // imply darkMode; only an explicit darkMode value changes the scale transform branch.
    let cases = [
        (
            "neo",
            "hsl(210, 68%, 45.3921568627%)",
            "rgb(191.1000000002, 113.7500000001, 36.4)",
            "#333",
        ),
        (
            "neo-dark",
            "hsl(210, 68%, 45.3921568627%)",
            "rgb(191.1000000002, 113.7500000001, 36.4)",
            "#e0dfdf",
        ),
        (
            "redux",
            "hsl(0, 0%, 65%)",
            "rgb(63.75, 63.75, 63.75)",
            "#28253D",
        ),
        (
            "redux-dark",
            "hsl(210, 68%, 45.3921568627%)",
            "rgb(191.1000000002, 113.7500000001, 36.4)",
            "#e0dfdf",
        ),
        (
            "redux-color",
            "hsl(210, 68%, 70.3921568627%)",
            "#543210",
            "#28253D",
        ),
        (
            "redux-dark-color",
            "hsl(210, 68%, 70.3921568627%)",
            "#543210",
            "hsl(210, 68%, 5.3921568627%)",
        ),
    ];

    for (theme, expected_peer, expected_inverse, expected_label) in cases {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": { "cScale0": "#abcdef" }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        for (key, expected) in [
            ("cScale0", "#abcdef"),
            ("cScalePeer0", expected_peer),
            ("cScaleInv0", expected_inverse),
            ("cScaleLabel0", expected_label),
        ] {
            assert_eq!(
                variables.get(key).and_then(Value::as_str),
                Some(expected),
                "theme {theme} has incorrect {key}"
            );
        }
    }
}

#[test]
fn primary_color_override_follows_each_upstream_theme_scale_contract() {
    // Oracle values from Mermaid 11.16.1 `getThemeVariables()` with the same overrides.
    let cases = [
        (
            "default",
            "hsl(240, 100%, 76.2745098039%)",
            "hsl(240, 100%, 61.2745098039%)",
            "hsl(60, 100%, 86.2745098039%)",
        ),
        (
            "dark",
            "#123456",
            "hsl(210, 65.3846153846%, 30.3921568627%)",
            "#edcba9",
        ),
        (
            "forest",
            "hsl(210, 65.3846153846%, 10.3921568627%)",
            "hsl(210, 65.3846153846%, 0%)",
            "hsl(30, 65.3846153846%, 10.3921568627%)",
        ),
        ("neutral", "#555", "hsl(0, 0%, 23.3333333333%)", "#aaaaaa"),
        (
            "base",
            "hsl(210, 65.3846153846%, 0%)",
            "hsl(210, 65.3846153846%, 0%)",
            "#ffffff",
        ),
    ];

    for (theme, expected_scale, expected_peer, expected_inverse) in cases {
        let mut cfg = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": {
                "primaryColor": "#123456"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();
        let actual = cfg
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        for (key, expected) in [
            ("cScale0", expected_scale),
            ("cScalePeer0", expected_peer),
            ("cScaleInv0", expected_inverse),
        ] {
            assert_eq!(
                actual.get(key).and_then(Value::as_str),
                Some(expected),
                "theme {theme} has incorrect {key} after primaryColor override"
            );
        }
    }
}

#[test]
fn quadrant_primary_override_matches_mermaid_11_16_theme_lifecycles() {
    // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()`.
    let cases = [
        ("default", "#ECECFF", "#f1f1ff", "hsl(240, 100%, NaN%)"),
        (
            "dark",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        (
            "forest",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        (
            "neutral",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        (
            "base",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        ("neo", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
        (
            "neo-dark",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        ("redux", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
        (
            "redux-dark",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
        ("redux-color", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
        (
            "redux-dark-color",
            "#123456",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
        ),
    ];

    for (theme, expected_q1, expected_q2, expected_point) in cases {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": { "primaryColor": "#123456" }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        for (key, expected) in [
            ("quadrant1Fill", expected_q1),
            ("quadrant2Fill", expected_q2),
            ("quadrantPointFill", expected_point),
        ] {
            assert_eq!(
                variables.get(key).and_then(Value::as_str),
                Some(expected),
                "theme {theme} has incorrect {key}"
            );
        }
    }
}

#[test]
fn quadrant_partial_and_text_overrides_match_mermaid_11_16_replay_order() {
    let mut config = SourcePresentationConfig::from_value(json!({
        "theme": "base",
        "themeVariables": {
            "primaryTextColor": "#123456",
            "quadrant1Fill": "rgba(18, 52, 86, 0.5)",
            "quadrant2Fill": "#abcdef"
        }
    }));
    apply_theme_defaults(&mut config).unwrap();
    let variables = config
        .as_value()
        .get("themeVariables")
        .and_then(Value::as_object)
        .unwrap();

    for (key, expected) in [
        ("quadrant1Fill", "rgba(18, 52, 86, 0.5)"),
        ("quadrant2Fill", "#abcdef"),
        ("quadrant3Fill", "#fffee7"),
        ("quadrant1TextFill", "#123456"),
        ("quadrant2TextFill", "#0d2f51"),
        ("quadrantPointFill", "hsla(210, 65.3846153846%, NaN%, 0.5)"),
    ] {
        assert_eq!(variables.get(key).and_then(Value::as_str), Some(expected));
    }

    let mut default_config = SourcePresentationConfig::from_value(json!({
        "theme": "default",
        "themeVariables": { "primaryTextColor": "#123456" }
    }));
    apply_theme_defaults(&mut default_config).unwrap();
    assert_eq!(
        default_config.get_str("themeVariables.quadrant1TextFill"),
        Some("#131300")
    );
}

#[test]
fn quadrant_accepts_khroma_named_rgb_and_alpha_colors() {
    // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()`.
    let cases = [
        (
            "rebeccapurple",
            "#6b389e",
            "hsl(270, 50%, NaN%)",
            "hsl(270, 10%, 30%)",
        ),
        (
            "rgb(18, 52, 86)",
            "#17395b",
            "hsl(210, 65.3846153846%, NaN%)",
            "hsl(210, 25.3846153846%, 10.3921568627%)",
        ),
        (
            "rgba(18, 52, 86, 0.5)",
            "rgba(23, 57, 91, 0.5)",
            "hsla(210, 65.3846153846%, NaN%, 0.5)",
            "hsla(210, 25.3846153846%, 10.3921568627%, 0.5)",
        ),
    ];

    for (primary, expected_q2, expected_point, expected_border) in cases {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": primary }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        for (key, expected) in [
            ("quadrant2Fill", expected_q2),
            ("quadrantPointFill", expected_point),
            ("quadrantInternalBorderStrokeFill", expected_border),
        ] {
            assert_eq!(
                variables.get(key).and_then(Value::as_str),
                Some(expected),
                "primaryColor {primary} has incorrect {key}"
            );
        }
    }
}

#[test]
fn invalid_color_timing_matches_mermaid_11_16_initialize_matrix() {
    // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()` using
    // `not-a-color` for each field independently. Fields absent from `errors` are deliberate
    // pass-through values at theme-calculation time and must not be validated early.
    let keys = [
        "primaryColor",
        "secondaryColor",
        "tertiaryColor",
        "background",
        "primaryBorderColor",
        "secondaryBorderColor",
        "tertiaryBorderColor",
        "border1",
        "cScale0",
        "cScale1",
        "git0",
        "git1",
        "gitInv0",
        "pie1",
        "pie3",
        "quadrant1Fill",
    ];
    let cases: [(&str, &[&str]); 11] = [
        (
            "default",
            &[
                "primaryColor",
                "secondaryColor",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "dark",
            &[
                "primaryColor",
                "secondaryColor",
                "background",
                "cScale0",
                "cScale1",
                "quadrant1Fill",
            ],
        ),
        (
            "forest",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "neutral",
            &[
                "primaryColor",
                "secondaryColor",
                "border1",
                "cScale0",
                "cScale1",
                "quadrant1Fill",
            ],
        ),
        (
            "base",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "neo",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "neo-dark",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "redux",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "redux-dark",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "redux-color",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
        (
            "redux-dark-color",
            &[
                "primaryColor",
                "secondaryColor",
                "tertiaryColor",
                "background",
                "cScale0",
                "cScale1",
                "git0",
                "git1",
                "quadrant1Fill",
            ],
        ),
    ];

    let mut mismatches = Vec::new();
    for (theme, error_keys) in cases {
        for key in keys {
            let mut config = SourcePresentationConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { (key): "not-a-color" }
            }));
            let actual_error = apply_theme_defaults(&mut config).is_err();
            let expected_error = error_keys.contains(&key);
            if actual_error != expected_error {
                mismatches.push(format!(
                    "{theme}.{key}: expected error={expected_error}, actual={actual_error}"
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "invalid-color timing mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn invalid_theme_colors_fail_at_direct_and_site_config_operation_boundaries() {
    let mut config = SourcePresentationConfig::from_value(json!({
        "theme": "base",
        "themeVariables": { "primaryColor": "not-a-color" }
    }));
    assert!(matches!(
        apply_theme_defaults(&mut config),
        Err(ThemeResolutionError::Color(
            ColorError::UnsupportedFormat { .. }
        ))
    ));

    let engine =
        crate::MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "not-a-color" }
        })));
    assert!(matches!(
        engine.parse_metadata_sync("flowchart TD\n  A"),
        Err(crate::Error::ThemeColor(
            ColorError::UnsupportedFormat { .. }
        ))
    ));
}

#[test]
fn invalid_frontmatter_theme_color_fails_at_parse_operation_boundary() {
    let engine =
        crate::MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "secure": [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        })));
    let source = r#"---
config:
  theme: base
  themeVariables:
    primaryColor: not-a-color
---
flowchart TD
  A
"#;

    assert!(matches!(
        engine.parse_metadata_sync(source),
        Err(crate::Error::ThemeColor(
            ColorError::UnsupportedFormat { .. }
        ))
    ));
}

#[test]
fn theme_failure_leaves_values_and_authored_paths_intact() {
    let staged_cases = [("dark", "mainBkg"), ("base", "cScale2")];
    let other_cases = MermaidThemeId::ALL
        .iter()
        .filter(|theme| ThemeProgram::resolve(**theme).staged_program().is_none())
        .map(|theme| (theme.as_str(), "primaryColor"));
    for (theme, variable) in staged_cases.into_iter().chain(other_cases) {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": theme,
            "themeVariables": {"nodeBkg": null},
        }));
        config.set_value(&format!("themeVariables.{variable}"), json!("not-a-color"));
        let before = config.clone();
        let (result, work) =
            crate::config::measure_config_work(|| apply_theme_defaults(&mut config));
        assert!(
            result.is_err(),
            "{theme}: {variable} must reject the invalid color"
        );
        assert_eq!(config.as_value(), before.as_value(), "{theme}");
        assert_eq!(work.json_cow_copies, 0, "{theme}: {work:?}");
        assert!(config.explicit_config_owns_path(&format!("themeVariables.{variable}")));
        assert!(!config.explicit_config_owns_path("themeVariables.nodeBkg"));
    }
}

#[test]
fn staged_theme_color_limit_fails_closed_before_mutation() {
    for value in [json!(65), json!("0x41"), json!("Infinity")] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": {
                "THEME_COLOR_LIMIT": value,
                "nodeBkg": "#123456"
            }
        }));
        let before = config.as_value().clone();

        let error = apply_theme_defaults(&mut config).unwrap_err();
        let ThemeResolutionError::EvaluationLimit(error) = error else {
            panic!("expected a theme evaluation limit error");
        };
        assert_eq!(error.limit, "THEME_COLOR_LIMIT");
        assert_eq!(
            error.max,
            merman_model::presentation::MAX_THEME_COLOR_ITERATIONS
        );
        assert_eq!(config.as_value(), &before);
    }

    let engine =
        crate::MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": { "THEME_COLOR_LIMIT": 65 }
        })));
    assert!(matches!(
        engine.parse_metadata_sync("flowchart TD\n  A"),
        Err(crate::Error::ThemeEvaluationLimit(
            merman_model::presentation::ThemeEvaluationLimitExceeded {
                limit: "THEME_COLOR_LIMIT",
                max: merman_model::presentation::MAX_THEME_COLOR_ITERATIONS,
                ..
            }
        ))
    ));
}

#[test]
fn staged_object_dependencies_preserve_background_color() {
    let metadata = crate::MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": { "background": "#123456" }
        })))
        .parse_metadata_sync("flowchart TD\n  A")
        .expect("materialize a Dark theme with an explicit background");
    let config = metadata.effective_config;

    assert_eq!(
        config.get_str("themeVariables.xyChart.backgroundColor"),
        Some("#123456")
    );
}

#[test]
fn staged_theme_interface_matches_upstream_stage_oracles() {
    let mut mismatches = Vec::new();
    for case in &generated_theme_oracles().stage_oracle_cases {
        let theme_name = case.get("theme").and_then(Value::as_str).unwrap();
        let theme = MermaidThemeId::parse(theme_name).unwrap();
        let id = case.get("id").and_then(Value::as_str).unwrap();
        let program = *ThemeProgram::resolve(theme);
        let Some(staged_program) = program.staged_program() else {
            mismatches.push(format!(
                "{theme_name}/{id}: stage oracle targets a non-staged theme program"
            ));
            continue;
        };
        let raw = case
            .get("overrides")
            .and_then(Value::as_object)
            .unwrap()
            .clone();
        let explicit = program.normalize_overrides(raw);
        let resolution =
            staged::Resolution::execute(staged_program, program.prepared_constructor(), &explicit)
                .unwrap();
        let trace = resolution.trace();
        let actual_stages = [
            ("constructorPrepared", &trace.constructor_prepared),
            ("overridesApplied", &trace.overrides_applied),
            ("afterUpdate", &trace.after_update),
            ("explicitReplay", &trace.explicit_replay),
            ("finalResolved", &trace.final_resolved),
        ];
        let expected_stages = case.get("stages").and_then(Value::as_object).unwrap();
        for (stage_name, actual) in actual_stages {
            let expected = expected_stages
                .get(stage_name)
                .and_then(Value::as_object)
                .unwrap();
            let actual = Value::Object(actual.clone());
            for (path, observation) in expected {
                let state = observation.get("state").and_then(Value::as_str).unwrap();
                let actual_value = value_at_path(&actual, path);
                match state {
                    "missing" if actual_value.is_some() => mismatches.push(format!(
                        "{theme_name}/{id}/{stage_name}/{path}: expected missing, found {}",
                        actual_value.unwrap()
                    )),
                    "value" if actual_value != observation.get("value") => {
                        mismatches.push(format!(
                            "{theme_name}/{id}/{stage_name}/{path}: expected {}, found {}",
                            observation.get("value").unwrap(),
                            actual_value
                                .map(Value::to_string)
                                .unwrap_or_else(|| "missing".to_string())
                        ));
                    }
                    "missing" | "value" => {}
                    other => panic!("unexpected stage observation state `{other}`"),
                }
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "staged Mermaid theme mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn theme_resolution_records_stage_and_final_value_provenance() {
    let explicit = json!({"fontFamily": "Inter, sans-serif", "cScale0": "#abcdef"})
        .as_object()
        .unwrap()
        .clone();
    let calculated = json!({
        "fontFamily": "Inter, sans-serif",
        "cScale0": "hsl(210, 68%, 70.3921568627%)",
        "cScalePeer0": "hsl(210, 68%, 55.3921568627%)"
    })
    .as_object()
    .unwrap()
    .clone();
    let resolution =
        ThemeResolution::new(MermaidThemeId::Default, &explicit, || Ok(calculated)).unwrap();
    assert_eq!(
        resolution.default_snapshot.stage,
        ThemeResolutionStage::DefaultSnapshot
    );
    assert_eq!(
        resolution.overrides_applied.stage,
        ThemeResolutionStage::OverridesApplied
    );
    assert_eq!(
        resolution.calculated.stage,
        ThemeResolutionStage::Calculated
    );
    assert_eq!(
        resolution.explicit_replay.stage,
        ThemeResolutionStage::ExplicitReplay
    );
    for key in ["fontFamily", "cScale0"] {
        assert_eq!(
            resolution.explicit_replay.origins.get(key),
            Some(&ThemeValueOrigin::ExplicitOverride)
        );
    }
    assert_eq!(
        resolution.explicit_replay.origins.get("cScalePeer0"),
        Some(&ThemeValueOrigin::DefaultSnapshot)
    );
}

#[test]
fn default_theme_populates_mermaid_theme_variables() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "default"
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("background").and_then(|v| v.as_str()), Some("white"));
    assert_eq!(
        tv.get("primaryColor").and_then(|v| v.as_str()),
        Some("#ECECFF")
    );
    assert_eq!(
        tv.get("secondaryColor").and_then(|v| v.as_str()),
        Some("#ffffde")
    );
    assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
    assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
    assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#ECECFF"));
    assert_eq!(
        tv.get("nodeBorder").and_then(|v| v.as_str()),
        Some("#9370DB")
    );
    assert_eq!(
        tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
        Some("rgba(232,232,232, 0.8)")
    );
    assert_eq!(
        tv.get("classText").and_then(|v| v.as_str()),
        Some("#131300")
    );
    assert_eq!(
        tv.get("noteTextColor").and_then(|v| v.as_str()),
        Some("black")
    );
    assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        tv.get("gradientStart").and_then(|v| v.as_str()),
        Some("hsl(240, 60%, 86.2745098039%)")
    );

    let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
    assert_eq!(
        xy.get("backgroundColor").and_then(|v| v.as_str()),
        Some("white")
    );
    assert_eq!(
        xy.get("dataLabelColor").and_then(|v| v.as_str()),
        Some("#131300")
    );
}

#[test]
fn default_xychart_keeps_constructor_colors_until_its_object_is_replaced() {
    let program = *ThemeProgram::resolve(MermaidThemeId::Default);
    for (xy, title, sibling, background) in [
        (None, json!("#131300"), "#131300", "white"),
        (Some(json!({})), json!("#125634"), "#125634", "#142536"),
        (
            Some(json!({"titleColor":"#446688"})),
            json!("#446688"),
            "#125634",
            "#142536",
        ),
        (
            Some(json!({"titleColor":null})),
            Value::Null,
            "#125634",
            "#142536",
        ),
        (
            Some(json!({"titleColor":false})),
            json!(false),
            "#125634",
            "#142536",
        ),
    ] {
        let mut explicit = json!({"primaryTextColor":"#125634", "background":"#142536"});
        if let Some(xy) = xy {
            explicit["xyChart"] = xy;
        }
        let actual = program
            .resolve_legacy(explicit.as_object().unwrap())
            .unwrap();
        assert_eq!(actual["xyChart"]["titleColor"], title);
        assert_eq!(actual["xyChart"]["dataLabelColor"], sibling);
        assert_eq!(actual["xyChart"]["backgroundColor"], background);
    }
}

#[test]
fn default_xychart_object_replacement_uses_root_colors() {
    for replace_object in [false, true] {
        let mut site = json!({"themeVariables":{
            "primaryTextColor":"#131300", "background":"#142536"
        }});
        if replace_object {
            site["themeVariables"]["xyChart"] = json!({"titleColor":"#446688"});
        }
        let config = crate::MermaidEngine::new()
            .with_site_config(SourcePresentationConfig::from_value(site))
            .parse_metadata_sync("xychart\nx-axis [A]\nbar [4]\n")
            .unwrap()
            .effective_config;
        assert_eq!(
            config.get_str("themeVariables.xyChart.titleColor"),
            Some(if replace_object { "#446688" } else { "#131300" })
        );
        assert_eq!(
            config.get_str("themeVariables.xyChart.backgroundColor"),
            Some(if replace_object { "#142536" } else { "white" })
        );
    }
}

#[test]
fn default_theme_preserves_user_overrides_after_derivation() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "default",
        "themeVariables": {
            "primaryColor": "#111111",
            "mainBkg": "#101010",
            "classText": "#abcdef",
            "xyChart": {
                "titleColor": "red"
            }
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(
        tv.get("primaryColor").and_then(|v| v.as_str()),
        Some("#111111")
    );
    assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
    assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
    assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#101010"));
    assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#101010"));
    assert_eq!(
        tv.get("classText").and_then(|v| v.as_str()),
        Some("#abcdef")
    );
    assert_eq!(
        tv.get("primaryTextColor").and_then(|v| v.as_str()),
        Some("#131300")
    );

    let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
    assert_eq!(xy.get("titleColor").and_then(|v| v.as_str()), Some("red"));
    // applyOverride keeps updateColors' generated keys when the user supplies only a title.
    assert_eq!(xy.get("dataLabelColor"), tv.get("primaryTextColor"));
    assert!(xy.get("plotColorPalette").is_some_and(Value::is_string));
}

#[test]
fn default_theme_merges_unrelated_theme_variable_overrides_without_hsl_rewriting_pie_base() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "default",
        "themeVariables": {
            "pieOuterStrokeWidth": "5px"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
    assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
    assert_eq!(
        tv.get("pieOuterStrokeWidth").and_then(|v| v.as_str()),
        Some("5px")
    );
}

#[test]
fn raw_mermaid_config_preserves_upstream_unsupported_theme_fallback() {
    for requested in ["unknown", "null"] {
        let mut config = SourcePresentationConfig::from_value(json!({
            "theme": requested
        }));
        assert!(MermaidThemeId::parse(requested).is_err());
        apply_theme_defaults(&mut config).unwrap();

        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        assert_eq!(
            variables,
            ThemeProgram::resolve(MermaidThemeId::Default).resolved_without_overrides()
        );
        assert_eq!(config.get_str("theme"), Some(requested));
    }
}

#[test]
fn mermaid_11_16_extended_theme_names_use_their_snapshots() {
    let cases = [
        ("neo", "#cccccc", "#000000"),
        ("neo-dark", "#1f2020", "#ccc"),
        ("redux", "#cccccc", "#28253D"),
        ("redux-dark", "#1f2020", "#FFFFFF"),
        ("redux-color", "#cccccc", "#28253D"),
        ("redux-dark-color", "#1f2020", "#FFFFFF"),
    ];

    for (theme, primary, node_border) in cases {
        let mut cfg = SourcePresentationConfig::from_value(json!({
            "theme": theme
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some(primary),
            "theme {theme}"
        );
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some(node_border),
            "theme {theme}"
        );
    }
}

#[test]
fn extended_theme_preserves_explicit_theme_variable_overrides() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux",
        "themeVariables": {
            "primaryColor": "#123456",
            "nodeBorder": "#abcdef"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(
        tv.get("primaryColor").and_then(|v| v.as_str()),
        Some("#123456")
    );
    assert_eq!(
        tv.get("nodeBorder").and_then(|v| v.as_str()),
        Some("#abcdef")
    );
    assert_eq!(
        tv.get("fontFamily").and_then(|v| v.as_str()),
        Some("\"Recursive Variable\", arial, sans-serif")
    );
}

#[test]
fn extended_theme_recomputes_visible_derivations_from_base_overrides() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux",
        "themeVariables": {
            "primaryColor": "#123456"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(
        tv.get("primaryColor").and_then(|v| v.as_str()),
        Some("#123456")
    );
    assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#123456"));
    assert_eq!(
        tv.get("secondaryColor").and_then(|v| v.as_str()),
        Some("hsl(90, 65.3846153846%, 20.3921568627%)")
    );
    assert_eq!(
        tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
        Some("hsl(90, 65.3846153846%, 20.3921568627%)")
    );
    assert_eq!(
        tv.get("tagLabelBackground").and_then(|v| v.as_str()),
        Some("#123456")
    );
    assert_eq!(
        tv.get("fontFamily").and_then(|v| v.as_str()),
        Some("\"Recursive Variable\", arial, sans-serif")
    );
    assert_eq!(
        tv.get("git0").and_then(|v| v.as_str()),
        Some("hsl(240, 90%, 71.0784313725%)")
    );
}

#[test]
fn extended_theme_explicit_derived_overrides_still_win() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux",
        "themeVariables": {
            "primaryColor": "#123456",
            "nodeBkg": "#abcdef",
            "edgeLabelBackground": "#fedcba"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#abcdef"));
    assert_eq!(
        tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
        Some("#fedcba")
    );
    assert_eq!(
        tv.get("secondaryColor").and_then(|v| v.as_str()),
        Some("hsl(90, 65.3846153846%, 20.3921568627%)")
    );
}

#[test]
fn extended_theme_recomputes_background_and_main_background_derivations() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux",
        "themeVariables": {
            "background": "#010203",
            "mainBkg": "#101112"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    for key in [
        "lineColor",
        "arrowheadColor",
        "defaultLinkColor",
        "archEdgeColor",
        "archEdgeArrowColor",
        "relationColor",
        "transitionColor",
        "specialStateColor",
    ] {
        assert_eq!(
            tv.get(key).and_then(|v| v.as_str()),
            Some("#fefdfc"),
            "key {key}"
        );
    }

    for key in [
        "actorBkg",
        "labelBoxBkgColor",
        "personBkg",
        "stateBkg",
        "labelBackgroundColor",
    ] {
        assert_eq!(
            tv.get(key).and_then(|v| v.as_str()),
            Some("#101112"),
            "key {key}"
        );
    }
    assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#cccccc"));
}

#[test]
fn dark_extended_theme_recomputes_primary_visible_derivations() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux-dark",
        "themeVariables": {
            "primaryColor": "#123456"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    for key in ["requirementBackground", "pie1", "quadrant1Fill"] {
        assert_eq!(
            tv.get(key).and_then(|v| v.as_str()),
            Some("#123456"),
            "key {key}"
        );
    }
    assert_eq!(
        tv.get("git0").and_then(|v| v.as_str()),
        Some("hsl(210, 65.3846153846%, 0%)")
    );
    assert_eq!(
        tv.get("git1").and_then(|v| v.as_str()),
        Some("hsl(180, 1.5873015873%, 3.3529411765%)")
    );
    assert_eq!(
        tv.get("git3").and_then(|v| v.as_str()),
        Some("hsl(180, 65.3846153846%, 0%)")
    );
    assert_eq!(tv.get("gitInv0").and_then(|v| v.as_str()), Some("#ffffff"));
    assert_eq!(
        tv.get("gitInv1").and_then(|v| v.as_str()),
        Some("rgb(246.5857142856, 246.3142857142, 246.3142857142)")
    );
}

#[test]
fn extended_theme_explicit_git_color_derives_git_inverse_unless_explicit() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "redux",
        "themeVariables": {
            "git0": "#000000",
            "git1": "#111111",
            "gitInv1": "#222222"
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("git0").and_then(|v| v.as_str()), Some("#000000"));
    assert_eq!(tv.get("gitInv0").and_then(|v| v.as_str()), Some("#ffffff"));
    assert_eq!(tv.get("git1").and_then(|v| v.as_str()), Some("#111111"));
    assert_eq!(tv.get("gitInv1").and_then(|v| v.as_str()), Some("#222222"));
}

#[test]
fn base_theme_derivation_matches_upstream_fixture_values() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "base",
        "themeVariables": {
            "primaryColor": "#411d4e",
            "titleColor": "white",
            "darkMode": true
        }
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("textColor").and_then(|v| v.as_str()), Some("#eee"));
    assert_eq!(
        tv.get("lineColor").and_then(|v| v.as_str()),
        Some("#0b0b0b")
    );
    assert_eq!(
        tv.get("nodeBorder").and_then(|v| v.as_str()),
        Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
    );
    assert_eq!(
        tv.get("secondaryBorderColor").and_then(|v| v.as_str()),
        Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
    );
    assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        tv.get("gradientStart").and_then(|v| v.as_str()),
        Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
    );
    assert_eq!(
        tv.get("gradientStop").and_then(|v| v.as_str()),
        Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
    );
    assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#411d4e"));
    assert_eq!(
        tv.get("clusterBkg").and_then(|v| v.as_str()),
        Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
    );
    assert_eq!(
        tv.get("clusterBorder").and_then(|v| v.as_str()),
        Some("hsl(104.0816326531, 5.7943925234%, 35.9803921569%)")
    );
    assert_eq!(
        tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
        Some("hsl(164.0816326531, 45.7943925234%, 0%)")
    );
    assert_eq!(
        tv.get("errorBkgColor").and_then(|v| v.as_str()),
        Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
    );
    assert_eq!(
        tv.get("errorTextColor").and_then(|v| v.as_str()),
        Some("rgb(202.9906542056, 158.4112149531, 219.0887850467)")
    );
    assert_eq!(tv.get("titleColor").and_then(|v| v.as_str()), Some("white"));
}

#[test]
fn forest_theme_derives_cscale_palette_like_upstream() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "forest"
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(
        tv.get("cScale0").and_then(|v| v.as_str()),
        Some("hsl(78.1578947368, 58.4615384615%, 64.5098039216%)")
    );
    assert_eq!(
        tv.get("cScalePeer0").and_then(|v| v.as_str()),
        Some("hsl(78.1578947368, 58.4615384615%, 39.5098039216%)")
    );
    assert_eq!(
        tv.get("cScalePeer1").and_then(|v| v.as_str()),
        Some("hsl(98.961038961, 100%, 39.9019607843%)")
    );
    assert_eq!(
        tv.get("cScalePeer2").and_then(|v| v.as_str()),
        Some("hsl(78.1578947368, 58.4615384615%, 44.5098039216%)")
    );
    assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        tv.get("gradientStart").and_then(|v| v.as_str()),
        Some("hsl(78.1578947368, 18.4615384615%, 64.5098039216%)")
    );
    assert_eq!(
        tv.get("gradientStop").and_then(|v| v.as_str()),
        Some("hsl(98.961038961, 60%, 74.9019607843%)")
    );
}

#[test]
fn dark_theme_derives_peer_and_inverted_scales_like_upstream() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "dark"
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("cScale1").and_then(|v| v.as_str()), Some("#0b0000"));
    assert_eq!(
        tv.get("cScalePeer1").and_then(|v| v.as_str()),
        Some("hsl(0, 100%, 12.1568627451%)")
    );
    assert_eq!(
        tv.get("cScaleInv1").and_then(|v| v.as_str()),
        Some("#f4ffff")
    );
    assert_eq!(
        tv.get("cScaleLabel1").and_then(|v| v.as_str()),
        Some("lightgrey")
    );
    assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        tv.get("gradientStart").and_then(|v| v.as_str()),
        Some("#cccccc")
    );
    assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#1f2020"));
    assert_eq!(
        tv.get("lineColor").and_then(|v| v.as_str()),
        Some("lightgrey")
    );
    assert_eq!(
        tv.get("actorTextColor").and_then(|v| v.as_str()),
        Some("lightgrey")
    );
    assert_eq!(
        tv.get("classText").and_then(|v| v.as_str()),
        Some("#e0dfdf")
    );
    assert_eq!(
        tv.get("noteTextColor").and_then(|v| v.as_str()),
        Some("rgb(183.8476190475, 181.5523809523, 181.5523809523)")
    );
    assert_eq!(
        tv.get("taskTextDarkColor").and_then(|v| v.as_str()),
        Some("#2c2c2c")
    );
    assert_eq!(
        tv.get("attributeBackgroundColorOdd")
            .and_then(|v| v.as_str()),
        Some("hsl(0, 0%, 32%)")
    );
}

#[test]
fn neutral_theme_derives_peer_and_label_scales_like_upstream() {
    let mut cfg = SourcePresentationConfig::from_value(json!({
        "theme": "neutral"
    }));
    apply_theme_defaults(&mut cfg).unwrap();

    let tv = cfg
        .as_value()
        .get("themeVariables")
        .and_then(|v| v.as_object())
        .unwrap();

    assert_eq!(tv.get("cScale0").and_then(|v| v.as_str()), Some("#555"));
    assert_eq!(
        tv.get("cScalePeer0").and_then(|v| v.as_str()),
        Some("hsl(0, 0%, 23.3333333333%)")
    );
    assert_eq!(
        tv.get("cScaleInv0").and_then(|v| v.as_str()),
        Some("#aaaaaa")
    );
    assert_eq!(
        tv.get("cScaleLabel0").and_then(|v| v.as_str()),
        Some("#F4F4F4")
    );
    assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        tv.get("gradientStart").and_then(|v| v.as_str()),
        Some("hsl(0, 0%, 83.3333333333%)")
    );
    assert_eq!(
        tv.get("gradientStop").and_then(|v| v.as_str()),
        Some("hsl(0, 0%, 88.9215686275%)")
    );
    assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#eee"));
    assert_eq!(
        tv.get("textColor").and_then(|v| v.as_str()),
        Some("#000000")
    );
    assert_eq!(
        tv.get("actorTextColor").and_then(|v| v.as_str()),
        Some("#333")
    );
    assert_eq!(
        tv.get("classText").and_then(|v| v.as_str()),
        Some("#111111")
    );
    assert_eq!(
        tv.get("noteBkgColor").and_then(|v| v.as_str()),
        Some("#666")
    );
    assert_eq!(
        tv.get("noteTextColor").and_then(|v| v.as_str()),
        Some("#fff")
    );
    assert_eq!(
        tv.get("taskTextOutsideColor").and_then(|v| v.as_str()),
        Some("#333")
    );
}
