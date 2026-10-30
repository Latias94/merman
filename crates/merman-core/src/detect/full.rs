use super::{clean_detection_input_controlled, detect_precleaned_controlled};
use crate::family::DetectorFact;
use crate::presentation::SourcePresentationConfig;
use crate::{OperationControl, OperationControlResult, Result};
use std::sync::Arc;

/// Predicate used by [`DetectorRegistry`] to recognize one Mermaid diagram family.
pub type DetectorFn = fn(text: &str, config: &mut SourcePresentationConfig) -> bool;

/// One diagram detector entry.
#[derive(Debug, Clone)]
pub struct Detector {
    /// Mermaid diagram type id returned when the detector matches.
    pub id: &'static str,
    /// Detection predicate. It may read and update Mermaid config, matching upstream behavior.
    pub detector: DetectorFn,
}

#[derive(Debug, Clone)]
enum DetectorEntry {
    Builtin(DetectorFact),
    Custom(Detector),
}

impl DetectorEntry {
    fn id(&self) -> &'static str {
        match self {
            Self::Builtin(fact) => fact.id,
            Self::Custom(detector) => detector.id,
        }
    }

    fn matches(&self, text: &str, config: &mut SourcePresentationConfig) -> bool {
        match self {
            Self::Builtin(fact) => {
                if !(fact.detector)(text) {
                    return false;
                }
                if let Some((path, value)) = fact.config_effect.assignment() {
                    // Public assignment records the detector's ownership even when the value
                    // was already `elk`; overlay replay relies on that provenance.
                    config.set_value(path, serde_json::Value::String(value.to_string()));
                }
                true
            }
            Self::Custom(detector) => (detector.detector)(text, config),
        }
    }
}

/// Ordered registry that detects Mermaid diagram types.
///
/// Detector order is semantically significant because Mermaid registers overlapping diagram
/// syntaxes in a fixed order.
#[derive(Debug, Clone)]
pub struct DetectorRegistry {
    detectors: Arc<Vec<DetectorEntry>>,
}

impl DetectorRegistry {
    /// Creates an empty detector registry.
    pub fn new() -> Self {
        Self {
            detectors: Arc::new(Vec::new()),
        }
    }

    /// Adds a detector entry to the end of the ordered registry.
    pub fn add(&mut self, detector: Detector) {
        Arc::make_mut(&mut self.detectors).push(DetectorEntry::Custom(detector));
    }

    /// Adds a detector function to the end of the ordered registry.
    pub fn add_fn(&mut self, id: &'static str, detector: DetectorFn) {
        self.add(Detector { id, detector });
    }

    /// Detects a Mermaid diagram type after stripping front-matter, directives, and comments.
    pub fn detect_type(
        &self,
        text: &str,
        config: &mut SourcePresentationConfig,
    ) -> Result<&'static str> {
        let control = OperationControl::new();
        self.detect_type_controlled(text, config, &control)
            .expect("a private parse control cannot be cancelled")
    }

    pub(crate) fn detect_type_controlled(
        &self,
        text: &str,
        config: &mut SourcePresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<&'static str>> {
        let cleaned = clean_detection_input_controlled(text, control)?;
        self.detect_type_cleaned_controlled(cleaned.as_ref(), config, control)
    }

    /// Detects a diagram type assuming the input is already pre-cleaned:
    /// no front-matter, no directives, and no Mermaid `%%` comments.
    pub fn detect_type_precleaned(
        &self,
        text: &str,
        config: &mut SourcePresentationConfig,
    ) -> Result<&'static str> {
        let control = OperationControl::new();
        self.detect_type_precleaned_controlled(text, config, &control)
            .expect("a private parse control cannot be cancelled")
    }

    pub(crate) fn detect_type_precleaned_controlled(
        &self,
        text: &str,
        config: &mut SourcePresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<&'static str>> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        self.detect_type_cleaned_controlled(text, config, control)
    }

    fn detect_type_cleaned_controlled(
        &self,
        text: &str,
        config: &mut SourcePresentationConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<&'static str>> {
        Ok(detect_precleaned_controlled(
            text,
            self.detectors.as_slice(),
            |detector, text| detector.matches(text, config),
            control,
        )?
        .map(DetectorEntry::id))
    }

    /// Builds the detector registry for the pinned Mermaid baseline.
    pub fn pinned_mermaid_baseline() -> Self {
        Self {
            detectors: Arc::new(
                crate::family::detector_facts()
                    .iter()
                    .copied()
                    .map(DetectorEntry::Builtin)
                    .collect(),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn detector_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.detectors.iter().map(DetectorEntry::id)
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::detect_builtin_controlled;
    use super::*;
    use crate::{MermaidConfig, OperationCancelled};
    use serde_json::json;

    #[test]
    fn builtin_and_full_detection_share_catalog_order_and_configuration_effects() {
        let registry = DetectorRegistry::pinned_mermaid_baseline();
        assert_eq!(
            registry.detector_ids().collect::<Vec<_>>(),
            crate::family::detector_facts()
                .iter()
                .map(|fact| fact.id)
                .collect::<Vec<_>>()
        );
        for (source, expected) in [
            (
                "\u{feff}---\ntitle: Diagram\n---\n%%{init: {}}%%\n%% comment\nflowchart-elk TD\nA-->B",
                "flowchart-elk",
            ),
            ("--- unparsed", "---"),
            ("flowchart TD\nA-->B", "flowchart-v2"),
            ("sequenceDiagram\nAlice->>Bob: Hi", "sequence"),
            ("railroad-ebnf-beta", "railroadEbnf"),
            ("C4Container", "c4"),
        ] {
            let fact = detect_builtin_controlled(source, &OperationControl::new())
                .expect("active operation")
                .expect("built-in detection");
            let mut semantic_config = MermaidConfig::empty_object();
            fact.config_effect.apply(&mut semantic_config);
            let mut full_config = SourcePresentationConfig::empty_object();
            let full = registry
                .detect_type(source, &mut full_config)
                .expect("full detection");
            assert_eq!(fact.id, expected, "{source}");
            assert_eq!(full, expected, "{source}");
            assert_eq!(semantic_config.as_value(), full_config.as_value());
        }
    }

    #[test]
    fn builtin_elk_effect_preserves_same_value_assignment_provenance() {
        let registry = DetectorRegistry::pinned_mermaid_baseline();
        let before = SourcePresentationConfig::from_value(json!({"layout": "elk"}));
        let mut after = before.clone();
        assert_eq!(
            registry
                .detect_type_precleaned("flowchart-elk TD", &mut after)
                .expect("ELK detection"),
            "flowchart-elk"
        );
        assert_eq!(before.as_value(), after.as_value());
        assert!(after.path_was_mutated_after(&before, "layout"));
    }

    #[test]
    fn controlled_detection_stops_before_iterating_detectors() {
        let control = OperationControl::new();
        control.cancel();

        let result = DetectorRegistry::pinned_mermaid_baseline().detect_type_controlled(
            "flowchart TD",
            &mut SourcePresentationConfig::empty_object(),
            &control,
        );

        assert!(matches!(result, Err(OperationCancelled { .. })));
    }

    #[test]
    fn detection_uses_the_greedy_frontmatter_match_without_parsing_yaml() {
        let registry = DetectorRegistry::pinned_mermaid_baseline();
        let mut config = SourcePresentationConfig::empty_object();
        let source = "---\n\n---\n\nMORE\n---\nerror";
        assert_eq!(registry.detect_type(source, &mut config).unwrap(), "error");
    }

    #[test]
    fn detection_observes_cancellation_inside_frontmatter_whitespace() {
        let source = format!("---\n{}", " \n".repeat(16_000));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let result = DetectorRegistry::pinned_mermaid_baseline().detect_type_controlled(
            &source,
            &mut SourcePresentationConfig::empty_object(),
            &control,
        );
        assert!(result.is_err());
    }

    fn always_detects(_text: &str, _config: &mut SourcePresentationConfig) -> bool {
        true
    }

    #[test]
    fn detector_registry_clone_uses_copy_on_write_storage() {
        let original = DetectorRegistry::pinned_mermaid_baseline();
        let mut cloned = original.clone();
        assert!(Arc::ptr_eq(&original.detectors, &cloned.detectors));
        cloned.add_fn("copy-on-write-test", always_detects);
        assert!(!Arc::ptr_eq(&original.detectors, &cloned.detectors));
        assert!(!original.detector_ids().any(|id| id == "copy-on-write-test"));
        assert!(cloned.detector_ids().any(|id| id == "copy-on-write-test"));
    }
}
