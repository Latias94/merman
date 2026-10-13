//! Resolve diagram appearance while the source and initialization layers are still distinct.

use crate::{MermaidConfig, MermaidThemeId, family};
use serde_json::Value;
use std::borrow::Cow;

const APPEARANCE_KEYS: [&str; 3] = ["theme", "look", "layout"];

#[derive(Clone, Copy)]
enum AppearanceOrigin {
    Source,
    Initialize,
    Compatibility,
    Default,
}

struct AppearanceWinner<'a> {
    value: Cow<'a, Value>,
    origin: AppearanceOrigin,
}

/// A pure selection from authored layers. Applying it never re-reads or collapses those layers.
pub(crate) struct AppearanceDecision<'a> {
    section: &'a str,
    winners: [Option<AppearanceWinner<'a>>; 3],
    source_root_theme_is_truthy: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MaterializationPlan {
    InheritInitialized,
    Materialize { selected: MermaidThemeId },
}

impl<'a> AppearanceDecision<'a> {
    pub(crate) fn resolve(
        diagram_type: &'a str,
        source: &'a MermaidConfig,
        initialize: &'a MermaidConfig,
        compatibility: Option<&'a MermaidConfig>,
        defaults: &'a MermaidConfig,
    ) -> Self {
        #[cfg(test)]
        super::record_config_work(|work| work.appearance_selections += 1);
        let section =
            family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type);
        let winners = APPEARANCE_KEYS.map(|key| {
            [
                (source, AppearanceOrigin::Source),
                (initialize, AppearanceOrigin::Initialize),
            ]
            .into_iter()
            .find_map(|(layer, origin)| {
                read_appearance(layer, section, key).map(|value| AppearanceWinner {
                    value: Cow::Borrowed(value),
                    origin,
                })
            })
            .or_else(|| {
                compatibility
                    .and_then(|layer| read_appearance(layer, section, key))
                    .map(|value| AppearanceWinner {
                        value: Cow::Borrowed(value),
                        origin: AppearanceOrigin::Compatibility,
                    })
            })
            // Mindmap chooses Cose only in the absence of an authored layout, including null.
            .or_else(|| {
                (section == "mindmap" && key == "layout").then(|| AppearanceWinner {
                    value: Cow::Owned(Value::String("cose-bilkent".to_string())),
                    origin: AppearanceOrigin::Default,
                })
            })
            .or_else(|| {
                read_appearance(defaults, section, key).map(|value| AppearanceWinner {
                    value: Cow::Borrowed(value),
                    origin: AppearanceOrigin::Default,
                })
            })
        });
        Self {
            section,
            winners,
            source_root_theme_is_truthy: source
                .as_value()
                .get("theme")
                .is_some_and(crate::theme::is_js_truthy),
        }
    }

    pub(crate) fn materialization_plan(
        &self,
        current_theme: Option<&str>,
        site_theme: Option<&str>,
    ) -> MaterializationPlan {
        let resolved_theme = self.winners[0]
            .as_ref()
            .and_then(|winner| winner.value.as_str())
            .or(current_theme);
        // Mermaid evaluates root directive truthiness independently of appearance precedence.
        // A scoped same-value source winner still owns appearance without forcing recalculation.
        if resolved_theme == Some("null")
            || (!self.source_root_theme_is_truthy && resolved_theme == site_theme)
        {
            MaterializationPlan::InheritInitialized
        } else {
            MaterializationPlan::Materialize {
                selected: MermaidThemeId::parse(
                    resolved_theme.expect("appearance resolves a registered theme"),
                )
                .expect("appearance validates registered themes"),
            }
        }
    }

    pub(crate) fn apply_to(&self, effective: &mut MermaidConfig) {
        for (key, winner) in APPEARANCE_KEYS.into_iter().zip(&self.winners) {
            let Some(winner) = winner else {
                continue;
            };
            let value = winner.value.as_ref();
            let explicit = matches!(
                winner.origin,
                AppearanceOrigin::Source | AppearanceOrigin::Initialize
            );
            let scoped_key = format!("{}.{key}", self.section);
            // The detector may add or delete scoped keys between applications of the same decision.
            let section_has_key = effective
                .as_value()
                .get(self.section)
                .and_then(|section| section.get(key))
                .is_some();
            if explicit {
                // Equal values can still acquire a different owner.
                effective.set_value_explicit(key, value.clone());
            } else if effective.as_value().get(key) != Some(value) {
                effective.set_value_preserving_theme_compatibility(key, value.clone());
            }
            if section_has_key {
                if explicit {
                    effective.set_value_explicit(&scoped_key, value.clone());
                } else if effective.as_value()[self.section].get(key) != Some(value) {
                    effective.set_value_preserving_theme_compatibility(&scoped_key, value.clone());
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn resolve_appearance(
    diagram_type: &str,
    source: &MermaidConfig,
    initialize: &MermaidConfig,
    compatibility: Option<&MermaidConfig>,
    defaults: &MermaidConfig,
    effective: &mut MermaidConfig,
) {
    let decision =
        AppearanceDecision::resolve(diagram_type, source, initialize, compatibility, defaults);
    decision.apply_to(effective);
}

fn read_appearance<'a>(layer: &'a MermaidConfig, section: &str, key: &str) -> Option<&'a Value> {
    let root = layer.as_value();
    [
        root.get(section).and_then(|section| section.get(key)),
        root.get(key),
    ]
    .into_iter()
    .flatten()
    .find(|value| match key {
        "theme" => value
            .as_str()
            .is_some_and(|name| name == "null" || MermaidThemeId::NAMES.contains(&name)),
        "look" => value
            .as_str()
            .is_some_and(|look| matches!(look, "classic" | "handDrawn" | "neo")),
        // The extensible layout registry owns invalid/unknown-name fallback. In
        // particular a JSON null is present in JavaScript, unlike undefined.
        "layout" => true,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolve(initialize: Value, source: Value) -> MermaidConfig {
        let defaults = MermaidConfig::from_value(json!({
            "theme": "default", "look": "classic", "layout": "elk",
            "flowchart": { "theme": "redux-color", "look": "neo" },
        }));
        let initialize = MermaidConfig::from_value(initialize);
        let source = MermaidConfig::from_value(source);
        let mut effective = defaults.clone();
        effective.deep_merge(initialize.as_value());
        effective.deep_merge(source.as_value());
        resolve_appearance(
            "flowchart-v2",
            &source,
            &initialize,
            None,
            &defaults,
            &mut effective,
        );
        effective
    }

    #[test]
    fn one_decision_rechecks_scoped_presence_and_keeps_equal_value_ownership() {
        let source = MermaidConfig::from_value(json!({"theme": "dark", "layout": null}));
        let empty = MermaidConfig::empty_object();
        let defaults = MermaidConfig::from_value(json!({"theme": "default", "layout": "dagre"}));
        let decision =
            AppearanceDecision::resolve("flowchart-v2", &source, &empty, None, &defaults);
        let mut effective = MermaidConfig::from_value(json!({"theme": "dark"}));
        decision.apply_to(&mut effective);
        assert!(effective.explicit_config_owns_path("theme"));
        assert!(effective.explicit_config_owns_path("layout"));
        assert_eq!(effective.as_value()["layout"], Value::Null);
        assert!(effective.as_value().get("flowchart").is_none());

        effective.set_value("flowchart.theme", json!("forest"));
        decision.apply_to(&mut effective);
        assert_eq!(effective.get_str("flowchart.theme"), Some("dark"));
        assert!(effective.explicit_config_owns_path("flowchart.theme"));
        assert!(effective.as_value()["flowchart"].get("layout").is_none());
    }

    #[test]
    fn materialization_plan_preserves_null_and_scoped_selection_rules() {
        let empty = MermaidConfig::empty_object();
        let defaults = MermaidConfig::from_value(json!({"theme": "dark"}));
        for (source, expected) in [
            (json!({}), MaterializationPlan::InheritInitialized),
            (
                json!({"theme": null}),
                MaterializationPlan::InheritInitialized,
            ),
            (
                json!({"theme": false}),
                MaterializationPlan::InheritInitialized,
            ),
            (
                json!({"flowchart": {"theme": "dark"}}),
                MaterializationPlan::InheritInitialized,
            ),
            (
                json!({"theme": "unknown"}),
                MaterializationPlan::Materialize {
                    selected: MermaidThemeId::Dark,
                },
            ),
            (
                json!({"theme": "null"}),
                MaterializationPlan::InheritInitialized,
            ),
            (
                json!({"theme": "dark"}),
                MaterializationPlan::Materialize {
                    selected: MermaidThemeId::Dark,
                },
            ),
            (
                json!({"theme": "base", "flowchart": {"theme": "forest"}}),
                MaterializationPlan::Materialize {
                    selected: MermaidThemeId::Forest,
                },
            ),
        ] {
            let source = MermaidConfig::from_value(source);
            let decision =
                AppearanceDecision::resolve("flowchart-v2", &source, &empty, None, &defaults);
            assert_eq!(
                decision.materialization_plan(Some("dark"), Some("dark")),
                expected
            );
        }
        let initialize = MermaidConfig::from_value(json!({"flowchart": {"theme": "forest"}}));
        let decision =
            AppearanceDecision::resolve("flowchart-v2", &empty, &initialize, None, &defaults);
        assert_eq!(
            decision.materialization_plan(Some("dark"), Some("dark")),
            MaterializationPlan::Materialize {
                selected: MermaidThemeId::Forest,
            }
        );
    }

    #[test]
    fn appearance_precedence_preserves_layers_and_explicit_defaults() {
        for (initialize, source, expected) in [
            (json!({}), json!({}), "redux-color"),
            (json!({ "theme": "default" }), json!({}), "default"),
            (
                json!({ "theme": "dark", "flowchart": { "theme": "forest" } }),
                json!({}),
                "forest",
            ),
            (
                json!({ "flowchart": { "theme": "forest" } }),
                json!({ "theme": "dark" }),
                "dark",
            ),
            (
                json!({ "theme": "dark" }),
                json!({ "theme": "forest", "flowchart": { "theme": "base" } }),
                "base",
            ),
        ] {
            let effective = resolve(initialize, source);
            assert_eq!(effective.get_str("theme"), Some(expected));
            assert_eq!(effective.get_str("flowchart.theme"), Some(expected));
        }
    }

    #[test]
    fn unusable_values_fall_through_without_rejecting_the_null_sentinel() {
        for invalid in [
            json!(null),
            json!(false),
            json!("constructor"),
            json!("bogus"),
        ] {
            let effective = resolve(
                json!({ "theme": "dark" }),
                json!({
                    "theme": "forest", "flowchart": { "theme": invalid, "look": "bogus" }
                }),
            );
            assert_eq!(effective.get_str("theme"), Some("forest"));
            assert_eq!(effective.get_str("look"), Some("neo"));
        }
        let effective = resolve(json!({}), json!({ "flowchart": { "theme": "null" } }));
        assert_eq!(effective.get_str("theme"), Some("null"));
    }

    #[test]
    fn mindmap_default_does_not_override_an_authored_layout() {
        for (initialize, source, expected) in [
            (json!({}), json!({}), json!("cose-bilkent")),
            (json!({"layout": "elk"}), json!({}), json!("elk")),
            (
                json!({}),
                json!({"layout": "tidy-tree"}),
                json!("tidy-tree"),
            ),
            (
                json!({"layout": "elk"}),
                json!({"layout": null}),
                Value::Null,
            ),
            (
                json!({}),
                json!({"mindmap": {"layout": "dagre"}}),
                json!("dagre"),
            ),
        ] {
            let defaults = MermaidConfig::from_value(json!({"layout": "elk"}));
            let mut effective = defaults.clone();
            let initialize = MermaidConfig::from_value(initialize);
            let source = MermaidConfig::from_value(source);
            effective.deep_merge(initialize.as_value());
            effective.deep_merge(source.as_value());
            resolve_appearance(
                "mindmap",
                &source,
                &initialize,
                None,
                &defaults,
                &mut effective,
            );
            assert_eq!(effective.as_value()["layout"], expected);
        }
    }

    #[test]
    fn layout_values_reach_the_registry_without_appearance_validation() {
        for layout in [json!("custom-layout"), json!(null), json!(false)] {
            let effective = resolve(
                json!({ "layout": "dagre" }),
                json!({
                    "flowchart": { "layout": layout }
                }),
            );
            assert_eq!(effective.as_value()["layout"], layout);
            assert_eq!(effective.as_value()["flowchart"]["layout"], layout);
        }
    }
}
