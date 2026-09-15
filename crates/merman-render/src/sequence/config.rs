use std::collections::BTreeSet;

use crate::diagram_theme::ThemeTypographyProperty;
use crate::text::TextStyle;
use merman_core::MermaidConfig;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceTypographyConfigRole {
    Actor,
    Message,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SequenceTypographyMeasurementWinner {
    path: &'static str,
}

impl SequenceTypographyMeasurementWinner {
    pub(crate) const fn path(self) -> &'static str {
        self.path
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SequenceRoleTypographyConfig {
    measurement_style: TextStyle,
    font_stack_measurement_winner: Option<SequenceTypographyMeasurementWinner>,
    font_size_measurement_winner: Option<SequenceTypographyMeasurementWinner>,
    font_weight_measurement_winner: Option<SequenceTypographyMeasurementWinner>,
    config_owned_properties: BTreeSet<ThemeTypographyProperty>,
}

impl SequenceRoleTypographyConfig {
    pub(crate) const fn measurement_style(&self) -> &TextStyle {
        &self.measurement_style
    }

    pub(crate) const fn measurement_winner(
        &self,
        property: ThemeTypographyProperty,
    ) -> Option<SequenceTypographyMeasurementWinner> {
        match property {
            ThemeTypographyProperty::FontStack => self.font_stack_measurement_winner,
            ThemeTypographyProperty::FontSize => self.font_size_measurement_winner,
            ThemeTypographyProperty::FontWeight => self.font_weight_measurement_winner,
            ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::LetterSpacing
            | ThemeTypographyProperty::WordSpacing
            | ThemeTypographyProperty::Transform
            | ThemeTypographyProperty::Decoration
            | ThemeTypographyProperty::TextAlign
            | ThemeTypographyProperty::WhiteSpace
            | ThemeTypographyProperty::Wrap => None,
        }
    }

    pub(crate) fn config_owns(&self, property: ThemeTypographyProperty) -> bool {
        self.config_owned_properties.contains(&property)
    }
}

pub(crate) struct SequenceConfigView<'a> {
    effective_config: &'a Value,
    sequence_config: &'a Value,
    config_provenance: Option<&'a MermaidConfig>,
}

impl<'a> SequenceConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            sequence_config: effective_config.get("sequence").unwrap_or(&Value::Null),
            config_provenance: None,
        }
    }

    pub(crate) fn from_mermaid_config(effective_config: &'a MermaidConfig) -> Self {
        Self {
            effective_config: effective_config.as_value(),
            sequence_config: effective_config
                .as_value()
                .get("sequence")
                .unwrap_or(&Value::Null),
            config_provenance: Some(effective_config),
        }
    }

    fn path_overrides_typed_default(&self, path: &str) -> bool {
        self.config_provenance.is_some_and(|config| {
            merman_core::__private::config_path_overrides_typed_default(config, path)
        })
    }

    pub(crate) fn sequence_bool(&self, key: &str, default: bool) -> bool {
        self.sequence_config_bool(key).unwrap_or(default)
    }

    pub(crate) fn sequence_config_bool(&self, key: &str) -> Option<bool> {
        self.sequence_config.get(key).and_then(Value::as_bool)
    }

    fn sequence_json_number_or(&self, key: &str, default: f64) -> f64 {
        self.sequence_json_number(key).unwrap_or(default)
    }

    pub(crate) fn sequence_json_number(&self, key: &str) -> Option<f64> {
        self.sequence_config.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn sequence_json_number_min(&self, key: &str, default: f64, min: f64) -> f64 {
        self.sequence_json_number_or(key, default).max(min)
    }

    pub(crate) fn root_json_number(&self, key: &str) -> Option<f64> {
        self.effective_config.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn root_bool(&self, key: &str) -> Option<bool> {
        self.effective_config.get(key).and_then(Value::as_bool)
    }

    pub(crate) fn root_string(&self, key: &str) -> Option<String> {
        crate::config::config_string(self.effective_config, &[key])
    }

    pub(crate) fn sequence_string(&self, key: &str) -> Option<String> {
        crate::config::config_string(self.sequence_config, &[key])
    }

    fn root_font_weight(&self, key: &str) -> Option<String> {
        self.effective_config
            .get(key)
            .and_then(sequence_font_weight_value)
    }

    fn sequence_font_weight(&self, key: &str) -> Option<String> {
        self.sequence_config
            .get(key)
            .and_then(sequence_font_weight_value)
    }

    pub(crate) fn theme_variable_font_weight(&self, key: &str) -> Option<String> {
        self.effective_config
            .get("themeVariables")
            .and_then(|theme_variables| theme_variables.get(key))
            .and_then(sequence_font_weight_value)
    }

    fn sequence_compat_f64(&self, key: &str, default: f64) -> f64 {
        crate::config::config_f64(self.sequence_config, &[key]).unwrap_or(default)
    }

    fn sequence_compat_f64_min(&self, key: &str, default: f64, min: f64) -> f64 {
        self.sequence_compat_f64(key, default).max(min)
    }

    fn root_compat_f64(&self, key: &str) -> Option<f64> {
        crate::config::config_f64(self.effective_config, &[key])
    }

    #[cfg(test)]
    pub(crate) fn resolve_role_typography(
        &self,
        role: SequenceTypographyConfigRole,
    ) -> SequenceRoleTypographyConfig {
        self.resolve_role_typography_with_typed_properties(role, &BTreeSet::new())
    }

    pub(crate) fn resolve_role_typography_with_typed_properties(
        &self,
        role: SequenceTypographyConfigRole,
        typed_theme_properties: &BTreeSet<ThemeTypographyProperty>,
    ) -> SequenceRoleTypographyConfig {
        let (family_key, family_path, size_key, size_path, weight_key, weight_path) = match role {
            SequenceTypographyConfigRole::Actor => (
                "actorFontFamily",
                "sequence.actorFontFamily",
                "actorFontSize",
                "sequence.actorFontSize",
                "actorFontWeight",
                "sequence.actorFontWeight",
            ),
            SequenceTypographyConfigRole::Message => (
                "messageFontFamily",
                "sequence.messageFontFamily",
                "messageFontSize",
                "sequence.messageFontSize",
                "messageFontWeight",
                "sequence.messageFontWeight",
            ),
            SequenceTypographyConfigRole::Note => (
                "noteFontFamily",
                "sequence.noteFontFamily",
                "noteFontSize",
                "sequence.noteFontSize",
                "noteFontWeight",
                "sequence.noteFontWeight",
            ),
        };

        let root_font_family = self.root_string("fontFamily");
        let role_font_family = self.sequence_string(family_key);
        let theme_variable_font_family =
            crate::config::config_string(self.effective_config, &["themeVariables", "fontFamily"]);
        let theme_variable_font_family_owns = self
            .path_overrides_typed_default("themeVariables.fontFamily")
            && theme_variable_font_family.is_some();
        let root_font_family_owns =
            root_font_family.is_some() && self.path_overrides_typed_default("fontFamily");
        let role_font_family_owns =
            role_font_family.is_some() && self.path_overrides_typed_default(family_path);
        let typed_font_stack = typed_theme_properties.contains(&ThemeTypographyProperty::FontStack);
        let root_font_family_present = root_font_family.is_some();
        // Mermaid mirrors every root font value, including generated defaults, over the
        // role-local Sequence value. A role-local config only bypasses a generated root value
        // when that property participates in typed-theme ownership.
        let (font_family, font_stack_measurement_winner) = if root_font_family_owns {
            (
                root_font_family,
                Some(SequenceTypographyMeasurementWinner { path: "fontFamily" }),
            )
        } else if typed_font_stack && role_font_family_owns {
            (
                role_font_family,
                Some(SequenceTypographyMeasurementWinner { path: family_path }),
            )
        } else if let Some(value) = root_font_family {
            (
                Some(value),
                Some(SequenceTypographyMeasurementWinner { path: "fontFamily" }),
            )
        } else if let Some(value) = role_font_family {
            (
                Some(value),
                Some(SequenceTypographyMeasurementWinner { path: family_path }),
            )
        } else {
            (None, None)
        };

        let root_font_size = self.root_compat_f64("fontSize");
        let role_font_size = crate::config::config_f64(self.sequence_config, &[size_key]);
        let root_font_size_owns =
            root_font_size.is_some() && self.path_overrides_typed_default("fontSize");
        let role_font_size_owns =
            role_font_size.is_some() && self.path_overrides_typed_default(size_path);
        let typed_font_size = typed_theme_properties.contains(&ThemeTypographyProperty::FontSize);
        let root_font_size_present = root_font_size.is_some();
        let (font_size, font_size_measurement_winner) = if root_font_size_owns {
            (
                root_font_size.expect("owned root Sequence font size must exist"),
                Some(SequenceTypographyMeasurementWinner { path: "fontSize" }),
            )
        } else if typed_font_size && role_font_size_owns {
            (
                role_font_size.expect("owned role Sequence font size must exist"),
                Some(SequenceTypographyMeasurementWinner { path: size_path }),
            )
        } else if let Some(value) = root_font_size {
            (
                value,
                Some(SequenceTypographyMeasurementWinner { path: "fontSize" }),
            )
        } else if let Some(value) = role_font_size {
            (
                value,
                Some(SequenceTypographyMeasurementWinner { path: size_path }),
            )
        } else {
            (16.0, None)
        };

        let theme_note_font_weight = (role == SequenceTypographyConfigRole::Note)
            .then(|| self.theme_variable_font_weight("noteFontWeight"))
            .flatten();
        let theme_note_font_weight_owns = theme_note_font_weight.is_some()
            && self.path_overrides_typed_default("themeVariables.noteFontWeight");
        let root_font_weight = self.root_font_weight("fontWeight");
        let role_font_weight = self.sequence_font_weight(weight_key);
        let root_font_weight_owns =
            root_font_weight.is_some() && self.path_overrides_typed_default("fontWeight");
        let role_font_weight_owns =
            role_font_weight.is_some() && self.path_overrides_typed_default(weight_path);
        let (font_weight, font_weight_measurement_winner) = if theme_note_font_weight_owns {
            (
                theme_note_font_weight,
                Some(SequenceTypographyMeasurementWinner {
                    path: "themeVariables.noteFontWeight",
                }),
            )
        } else if root_font_weight_owns {
            (
                root_font_weight,
                Some(SequenceTypographyMeasurementWinner { path: "fontWeight" }),
            )
        } else if role_font_weight_owns {
            (
                role_font_weight,
                Some(SequenceTypographyMeasurementWinner { path: weight_path }),
            )
        } else if let Some(value) = root_font_weight {
            (
                Some(value),
                Some(SequenceTypographyMeasurementWinner { path: "fontWeight" }),
            )
        } else if let Some(value) = role_font_weight {
            (
                Some(value),
                Some(SequenceTypographyMeasurementWinner { path: weight_path }),
            )
        } else if let Some(value) = theme_note_font_weight {
            (
                Some(value),
                Some(SequenceTypographyMeasurementWinner {
                    path: "themeVariables.noteFontWeight",
                }),
            )
        } else {
            (None, None)
        };

        let mut config_owned_properties = BTreeSet::new();
        if theme_variable_font_family_owns
            || root_font_family_owns
            || (role_font_family_owns && (typed_font_stack || !root_font_family_present))
        {
            config_owned_properties.insert(ThemeTypographyProperty::FontStack);
        }
        if root_font_size_owns
            || (role_font_size_owns && (typed_font_size || !root_font_size_present))
        {
            config_owned_properties.insert(ThemeTypographyProperty::FontSize);
        }
        if theme_note_font_weight_owns || root_font_weight_owns || role_font_weight_owns {
            config_owned_properties.insert(ThemeTypographyProperty::FontWeight);
        }

        SequenceRoleTypographyConfig {
            measurement_style: TextStyle {
                font_family,
                font_size,
                font_weight,
                font_style: None,
            },
            font_stack_measurement_winner,
            font_size_measurement_winner,
            font_weight_measurement_winner,
            config_owned_properties,
        }
    }
}

pub(super) struct SequenceLayoutSettings {
    pub(super) diagram_margin_x: f64,
    pub(super) diagram_margin_y: f64,
    pub(super) bottom_margin_adj: f64,
    pub(super) box_margin: f64,
    pub(super) actor_margin: f64,
    pub(super) sequence_default_width: f64,
    pub(super) actor_height: f64,
    pub(super) wrap_padding: f64,
    pub(super) note_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) label_box_height: f64,
    pub(super) label_box_width: f64,
    pub(super) mirror_actors: bool,
    pub(super) right_angles: bool,
    pub(super) is_neo: bool,
    pub(super) activation_width: f64,
    pub(super) actor_text_style: TextStyle,
    pub(super) note_text_style: TextStyle,
    pub(super) msg_text_style: TextStyle,
    pub(super) loop_text_style: TextStyle,
}

impl SequenceLayoutSettings {
    pub(super) fn from_effective_config(
        effective_config: &Value,
        typography: &super::typography::SequenceTypographyPlan,
    ) -> Self {
        let config = SequenceConfigView::new(effective_config);

        let diagram_margin_x = config.sequence_compat_f64("diagramMarginX", 50.0);
        let diagram_margin_y = config.sequence_compat_f64("diagramMarginY", 10.0);
        let bottom_margin_adj = config.sequence_compat_f64("bottomMarginAdj", 1.0);
        let box_margin = config.sequence_compat_f64("boxMargin", 10.0);
        let actor_margin = config.sequence_compat_f64("actorMargin", 50.0);
        let sequence_default_width = config.sequence_compat_f64("width", 150.0);
        let actor_height = config.sequence_compat_f64("height", 65.0);
        let wrap_padding = config.sequence_compat_f64("wrapPadding", 10.0);
        let note_margin = config.sequence_compat_f64("noteMargin", 10.0);
        let box_text_margin = config.sequence_compat_f64("boxTextMargin", 5.0);
        let label_box_height = config.sequence_compat_f64("labelBoxHeight", 20.0);
        let label_box_width = config.sequence_compat_f64("labelBoxWidth", 50.0);
        let mirror_actors = config.sequence_bool("mirrorActors", true);
        let right_angles = config.sequence_bool("rightAngles", false);
        let is_neo = crate::config::config_diagram_look(effective_config).is_neo();
        let activation_width = config.sequence_compat_f64_min("activationWidth", 10.0, 1.0);

        Self {
            diagram_margin_x,
            diagram_margin_y,
            bottom_margin_adj,
            box_margin,
            actor_margin,
            sequence_default_width,
            actor_height,
            wrap_padding,
            note_margin,
            box_text_margin,
            label_box_height,
            label_box_width,
            mirror_actors,
            right_angles,
            is_neo,
            activation_width,
            actor_text_style: typography.actor().measurement_style().clone(),
            note_text_style: typography.note().measurement_style().clone(),
            msg_text_style: typography.message().measurement_style().clone(),
            loop_text_style: typography.loop_label().measurement_style().clone(),
        }
    }
}

fn sequence_font_weight_value(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string).or_else(|| {
        value
            .as_u64()
            .filter(|weight| (1..=1000).contains(weight))
            .map(|weight| weight.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sequence_layout_settings(config: Value) -> SequenceLayoutSettings {
        let effective_config = MermaidConfig::from_value(config);
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let typography = super::super::typography::SequenceTypographyPlan::resolve(
            &effective_config,
            None,
            &work_meter,
        )
        .expect("resolve Sequence typography");
        SequenceLayoutSettings::from_effective_config(effective_config.as_value(), &typography)
    }

    #[test]
    fn sequence_layout_settings_preserve_layout_numeric_string_config() {
        let cfg = json!({
            "look": "neo",
            "fontFamily": "Global, Arial",
            "fontSize": "22",
            "fontWeight": 700,
            "sequence": {
                "width": "240",
                "height": "80",
                "activationWidth": "0.25",
                "mirrorActors": false,
                "messageFontFamily": "Message",
                "messageFontSize": "18",
                "messageFontWeight": "300"
            }
        });

        let settings = sequence_layout_settings(cfg);

        assert_eq!(settings.sequence_default_width, 240.0);
        assert_eq!(settings.actor_height, 80.0);
        assert_eq!(settings.activation_width, 1.0);
        assert!(!settings.mirror_actors);
        assert!(settings.is_neo);
        assert_eq!(
            settings.msg_text_style.font_family.as_deref(),
            Some("Global, Arial")
        );
        assert_eq!(settings.msg_text_style.font_size, 22.0);
        assert_eq!(settings.msg_text_style.font_weight.as_deref(), Some("700"));
    }

    #[test]
    fn sequence_layout_settings_use_family_font_fallbacks_without_global_font() {
        let cfg = json!({
            "sequence": {
                "actorFontFamily": "Actor",
                "actorFontSize": "19",
                "actorFontWeight": "500",
                "noteFontFamily": "Note",
                "noteFontSize": 20,
                "noteFontWeight": "600",
                "messageFontFamily": "Message",
                "messageFontSize": 21,
                "messageFontWeight": "700"
            }
        });

        let settings = sequence_layout_settings(cfg);

        assert_eq!(
            settings.actor_text_style.font_family.as_deref(),
            Some("Actor")
        );
        assert_eq!(settings.actor_text_style.font_size, 19.0);
        assert_eq!(
            settings.actor_text_style.font_weight.as_deref(),
            Some("500")
        );
        assert_eq!(
            settings.note_text_style.font_family.as_deref(),
            Some("Note")
        );
        assert_eq!(settings.note_text_style.font_size, 20.0);
        assert_eq!(settings.note_text_style.font_weight.as_deref(), Some("600"));
        assert_eq!(
            settings.msg_text_style.font_family.as_deref(),
            Some("Message")
        );
        assert_eq!(settings.msg_text_style.font_size, 21.0);
        assert_eq!(settings.msg_text_style.font_weight.as_deref(), Some("700"));
    }

    #[test]
    fn sequence_role_typography_reports_root_winners_ahead_of_role_local_config() {
        let cfg = json!({
            "fontFamily": "Root Family",
            "fontSize": 22,
            "fontWeight": 700,
            "sequence": {
                "actorFontFamily": "Actor Family",
                "actorFontSize": 19,
                "actorFontWeight": 500
            }
        });

        let resolved = SequenceConfigView::new(&cfg)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Root Family")
        );
        assert_eq!(resolved.measurement_style().font_size, 22.0);
        assert_eq!(
            resolved.measurement_style().font_weight.as_deref(),
            Some("700")
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontStack),
            Some(SequenceTypographyMeasurementWinner { path: "fontFamily" })
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontSize),
            Some(SequenceTypographyMeasurementWinner { path: "fontSize" })
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontWeight),
            Some(SequenceTypographyMeasurementWinner { path: "fontWeight" })
        );
    }

    #[test]
    fn sequence_role_typography_ignores_unowned_theme_defaults_ahead_of_root_config() {
        let cfg = json!({
            "fontFamily": "Root Family",
            "themeVariables": {"fontFamily": "Theme Family"},
            "sequence": {"actorFontFamily": "Actor Family"}
        });

        let resolved = SequenceConfigView::new(&cfg)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Root Family")
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontStack),
            Some(SequenceTypographyMeasurementWinner { path: "fontFamily" })
        );
    }

    #[test]
    fn sequence_role_typography_keeps_explicit_root_font_ownership_after_mirroring() {
        let metadata = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "fontFamily": "Root Family",
            })))
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello")
            .expect("parse Sequence site font config");
        let resolved = SequenceConfigView::from_mermaid_config(&metadata.effective_config)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Root Family")
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontStack),
            Some(SequenceTypographyMeasurementWinner { path: "fontFamily" })
        );
        assert!(resolved.config_owns(ThemeTypographyProperty::FontStack));
    }

    #[test]
    fn sequence_role_typography_keeps_theme_variable_ownership_separate_from_measurement() {
        let metadata = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "fontFamily": "Root Family",
                "themeVariables": {"fontFamily": "Theme Family"},
            })))
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello")
            .expect("parse Sequence theme-variable font config");
        let resolved = SequenceConfigView::from_mermaid_config(&metadata.effective_config)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Root Family")
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontStack),
            Some(SequenceTypographyMeasurementWinner { path: "fontFamily" })
        );
        assert!(resolved.config_owns(ThemeTypographyProperty::FontStack));
    }

    #[test]
    fn sequence_role_typography_uses_generated_root_defaults_on_mermaid_compatibility_path() {
        let metadata = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "sequence": {
                    "actorFontFamily": "Actor Family",
                    "actorFontSize": 19,
                    "actorFontWeight": 500,
                },
            })))
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello")
            .expect("parse Sequence role typography config");
        let resolved = SequenceConfigView::from_mermaid_config(&metadata.effective_config)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);
        let generated_root_family = metadata
            .effective_config
            .as_value()
            .get("fontFamily")
            .and_then(Value::as_str)
            .expect("generated root fontFamily");

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some(generated_root_family)
        );
        assert_eq!(resolved.measurement_style().font_size, 16.0);
        assert_eq!(
            resolved.measurement_style().font_weight.as_deref(),
            Some("500")
        );
        assert!(!resolved.config_owns(ThemeTypographyProperty::FontStack));
        assert!(!resolved.config_owns(ThemeTypographyProperty::FontSize));
        assert!(resolved.config_owns(ThemeTypographyProperty::FontWeight));
    }

    #[test]
    fn sequence_role_typography_keeps_explicit_role_ownership_for_typed_properties() {
        let metadata = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "sequence": {
                    "actorFontFamily": "Actor Family",
                    "actorFontSize": 19,
                    "actorFontWeight": 500,
                },
            })))
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello")
            .expect("parse Sequence role typography config");
        let typed_theme_properties = BTreeSet::from([
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
        ]);
        let resolved = SequenceConfigView::from_mermaid_config(&metadata.effective_config)
            .resolve_role_typography_with_typed_properties(
                SequenceTypographyConfigRole::Actor,
                &typed_theme_properties,
            );

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Actor Family")
        );
        assert_eq!(resolved.measurement_style().font_size, 19.0);
        assert_eq!(
            resolved.measurement_style().font_weight.as_deref(),
            Some("500")
        );
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
            ThemeTypographyProperty::FontWeight,
        ] {
            assert!(resolved.config_owns(property));
        }
    }

    #[test]
    fn sequence_role_typography_reports_role_winners_only_without_global_values() {
        let cfg = json!({
            "sequence": {
                "actorFontFamily": "Actor Family",
                "actorFontSize": 19,
                "actorFontWeight": 500
            }
        });

        let resolved = SequenceConfigView::new(&cfg)
            .resolve_role_typography(SequenceTypographyConfigRole::Actor);

        assert_eq!(
            resolved.measurement_style().font_family.as_deref(),
            Some("Actor Family")
        );
        assert_eq!(resolved.measurement_style().font_size, 19.0);
        assert_eq!(
            resolved.measurement_style().font_weight.as_deref(),
            Some("500")
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontStack),
            Some(SequenceTypographyMeasurementWinner {
                path: "sequence.actorFontFamily",
            })
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontSize),
            Some(SequenceTypographyMeasurementWinner {
                path: "sequence.actorFontSize",
            })
        );
        assert_eq!(
            resolved.measurement_winner(ThemeTypographyProperty::FontWeight),
            Some(SequenceTypographyMeasurementWinner {
                path: "sequence.actorFontWeight",
            })
        );
    }
}
