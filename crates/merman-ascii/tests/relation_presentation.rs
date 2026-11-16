#![cfg(feature = "diagram-class")]

mod support;

use merman_ascii::{AsciiRenderOptions, TerminalWidthProfile};
use support::{parse_model, render_model};

#[test]
fn class_horizontal_crop_preserves_multiline_endpoint_labels_without_a_main_label() {
    for direction in ["LR", "RL"] {
        let source = format!(
            "classDiagram\ndirection {direction}\nA \"end-one<br>end-two<br>end-three<br>end-four<br>end-five\" --> B\nB --> C\n"
        );
        let model = parse_model(&source);
        for base in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                let rendered = render_model(&model, &base.with_terminal_width_profile(profile))
                    .expect("unused label anchors outside the crop must not underflow");
                for label in ["end-one", "end-two", "end-three", "end-four", "end-five"] {
                    assert!(rendered.contains(label), "{direction}:\n{rendered}");
                }
                for node in ["A", "B", "C"] {
                    assert!(
                        rendered.contains(&format!(" {node} ")),
                        "{direction}:\n{rendered}"
                    );
                }
            }
        }
    }
}

#[test]
fn cropped_routed_component_can_discard_an_unused_anchor_above_the_scene() {
    for direction in ["LR", "RL"] {
        let source = format!(
            "classDiagram\ndirection {direction}\nA --> B\nB --> C\nC \"a<br>b<br>c<br>d<br>e\" --> D\nD --> E\nE --> F\nF --> G\n"
        );
        let model = parse_model(&source);
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let rendered = render_model(&model, &options).unwrap();
            assert!(
                !rendered.contains("relations:"),
                "the crop regression must exercise a routed scene:\n{rendered}"
            );
            assert!(
                rendered.lines().any(|line| line.trim() == "d"),
                "the multiline endpoint must remain in the diagram:\n{rendered}"
            );
        }
    }
}

#[test]
fn directed_ring_edges_survive_identifier_orders_that_differ_from_cycle_order() {
    for names in [
        vec!["A", "C", "B"],
        vec!["Worker", "Store", "Queue", "Root"],
        vec!["E", "B", "D", "A", "C"],
    ] {
        for direction in ["TB", "BT", "LR", "RL"] {
            let mut source = format!("classDiagram\ndirection {direction}\n");
            for (index, name) in names.iter().enumerate() {
                source.push_str(&format!(
                    "{name} --> {} : hop{index}\n",
                    names[(index + 1) % names.len()]
                ));
            }
            let model = parse_model(&source);
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let rendered = render_model(&model, &options).unwrap();
                assert!(!rendered.contains("relations:"), "{direction}:\n{rendered}");
                for index in 0..names.len() {
                    assert_eq!(
                        rendered.matches(&format!("hop{index}")).count(),
                        1,
                        "{rendered}"
                    );
                }
                assert_eq!(
                    rendered
                        .chars()
                        .filter(|ch| "^v<>▲▼◀▶".contains(*ch))
                        .count(),
                    names.len(),
                    "{rendered}"
                );
            }
        }
    }
}

#[test]
fn unicode_private_route_turns_do_not_look_like_branch_junctions() {
    let model = parse_model("classDiagram\nA --> B : ab\nB --> C : bc\nC --> A : ca\n");
    let rendered = render_model(&model, &AsciiRenderOptions::unicode()).unwrap();
    assert!(!rendered.contains("relations:"), "{rendered}");
    assert!(rendered.chars().any(|ch| "╭╮╯╰".contains(ch)), "{rendered}");
    assert!(
        !rendered.contains('┼'),
        "private bends must not imply a crossing:\n{rendered}"
    );
    let cjk = render_model(
        &model,
        &AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
    )
    .unwrap();
    assert!(
        cjk.is_ascii(),
        "CJK structural fallback must remain single-cell"
    );
}
