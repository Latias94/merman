#![cfg(feature = "diagram-sequence")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::sequence::{SequenceControlKind, SequenceControlRole};
use merman_model::presentation::RenderSemanticModel;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::SequenceDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::Value;

const NESTED_FRAMES: &str = "sequenceDiagram
participant A
participant B
loop retry
  alt accepted
    opt cached
      A->>B: read
    end
  else rejected
    par first
      A->>B: write
    and second
      B->>A: notify
    end
  end
  critical atomic
    A->>B: update
  option rollback
    B->>A: retry
  end
  break stopped
    A->>B: stop
  end
  par_over overlap
    A->>B: fetch
  and other
    B->>A: reply
  end
end
";

fn render(source: &str) -> (Value, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = projection["layout"]["SequenceDiagram"].clone();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("sequence-geometry".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();
    (layout, svg)
}

fn number(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name).unwrap().parse().unwrap()
}

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.000_01,
        "{actual} != {expected}"
    );
}

fn assert_rect(value: &Value, node: roxmltree::Node<'_, '_>) {
    for attribute in ["x", "y", "width", "height"] {
        near(value[attribute].as_f64().unwrap(), number(node, attribute));
    }
}

fn assert_text(value: &Value, node: roxmltree::Node<'_, '_>) {
    near(value["x"].as_f64().unwrap(), number(node, "x"));
    near(value["y"].as_f64().unwrap(), number(node, "y"));
    assert_eq!(
        value["text"].as_str().unwrap(),
        node.descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>()
    );
}

#[test]
fn sequence_frames_and_sections_share_svg_geometry_in_source_order() {
    // Exercise configured label-box widening as well as the default nested frame geometry.
    for prefix in ["", "%%{init: {'sequence': {'labelBoxWidth': 280}}}%%\n"] {
        let source = format!("{prefix}{NESTED_FRAMES}");
        let (layout, svg) = render(&source);
        let frames = layout["frames"].as_array().expect("export control frames");
        let kinds: Vec<_> = frames
            .iter()
            .map(|frame| frame["kind"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            ["loop", "alt", "opt", "par", "critical", "break", "par_over"]
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        let document = roxmltree::Document::parse(&svg).unwrap();
        let mut stack = Vec::new();
        for message in &model.messages {
            let Some(control) = message.control_semantics() else {
                continue;
            };
            if control.kind == SequenceControlKind::Rect {
                continue;
            }
            match control.role {
                SequenceControlRole::Start => stack.push(message),
                SequenceControlRole::Separator => {}
                SequenceControlRole::End => {
                    let opening = stack.pop().unwrap();
                    let frame = frames
                        .iter()
                        .find(|frame| frame["id"] == opening.id)
                        .unwrap();
                    assert_eq!(frame["label"], opening.message_text());
                    let group_id = format!("i{}", message.id);
                    let group = document
                        .descendants()
                        .find(|node| {
                            node.attribute("data-et") == Some("control-structure")
                                && node.attribute("data-id") == Some(group_id.as_str())
                        })
                        .unwrap();
                    let lines: Vec<_> = group
                        .children()
                        .filter(|node| node.has_tag_name("line"))
                        .collect();
                    let x = frame["x"].as_f64().unwrap();
                    let y = frame["y"].as_f64().unwrap();
                    let right = x + frame["width"].as_f64().unwrap();
                    let bottom = y + frame["height"].as_f64().unwrap();
                    for (line, expected) in lines.iter().zip([
                        [x, y, right, y],
                        [right, y, right, bottom],
                        [x, bottom, right, bottom],
                        [x, y, x, bottom],
                    ]) {
                        for (attribute, coordinate) in
                            ["x1", "y1", "x2", "y2"].into_iter().zip(expected)
                        {
                            near(number(*line, attribute), coordinate);
                        }
                    }
                    let sections = frame
                        .get("sections")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    assert_eq!(lines.len(), 4 + sections.len());
                    for (section, line) in sections.iter().zip(lines.iter().skip(4)) {
                        let semantic = model
                            .messages
                            .iter()
                            .find(|message| section["id"] == message.id)
                            .unwrap();
                        assert_eq!(section["label"], semantic.message_text());
                        near(number(*line, "y1"), section["y"].as_f64().unwrap());
                        near(number(*line, "y2"), section["y"].as_f64().unwrap());
                        near(number(*line, "x1"), x);
                        near(number(*line, "x2"), right);
                    }
                }
            }
        }
        let typed: SequenceDiagramLayout = serde_json::from_value(layout.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), layout);
    }
}

#[test]
fn sequence_activation_export_preserves_nesting_notes_and_open_placeholders() {
    let source = "sequenceDiagram
A->>+B: begin
activate B
Note over B: work<br/>more work
B->>B: nested
deactivate B
B-->>-A: reply
activate A
deactivate A
activate B
";
    let (layout, svg) = render(source);
    let bars = layout["activations"]
        .as_array()
        .expect("export activation bars");
    assert_eq!(bars.len(), 3, "an unclosed activation has no rectangle");
    assert_eq!(
        bars.iter()
            .map(|bar| (
                bar["actor"].as_str().unwrap(),
                bar["depth"].as_u64().unwrap()
            ))
            .collect::<Vec<_>>(),
        [("B", 0), ("B", 1), ("A", 0)]
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let rectangles: Vec<_> = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("activation"))
        })
        .collect();
    assert_eq!(rectangles.len(), bars.len());
    for (bar, rectangle) in bars.iter().zip(rectangles) {
        assert_rect(bar, rectangle);
    }
    near(bars[2]["height"].as_f64().unwrap(), 18.0);
}

#[test]
fn sequence_participant_boxes_and_title_share_svg_placements() {
    for mirror in [true, false] {
        let source = format!(
            "---\ntitle: Checkout\nconfig:\n  sequence:\n    mirrorActors: {mirror}\n---\nsequenceDiagram\nbox transparent Shop\nparticipant A\nparticipant B\nend\nbox rgb(230,230,255)\nparticipant C\nend\nA->>B: Checkout\nB->>C: fetch\n"
        );
        let (layout, svg) = render(&source);
        let boxes = layout["boxes"]
            .as_array()
            .expect("export participant boxes");
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0]["title"]["text"], "Shop");
        assert!(boxes[1].get("title").is_none());
        let document = roxmltree::Document::parse(&svg).unwrap();
        let groups: Vec<_> = document
            .root_element()
            .children()
            .filter(|node| {
                node.has_tag_name("g")
                    && node
                        .children()
                        .next()
                        .is_some_and(|child| child.attribute("class") == Some("rect"))
            })
            .collect();
        assert_eq!(groups.len(), boxes.len());
        for (group, value) in groups.iter().rev().zip(boxes) {
            let rectangle = group.children().next().unwrap();
            assert_rect(value, rectangle);
            assert_eq!(
                rectangle.attribute("fill").unwrap(),
                value["fill"].as_str().unwrap()
            );
            if let Some(title) = value.get("title") {
                assert_text(
                    title,
                    group
                        .children()
                        .find(|node| node.has_tag_name("text"))
                        .unwrap(),
                );
            }
        }
        // Messages are also root-level text nodes. A same-named message must not select the title.
        assert!(document.root_element().children().any(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("messageText")
                && node.text() == Some("Checkout")
        }));
        let titles: Vec<_> = document
            .root_element()
            .children()
            .filter(|node| node.has_tag_name("text") && node.attribute("class").is_none())
            .collect();
        assert_eq!(titles.len(), 1, "expected one unclassified diagram title");
        assert_text(&layout["title"], titles[0]);
    }
}

#[test]
fn sequence_empty_geometry_fields_are_absent_and_legacy_layouts_still_deserialize() {
    let (plain, _) = render("sequenceDiagram\nA->>B: hello\n");
    for field in ["frames", "activations", "boxes", "title"] {
        assert!(
            plain.get(field).is_none(),
            "empty {field} must remain absent"
        );
    }
    let typed: SequenceDiagramLayout = serde_json::from_value(plain.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), plain);
    let (empty_frame, svg) = render("sequenceDiagram\nparticipant A\nloop empty\nend\n");
    assert!(empty_frame.get("frames").is_none());
    assert!(!svg.contains("data-et=\"control-structure\""));
}

#[test]
fn sequence_geometry_refactor_preserves_existing_svg_coordinates() {
    // Characterized with the baseline CLI before adding the geometry exports.
    let source = "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop retry\nA->>+B: request\nalt ok\nB-->>A: success\nelse error\nB-->>-A: failure\nend\nend\n";
    let (_, svg) = render(source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let frames: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("control-structure"))
        .collect();
    assert_eq!(frames.len(), 2);
    for (frame, expected) in frames
        .iter()
        .zip([[69.0, 203.0, 295.0, 385.0], [59.0, 112.0, 305.0, 395.0]])
    {
        let lines: Vec<_> = frame
            .children()
            .filter(|node| node.has_tag_name("line"))
            .collect();
        near(number(lines[0], "x1"), expected[0]);
        near(number(lines[0], "y1"), expected[1]);
        near(number(lines[0], "x2"), expected[2]);
        near(number(lines[1], "y2"), expected[3]);
    }
    let separator = frames[0]
        .children()
        .filter(|node| node.has_tag_name("line"))
        .nth(4)
        .unwrap();
    near(number(separator, "y1"), 299.0);
    let activation = document
        .descendants()
        .find(|node| node.attribute("class") == Some("activation0"))
        .unwrap();
    for (attribute, expected) in [
        ("x", 275.0),
        ("y", 195.0),
        ("width", 10.0),
        ("height", 180.0),
    ] {
        near(number(activation, attribute), expected);
    }
}
