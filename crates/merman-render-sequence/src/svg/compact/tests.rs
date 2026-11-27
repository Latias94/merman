use super::*;

use std::cell::{Cell, RefCell};

use crate::preparation::prepare_sequence_diagram_typed_with_title_and_work_meter;
use crate::svg::render_parity;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{OperationControl, family::DiagramFamilyId, presentation::RenderSemanticModel};
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    root_svg::RootViewportContext,
    svg_css::SvgDiagramIdProjection,
    text::{DeterministicTextMeasurer, GenericFontFamily, TextMetrics, TextStyle},
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

const FRAMES: &str = r#"---
config:
  look: classic
  theme: default
  sequence:
    boxTextMargin: 5
---
sequenceDiagram
    participant A
    participant B
    loop Retry<br>Again & wait
        alt Ready
            A->>B: Frame signal
        else Busy
            B-->>A: Try later
        else
            A->>B: Retry
        end
        Note over B: After the inner frame
    end
"#;

fn with_pair<T>(
    source: &str,
    work_meter: Arc<OperationWorkMeter>,
    native: Option<&ResolvedThemeV2>,
    canvas_background: Option<Color>,
    run: impl FnOnce(SequenceSvgRequest<'_>, SequenceCompactPolicy<'_>) -> T,
) -> T {
    with_measured_pair(
        source,
        work_meter,
        native,
        canvas_background,
        &DeterministicTextMeasurer::default(),
        run,
    )
}

fn with_measured_pair<T>(
    source: &str,
    work_meter: Arc<OperationWorkMeter>,
    native: Option<&ResolvedThemeV2>,
    canvas_background: Option<Color>,
    measurer: &dyn TextMeasurer,
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
    let family_theme = native.map(|theme| theme.family("sequence"));
    let prepared = prepare_sequence_diagram_typed_with_title_and_work_meter(
        model,
        None,
        config,
        family_theme.map(|theme| (theme, canvas_background)),
        measurer,
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
            measurer,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameProbe {
    WidthReplay,
    PrimaryHeight,
    SectionHeight,
}

#[derive(Default)]
struct FrameMeasurer {
    inner: DeterministicTextMeasurer,
    probes: RefCell<Vec<(FrameProbe, String)>>,
    cancel_on_primary: Cell<bool>,
    control: OperationControl,
}

impl TextMeasurer for FrameMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.inner.measure(text, style)
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        if text == "Frame signal" {
            self.probes.borrow_mut().push((
                FrameProbe::WidthReplay,
                style.font_family.clone().unwrap_or_default(),
            ));
        }
        self.inner
            .measure_mermaid_calculate_text_dimensions(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        if text.starts_with('[') || text.ends_with(']') {
            self.probes
                .borrow_mut()
                .push((FrameProbe::PrimaryHeight, text.to_owned()));
            if self.cancel_on_primary.replace(false) {
                self.control.cancel();
            }
        }
        self.inner
            .measure_svg_tspan_text_bbox_height_px(text, style)
    }

    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        if text.starts_with('[') || text.ends_with(']') {
            self.probes
                .borrow_mut()
                .push((FrameProbe::SectionHeight, text.to_owned()));
        }
        self.inner.measure_svg_raw_text_bbox_height_px(text, style)
    }
}

#[test]
fn compact_frames_keep_parity_geometry_paint_and_host_probe_order() {
    for margin in [0, 5] {
        let source = FRAMES.replace("boxTextMargin: 5", &format!("boxTextMargin: {margin}"));
        let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
        let measurer = FrameMeasurer::default();
        with_measured_pair(&source, meter, None, None, &measurer, |request, policy| {
            measurer.probes.borrow_mut().clear();
            let parity_svg = render_parity(SequenceSvgRequest {
                viewport: request.viewport.clone(),
                ..request
            })
            .unwrap()
            .into_string_for(DiagramFamilyId::SEQUENCE)
            .unwrap();
            let parity_probes = measurer.probes.take();
            let compact_svg = render_compact(&request, policy).unwrap();
            assert_eq!(measurer.probes.take(), parity_probes);
            assert!(matches!(
                parity_probes.first(),
                Some((FrameProbe::WidthReplay, _))
            ));
            for height in [FrameProbe::PrimaryHeight, FrameProbe::SectionHeight] {
                assert_eq!(
                    parity_probes.iter().any(|(probe, _)| *probe == height),
                    margin > 0
                );
            }

            let compact = Document::parse(&compact_svg).unwrap();
            let parity = Document::parse(&parity_svg).unwrap();
            assert!(!compact_svg.contains("<foreignObject"));
            let overlays = compact
                .root_element()
                .children()
                .filter(|node| {
                    matches!(node.attribute("data-scene-role"), Some("cluster" | "note"))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                overlays
                    .iter()
                    .map(|node| node.attribute("data-scene-role").unwrap())
                    .collect::<Vec<_>>(),
                ["cluster", "note", "cluster"]
            );
            let parity_frames = parity
                .descendants()
                .filter(|node| node.attribute("data-et") == Some("control-structure"))
                .collect::<Vec<_>>();
            let compat = request.prepared.typography().compat_binding();
            let rgba = |color: &str| ThemeColor::parse(color).unwrap().rgba_channels();
            for (frame, parity_frame, keyword, separators) in [
                (overlays[0], parity_frames[0], "alt", 2),
                (overlays[2], parity_frames[1], "loop", 0),
            ] {
                let paths = frame
                    .children()
                    .filter(|child| child.has_tag_name("path"))
                    .collect::<Vec<_>>();
                assert_eq!(paths.len(), 5 + separators);
                let lines = parity_frame
                    .children()
                    .filter(|child| child.has_tag_name("line"))
                    .collect::<Vec<_>>();
                assert_eq!(lines.len(), 4 + separators);
                for (index, (path, line)) in paths.iter().zip(lines).enumerate() {
                    let [x1, y1, x2, y2] =
                        ["x1", "y1", "x2", "y2"].map(|name| line.attribute(name).unwrap());
                    assert_eq!(
                        path.attribute("d").unwrap(),
                        format!("M{x1} {y1} L{x2} {y2}")
                    );
                    assert_eq!(
                        path.attribute("stroke-dasharray"),
                        Some(if index < 4 { "2 2" } else { "3 3" })
                    );
                    assert_eq!(path.attribute("stroke-width"), Some("2"));
                    assert_eq!(
                        rgba(path.attribute("stroke").unwrap()),
                        rgba(&compat.label_box_border)
                    );
                }
                let keyword_shape = paths.last().unwrap();
                let points = parity_frame
                    .children()
                    .find(|child| child.has_tag_name("polygon"))
                    .unwrap()
                    .attribute("points")
                    .unwrap();
                let path = points
                    .split_whitespace()
                    .map(|point| point.replace(',', " "))
                    .collect::<Vec<_>>()
                    .join(" L");
                assert_eq!(keyword_shape.attribute("d").unwrap(), format!("M{path} Z"));
                assert_eq!(keyword_shape.attribute("stroke-width"), Some("1"));
                assert_eq!(
                    rgba(keyword_shape.attribute("fill").unwrap()),
                    rgba(&compat.label_box_fill)
                );

                let rows = frame
                    .children()
                    .filter(|child| child.has_tag_name("text"))
                    .collect::<Vec<_>>();
                let parity_rows = parity_frame
                    .children()
                    .filter(|child| child.has_tag_name("text"))
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), parity_rows.len());
                assert_eq!(first_text(rows[0]), keyword);
                for (row, parity_row) in rows.into_iter().zip(parity_rows) {
                    assert_eq!(first_text(row), first_text(parity_row));
                    assert_eq!(
                        attributes(row, &["x", "y", "text-anchor"]),
                        attributes(parity_row, &["x", "y", "text-anchor"])
                    );
                    assert_eq!(
                        row.attribute("dy").unwrap(),
                        parity_row.attribute("dy").unwrap_or("0")
                    );
                    assert_eq!(
                        row.attribute("dominant-baseline").unwrap(),
                        parity_row
                            .attribute("dominant-baseline")
                            .unwrap_or("alphabetic")
                    );
                }
            }
            assert!(compact_svg.contains("Again &amp; wait]"));
        });
    }
}

#[test]
fn compact_frames_reject_other_controls_and_unplaced_frames() {
    for (body, reason) in [
        ("opt Optional\nA->>B: In frame\nend", "sequence-control"),
        ("par Parallel\nA->>B: In frame\nend", "sequence-control"),
        (
            "rect rgb(0, 0, 0)\nA->>B: In frame\nend",
            "sequence-control",
        ),
        ("loop Empty\nend", "sequence-frame-placement"),
    ] {
        let source = format!(
            "---\nconfig:\n  look: classic\n  theme: default\n---\nsequenceDiagram\nA->>B: Outside frame\n{body}\n"
        );
        let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
        with_pair(&source, meter, None, None, |request, policy| {
            assert!(
                matches!(render_compact(&request, policy), Err(SequenceCompactError::Unsupported(actual)) if actual == reason)
            );
            assert_eq!(request.work_meter.projected_svg_bytes(), 0);
        });
    }
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Prism, None)
        .unwrap();
    let budget = RenderResourcePolicy::unbounded_for_trusted_input();
    let layout_work = with_pair(
        FRAMES,
        Arc::new(OperationWorkMeter::new(budget)),
        Some(&theme),
        None,
        |request, _| request.work_meter.used(),
    );
    let policy = budget
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, layout_work)
        .unwrap();
    with_pair(
        FRAMES,
        Arc::new(OperationWorkMeter::new(policy)),
        Some(&theme),
        None,
        |request, policy| {
            assert!(matches!(
                render_compact(&request, policy),
                Err(SequenceCompactError::Unsupported("sequence-native-frame"))
            ));
            assert_eq!(request.work_meter.used(), layout_work);
        },
    );
}

#[test]
fn compact_frames_keep_keyword_and_stroke_inside_the_viewport() {
    let source = "---\nconfig:\n  look: classic\n  theme: default\n  sequence:\n    diagramMarginX: 0\n    labelBoxWidth: 500\n---\nsequenceDiagram\nloop\nA->>B: Go\nend\n";
    let meter = Arc::new(OperationWorkMeter::new(RenderResourcePolicy::interactive()));
    with_pair(source, meter, None, None, |request, policy| {
        let projected = prepare_sequence_compact(&request, policy).unwrap();
        let frame = &request.prepared.layout().frames[0];
        let view_box = projected.viewport.view_box;
        assert!(view_box.x <= frame.x - 2.0);
        assert!(view_box.x + view_box.width >= frame.x + 502.0);
    });
}

#[test]
fn compact_frames_admit_exact_cumulative_work_and_release_rows_on_cancellation() {
    let budget = RenderResourcePolicy::unbounded_for_trusted_input();
    let meter = Arc::new(OperationWorkMeter::new(budget));
    let (expected, work) = with_pair(FRAMES, meter, None, None, |request, policy| {
        (
            render_compact(&request, policy).unwrap(),
            request.work_meter.used(),
        )
    });
    for limit in [work - 1, work] {
        let meter = Arc::new(OperationWorkMeter::new(
            budget
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        with_pair(FRAMES, meter, None, None, |request, policy| {
            let result = render_compact(&request, policy);
            if limit == work {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert!(
                    matches!(result, Err(SequenceCompactError::Render(Error::ResourceLimitExceeded(error))) if error.limit == "max_layout_work_units")
                );
            }
        });
    }

    let measurer = FrameMeasurer::default();
    let meter = Arc::new(OperationWorkMeter::new_with_control(
        budget,
        measurer.control.clone(),
    ));
    with_measured_pair(FRAMES, meter, None, None, &measurer, |request, policy| {
        let retained = request.work_meter.prepared_text_retained_bytes();
        measurer.probes.borrow_mut().clear();
        measurer.cancel_on_primary.set(true);
        assert!(
            matches!(render_compact(&request, policy), Err(SequenceCompactError::Render(Error::Cancelled(error))) if error.phase == OperationPhase::Emit)
        );
        let probes = measurer.probes.take();
        assert!(matches!(
            probes.last(),
            Some((FrameProbe::PrimaryHeight, _))
        ));
        assert_eq!(
            probes
                .iter()
                .filter(|(probe, _)| *probe == FrameProbe::PrimaryHeight)
                .count(),
            1
        );
        assert_eq!(request.work_meter.prepared_text_retained_bytes(), retained);
        assert_eq!(request.work_meter.projected_svg_bytes(), 0);
    });
}
