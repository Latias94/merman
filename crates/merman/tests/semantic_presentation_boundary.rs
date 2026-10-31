#![cfg(feature = "diagram-flowchart")]

#[cfg(feature = "svg")]
use merman::SvgRequest;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

const INVALID_VISUAL_THEME: &str =
    "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\nflowchart TD\nA-->B";

#[test]
fn semantic_target_retains_authored_config_without_evaluating_visual_programs() {
    let RenderOutput::Semantic(Some(artifact)) = Renderer::new()
        .render(RenderRequest::semantic(
            INVALID_VISUAL_THEME,
            OperationControl::new(),
        ))
        .expect("semantic requests must not evaluate visual theme programs")
    else {
        panic!("expected canonical semantic output");
    };
    assert_eq!(artifact.semantic_kind(), "flowchart");
    assert_eq!(
        artifact.metadata().config.as_value()["themeVariables"]["primaryColor"],
        "not-a-color"
    );
}

#[test]
#[cfg(feature = "svg")]
fn presentation_target_preserves_visual_theme_errors() {
    assert!(
        Renderer::new()
            .render(RenderRequest::svg(
                INVALID_VISUAL_THEME,
                OperationControl::new(),
                SvgRequest::default(),
            ))
            .is_err()
    );
}

#[cfg(feature = "svg")]
#[test]
fn lean_artifact_cannot_be_promoted_to_graphical_output() {
    let artifact = Renderer::new()
        .parse_semantic("flowchart TD\nA-->B", OperationControl::new())
        .unwrap()
        .unwrap();
    assert!(matches!(
        artifact.render(merman::RenderTarget::Svg(SvgRequest::default())),
        Err(merman::RenderError::UnsupportedTarget(_))
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_does_not_evaluate_visual_theme_programs() {
    let render = |source| {
        let RenderOutput::Ascii(Some(output)) = Renderer::new()
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                merman::AsciiRequest::default(),
            ))
            .expect("terminal output depends on authored semantics")
        else {
            panic!("expected terminal output");
        };
        output
    };
    assert_eq!(render(INVALID_VISUAL_THEME), render("flowchart TD\nA-->B"));
}

#[cfg(feature = "presentation")]
#[test]
fn replacing_semantic_parser_refreshes_inherited_presentation_config() {
    for warm in [false, true] {
        let renderer = Renderer::new();
        if warm {
            renderer.presentation_engine();
        }
        let renderer =
            renderer.with_semantic_parser(merman::SemanticParser::new().with_site_config(
                merman::MermaidConfig::from_value(serde_json::json!({"theme": "dark"})),
            ));
        let artifact = renderer
            .prepare_presentation("flowchart TD\nA-->B", OperationControl::new())
            .unwrap()
            .unwrap();
        assert_eq!(
            artifact.metadata().effective_config.get_str("theme"),
            Some("dark"),
            "inherited presentation config must follow the parser after warm={warm}"
        );
    }
}

#[cfg(feature = "presentation")]
#[test]
fn replacing_semantic_parser_refreshes_inherited_presentation_runtime() {
    const UNIX_MILLIS: i64 = 1_771_113_600_000;
    for warm in [false, true] {
        let renderer = Renderer::new();
        if warm {
            renderer.presentation_engine();
        }
        let renderer = renderer.with_semantic_parser(
            merman::SemanticParser::new().with_runtime_policy(
                merman::model::runtime::RuntimePolicy::deterministic()
                    .with_fixed_unix_millis(UNIX_MILLIS),
            ),
        );
        let prepared = renderer
            .prepare_source(merman::PrepareSourceRequest::new(
                "flowchart TD\nA-->B",
                OperationControl::new(),
            ))
            .unwrap()
            .unwrap();
        assert_eq!(
            prepared.parse_context().unix_millis(),
            UNIX_MILLIS,
            "inherited presentation runtime must follow the parser after warm={warm}"
        );
    }
}

#[cfg(feature = "presentation")]
#[test]
fn replacing_semantic_parser_retains_explicit_presentation_configuration() {
    const UNIX_MILLIS: i64 = 1_771_113_600_000;
    let engine = merman::MermaidEngine::new().with_site_config(
        merman::SourcePresentationConfig::from_value(serde_json::json!({"theme": "dark"})),
    );
    let renderer = Renderer::new()
        .with_presentation_engine(engine)
        .with_semantic_parser(
            merman::SemanticParser::new().with_runtime_policy(
                merman::model::runtime::RuntimePolicy::deterministic()
                    .with_fixed_unix_millis(UNIX_MILLIS),
            ),
        );
    let artifact = renderer
        .prepare_presentation("flowchart TD\nA-->B", OperationControl::new())
        .unwrap()
        .unwrap();
    assert_eq!(
        artifact.metadata().effective_config.get_str("theme"),
        Some("dark")
    );
    assert_eq!(
        artifact
            .into_prepared_source()
            .parse_context()
            .unix_millis(),
        0
    );
    assert_eq!(
        renderer
            .semantic_parser()
            .begin_operation()
            .unwrap()
            .unix_millis(),
        UNIX_MILLIS
    );
}

#[cfg(feature = "presentation")]
#[test]
fn presentation_artifact_observes_late_cancellation_and_deadline() {
    let renderer = Renderer::new();
    for reason in [
        merman::CancelReason::Requested,
        merman::CancelReason::DeadlineExceeded,
    ] {
        let control = OperationControl::new();
        let artifact = renderer
            .prepare_presentation("flowchart TD\nA-->B", control.clone())
            .unwrap()
            .unwrap();
        match reason {
            merman::CancelReason::Requested => control.cancel(),
            merman::CancelReason::DeadlineExceeded => {
                assert!(control.set_deadline(std::time::Duration::ZERO));
            }
        }
        let error = artifact
            .render(merman::RenderTarget::Presentation)
            .expect_err("presentation output must observe the original operation's terminal state");
        assert!(matches!(
            error,
            merman::RenderError::Cancelled(cancelled)
                if cancelled.reason == reason
                    && cancelled.phase == merman::OperationPhase::Semantic
        ));
        assert!(matches!(
            renderer
                .render(RenderRequest::presentation(
                    "flowchart TD\nA-->B",
                    OperationControl::new(),
                ))
                .unwrap(),
            RenderOutput::Presentation(Some(_))
        ));
    }
}
