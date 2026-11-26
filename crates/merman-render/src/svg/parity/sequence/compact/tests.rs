use super::*;

use crate::{
    environment::{RenderEnvironment, RenderSession},
    resources::{RenderResourcePolicy, ResourceLimitId},
    svg::{SvgDebugOptions, SvgRenderOptions},
    text::DeterministicTextMeasurer,
    xml_dom::{Document, Node},
};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{OperationControl, presentation::RenderSemanticModel};
use merman_render_core::text::GenericFontFamily;

const FIXTURE: &str = r#"---
config:
  look: classic
  theme: default
---
sequenceDiagram
    accTitle: Flow #lt; control
    accDescr: Two participants
    participant A
    participant B
    A->>+B: Start & check
    Note over B: One<br>Two
    B-->>-A: Done
    activate B
"#;

const SELF_MESSAGE: &str = r#"---
config:
  look: classic
  theme: default
---
sequenceDiagram
    A->>A: Self
"#;

fn with_pair<T>(
    source: &str,
    session: RenderSession,
    run: impl FnOnce(
        &SequencePreparedArtifact,
        &SequenceSvgModel,
        &PresentationConfig,
        &SvgExecution<'_>,
    ) -> T,
) -> T {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Sequence(model) = parsed.model() else {
        panic!("expected Sequence presentation model");
    };
    let config = &parsed.metadata().effective_config;
    let request = SvgRenderOptions {
        diagram_id: Some("compact-sequence".to_owned()),
        ..Default::default()
    };
    let debug = SvgDebugOptions::default();
    let execution =
        SvgExecution::unthemed_for_test(&request, &debug, &session, DiagramFamilyId::SEQUENCE)
            .unwrap();
    let prepared = crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
        model,
        None,
        config,
        execution
            .native_family_theme()
            .map(|theme| (theme, session.canvas_background())),
        &DeterministicTextMeasurer::default(),
        None,
        Arc::clone(session.work_meter()),
    )
    .unwrap();
    run(&prepared, model, config, &execution)
}

fn attributes<'tree>(node: Node<'tree, '_>, names: &[&str]) -> Vec<&'tree str> {
    names
        .iter()
        .map(|&name| node.attribute(name).unwrap())
        .collect()
}

fn first_text<'tree>(node: Node<'tree, '_>) -> &'tree str {
    node.descendants()
        .find(|child| child.is_text())
        .and_then(|child| child.text())
        .unwrap()
}

fn first_tag<'tree, 'input: 'tree>(
    document: &'tree Document<'input>,
    tag: &str,
) -> Node<'tree, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name(tag))
        .unwrap()
}

#[test]
fn compact_keeps_prepared_sequence_geometry_and_order() {
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .unwrap()
        .with_canvas_background(crate::theme::Color::rgb(242, 228, 214));
    with_pair(FIXTURE, session, |prepared, model, config, execution| {
        let meter = execution.work_meter();
        let open = prepared.model_for_render(model).messages.last().unwrap();
        assert_eq!(open.semantic_kind(), SequenceMessageKind::ActivationStart);
        assert!(matches!(
            prepared.activation_geometry().group_for_start(&open.id),
            Some(None)
        ));
        let before = meter.prepared_text_retained_bytes();
        let sidecar = prepare_sequence_compact(prepared, model, config, execution).unwrap();
        assert!(meter.prepared_text_retained_bytes() > before);
        let compact_svg = sidecar
            .write_svg(execution.diagram_id_or("merman"))
            .unwrap();
        drop(sidecar);
        assert_eq!(meter.prepared_text_retained_bytes(), before);

        let parity_svg = super::super::render_sequence_diagram_svg_model_with_config(
            prepared,
            model,
            config,
            execution.text_measurer(),
            execution,
        )
        .unwrap()
        .into_string_for(DiagramFamilyId::SEQUENCE)
        .unwrap();
        let compact = Document::parse(&compact_svg).unwrap();
        let parity = Document::parse(&parity_svg).unwrap();
        let root = compact.root_element();
        assert_eq!(
            root.attribute("viewBox"),
            parity.root_element().attribute("viewBox"),
        );
        assert_eq!(root.attribute("id"), Some("compact-sequence"));
        assert!(!compact_svg.contains("<foreignObject"));
        assert!(!compact_svg.contains("data-points"));
        assert!(compact_svg.contains("Start &amp; check"));
        for (tag, reference) in [("title", "aria-labelledby"), ("desc", "aria-describedby")] {
            let metadata = root.children().find(|node| node.has_tag_name(tag)).unwrap();
            assert_eq!(metadata.attribute("id"), root.attribute(reference));
            assert_eq!(first_text(metadata), first_text(first_tag(&parity, tag)));
        }

        let groups = root
            .children()
            .filter(|node| node.has_tag_name("g"))
            .collect::<Vec<_>>();
        assert!(
            groups
                .iter()
                .all(|group| group.first_element_child().is_some())
        );
        let roles = groups
            .iter()
            .map(|group| group.attribute("data-scene-role").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            [
                "node",
                "node",
                "node",
                "node",
                "activation",
                "note",
                "edge-label",
                "edge",
                "edge-label",
                "edge",
            ]
        );
        let canvas = root.children().filter(|node| {
            node.has_tag_name("rect") && node.attribute("data-scene-role") == Some("canvas")
        });
        let fills = canvas
            .map(|node| node.attribute("fill").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(fills, ["#F2E4D6"]);
        for (group_index, class) in [(4, "activation0"), (5, "note")] {
            let compact_rect = groups[group_index].first_element_child().unwrap();
            let parity_rect = parity
                .descendants()
                .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some(class))
                .unwrap();
            let geometry = ["x", "y", "width", "height"];
            assert_eq!(
                attributes(compact_rect, &geometry),
                attributes(parity_rect, &geometry)
            );
        }
        let note_rows = groups[5]
            .children()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        let parity_notes = parity
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("noteText"))
            .collect::<Vec<_>>();
        assert_eq!(note_rows.len(), 2);
        assert_eq!(parity_notes.len(), 2);
        for (compact_row, parity_row) in note_rows.into_iter().zip(parity_notes) {
            let placement = ["x", "y", "text-anchor", "dominant-baseline"];
            assert_eq!(
                attributes(compact_row, &placement),
                attributes(parity_row, &placement)
            );
            assert_eq!(first_text(compact_row), first_text(parity_row));
        }
        let shafts = groups
            .iter()
            .filter(|group| group.attribute("data-scene-role") == Some("edge"))
            .map(|group| group.first_element_child().unwrap())
            .collect::<Vec<_>>();
        let parity_shafts = parity
            .descendants()
            .filter(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .collect::<Vec<_>>();
        assert_eq!(shafts.len(), 2);
        assert_eq!(parity_shafts.len(), 2);
        let compact_marker = first_tag(&compact, "marker");
        let parity_marker = parity
            .descendants()
            .find(|node| node.attribute("id") == Some("compact-sequence-arrowhead"))
            .unwrap();
        let placement = [
            "refX",
            "refY",
            "markerWidth",
            "markerHeight",
            "markerUnits",
            "orient",
        ];
        assert_eq!(
            attributes(compact_marker, &placement),
            attributes(parity_marker, &placement)
        );
        assert_eq!(
            compact_marker.first_element_child().unwrap().attribute("d"),
            Some("M-1 0 L10 5 L0 10 Z")
        );
        let marker_id = compact_marker.attribute("id").unwrap();
        assert_eq!(marker_id, "compact-sequence-scene-marker-0");
        let end_marker = format!("url(#{marker_id})");
        for (shaft, parity_shaft) in shafts.iter().zip(parity_shafts) {
            let endpoints = attributes(parity_shaft, &["x1", "y1", "x2", "y2"]);
            let expected = format!(
                "M{} {} L{} {}",
                endpoints[0], endpoints[1], endpoints[2], endpoints[3]
            );
            assert_eq!(shaft.attribute("d"), Some(expected.as_str()));
            assert_eq!(shaft.attribute("marker-end"), Some(end_marker.as_str()));
        }
        assert_eq!(shafts[0].attribute("stroke-dasharray"), None);
        assert_eq!(shafts[1].attribute("stroke-dasharray"), Some("3 3"));
    });
}

#[test]
fn compact_rejects_self_messages_and_replays_an_existing_cancellation() {
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    with_pair(
        SELF_MESSAGE,
        session,
        |prepared, model, config, execution| {
            assert!(matches!(
                render_sequence_compact(prepared, model, config, execution),
                Err(SequenceCompactError::Unsupported("self-message"))
            ));
            control.cancel();
            assert!(matches!(
                render_sequence_compact(prepared, model, config, execution),
                Err(SequenceCompactError::Render(Error::Cancelled(error)))
                    if error.phase == OperationPhase::Emit
            ));
        },
    );
}

#[test]
fn compact_projection_draws_from_the_same_cumulative_work_limit() {
    const LIMIT: usize = 100_000;
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, LIMIT)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    with_pair(FIXTURE, session, |prepared, model, config, execution| {
        let meter = execution.work_meter();
        let layout = meter.used();
        assert!((1..LIMIT).contains(&layout));
        meter.charge(LIMIT - layout).unwrap();
        assert!(matches!(
            render_sequence_compact(prepared, model, config, execution),
            Err(SequenceCompactError::Render(Error::ResourceLimitExceeded(error)))
                if error.limit == "max_layout_work_units"
        ));
        assert_eq!(meter.used(), LIMIT);
        assert_eq!(meter.projected_svg_bytes(), 0);
    });
}

#[test]
fn compact_fonts_distinguish_literal_family_names_from_generics() {
    let fonts = parse_css_font_stack(r#""sans-serif", sans-serif"#).unwrap();
    assert_eq!(
        fonts.resolved_families().collect::<Vec<_>>(),
        [
            ("sans-serif", None),
            ("sans-serif", Some(GenericFontFamily::SansSerif)),
        ]
    );
}

#[test]
fn compact_keeps_native_roles_with_a_transparent_host_canvas() {
    use crate::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};

    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Prism, None)
        .unwrap();
    let resolved = theme.family("sequence");
    let actor_fill = resolved.roles.surface_node.first_color().to_string();
    let signal_stroke = resolved.roles.stroke_connector.first_color().to_string();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(Color::TRANSPARENT);
    with_pair(FIXTURE, session, |prepared, model, config, execution| {
        let svg = render_sequence_compact(prepared, model, config, execution).unwrap();
        assert!(!svg.contains(r#"data-scene-role="canvas""#));
        let document = Document::parse(&svg).unwrap();
        let role_paint = |role: &str, tag, property| {
            let group = document
                .root_element()
                .children()
                .find(|node| node.attribute("data-scene-role") == Some(role))
                .unwrap();
            group
                .children()
                .find(|node| node.has_tag_name(tag))
                .unwrap()
                .attribute(property)
                .unwrap()
        };
        assert!(role_paint("node", "rect", "fill").eq_ignore_ascii_case(&actor_fill));
        assert!(role_paint("edge", "path", "stroke").eq_ignore_ascii_case(&signal_stroke));
    });
}
