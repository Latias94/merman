use std::sync::Arc;

use crate::{OperationControl, OperationControlResult};

use super::{
    DetectorConfigChanges, ErPaintInputs, FamilyPaintInputs, GitGraphPaintInputs, MermaidConfig,
    PresentationConfig, ThemeCompatibilityState, dotted_paths_overlap,
};

impl PresentationConfig {
    /// Freezes surviving raw paint authority independently of derived theme variable ownership.
    pub(crate) fn freeze_family_paint_inputs(
        &mut self,
        family: &str,
        explicit_site_config: &MermaidConfig,
        explicit_source_config: &MermaidConfig,
        detector_changes: Option<&DetectorConfigChanges>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        if !matches!(
            &self.theme_compatibility,
            Some(ThemeCompatibilityState::Tracking(_))
        ) {
            return Ok(());
        }
        let paths: &[&str] = match family {
            "gitGraph" => &[
                "themeVariables.primaryColor",
                "themeVariables.mainBkg",
                "themeVariables.tagLabelBackground",
                "themeVariables.primaryBorderColor",
                "themeVariables.nodeBorder",
                "themeVariables.tagLabelBorder",
            ],
            "er" => &[
                "themeVariables.textColor",
                "themeVariables.nodeTextColor",
                "themeVariables.lineColor",
                "themeVariables.rowOdd",
                "themeVariables.rowEven",
            ],
            _ => return Ok(()),
        };
        let mut owned = 0;
        for (index, path) in paths.iter().enumerate() {
            control.checkpoint()?;
            if detector_changes.is_some_and(|changes| {
                changes
                    .claimed_paths()
                    .any(|claimed| claimed.is_empty() || dotted_paths_overlap(claimed, path))
                    || changes.value_changed_at(path)
            }) || owns_path(explicit_site_config, self.as_config(), path)
                || owns_path(explicit_source_config, self.as_config(), path)
            {
                owned |= 1 << index;
            }
        }
        let inputs = match family {
            "gitGraph" => FamilyPaintInputs::GitGraph(GitGraphPaintInputs(owned)),
            "er" => FamilyPaintInputs::Er(ErPaintInputs(owned)),
            _ => unreachable!("selected paint family checked above"),
        };
        if let Some(ThemeCompatibilityState::Tracking(ownership)) =
            self.theme_compatibility.as_mut()
        {
            Arc::make_mut(ownership).family_paint_inputs = Some(inputs);
        }
        Ok(())
    }
}

fn owns_path(
    explicit_config: &MermaidConfig,
    effective_config: &MermaidConfig,
    dotted_path: &str,
) -> bool {
    let mut explicit_current = explicit_config.as_value();
    let mut effective_current = effective_config.as_value();
    for segment in dotted_path.split('.') {
        let Some(explicit_object) = explicit_current.as_object() else {
            // A scalar/array parent only owns a descendant when assignWithDepth actually left that
            // parent in the effective config. Type-conflicting assignments are ignored upstream.
            return explicit_current == effective_current;
        };
        let Some(explicit_next) = explicit_object.get(segment) else {
            return false;
        };
        let Some(effective_next) = effective_current
            .as_object()
            .and_then(|object| object.get(segment))
        else {
            return false;
        };
        explicit_current = explicit_next;
        effective_current = effective_next;
    }
    explicit_current == effective_current
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn raw_paint_authority_requires_surviving_values_and_parent_types() {
        for (explicit, effective, path, expected) in [
            (
                json!({"themeVariables": {"primaryColor": "#123456"}}),
                json!({"themeVariables": {"primaryColor": "#123456", "mainBkg": "#123456"}}),
                "themeVariables.primaryColor",
                true,
            ),
            (
                json!({"themeVariables": {"primaryColor": "#123456"}}),
                json!({"themeVariables": {"primaryColor": "#123456", "mainBkg": "#123456"}}),
                "themeVariables.mainBkg",
                false,
            ),
            (
                json!({"themeVariables": {"primaryColor": null}}),
                json!({"themeVariables": {"primaryColor": "#fff4dd"}}),
                "themeVariables.primaryColor",
                false,
            ),
            (
                json!({"themeVariables": "ignored"}),
                json!({"themeVariables": {"primaryColor": "#fff4dd"}}),
                "themeVariables.primaryColor",
                false,
            ),
            (
                json!({"themeVariables": "owned"}),
                json!({"themeVariables": "owned"}),
                "themeVariables.primaryColor",
                true,
            ),
        ] {
            assert_eq!(
                owns_path(
                    &MermaidConfig::from_value(explicit),
                    &MermaidConfig::from_value(effective),
                    path,
                ),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn capturing_paint_inputs_is_selected_family_only_and_cancellable() {
        let binding =
            super::super::ThemeParseBinding::try_new([0x5a; 32], MermaidConfig::empty_object())
                .unwrap();
        let mut config = PresentationConfig::from_theme_parse_binding(binding.clone());
        let empty = MermaidConfig::empty_object();
        let cancelled = OperationControl::new();
        cancelled.cancel();
        assert!(
            config
                .freeze_family_paint_inputs("sequence", &empty, &empty, None, &cancelled)
                .is_ok()
        );
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            config
                .freeze_family_paint_inputs("gitGraph", &empty, &empty, None, &control)
                .is_err()
        );
        config.freeze_theme_compatibility();
        assert_eq!(config.gitgraph_paint_inputs().map(|inputs| inputs.0), None);

        let mut config = PresentationConfig::from_theme_parse_binding(binding.clone());
        config
            .freeze_family_paint_inputs("gitGraph", &empty, &empty, None, &OperationControl::new())
            .unwrap();
        assert_eq!(config.gitgraph_paint_inputs().map(|inputs| inputs.0), None);
        config.freeze_theme_compatibility();
        assert_eq!(
            config.gitgraph_paint_inputs().map(|inputs| inputs.0),
            Some(0)
        );
        let fields = config.mermaid_compatibility_fields().unwrap();
        config.freeze_theme_compatibility();
        assert_eq!(config.theme_parse_binding(), Some(&binding));
        assert_eq!(
            config.mermaid_compatibility_fields().as_ref(),
            Some(&fields)
        );
        assert_eq!(
            config.gitgraph_paint_inputs().map(|inputs| inputs.0),
            Some(0)
        );
    }
}
