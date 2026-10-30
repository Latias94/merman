#[cfg(test)]
use super::{Directive, detect_init_controlled};
use super::{DirectiveRecoveryMode, PreprocessResult, preprocess_source_capture_controlled};
#[cfg(feature = "editor-facts")]
use super::{PreprocessCaptureOutcome, SourceConfigCaptureMode};
#[cfg(test)]
use crate::MermaidConfig;
use crate::presentation::SourcePresentationConfig;
use crate::{DetectorRegistry, OperationControl, OperationControlResult, Result};

fn registry_detector(
    registry: &DetectorRegistry,
) -> impl Fn(&str, &OperationControl) -> OperationControlResult<Result<&'static str>> + '_ {
    move |input, control| {
        registry.detect_type_controlled(
            input,
            &mut SourcePresentationConfig::empty_object(),
            control,
        )
    }
}

pub fn preprocess_diagram(input: &str, registry: &DetectorRegistry) -> Result<PreprocessResult> {
    preprocess_diagram_with_known_type(input, registry, None)
}

pub fn preprocess_diagram_with_known_type(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    let control = OperationControl::new();
    preprocess_diagram_with_known_type_and_directive_recovery_controlled(
        input,
        registry,
        diagram_type,
        DirectiveRecoveryMode::Strict,
        &control,
    )
    .expect("a private parse control cannot be cancelled")
}

pub(crate) fn preprocess_diagram_with_known_type_and_directive_recovery_controlled(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
    directive_recovery: DirectiveRecoveryMode,
    control: &OperationControl,
) -> OperationControlResult<Result<PreprocessResult>> {
    Ok(preprocess_source_capture_controlled(
        input,
        &registry_detector(registry),
        diagram_type,
        directive_recovery,
        #[cfg(feature = "editor-facts")]
        SourceConfigCaptureMode::Omit,
        control,
    )?
    .into_result())
}

#[cfg(feature = "editor-facts")]
pub(crate) fn preprocess_diagram_with_known_type_and_directive_recovery_evidence_controlled(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
    directive_recovery: DirectiveRecoveryMode,
    control: &OperationControl,
) -> OperationControlResult<PreprocessCaptureOutcome> {
    preprocess_source_capture_controlled(
        input,
        &registry_detector(registry),
        diagram_type,
        directive_recovery,
        SourceConfigCaptureMode::Collect,
        control,
    )
}

#[cfg(test)]
pub(super) fn detect_init(
    directives: &[Directive],
    input: &str,
    registry: &DetectorRegistry,
) -> Result<MermaidConfig> {
    let control = OperationControl::new();
    detect_init_controlled(directives, input, &registry_detector(registry), &control)
        .expect("a private parse control cannot be cancelled")
}

#[cfg(test)]
mod tests {
    use super::super::{Value, detect_directives, preprocess_builtin_semantic_controlled};
    use super::*;
    use crate::SourceSpan;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static INIT_DETECTOR_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn counting_flowchart_detector(
        _text: &str,
        _config: &mut crate::presentation::SourcePresentationConfig,
    ) -> bool {
        INIT_DETECTOR_CALLS.fetch_add(1, Ordering::Relaxed);
        true
    }

    #[test]
    fn builtin_preprocess_reuses_directive_namespaces_and_source_maps() {
        let input = concat!(
            "---\r\ntitle: Diagram\r\nconfig:\r\n  flowchart:\r\n    inheritDir: true\r\n---\r\n",
            "%%{init: {flowchart: {htmlLabels: false}, fontFamily: 'Example'}}%%\r\n",
            "%%{initialize: {flowchart: {curve: 'step'}}}%%\r\n",
            "flowchart-elk TD\r\n%% comment\r\nA-->B",
        );
        let registry = DetectorRegistry::pinned_mermaid_baseline();
        for diagram_type in [None, Some("flowchart-elk")] {
            let builtin = preprocess_builtin_semantic_controlled(
                input,
                diagram_type,
                &OperationControl::new(),
            )
            .expect("active operation")
            .expect("built-in preprocessing");
            let full = preprocess_diagram_with_known_type(input, &registry, diagram_type)
                .expect("full preprocessing");
            assert_eq!(builtin.code(), full.code());
            assert_eq!(builtin.title, full.title);
            assert_eq!(builtin.config.as_value(), full.config.as_value());
            assert_eq!(
                builtin.config.as_value()["flowchart"]["inheritDir"],
                json!(true)
            );
            assert_eq!(
                builtin.config.as_value()["flowchart"]["htmlLabels"],
                json!(false)
            );
            assert_eq!(builtin.config.get_str("flowchart.curve"), Some("step"));
            let start = builtin.code().find("A-->B").expect("edge");
            assert_eq!(
                builtin
                    .source
                    .try_map_span(SourceSpan::new(start, start + 5)),
                full.source.try_map_span(SourceSpan::new(start, start + 5))
            );
        }
    }

    #[test]
    fn multiple_init_directives_merge_explicit_namespaces_without_extra_detection() {
        INIT_DETECTOR_CALLS.store(0, Ordering::Relaxed);
        let input = concat!(
            "%%{init: {\"flowchart\": {\"curve\": \"linear\"}}}%%\n",
            "%%{initialize: {\"flowchart\": {\"htmlLabels\": false}}}%%\n",
            "flowchart TD\nA-->B\n",
        );
        let directives = detect_directives(input).expect("directives parse");
        let mut registry = DetectorRegistry::new();
        registry.add_fn("flowchart-v2", counting_flowchart_detector);

        let config = detect_init(&directives, input, &registry).expect("init merges");

        assert_eq!(INIT_DETECTOR_CALLS.load(Ordering::Relaxed), 0);
        assert_eq!(
            config
                .as_value()
                .pointer("/flowchart/curve")
                .and_then(Value::as_str),
            Some("linear")
        );
        assert_eq!(
            config
                .as_value()
                .pointer("/flowchart/htmlLabels")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
}
