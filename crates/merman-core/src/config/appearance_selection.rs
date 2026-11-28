use crate::{MermaidConfig, MermaidThemeId, OperationControl, OperationControlResult, family};
use serde_json::Value;
use std::borrow::Cow;

pub(super) const APPEARANCE_KEYS: [&str; 3] = ["theme", "look", "layout"];

#[derive(Clone, Copy)]
pub(super) enum AppearanceOrigin {
    Source,
    Initialize,
    Default,
}

pub(super) fn select_appearance_value<'a>(
    section: &str,
    key: &str,
    source: &'a MermaidConfig,
    initialize: &'a MermaidConfig,
    defaults: &'a MermaidConfig,
) -> Option<(Cow<'a, Value>, AppearanceOrigin)> {
    // MindmapDB asks whether user configuration supplied the root layout key before reading
    // the effective layout. A family-only layout leaves its CoSE-Bilkent default in place.
    if section == "mindmap"
        && key == "layout"
        && source.get(key).is_none()
        && initialize.get(key).is_none()
    {
        return Some((
            Cow::Owned(Value::String("cose-bilkent".to_owned())),
            AppearanceOrigin::Default,
        ));
    }
    [
        (source, AppearanceOrigin::Source),
        (initialize, AppearanceOrigin::Initialize),
    ]
    .into_iter()
    .find_map(|(layer, origin)| {
        read_appearance(layer, section, key).map(|value| (Cow::Borrowed(value), origin))
    })
    .or_else(|| {
        read_appearance(defaults, section, key)
            .map(|value| (Cow::Borrowed(value), AppearanceOrigin::Default))
    })
}

/// Selects the identifiers used by semantic consumers without evaluating a theme.
pub(crate) fn resolve_semantic_appearance(
    diagram_type: &str,
    source: &MermaidConfig,
    initialize: &MermaidConfig,
    defaults: &MermaidConfig,
    effective: &mut MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let section = family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type);
    for key in APPEARANCE_KEYS {
        control.checkpoint()?;
        let Some((value, _)) = select_appearance_value(section, key, source, initialize, defaults)
        else {
            continue;
        };
        let value = value.as_ref();
        let section_has_key = effective
            .as_value()
            .get(section)
            .and_then(|section| section.get(key))
            .is_some();
        if effective.as_value().get(key) != Some(value) {
            effective.set_value(
                key,
                crate::json_value::clone_value_nonrecursive_with_control(value, control)?,
            );
        }
        if section_has_key && effective.as_value()[section].get(key) != Some(value) {
            effective.set_value(
                &format!("{section}.{key}"),
                crate::json_value::clone_value_nonrecursive_with_control(value, control)?,
            );
        }
    }
    control.checkpoint()
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
        // Unknown names and non-string values remain the layout registry's responsibility.
        "layout" => true,
        _ => false,
    })
}

/// Reports whether a source-authored config selects a Mermaid theme for this diagram type.
/// Pass the parse result's authored `config` rather than its effective config.
pub fn source_declares_theme(config: &MermaidConfig, diagram_type: &str) -> bool {
    let section = family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type);
    read_appearance(config, section, "theme").is_some()
}
