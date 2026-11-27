use super::*;

use crate::preparation::prepare_sequence_diagram_typed_with_title_and_work_meter;
use crate::svg::render_parity;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{OperationControl, family::DiagramFamilyId, presentation::RenderSemanticModel};
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    root_svg::RootViewportContext,
    svg_css::SvgDiagramIdProjection,
    text::{DeterministicTextMeasurer, GenericFontFamily},
};
use merman_theme_contract::v2::{BuiltinPreset, ResolvedThemeV2, ThemeCompiler};
use roxmltree::{Document, Node};

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
    work_meter: Arc<OperationWorkMeter>,
    native: Option<&ResolvedThemeV2>,
    canvas_background: Option<Color>,
    run: impl FnOnce(SequenceSvgRequest<'_>, SequenceCompactPolicy<'_>) -> T,
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
    let measurer = DeterministicTextMeasurer::default();
    let family_theme = native.map(|theme| theme.family("sequence"));
    let prepared = prepare_sequence_diagram_typed_with_title_and_work_meter(
        model,
        None,
        config,
        family_theme.map(|theme| (theme, canvas_background)),
        &measurer,
        None,
        Arc::clone(&work_meter),
    )
    .unwrap();
    let projection = SvgDiagramIdProjection::new(&work_meter);
    let diagram_id = SvgId::new("compact-sequence")
        .unwrap()
        .with_projection(&projection);
    run(
        SequenceSvgRequest {
            prepared: &prepared,
            semantic: model,
            config,
            measurer: &measurer,
            diagram_id,
            viewport: RootViewportContext::new(DiagramFamilyId::SEQUENCE, diagram_id)
                .with_canvas_padding(native.map_or(0.0, |theme| theme.canvas().padding_px)),
            work_meter: &work_meter,
            canvas_background,
        },
        SequenceCompactPolicy {
            canvas: family_theme.map(|theme| &theme.roles.canvas),
            textures: native.map_or(&[], |theme| theme.canvas().textures.as_slice()),
            allows_trusted_theme_css: false,
        },
    )
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
fn parity_sequence_preserves_tabs_in_actor_and_paint_attributes() {
    let source = "sequenceDiagram\n\
        box rgb(10\t20 30) Group\n\
        participant User\tInterface\n\
        end\n\
        actor Build\tBot\n\
        rect rgb(40\t50 60)\n\
        User\tInterface->>Build\tBot: Start\n\
        end\n";
    let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
    with_pair(source, meter, None, None, |request, _| {
        let svg = render_parity(request)
            .unwrap()
            .into_string_for(DiagramFamilyId::SEQUENCE)
            .unwrap();
        let doc = Document::parse(&svg).unwrap();
        for id in ["User\tInterface", "Build\tBot"] {
            let lifeline = doc.descendants().find(|node| {
                node.attribute("data-et") == Some("life-line")
                    && node.attribute("data-id") == Some(id)
            });
            assert_eq!(lifeline.and_then(|node| node.attribute("name")), Some(id));
        }
        assert!(doc.descendants().any(|node| {
            node.has_tag_name("rect") && node.attribute("name") == Some("User\tInterface")
        }));
        let frame_fills: Vec<_> = doc
            .descendants()
            .filter(|node| node.has_tag_name("rect") && node.attribute("class") == Some("rect"))
            .filter_map(|node| node.attribute("fill"))
            .collect();
        assert_eq!(frame_fills, ["rgb(10\t20 30)", "rgb(40\t50 60)"]);
    });
}

#[test]
fn compact_keeps_prepared_sequence_geometry_and_order() {
    let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
    with_pair(
        FIXTURE,
        meter,
        None,
        Some(Color::rgb(242, 228, 214)),
        |request, policy| {
            let meter = request.work_meter;
            let prepared = request.prepared;
            let open = prepared
                .model_for_render(request.semantic)
                .messages
                .last()
                .unwrap();
            assert_eq!(open.semantic_kind(), SequenceMessageKind::ActivationStart);
            assert!(matches!(
                prepared.activation_geometry().group_for_start(&open.id),
                Some(None)
            ));
            let before = meter.prepared_text_retained_bytes();
            let sidecar = prepare_sequence_compact(&request, policy).unwrap();
            assert!(meter.prepared_text_retained_bytes() > before);
            let compact_svg = sidecar.write_svg(request.diagram_id).unwrap();
            drop(sidecar);
            assert_eq!(meter.prepared_text_retained_bytes(), before);

            let parity_svg = render_parity(request)
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
                    .find(|node| {
                        node.has_tag_name("rect") && node.attribute("class") == Some(class)
                    })
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
                .filter(|node| {
                    node.has_tag_name("text") && node.attribute("class") == Some("noteText")
                })
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
        },
    );
}

#[test]
fn compact_rejects_self_messages_and_replays_an_existing_cancellation() {
    let control = OperationControl::new();
    let meter = Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::interactive(),
        control.clone(),
    ));
    with_pair(SELF_MESSAGE, meter, None, None, |request, policy| {
        assert!(matches!(
            render_compact(&request, policy),
            Err(SequenceCompactError::Unsupported("self-message"))
        ));
        control.cancel();
        assert!(matches!(
            render_compact(&request, policy),
            Err(SequenceCompactError::Render(Error::Cancelled(error)))
                if error.phase == OperationPhase::Emit
        ));
    });
}

#[test]
fn compact_projection_draws_from_the_same_cumulative_work_limit() {
    const LIMIT: usize = 100_000;
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, LIMIT)
        .unwrap();
    let meter = Arc::new(OperationWorkMeter::new(policy));
    with_pair(FIXTURE, meter, None, None, |request, policy| {
        let meter = request.work_meter;
        let layout = meter.used();
        assert!((1..LIMIT).contains(&layout));
        meter.charge(LIMIT - layout).unwrap();
        assert!(matches!(
            render_compact(&request, policy),
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
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Prism, None)
        .unwrap();
    let resolved = theme.family("sequence");
    let actor_fill = resolved.roles.surface_node.first_color().to_string();
    let signal_stroke = resolved.roles.stroke_connector.first_color().to_string();
    let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
    with_pair(
        FIXTURE,
        meter,
        Some(&theme),
        Some(Color::TRANSPARENT),
        |request, policy| {
            let svg = render_compact(&request, policy).unwrap();
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
        },
    );
}
