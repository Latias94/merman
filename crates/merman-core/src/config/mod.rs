#[cfg(feature = "presentation")]
pub(crate) mod presentation;

use crate::json_value::drop_value_nonrecursive;
use crate::{OperationControl, OperationControlResult};
pub use merman_model::config::MermaidConfig;
pub(crate) use merman_model::config::replace_value_nonrecursive;
#[cfg(feature = "presentation")]
use merman_model::config::values_equal_controlled as config_values_equal;
use serde_json::{Map, Value};

#[cfg(feature = "presentation")]
mod appearance;
mod appearance_selection;
mod source_presentation;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use appearance::resolve_appearance;
#[cfg(feature = "presentation")]
pub(crate) use appearance::{AppearanceDecision, MaterializationPlan};
pub(crate) use appearance_selection::resolve_semantic_appearance;
pub(crate) use source_presentation::is_presentation_field;

/// Test-only observations of actual JSON copy-on-write and operation appearance work.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConfigWork {
    pub(crate) json_cow_copies: usize,
    pub(crate) appearance_selections: usize,
    pub(crate) selected_theme_materializations: usize,
    pub(crate) detector_replay_steps: usize,
    pub(crate) detector_replay_peak_frames: usize,
    pub(crate) detector_replay_peak_path_segments: usize,
    pub(crate) explicit_path_candidates: usize,
}

#[cfg(test)]
thread_local! {
    static CONFIG_WORK: std::cell::RefCell<Vec<ConfigWork>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn record_config_work(mut record: impl FnMut(&mut ConfigWork)) {
    CONFIG_WORK.with(|scopes| {
        for scope in scopes.borrow_mut().iter_mut() {
            record(scope);
        }
    });
}

#[cfg(test)]
pub(crate) fn measure_config_work<T>(operation: impl FnOnce() -> T) -> (T, ConfigWork) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            CONFIG_WORK.with(|scopes| {
                scopes.borrow_mut().pop();
            });
        }
    }
    CONFIG_WORK.with(|scopes| scopes.borrow_mut().push(ConfigWork::default()));
    let scope = Scope;
    let (result, json_cow_copies) =
        merman_model::config::test_support::measure_cow_copies(operation);
    let mut work =
        CONFIG_WORK.with(|scopes| *scopes.borrow().last().expect("active config work scope"));
    work.json_cow_copies = json_cow_copies;
    drop(scope);
    (result, work)
}

pub(crate) const HARDENED_SECURE_KEYS: &[&str] = &[
    "secure",
    "securityLevel",
    "startOnLoad",
    "maxTextSize",
    "suppressErrorRendering",
    "maxEdges",
    "themeCSS",
];

pub(crate) fn apply_hardened_site_policy(config: &mut MermaidConfig) {
    config.set_value(
        "secure",
        Value::Array(
            HARDENED_SECURE_KEYS
                .iter()
                .map(|key| Value::String((*key).to_string()))
                .collect(),
        ),
    );
}

pub(crate) fn authored_source_overrides(
    authority: &MermaidConfig,
    mut overrides: MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<MermaidConfig> {
    control.checkpoint()?;
    if overrides.is_empty_object() {
        return Ok(overrides);
    }
    remove_secure_keys_recursive(authority.as_value(), overrides.as_value_mut());
    control.checkpoint()?;
    Ok(overrides)
}

pub(crate) fn source_filtered_overrides(
    authority: &MermaidConfig,
    overrides: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<MermaidConfig> {
    let mut filtered = overrides.clone();
    // Normalize legacy typography before authority checks so a locked themeVariables or
    // fontFamily key cannot be reintroduced through the other spelling after filtering.
    mirror_legacy_font_family_into_theme_variables(&mut filtered);
    remove_secure_keys_recursive(authority.as_value(), filtered.as_value_mut());
    source_presentation::filter(filtered.as_value_mut(), control)?;
    Ok(filtered)
}

fn remove_secure_keys_recursive(site_config: &Value, overrides: &mut Value) {
    let secure_keys = site_config
        .get("secure")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut stack = vec![overrides];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    drop_value_nonrecursive(old);
                }
                for key in &secure_keys {
                    if let Some(old) = map.remove(*key) {
                        drop_value_nonrecursive(old);
                    }
                }
                for child in map.values_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Array(items) => {
                for child in items.iter_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables(config: &mut MermaidConfig) {
    let value = config.as_value_mut();
    mirror_legacy_font_family_into_theme_variables_value(value);
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables_value(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(font_family) = root
        .get("fontFamily")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    else {
        return;
    };

    let has_theme_font_family = root
        .get("themeVariables")
        .and_then(Value::as_object)
        .and_then(|theme_variables| theme_variables.get("fontFamily"))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if has_theme_font_family {
        return;
    }

    let theme_variables = root
        .entry("themeVariables")
        .or_insert_with(|| Value::Object(Map::new()));
    if !theme_variables.is_object() {
        replace_value_nonrecursive(theme_variables, Value::Object(Map::new()));
    }
    if let Some(theme_variables) = theme_variables.as_object_mut()
        && let Some(old) =
            theme_variables.insert("fontFamily".to_string(), Value::String(font_family))
    {
        drop_value_nonrecursive(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mirror_legacy_font_family_populates_missing_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier"
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Courier"));
    }

    #[test]
    fn mirror_legacy_font_family_preserves_explicit_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier",
            "themeVariables": {
                "fontFamily": "Inter"
            }
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Inter"));
    }

    #[test]
    fn upstream_secure_key_list_matches_mermaid_runtime() {
        let upstream = crate::generated::upstream_default_config();
        let secure = upstream
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("upstream secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(
            secure,
            [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        );
    }

    #[test]
    fn default_site_config_applies_hardened_secure_policy() {
        let default = crate::generated::default_site_config();
        let secure = default
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("hardened secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(secure, HARDENED_SECURE_KEYS);
    }

    #[test]
    fn secure_filtered_overrides_removes_default_secure_keys_recursively() {
        let site_config = crate::generated::default_site_config();
        let overrides = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "fontFamily": "diagram-font",
            "flowchart": {
                "securityLevel": "sandbox",
                "htmlLabels": false,
                "nested": [
                    {
                        "securityLevel": "loose",
                        "shape": "rect"
                    }
                ]
            }
        }));

        let filtered =
            source_filtered_overrides(&site_config, &overrides, &OperationControl::new()).unwrap();

        assert_eq!(filtered.get_str("fontFamily"), Some("diagram-font"));
        assert_eq!(filtered.get_bool("flowchart.htmlLabels"), Some(false));
        assert_eq!(
            filtered.as_value()["flowchart"]["nested"][0]["shape"],
            json!("rect")
        );
        assert!(filtered.get_str("securityLevel").is_none());
        assert!(filtered.get_str("flowchart.securityLevel").is_none());
        assert!(
            filtered.as_value()["flowchart"]["nested"][0]
                .get("securityLevel")
                .is_none()
        );
    }

    #[test]
    fn secure_filtered_overrides_keeps_the_merged_secure_policy() {
        let mut site_config = crate::generated::default_site_config();
        site_config.deep_merge(&json!({
            "secure": ["fontSize"]
        }));
        let overrides = MermaidConfig::from_value(json!({
            "secure": ["theme"],
            "fontSize": 99,
            "securityLevel": "loose",
            "theme": "dark"
        }));

        let filtered =
            source_filtered_overrides(&site_config, &overrides, &OperationControl::new()).unwrap();

        assert!(filtered.as_value().get("secure").is_none());
        assert!(filtered.as_value().get("fontSize").is_none());
        assert!(filtered.get_str("securityLevel").is_none());
        assert_eq!(filtered.get_str("theme"), Some("dark"));
    }
}
