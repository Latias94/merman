#![cfg(feature = "diagram-zenuml")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiGraphPresentation, AsciiLayoutProfile,
    AsciiOutput, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions, AsciiRenderer,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiViewportPolicy, OverflowPolicy,
    TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::zenuml::{ZenumlDiagramModel, ZenumlStatementKind};
use merman_model::{
    OperationControl, ParsedSemanticDiagram, SemanticDiagramModel, runtime::RuntimePolicy,
};

const BASIC: &str = "zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n";

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("ZenUML source parses")
}

fn model(source: &str) -> ZenumlDiagramModel {
    let SemanticDiagramModel::Zenuml(model) = parse(source).model else {
        panic!("ZenUML model")
    };
    model
}

fn render(
    source: &str,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)?.render_parsed_report(
        &parse(source),
        viewport,
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default(),
    )
}

fn detached(model: &ZenumlDiagramModel) -> AsciiOutput {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let config = merman_model::MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::Zenuml(model),
            merman_ascii::AsciiSourceContext {
                diagram_type: "zenuml",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn scene(output: &AsciiOutput) -> &str {
    assert_eq!(
        output.projection,
        AsciiProjection::Diagrammatic,
        "{}",
        output.text
    );
    output
        .text
        .split_once("ZenUML records")
        .expect("complete records")
        .0
}

fn compact(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn position(text: &str, needle: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(y, row)| row.find(needle).map(|x| (x, y)))
        .unwrap_or_else(|| panic!("missing {needle}:\n{text}"))
}

fn lifeline_center(text: &str, participant: &str) -> usize {
    let rows: Vec<_> = text.lines().collect();
    let (x, y) = position(text, participant);
    let border = rows[y - 1];
    let left = border[..x].rfind('+').unwrap();
    let right = left + 1 + border[left + 1..].find('+').unwrap();
    (left + right) / 2
}

#[test]
fn declared_order_identity_and_arrow_styles_remain_visible() {
    let output = render(
        "zenuml\nBob\nAlice\nAlice->Bob: Hello\nBob-->Alice: Reply",
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let text = scene(&output);
    assert!(
        position(text, "P1 Bob").0 < position(text, "P2 Alice").0,
        "{text}"
    );
    assert!(
        text.contains("S1 async: Hello") && text.contains("S2 return: Reply"),
        "{text}"
    );
    let rows: Vec<_> = text.lines().collect();
    let async_row = position(text, "S1 async").1 + 1;
    let reply_row = position(text, "S2 return").1 + 1;
    assert!(rows[async_row].contains("<---"), "{text}");
    assert!(rows[reply_row].contains("...>"), "{text}");
    assert_eq!(
        support::terminal_extent_with_profile(&output.text, output.width_profile),
        (output.emitted_extent.width, output.emitted_extent.height)
    );
}

#[test]
fn message_markers_land_on_the_resolved_lifeline_in_both_directions_and_self_calls() {
    for (source, caption, destination, self_message) in [
        ("zenuml\nA\nB\nA->B: ping", "S1 async: ping", "P2 B", false),
        ("zenuml\nA\nB\nB->A: ping", "S1 async: ping", "P1 A", false),
        (
            "zenuml\nA\nB\nA-->B: done",
            "S1 return: done",
            "P2 B",
            false,
        ),
        (
            "zenuml\nA\nB\nB-->A: done",
            "S1 return: done",
            "P1 A",
            false,
        ),
        (
            "zenuml\nA\nB\n@Starter(A)\nB.call()",
            "S1 call: call()",
            "P2 B",
            false,
        ),
        (
            "zenuml\nA\nB\n@Starter(B)\nA.call()",
            "S1 call: call()",
            "P1 A",
            false,
        ),
        ("zenuml\nA->A: ping", "S1 async: ping", "P1 A", true),
        (
            "zenuml\n@Starter(A)\nA.call()",
            "S1 call: call()",
            "P1 A",
            true,
        ),
        ("zenuml\nA-->A: done", "S1 return: done", "P1 A", true),
    ] {
        let output = render(
            source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        let text = scene(&output);
        let rows: Vec<_> = text.lines().collect();
        let (x, y) = position(text, destination);
        let border = rows[y - 1];
        let left = border[..x].rfind('+').unwrap();
        let right = left + 1 + border[left + 1..].find('+').unwrap();
        let center = (left + right) / 2;
        let caption_y = position(text, caption).1;
        let arrow_y = caption_y + if self_message { 3 } else { 1 };
        assert!(
            matches!(rows[arrow_y].chars().nth(center), Some('<' | '>')),
            "{text}"
        );
        if self_message {
            assert!(
                rows[arrow_y]
                    .chars()
                    .skip(center + 1)
                    .any(|ch| matches!(ch, '-' | '.')),
                "{text}"
            );
        }
    }
}

#[test]
fn nested_sync_self_async_and_lexical_returns_keep_event_order() {
    let source = "zenuml\n@Starter(Client)\nr = A.outer() {\n B.inner()\n local()\n B->C: notify\n return done\n}\nClient->C: after";
    let output = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let text = scene(&output);
    let mut previous = 0;
    for value in [
        "S1 call:",
        "S2 call:",
        "S3 call:",
        "S4 async:",
        "S5 return:",
        "S1 result: r",
        "[end S1]",
        "S6 async:",
    ] {
        let y = position(text, value).1;
        assert!(y >= previous, "{value}: {text}");
        previous = y;
    }
    assert!(text.contains('#'), "{text}");
    let records = compact(output.text.split_once("ZenUML records").unwrap().1);
    assert!(records.contains("parent=S1"), "{}", output.text);
    assert!(records.contains("done"));
}

#[test]
fn creation_delays_header_and_lifeline_and_returns_assignment() {
    let source = "zenuml\n@Starter(Client)\nClient->Service: before\nr = new Widget(x) {\n local()\n}\n\"r:Widget\".work()";
    let output = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let text = scene(&output);
    let top = position(text, "r:Widget").1;
    assert!(top > position(text, "before").1, "{text}");
    let rows: Vec<_> = text.lines().collect();
    let border = rows[top - 1];
    let x = (border.find('+').expect("created left border")
        + border.rfind('+').expect("created right border"))
        / 2;
    assert!(
        rows[..top - 1]
            .iter()
            .all(|row| row.chars().nth(x) != Some('|')),
        "{text}"
    );
    assert!(
        rows.iter()
            .rev()
            .find(|row| !row.is_empty())
            .unwrap()
            .chars()
            .nth(x)
            == Some('|'),
        "{text}"
    );
    assert!(
        text.contains("S2 new: r =") && text.contains("S2 result: r"),
        "{text}"
    );
    assert!(output.text.contains("Widget") && output.text.contains("constructor"));
}

#[test]
fn every_control_kind_and_empty_sections_has_an_explicit_frame() {
    let cases = [
        ("while(x) { A.call() }", "loop"),
        ("if(a) { A.call() } else if(b) {} else { B.call() }", "alt"),
        ("par(x) { A.call() { C.inner() } B.call() }", "par"),
        ("opt(x) {}", "opt"),
        ("critical(x) { A.call() }", "critical"),
        ("section(Phase) { A.call() }", "section"),
        (
            "try { A.call() } catch(E e) {} catch { B.call() } finally {}",
            "tcf",
        ),
    ];
    for (body, kind) in cases {
        let output = render(
            &format!("zenuml\n@Starter(Client)\n{body}"),
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        let text = scene(&output);
        assert!(
            text.contains(&format!("S1 {kind}")) && text.contains("end S1"),
            "{text}"
        );
        if kind == "par" {
            assert!(
                text.contains("parallel item 1") && text.contains("parallel item 2"),
                "{text}"
            );
            assert!(!text.contains("parallel item 3"));
        }
        if kind == "alt" {
            assert!(text.contains("branch 3: else"), "{text}");
        }
        if kind == "tcf" {
            assert!(text.contains("branch 4: finally"), "{text}");
        }
    }
}

#[test]
fn groups_references_and_dividers_keep_scope_and_order() {
    let source = "zenuml\ngroup Services { A B }\n@Starter(Client)\nA.outer() {\n ref(Order,B)\n == Phase two ==\n ref(Resume)\n}\n";
    let output = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let text = scene(&output);
    assert!(
        text.contains("G1 Services") && text.contains("S2 ref (selected): Order"),
        "{text}"
    );
    assert!(
        position(text, "S2 ref").1 < position(text, "S3 Phase two").1,
        "{text}"
    );
    assert!(text.contains("S4 ref (context): Resume"), "{text}");
    assert!(output.text.contains("participantNames"));
}

#[test]
fn reference_frames_stay_with_their_selection_and_lexical_origin() {
    for (reference, caption) in [
        ("ref(Check,C)", "S2 ref (selected): Check"),
        ("ref(Check)", "S2 ref (context): Check"),
    ] {
        let source = format!("zenuml\nA\nB\nC\nD\nE\n@Starter(B)\nB.outer(){{ {reference} }}");
        let output = render(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap();
        let text = scene(&output);
        let rows: Vec<_> = text.lines().collect();
        let top = rows[position(text, caption).1 - 1];
        let left = top.find('+').unwrap();
        let right = top.rfind('+').unwrap();
        assert!(left > lifeline_center(text, "P1 A"), "{text}");
        assert!(right < lifeline_center(text, "P4 D"), "{text}");
        assert!(
            left < lifeline_center(text, "P2 B") && right > lifeline_center(text, "P2 B"),
            "{text}"
        );
        if reference.contains(',') {
            assert!(right > lifeline_center(text, "P3 C"), "{text}");
        } else {
            assert!(right < lifeline_center(text, "P3 C"), "{text}");
        }
    }
}

#[test]
fn parallel_item_separators_enclose_the_whole_nested_call_and_assignment_return() {
    let output = render(
        "zenuml\n@Starter(Client)\npar { r = A.outer(){ B.call() } C.call() }",
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let text = scene(&output);
    let first = position(text, "parallel item 1").1;
    let second = position(text, "parallel item 2").1;
    for caption in ["S2 call:", "S3 call:", "S2 result:", "[end S2]"] {
        let y = position(text, caption).1;
        assert!(first < y && y < second, "{caption}: {text}");
    }
    assert!(position(text, "S4 call:").1 > second, "{text}");
}

#[test]
fn selected_source_fixture_corpus_is_diagrammatic() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/zenuml");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "mmd") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let output = render(
            &source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::unrestricted(),
        )
        .unwrap_or_else(|error| panic!("{path:?}: {error:?}"));
        assert_eq!(
            output.projection,
            AsciiProjection::Diagrammatic,
            "{path:?}: {}",
            output.text
        );
        count += 1;
    }
    assert!(count >= 17);
}

#[test]
fn ambiguous_identities_endpoints_groups_and_lifetimes_use_complete_primaries() {
    let base = model(BASIC);
    let mut cases = Vec::new();
    let mut duplicate = base.clone();
    duplicate
        .participants
        .push(duplicate.participants[0].clone());
    cases.push(duplicate);
    let mut missing = base.clone();
    if let ZenumlStatementKind::Message { resolved_to, .. } = &mut missing.statements[0].kind {
        *resolved_to = Some("missing".into());
    }
    cases.push(missing);
    let mut group = base;
    group.participants[0].group_id = Some("missing-group".into());
    cases.push(group);
    cases.push(model("zenuml\nnew A\nnew A"));
    cases.push(model("zenuml\nA->B: used\nnew B"));
    cases.push(model("zenuml\nif(x) { new A }"));
    cases.push(model("zenuml\n@Starter(A)\nmethod()"));
    cases.push(model("zenuml\nref(UnknownContext)"));
    cases.push(model("zenuml\n@Starter(Client)\nref(Pre,A)\nnew A"));
    let nested = "zenuml\n@Starter(Client)\nA.outer(){ B.inner(){ return value } }";
    let mut missing_sender = model(nested);
    if let ZenumlStatementKind::Message { body, .. } = &mut missing_sender.statements[0].kind
        && let ZenumlStatementKind::Message { resolved_from, .. } = &mut body[0].kind
    {
        *resolved_from = None;
    }
    cases.push(missing_sender);
    let mut missing_return = model(nested);
    if let ZenumlStatementKind::Message { body, .. } = &mut missing_return.statements[0].kind
        && let ZenumlStatementKind::Message { body, .. } = &mut body[0].kind
        && let ZenumlStatementKind::Return { resolved_to, .. } = &mut body[0].kind
    {
        *resolved_to = None;
    }
    cases.push(missing_return);
    for model in cases {
        let output = detached(&model);
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        assert!(output.text.contains("ZenUML records"));
        for participant in &model.participants {
            assert!(compact(&output.text).contains(&compact(&participant.name)));
        }
    }
}

#[test]
fn width_fallback_is_complete_at_representative_terminal_widths() {
    let source =
        "zenuml\n@Starter(Client)\nA.outer(){ B.inner(){ C.call(){ D.call(){ E.work() } } } }";
    let natural = render(
        source,
        AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .unwrap();
    let baseline = compact(natural.text.split_once("ZenUML records").unwrap().1);
    for width in [10, 60, 80, 100, 120] {
        let allowed = render(
            source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width),
        )
        .unwrap();
        let output = render(
            source,
            AsciiRenderOptions::ascii(),
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        )
        .unwrap();
        assert!(output.emitted_extent.width <= width, "{}", output.text);
        assert_eq!(output.primary_extent, allowed.primary_extent);
        if allowed.emitted_extent.width > width {
            assert_eq!(output.projection, AsciiProjection::StructuredText);
            let records = compact(&output.text);
            assert_eq!(records.strip_prefix("family:zenuml").unwrap(), baseline);
        } else {
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert_eq!(output.text, allowed.text);
            assert_eq!(
                compact(output.text.split_once("ZenUML records").unwrap().1),
                baseline
            );
        }
    }
}

#[test]
fn cjk_and_all_primary_encodings_preserve_authored_text_and_measured_extent() {
    let source = "zenuml\nA as \"接收请求\"\nB as \"写入队列\"\nA->B: 校验 <请求> & 条件\n";
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        for color in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let output = render(
                source,
                AsciiRenderOptions::unicode()
                    .with_color_mode(color)
                    .with_terminal_width_profile(profile),
                AsciiViewportPolicy::unrestricted(),
            )
            .unwrap();
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert!(output.text.contains("接收请求") && output.text.contains("写入队列"));
            if color == AsciiColorMode::Plain {
                assert_eq!(
                    support::terminal_extent_with_profile(&output.text, profile),
                    (output.emitted_extent.width, output.emitted_extent.height)
                );
            }
            if profile == TerminalWidthProfile::Cjk {
                assert!(!output.text.contains('│'));
            }
            if color == AsciiColorMode::Html {
                assert!(output.text.contains("&lt;请求&gt;") && output.text.contains("&amp;"));
            }
        }
    }
}

#[test]
fn unsupported_profiles_and_styled_fallback_are_rejected() {
    for profile in [AsciiLayoutProfile::Auto, AsciiLayoutProfile::Compact] {
        assert!(
            render(
                BASIC,
                AsciiRenderOptions::ascii().with_layout_profile(profile),
                AsciiViewportPolicy::unrestricted()
            )
            .is_err()
        );
    }
    assert!(
        render(
            BASIC,
            AsciiRenderOptions::ascii().with_graph_presentation(AsciiGraphPresentation::Adaptive),
            AsciiViewportPolicy::unrestricted()
        )
        .is_err()
    );
    assert!(
        render(
            BASIC,
            AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html),
            AsciiViewportPolicy::with_max_width(100).overflow(OverflowPolicy::Fallback)
        )
        .is_err()
    );
}

#[test]
fn resource_cancellation_and_deadline_failures_remain_errors() {
    let parsed = parse("zenuml\n@Starter(Client)\nA.outer(){if(x){ B.inner() }}");
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    for limit in [
        AsciiResourceLimitId::MaxGridCells,
        AsciiResourceLimitId::MaxOutputBytes,
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        AsciiResourceLimitId::MaxNestingDepth,
        AsciiResourceLimitId::MaxDocumentCells,
    ] {
        let result = renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default().with_limit(limit, 1).unwrap(),
        );
        assert!(
            matches!(&result,Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit),
            "{limit:?}: {result:?}"
        );
    }
    for after in [0, 32, 200] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(after);
        let result = renderer.render_parsed_report(
            &parsed,
            AsciiViewportPolicy::with_max_width(10).overflow(OverflowPolicy::Fallback),
            &control,
            &context,
            AsciiResourcePolicy::default(),
        );
        assert!(
            matches!(result, Err(AsciiError::Cancelled(_))),
            "{result:?}"
        );
    }
    let result = renderer.render_parsed_report(
        &parse("zenuml\nA->B: 👩‍💻"),
        AsciiViewportPolicy::unrestricted(),
        &OperationControl::new(),
        &context,
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
            .unwrap(),
    );
    assert!(
        matches!(&result, Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxGraphemeBytes),
        "{result:?}"
    );
    let control = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    let result = renderer.render_parsed_report(
        &parsed,
        AsciiViewportPolicy::unrestricted(),
        &control,
        &context,
        AsciiResourcePolicy::default(),
    );
    assert!(
        matches!(result, Err(AsciiError::Cancelled(_))),
        "{result:?}"
    );
}

#[test]
fn record_only_wide_models_are_stopped_during_preflight() {
    let mut model = model("zenuml\nopt(x) {}");
    model.participants.clear();
    if let ZenumlStatementKind::Fragment { sections, .. } = &mut model.statements[0].kind {
        sections.resize(8_192, sections[0].clone());
    }
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    let error = renderer
        .render_model(
            &AsciiDiagram::Zenuml(&model),
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        )
        .unwrap_err();
    assert!(
        matches!(&error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual < 100),
        "{error:?}"
    );
    let control = OperationControl::new();
    control.cancel_after_checkpoints(32);
    let error = renderer
        .render_model(
            &AsciiDiagram::Zenuml(&model),
            &control,
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap_err();
    assert!(matches!(error, AsciiError::Cancelled(_)), "{error:?}");
}
