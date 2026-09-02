use merman_ascii::{
    ASCII_OUTPUT_SCHEMA_VERSION, AsciiColorMode, AsciiError, AsciiLayoutProfile,
    AsciiOutputEncoding, AsciiOutputOutcome, AsciiOverflowPolicy, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiViewportPolicy, OverflowPolicy,
};
use merman_core::diagram::RenderSemanticModel;
use merman_core::{Engine, OperationControl, ParseOptions};

fn parsed_model(source: &str) -> merman_core::diagram::ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected")
}

fn render_report(
    source: &str,
    policy: AsciiViewportPolicy,
) -> merman_ascii::Result<merman_ascii::AsciiOutput> {
    render_report_with_resources(source, policy, merman_ascii::AsciiResourcePolicy::default())
}

fn render_report_with_resources(
    source: &str,
    policy: AsciiViewportPolicy,
    resources: merman_ascii::AsciiResourcePolicy,
) -> merman_ascii::Result<merman_ascii::AsciiOutput> {
    render_report_with_options_and_resources(source, AsciiRenderOptions::ascii(), policy, resources)
}

fn render_report_with_options(
    source: &str,
    options: AsciiRenderOptions,
    policy: AsciiViewportPolicy,
) -> merman_ascii::Result<merman_ascii::AsciiOutput> {
    render_report_with_options_and_resources(
        source,
        options,
        policy,
        merman_ascii::AsciiResourcePolicy::default(),
    )
}

fn render_report_with_options_and_resources(
    source: &str,
    options: AsciiRenderOptions,
    policy: AsciiViewportPolicy,
    resources: merman_ascii::AsciiResourcePolicy,
) -> merman_ascii::Result<merman_ascii::AsciiOutput> {
    let parsed = parsed_model(source);
    let operation = Engine::new()
        .begin_operation()
        .expect("operation context should be available");
    AsciiRenderer::new(options)?.render_parsed_report(
        &parsed,
        policy,
        &OperationControl::new(),
        &operation,
        resources,
    )
}

#[test]
fn default_report_preserves_text_projection_and_records_extent() {
    let report = render_report("flowchart LR\nA --> B", AsciiViewportPolicy::default())
        .expect("default report should render");
    assert_eq!(report.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(report.projection, AsciiProjection::Diagrammatic);
    assert_eq!(report.primary_extent, report.emitted_extent);
    assert_eq!(report.primary_extent.width, 15);
    assert_eq!(report.primary_extent.height, 5);
    assert!(!report.overflowed);
    assert!(!report.fallback.attempted);
    assert_eq!(report.schema_version, ASCII_OUTPUT_SCHEMA_VERSION);
    assert_eq!(report.encoding, AsciiOutputEncoding::Plain);
    assert_eq!(report.as_text(), report.text);
}

#[test]
fn report_metadata_identifies_the_exact_output_encoding() {
    for (color_mode, expected) in [
        (AsciiColorMode::Plain, AsciiOutputEncoding::Plain),
        (AsciiColorMode::Ansi16, AsciiOutputEncoding::Ansi16),
        (AsciiColorMode::Ansi256, AsciiOutputEncoding::Ansi256),
        (AsciiColorMode::TrueColor, AsciiOutputEncoding::TrueColor),
        (AsciiColorMode::Html, AsciiOutputEncoding::Html),
    ] {
        let mut options = AsciiRenderOptions::unicode();
        options.color_mode = color_mode;
        let report = render_report_with_options(
            "flowchart LR\nA[Alpha] --> B[Beta]",
            options,
            AsciiViewportPolicy::default(),
        )
        .unwrap_or_else(|error| panic!("{color_mode:?} report should render: {error}"));

        assert_eq!(report.encoding, expected);
        assert_eq!(report.metadata().encoding, expected.as_str());
    }
}

#[test]
fn styled_output_rejects_a_plain_only_fallback_request_before_rendering() {
    let mut options = AsciiRenderOptions::unicode();
    options.color_mode = AsciiColorMode::Ansi16;
    let error = render_report_with_options(
        "flowchart LR\nA[Alpha] --> B[Beta]",
        options,
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
    )
    .expect_err("styled output cannot promise the plain-only complete fallback");

    assert_eq!(
        error,
        AsciiError::InvalidOption {
            field: "ascii_viewport.overflow",
            message: "fallback is not admitted for the selected output encoding",
        }
    );
}

#[test]
fn allow_returns_complete_wide_report() {
    let report = render_report(
        "flowchart LR\nA[Alpha] --> B[Beta]",
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Allow),
    )
    .expect("Allow should return a complete wide result");
    assert_eq!(report.outcome, AsciiOutputOutcome::WideAllowed);
    assert!(report.overflowed);
    assert!(report.primary_extent.width > 5);
    assert!(report.text.contains("Alpha"));
    assert!(report.text.contains("Beta"));
}

#[test]
fn error_policy_returns_independent_width_error() {
    let error = render_report(
        "flowchart LR\nA[Alpha] --> B[Beta]",
        AsciiViewportPolicy::with_max_width(5).overflow(AsciiOverflowPolicy::Error),
    )
    .expect_err("Error must reject an over-wide primary projection");
    assert!(matches!(
        error,
        AsciiError::WidthOverflow {
            max_width: 5,
            actual_width: _,
            ..
        }
    ));
}

#[test]
fn structured_fallback_reflows_complete_text() {
    let report = render_report(
        "timeline\ntitle Basic\n2024 : Event with a long authored value",
        AsciiViewportPolicy::with_max_width(10).overflow(OverflowPolicy::Fallback),
    )
    .expect("structured text should have a bounded fallback");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.projection, AsciiProjection::StructuredText);
    assert!(report.fallback.attempted);
    assert_eq!(
        report.fallback.reason.map(|reason| reason.as_str()),
        Some("primary_overflow")
    );
    assert!(report.emitted_extent.width <= 10);
    assert!(
        report.text.replace('\n', "").contains("Basic"),
        "{}",
        report.text
    );
    assert!(
        report.text.replace('\n', "").contains("Event"),
        "{}",
        report.text
    );
}

#[test]
fn diagrammatic_fallback_preserves_typed_semantics() {
    let report = render_report(
        "flowchart LR\nA[Alpha] --> B[Beta]",
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
    )
    .expect("typed semantic fallback should be available");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.projection, AsciiProjection::StructuredText);
    assert_eq!(report.lossiness.as_str(), "presentation_only");
    assert!(report.emitted_extent.width <= 5, "{}", report.text);
    let flattened = report.text.replace(['\n', ' ', '"'], "");
    assert!(flattened.contains("Alpha"), "{}", report.text);
    assert!(flattened.contains("Beta"), "{}", report.text);
}

#[test]
fn diagrammatic_fallback_keeps_typed_flowchart_edge_markers() {
    let report = render_report(
        "graph LR;A o--x B;",
        AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback),
    )
    .expect("typed flowchart fallback should preserve edge semantics");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    let flattened = report.text.replace('\n', "");
    assert!(flattened.contains("startMarker"), "{}", report.text);
    assert!(flattened.contains("circle"), "{}", report.text);
    assert!(flattened.contains("endMarker"), "{}", report.text);
    assert!(flattened.contains("cross"), "{}", report.text);
}

#[test]
fn diagrammatic_fallback_keeps_terminal_controls_visible() {
    let report = render_report(
        "flowchart LR\nA[\"alpha\u{202e}beta\"] --> B",
        AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback),
    )
    .expect("typed flowchart fallback should normalize terminal controls");

    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert!(report.emitted_extent.width <= 12, "{}", report.text);
    assert!(!report.text.contains('\u{202e}'), "{}", report.text);
    assert!(report.text.contains("\\u{202E}"), "{}", report.text);
}

#[test]
fn sequence_fallback_preserves_empty_authored_property_objects() {
    let source = concat!(
        "sequenceDiagram\n",
        "participant A\n",
        "properties A: {\"payload\": {}}\n",
        "participant B\n",
        "A->>B: A long authored message forcing a wide primary projection",
    );
    let report = render_report(
        source,
        AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback),
    )
    .expect("typed sequence fallback should preserve empty authored objects");

    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.lossiness.as_str(), "presentation_only");
    assert!(report.emitted_extent.width <= 12, "{}", report.text);
    let flattened = report.text.replace('\n', "");
    assert!(
        flattened.contains("model.actors.A.properties.payload: {}"),
        "{}",
        report.text
    );
}

#[test]
fn empty_extent_is_reportable_without_special_sentinel_text() {
    let model = RenderSemanticModel::Timeline(Default::default());
    let operation = Engine::new()
        .begin_operation()
        .expect("operation context should be available");
    let report = AsciiRenderer::new(AsciiRenderOptions::ascii())
        .expect("options should validate")
        .render_model_report(
            &model,
            AsciiViewportPolicy::default(),
            &OperationControl::new(),
            &operation,
            merman_ascii::AsciiResourcePolicy::default(),
        )
        .expect("empty typed model should render a report");
    assert!(!report.text.contains("<empty>"));
    assert_eq!(report.emitted_extent.height, report.text.lines().count());
}

#[test]
fn compact_profile_is_explicit_and_reported() {
    let renderer = AsciiRenderer::new(
        AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
    )
    .expect("compact options should validate");
    let parsed = parsed_model("flowchart LR\nA[Alpha] --> B[Beta]");
    let operation = Engine::new()
        .begin_operation()
        .expect("operation context should be available");
    let report = renderer
        .render_parsed_report(
            &parsed,
            AsciiViewportPolicy::default(),
            &OperationControl::new(),
            &operation,
            merman_ascii::AsciiResourcePolicy::default(),
        )
        .expect("compact profile should render");
    assert_eq!(report.layout_profile, AsciiLayoutProfile::Compact);
}

#[test]
fn compact_profile_preserves_explicit_canonical_values_from_builders() {
    let options = AsciiRenderOptions::ascii()
        .with_layout_profile(AsciiLayoutProfile::Compact)
        .with_graph_padding_x(5)
        .with_flowchart_node_label_wrap_width(40);
    let renderer = AsciiRenderer::new(options).expect("options should validate");
    let parsed = parsed_model("flowchart LR\nA[Alpha] --> B[Beta]");
    let operation = Engine::new()
        .begin_operation()
        .expect("operation context should be available");
    let report = renderer
        .render_parsed_report(
            &parsed,
            AsciiViewportPolicy::default(),
            &OperationControl::new(),
            &operation,
            merman_ascii::AsciiResourcePolicy::default(),
        )
        .expect("explicit canonical values should remain valid");
    assert_eq!(report.layout_profile, AsciiLayoutProfile::Compact);
    assert!(report.text.contains("Alpha"));
}

#[test]
fn fallback_uses_the_render_wide_work_ledger() {
    let source = "flowchart LR\nA[Alpha] --> B[Beta]";
    let resources = merman_ascii::AsciiResourcePolicy::default()
        .with_limit(
            merman_ascii::AsciiResourceLimitId::MaxLayoutWorkUnits,
            5_000,
        )
        .expect("limit should be valid");

    let error = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
        resources,
    )
    .expect_err("primary and fallback work must share one cumulative ledger");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == merman_ascii::AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual > details.max
    ));
}

#[test]
fn flowchart_fallback_rolls_back_speculative_document_cells() {
    let source = "flowchart LR\nA[Alpha] --> B[Beta]";
    let exact_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_834)
        .expect("limit should be valid");
    let report = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
        exact_resources,
    )
    .expect("flowchart fallback should not count the abandoned primary document twice");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);

    let below_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_833)
        .expect("limit should be valid");
    let error = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
        below_resources,
    )
    .expect_err("one fewer flowchart fallback document cell should fail");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == merman_ascii::AsciiResourceLimitId::MaxDocumentCells
                && details.actual == 1_834
                && details.max == 1_833
    ));
}

#[test]
fn sequence_fallback_preserves_primary_work_in_the_render_wide_ledger() {
    let source = concat!(
        "sequenceDiagram\n",
        "participant Alice\n",
        "participant Bob\n",
        "Alice->>Bob: Request with a long authored message\n",
        "Bob-->>Alice: Response with another long authored message\n",
    );
    let exact_work = 3_818;
    let exact_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(
            merman_ascii::AsciiResourceLimitId::MaxLayoutWorkUnits,
            exact_work,
        )
        .expect("limit should be valid");

    let report = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback),
        exact_resources,
    )
    .expect("the exact cumulative primary-plus-fallback work boundary should succeed");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);

    let below_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(
            merman_ascii::AsciiResourceLimitId::MaxLayoutWorkUnits,
            exact_work - 1,
        )
        .expect("limit should be valid");
    let error = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback),
        below_resources,
    )
    .expect_err("the N-1 cumulative work boundary should fail");

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == merman_ascii::AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == exact_work
                && details.max == exact_work - 1
    ));
}

#[test]
fn class_fallback_rolls_back_speculative_document_cells() {
    let source = "classDiagram\nclass A\nclass B\nA --> B : owns";
    let exact_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_803)
        .expect("limit should be valid");
    let report = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(4).overflow(OverflowPolicy::Fallback),
        exact_resources,
    )
    .expect("class fallback should not count the abandoned primary document twice");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);

    let below_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_802)
        .expect("limit should be valid");
    let error = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(4).overflow(OverflowPolicy::Fallback),
        below_resources,
    )
    .expect_err("one fewer class fallback document cell should fail");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == merman_ascii::AsciiResourceLimitId::MaxDocumentCells
                && details.actual == 1_803
                && details.max == 1_802
    ));
}

#[test]
fn er_fallback_rolls_back_speculative_document_cells() {
    let source = "erDiagram\nA\nB\nA ||--o{ B : owns";
    let exact_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_188)
        .expect("limit should be valid");
    let report = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(4).overflow(OverflowPolicy::Fallback),
        exact_resources,
    )
    .expect("ER fallback should not count the abandoned primary document twice");
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);

    let below_resources = merman_ascii::AsciiResourcePolicy::unbounded()
        .with_limit(merman_ascii::AsciiResourceLimitId::MaxDocumentCells, 1_187)
        .expect("limit should be valid");
    let error = render_report_with_resources(
        source,
        AsciiViewportPolicy::with_max_width(4).overflow(OverflowPolicy::Fallback),
        below_resources,
    )
    .expect_err("one fewer ER fallback document cell should fail");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == merman_ascii::AsciiResourceLimitId::MaxDocumentCells
                && details.actual == 1_188
                && details.max == 1_187
    ));
}

#[test]
fn canonical_report_payload_contains_the_binding_metadata_subset() {
    let report = render_report(
        "timeline\ntitle Basic\n2024 : Event",
        AsciiViewportPolicy::default(),
    )
    .expect("report should render");
    let json = serde_json::to_value(report.report()).expect("report should serialize");
    let metadata = report.metadata();

    assert_eq!(json["kind"], "ascii");
    assert_eq!(json["schema_version"], metadata.schema_version);
    assert_eq!(json["family"], metadata.family);
    assert_eq!(json["projection"], metadata.projection);
    assert_eq!(json["encoding"], metadata.encoding);
    assert_eq!(json["primary_width"], metadata.primary_width);
    assert_eq!(json["emitted_height"], metadata.emitted_height);
    assert_eq!(json["text"], report.as_text());
}
