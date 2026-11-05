#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};

const STEPS: &str =
    include_str!("../../../fixtures/flowchart/local_mermaid_12_3_step_duration_numbers.mmd");

fn render(source: &str, id: &str) -> family::RenderedFamilySvg {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(
        &parsed,
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some(id.to_owned()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
    .unwrap()
}

#[test]
fn step_svg_keeps_parallel_timing_gaps_badge_layer_and_dash_animation() {
    let rendered = render(STEPS, "steps");
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let motions: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("animateMotion"))
        .collect();
    assert_eq!(motions.len(), 3);
    let mut first = 0;
    let mut last = 0;
    for motion in motions {
        assert_eq!(motion.attribute("dur"), Some("0.25s"));
        let id = motion.attribute("id").unwrap();
        let begin = motion.attribute("begin").unwrap();
        assert!(begin.ends_with(&format!(";{id}.end+1s")));
        match motion.parent_element().unwrap().attribute("data-step") {
            Some("1") => {
                assert!(begin.starts_with("0s;"));
                first += 1;
            }
            Some("5") => {
                assert!(begin.starts_with("1s;"));
                last += 1;
            }
            step => panic!("unexpected step {step:?}"),
        }
    }
    assert_eq!((first, last), (2, 1));
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("edge-step-highlight"))
            .count(),
        3
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("edge-step-badge"))
            .count(),
        3
    );
    let badges = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edge-step-badges"))
        .unwrap();
    assert_eq!(
        badges.parent_element().unwrap().attribute("class"),
        Some("root")
    );
    assert!(rendered.svg().contains("edge-animation-fast"));
    assert!(
        rendered
            .svg()
            .contains("#steps .edge-step-dot, #steps .edge-step-highlight")
    );
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    merman_render::svg::validate_static_inline_svg_admission(rendered.svg(), &session).unwrap();
}

#[test]
fn step_svg_is_scoped_reusable_and_static_exports_keep_numbered_edges() {
    for id in ["first", "second", "first"] {
        let rendered = render(STEPS, id)
            .apply_pipeline(&SvgPipeline::parity().with_browser_inline_contract("embed"))
            .unwrap();
        assert!(rendered.svg().contains("animateMotion"));
        assert!(
            rendered
                .svg()
                .contains(&format!("#embed-{id} .edge-step-dot"))
        );
    }
    for pipeline in [
        SvgPipeline::resvg_safe(),
        SvgPipeline::resvg_safe().with_static_inline_contract("static"),
    ] {
        let rendered = render(STEPS, "steps").apply_pipeline(&pipeline).unwrap();
        assert!(!rendered.svg().contains("animateMotion"));
        assert!(!rendered.svg().contains("animateTransform"));
        assert!(!rendered.svg().contains("edge-step-dot"));
        assert!(!rendered.svg().contains("edge-step-highlight"));
        assert!(rendered.svg().contains("edge-step-badge"));
        assert_eq!(rendered.svg().matches("data-edge=\"true\"").count(), 3);
    }
}

#[test]
fn step_badges_avoid_prior_badges_and_do_not_leak_into_an_unstepped_render() {
    let rendered = render(STEPS, "positions");
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let positions: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("edge-step-badge"))
        .map(|node| {
            let value = node
                .attribute("transform")
                .unwrap()
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let (left, right) = value.split_once(',').unwrap();
            (left.parse::<f64>().unwrap(), right.parse::<f64>().unwrap())
        })
        .collect();
    for (index, first) in positions.iter().enumerate() {
        for second in &positions[index + 1..] {
            assert!((first.0 - second.0).hypot(first.1 - second.1) >= 18.0);
        }
    }
    assert!(
        !render("flowchart LR\n A --> B", "positions")
            .svg()
            .contains("edge-step-")
    );
}

#[test]
fn step_default_duration_and_strict_boolean_badges_match_upstream() {
    for duration in ["0", "-1", "'slow'"] {
        let source = STEPS
            .replace("stepDuration: 0.25", &format!("stepDuration: {duration}"))
            .replace("stepNumbers: true", "stepNumbers: 'true'");
        let rendered = render(&source, "defaults");
        assert!(rendered.svg().contains("dur=\"1s\""));
        assert!(!rendered.svg().contains("edge-step-badge"));
    }
}

#[test]
fn ordinary_step_routes_follow_their_semantic_edges_in_every_direction() {
    for direction in ["TB", "BT", "LR", "RL"] {
        for look in ["classic", "neo"] {
            let source = format!(
                "---\nconfig:\n  look: {look}\n  flowchart:\n    stepNumbers: true\n---\nflowchart {direction}\n subgraph group[Group]\n A e1@-->|first| B\n B e2@--> C\n end\n A e3@--> C\n e1@{{ step: 1 }}\n e2@{{ step: 2 }}\n e3@{{ step: 3 }}\n"
            );
            let rendered = render(&source, "routes");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let motions: Vec<_> = document
                .descendants()
                .filter(|node| node.has_tag_name("animateMotion"))
                .collect();
            assert_eq!(motions.len(), 3);
            let mut observed_edges = std::collections::BTreeSet::new();
            for motion in motions {
                let owners: Vec<_> = document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                    .filter(|node| {
                        let mut expected = String::from("step_");
                        for character in
                            node.attribute("id").unwrap().chars().chain("-step".chars())
                        {
                            if character.is_ascii_alphanumeric() {
                                expected.push(character);
                            } else {
                                use std::fmt::Write;
                                write!(&mut expected, "_{:x}_", character as u32).unwrap();
                            }
                        }
                        motion.attribute("id") == Some(expected.as_str())
                    })
                    .collect();
                assert_eq!(owners.len(), 1);
                let owner = owners[0];
                let dot = motion.parent().unwrap();
                let step = dot.attribute("data-step").unwrap();
                assert_eq!(
                    owner.attribute("data-id"),
                    Some(format!("e{step}").as_str())
                );
                assert!(observed_edges.insert(owner.attribute("data-id").unwrap()));
                assert_eq!(
                    motion.attribute("path"),
                    owner.attribute("d"),
                    "Step route must follow its actual edge for {look}/{direction}"
                );
                let begin = format!("{}.begin", motion.attribute("id").unwrap());
                let highlights: Vec<_> = dot
                    .parent()
                    .unwrap()
                    .children()
                    .filter(|node| node.attribute("class") == Some("edge-step-highlight"))
                    .filter(|node| node.attribute("data-step") == Some(step))
                    .collect();
                assert_eq!(highlights.len(), 1);
                assert_eq!(highlights[0].attribute("d"), owner.attribute("d"));
                assert!(highlights[0].children().any(|node| {
                    node.has_tag_name("animateTransform")
                        && node.attribute("begin") == Some(begin.as_str())
                }));
            }
            assert_eq!(
                observed_edges,
                std::collections::BTreeSet::from(["e1", "e2", "e3"])
            );
        }
    }
}
