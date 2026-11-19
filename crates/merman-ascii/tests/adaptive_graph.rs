#![cfg(feature = "diagram-flowchart")]

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiExtent, AsciiFallbackReason,
    AsciiGraphPresentation, AsciiLayoutProfile, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiViewportPolicy, Lossiness, OverflowPolicy,
};
use merman_core::SemanticParser;
use merman_model::{OperationControl, runtime::RuntimePolicy};

fn capacity_source() -> String {
    let mut source = String::from("flowchart LR\nA@{ shape: f-circ }\nB[Target 🦀]\n");
    for _ in 0..97 {
        source.push_str("A o--o B\n");
    }
    source
}

fn report(
    source: &str,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<merman_ascii::AsciiOutput> {
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        &parsed,
        viewport,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

fn adaptive() -> AsciiRenderOptions {
    AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive)
}

#[test]
fn capacity_recovery_has_no_invented_primary_geometry_or_overflow() {
    let source = capacity_source();
    assert!(matches!(
        report(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        ),
        Err(AsciiError::PortCapacityExceeded { .. })
    ));
    let output = report(&source, adaptive(), AsciiViewportPolicy::unrestricted()).unwrap();
    assert_eq!(output.schema_version, 4);
    assert_eq!(output.primary_extent, None);
    assert_eq!(output.primary_overflowed, None);
    assert_eq!(output.layout_profile, None);
    assert_eq!(
        output.requested_layout_profile,
        AsciiLayoutProfile::Canonical
    );
    assert_eq!(
        output.requested_graph_presentation,
        AsciiGraphPresentation::Adaptive
    );
    assert!(!output.compact_attempted);
    assert!(!output.emitted_overflowed);
    assert_eq!(output.projection, AsciiProjection::StructuredText);
    assert_eq!(output.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(output.lossiness, Lossiness::PresentationOnly);
    assert_eq!(
        output.fallback.reason,
        Some(AsciiFallbackReason::PortCapacityExceeded)
    );
    for index in 1..=97 {
        assert!(
            output.text.contains(&format!("E{index}:")),
            "missing edge {index}"
        );
    }
    let metadata = serde_json::to_value(output.metadata()).unwrap();
    assert!(metadata["primary_width"].is_null());
    assert!(metadata["primary_height"].is_null());
    assert!(metadata["primary_overflowed"].is_null());
    assert!(metadata["layout_profile"].is_null());
    assert_eq!(metadata["requested_graph_presentation"], "adaptive");
    assert_eq!(metadata["fallback_reason"], "port_capacity_exceeded");
    assert!(metadata.get("overflowed").is_none());
}

#[test]
fn valid_primary_and_real_empty_geometry_remain_measured() {
    for source in ["flowchart LR\nA --> B", "flowchart LR"] {
        let output = report(source, adaptive(), AsciiViewportPolicy::unrestricted()).unwrap();
        assert_eq!(output.projection, AsciiProjection::Diagrammatic);
        assert_eq!(output.primary_extent, Some(output.emitted_extent));
        assert_eq!(output.primary_overflowed, Some(false));
        assert_eq!(output.layout_profile, Some(AsciiLayoutProfile::Canonical));
        assert!(!output.fallback.attempted);
        if source == "flowchart LR" {
            assert_eq!(output.primary_extent, Some(AsciiExtent::new(0, 0)));
            assert_eq!(output.outcome, AsciiOutputOutcome::Empty);
            let metadata = serde_json::to_value(output.metadata()).unwrap();
            assert_eq!(metadata["primary_width"], 0);
            assert_eq!(metadata["primary_height"], 0);
            assert_eq!(metadata["primary_overflowed"], false);
            assert_eq!(metadata["layout_profile"], "canonical");
        }
    }
}

#[test]
fn adaptive_styled_requests_are_rejected_before_graph_work_in_both_entrypoints() {
    let parsed = SemanticParser::new()
        .parse_diagram_sync("flowchart LR\nA --> B")
        .unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    for color in [AsciiColorMode::Ansi16, AsciiColorMode::Html] {
        let options = adaptive().with_color_mode(color);
        let renderer = AsciiRenderer::new(options).unwrap();
        for result in [
            renderer.render_parsed(&parsed, &OperationControl::new(), &context, resources),
            renderer
                .render_parsed_report(
                    &parsed,
                    AsciiViewportPolicy::with_max_width(80),
                    &OperationControl::new(),
                    &context,
                    resources,
                )
                .map(|output| output.text),
        ] {
            assert!(matches!(
                result,
                Err(AsciiError::InvalidOption {
                    field: "graph_presentation",
                    ..
                })
            ));
        }
    }
}

#[test]
fn width_policy_applies_to_the_complete_recovery_document() {
    let source = capacity_source();
    let natural = report(&source, adaptive(), AsciiViewportPolicy::unrestricted()).unwrap();
    let width = natural.emitted_extent.width - 1;
    let allowed = report(
        &source,
        adaptive(),
        AsciiViewportPolicy::with_max_width(width),
    )
    .unwrap();
    assert_eq!(allowed.text, natural.text);
    assert_eq!(allowed.primary_extent, None);
    assert_eq!(allowed.primary_overflowed, None);
    assert!(allowed.emitted_overflowed);
    assert_eq!(allowed.outcome, AsciiOutputOutcome::Fallback);
    assert!(matches!(
        report(
            &source,
            adaptive(),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error),
        ),
        Err(AsciiError::WidthOverflow { actual_width, max_width, .. })
            if actual_width == natural.emitted_extent.width && max_width == width
    ));
    let reflowed = report(
        &source,
        adaptive(),
        AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
    )
    .unwrap();
    assert_eq!(reflowed.primary_overflowed, None);
    assert!(!reflowed.emitted_overflowed);
    assert!(reflowed.emitted_extent.width <= 80);
    assert_eq!(
        reflowed.emitted_extent.height,
        reflowed.text.lines().count()
    );
    assert_eq!(reflowed.fallback.reason, natural.fallback.reason);
    assert!(matches!(
        report(
            &source,
            adaptive(),
            AsciiViewportPolicy::with_max_width(1).overflow(OverflowPolicy::Fallback),
        ),
        Err(AsciiError::FallbackUnavailable { max_width: 1, actual_width, .. })
            if actual_width > 1
    ));
    for overflow in [
        OverflowPolicy::Allow,
        OverflowPolicy::Error,
        OverflowPolicy::Fallback,
    ] {
        let output = report(
            &source,
            adaptive(),
            AsciiViewportPolicy::unrestricted().overflow(overflow),
        )
        .unwrap();
        assert_eq!(output.text, natural.text);
        assert!(!output.emitted_overflowed);
        assert_eq!(output.primary_overflowed, None);
    }
}

#[test]
fn missing_primary_keeps_the_requested_profile_and_actual_compact_attempt() {
    let source = capacity_source();
    for profile in [
        AsciiLayoutProfile::Canonical,
        AsciiLayoutProfile::Compact,
        AsciiLayoutProfile::Auto,
    ] {
        let output = report(
            &source,
            adaptive().with_layout_profile(profile),
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert_eq!(output.primary_extent, None);
        assert_eq!(output.layout_profile, None);
        assert_eq!(output.requested_layout_profile, profile);
        assert_eq!(
            output.compact_attempted,
            profile == AsciiLayoutProfile::Auto
        );
    }
}

#[test]
fn parsed_and_borrowed_text_entrypoints_share_adaptive_selection() {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(&capacity_source())
        .unwrap();
    let merman_model::SemanticDiagramModel::Flowchart(model) = &parsed.model else {
        panic!("the fixture must be a flowchart");
    };
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let renderer = AsciiRenderer::new(adaptive()).unwrap();
    let parsed_text = renderer
        .render_parsed(
            &parsed,
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap();
    let borrowed_text = renderer
        .render_model(
            &AsciiDiagram::Flowchart(model),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap();
    // Detached models use their model kind; parsed input retains its declared language identity.
    let (borrowed_family, borrowed_document) = borrowed_text.split_once('\n').unwrap();
    let (parsed_family, parsed_document) = parsed_text.split_once('\n').unwrap();
    assert_eq!(borrowed_family, "family: flowchart");
    assert_eq!(
        parsed_family,
        format!("family: {}", parsed.metadata.diagram_type)
    );
    assert_eq!(borrowed_document, parsed_document);
    assert!(borrowed_text.contains("Target 🦀"));
    assert!(borrowed_text.contains("E97:"));
}

#[test]
fn adaptive_keeps_invalid_semantics_resource_and_cancellation_errors() {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let renderer = AsciiRenderer::new(adaptive()).unwrap();
    let mut parsed = SemanticParser::new()
        .parse_diagram_sync("flowchart LR\nA --> B")
        .unwrap();
    let merman_model::SemanticDiagramModel::Flowchart(model) = &mut parsed.model else {
        panic!("the fixture must be a flowchart");
    };
    model.edges[0].to = "missing".into();
    assert!(matches!(
        renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        ),
        Err(AsciiError::UnsupportedFeature {
            feature: "edges with missing endpoint nodes",
            ..
        })
    ));

    let parsed = SemanticParser::new()
        .parse_diagram_sync(&capacity_source())
        .unwrap();
    for limit in [
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        AsciiResourceLimitId::MaxDocumentCells,
    ] {
        let result = renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default().with_limit(limit, 1).unwrap(),
        );
        assert!(matches!(
            result,
            Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit
        ));
    }
    let control = OperationControl::new();
    control.cancel();
    assert!(matches!(
        renderer.render_parsed(&parsed, &control, &context, AsciiResourcePolicy::default()),
        Err(AsciiError::Cancelled(_))
    ));
}

#[cfg(feature = "diagram-state")]
#[test]
fn adaptive_state_remains_canonical_only() {
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> [*]";
    let output = report(source, adaptive(), AsciiViewportPolicy::with_max_width(80)).unwrap();
    assert_eq!(output.layout_profile, Some(AsciiLayoutProfile::Canonical));
    for profile in [AsciiLayoutProfile::Compact, AsciiLayoutProfile::Auto] {
        assert!(matches!(
            report(
                source,
                adaptive().with_layout_profile(profile),
                AsciiViewportPolicy::with_max_width(80),
            ),
            Err(AsciiError::InvalidOption {
                field: "layout_profile",
                ..
            })
        ));
    }
}

#[cfg(feature = "diagram-sequence")]
#[test]
fn adaptive_does_not_promote_other_supported_families() {
    assert!(matches!(
        report(
            "sequenceDiagram\nAlice->>Bob: ping",
            adaptive(),
            AsciiViewportPolicy::unrestricted(),
        ),
        Err(AsciiError::InvalidOption {
            field: "graph_presentation",
            ..
        })
    ));
}

#[test]
fn unsupported_models_keep_the_original_diagram_error_under_both_policies() {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    for presentation in [
        AsciiGraphPresentation::Strict,
        AsciiGraphPresentation::Adaptive,
    ] {
        let renderer =
            AsciiRenderer::new(AsciiRenderOptions::ascii().with_graph_presentation(presentation))
                .unwrap();
        assert!(matches!(
            renderer.render_model(
                &AsciiDiagram::Unsupported("radar"),
                &OperationControl::new(),
                &context,
                AsciiResourcePolicy::default(),
            ),
            Err(AsciiError::UnsupportedDiagram { diagram_type }) if diagram_type == "radar"
        ));
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn adaptive_swimlane_preserves_its_logical_family_and_edges() {
    let source = capacity_source().replacen("flowchart", "swimlane-beta", 1);
    let output = report(
        &source,
        adaptive(),
        AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
    )
    .unwrap();
    assert_eq!(output.family, "swimlane");
    assert_eq!(output.primary_extent, None);
    assert_eq!(
        output.fallback.reason,
        Some(AsciiFallbackReason::PortCapacityExceeded)
    );
    assert!(output.text.starts_with("family: swimlane\n"));
    assert!(output.text.contains("Target 🦀"));
    for index in 1..=97 {
        assert!(output.text.contains(&format!("E{index}:")));
    }
}
