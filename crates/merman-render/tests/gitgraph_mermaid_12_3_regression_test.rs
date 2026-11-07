use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::git_graph::GitGraphDiagramModel;
use merman_model::presentation::RenderSemanticModel;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::GitGraphDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

fn inspect(source: &str) -> (GitGraphDiagramModel, GitGraphDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse GitGraph")
        .expect("detect GitGraph");
    let RenderSemanticModel::GitGraph(model) = parsed.model() else {
        panic!("expected GitGraph model");
    };
    let model = model.clone();
    let presentation = parsed.into_presentation();
    let artifact = family::prepare(
        &presentation,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare GitGraph");
    let projection = artifact.layout_json().expect("GitGraph layout JSON");
    let layout = serde_json::from_value(projection["layout"]["GitGraphDiagram"].clone())
        .expect("GitGraph layout");
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("git-regression".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render GitGraph");
    (model, layout, rendered.svg().to_string())
}

fn configured(config: Value, source: &str) -> String {
    format!("---\nconfig: {config}\n---\n{source}")
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn branch_history(count: usize) -> String {
    let mut source = String::from("gitGraph\ncommit id:\"root\"\n");
    for index in 1..=count {
        let kind = match index {
            8 => " type:HIGHLIGHT",
            15 => " type:REVERSE",
            _ => "",
        };
        source.push_str(&format!("branch b{index}\ncommit id:\"c{index}\"{kind}\n"));
    }
    source
}

#[test]
fn fractional_main_order_reaches_semantics_lanes_and_svg_in_each_direction() {
    for direction in ["LR", "TB", "BT"] {
        for (order, expected) in [(0.5, ["early", "main"]), (0.0, ["main", "early"])] {
            let source = configured(
                json!({"gitGraph": {"mainBranchOrder": order}}),
                &format!(
                    "gitGraph {direction}:\ncommit id:\"root\"\nbranch early\ncommit id:\"feature\"\n"
                ),
            );
            let (model, layout, svg) = inspect(&source);
            assert_eq!(
                model
                    .branches
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                layout
                    .branches
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let branch_text = document
                .descendants()
                .filter(|node| has_class(*node, "row"))
                .filter_map(|node| node.text())
                .collect::<Vec<_>>();
            assert_eq!(branch_text, expected);
            let coordinate = if direction == "LR" { "cy" } else { "cx" };
            let positions = ["root", "feature"].map(|id| {
                document
                    .descendants()
                    .find(|node| node.has_tag_name("circle") && has_class(*node, id))
                    .unwrap()
                    .attribute(coordinate)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            });
            assert_eq!(positions[0] > positions[1], order == 0.5);
        }
    }
}

#[test]
fn multiline_tags_remain_one_payload_through_layout_and_svg_text() {
    for newline in ["\n", "\r\n"] {
        let source =
            format!("gitGraph{newline}commit id:\"C\" tag:\"first{newline}second\"{newline}");
        let (model, layout, svg) = inspect(&source);
        assert_eq!(model.commits[0].tags, ["first\nsecond"]);
        assert_eq!(layout.commits[0].tags, ["first\nsecond"]);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let tag = document
            .descendants()
            .find(|node| has_class(*node, "tag-label"))
            .unwrap();
        assert_eq!(tag.text(), Some("first\nsecond"));
    }
    assert!(
        MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "gitGraph\ncommit tag:\"first\\\nsecond\"\n",
                ParseOptions::strict(),
            )
            .is_err()
    );
}

#[test]
fn merge_markup_tags_are_literal_escaped_text_and_commit_tags_stay_sanitized() {
    let (model, layout, svg) = inspect(concat!(
        "gitGraph\ncommit id:\"root\" tag:\"<unknown>root</unknown>\"\n",
        "branch feature\ncommit id:\"F\"\ncheckout main\n",
        "merge feature id:\"M\" tag:'<unknown>release</unknown>' tag:'<script>alert(1)</script>'\n",
    ));
    let expected = ["<unknown>release</unknown>", "<script>alert(1)</script>"];
    assert_eq!(model.commits[0].tags, ["root"]);
    assert_eq!(model.commits[2].tags, expected);
    assert_eq!(layout.commits[2].tags, expected);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let tags = document
        .descendants()
        .filter(|node| has_class(*node, "tag-label"))
        .filter_map(|node| node.text())
        .collect::<Vec<_>>();
    assert_eq!(tags, ["root", expected[1], expected[0]]);
    assert!(
        !document
            .descendants()
            .any(|node| { node.has_tag_name("unknown") || node.has_tag_name("script") })
    );
}

#[test]
fn nbsp_is_payload_text_but_cannot_separate_command_tokens() {
    let engine = MermaidEngine::new();
    for source in [
        "gitGraph\ncommit\u{00a0}id:\"C\"\n",
        "gitGraph\ncommit id:\u{00a0}\"C\"\n",
    ] {
        assert!(
            engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .is_err()
        );
    }
    let (model, _, svg) = inspect("gitGraph\r\n\tcommit\tid:\"C\" tag:\"a\u{00a0}b\"\r\n");
    assert_eq!(model.commits[0].tags, ["a\u{00a0}b"]);
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| has_class(node, "tag-label") && node.text() == Some("a\u{00a0}b"))
    );
}

#[test]
fn redux_branch_labels_and_commit_symbols_use_their_own_color_rings() {
    // Commits/arrows always use eight slots; Redux branch labels use the configured limit.
    for (theme, limit, branch8, branch16, commit8, commit16) in [
        ("redux-color", 12, 8, 5, 1, 2),
        ("redux-dark-color", 12, 8, 5, 1, 2),
        ("redux-color", 5, 4, 4, 1, 2),
        ("redux", 12, 8, 4, 0, 0),
        ("redux-dark", 12, 8, 4, 0, 0),
        ("default", 12, 0, 0, 0, 0),
        ("neo", 12, 0, 0, 0, 0),
    ] {
        let source = configured(
            json!({
                "theme": theme,
                "themeVariables": {"THEME_COLOR_LIMIT": limit}
            }),
            &branch_history(16),
        );
        let (_, layout, svg) = inspect(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        for (index, slot) in [(8, branch8), (16, branch16)] {
            let group = document
                .descendants()
                .find(|node| {
                    has_class(*node, &format!("branch-label{slot}"))
                        && node
                            .descendants()
                            .any(|child| child.text() == Some(format!("b{index}").as_str()))
                })
                .expect("branch label with upstream color index");
            assert!(
                group
                    .parent()
                    .unwrap()
                    .prev_sibling_element()
                    .is_some_and(|node| has_class(node, &format!("label{slot}")))
            );
        }
        let highlight = document
            .descendants()
            .find(|node| has_class(*node, "c8") && has_class(*node, "commit-highlight-outer"))
            .unwrap();
        assert!(
            has_class(highlight, &format!("commit-highlight{commit8}")),
            "{theme}/{limit}"
        );
        assert!(document.descendants().any(|node| {
            has_class(node, "c16") && has_class(node, &format!("commit{commit16}"))
        }));
        assert_eq!(
            layout
                .arrows
                .iter()
                .find(|arrow| arrow.to == "c8")
                .unwrap()
                .class_index,
            commit8
        );
        assert_eq!(
            layout
                .arrows
                .iter()
                .find(|arrow| arrow.to == "c16")
                .unwrap()
                .class_index,
            commit16
        );
    }
}

#[test]
fn fractional_css_limits_follow_classic_and_redux_selector_generators() {
    for (theme, has_slot_one) in [("default", false), ("redux-color", true)] {
        let (_, _, svg) = inspect(&configured(
            json!({
                "theme": theme,
                "themeVariables": {
                    "THEME_COLOR_LIMIT": 1.5,
                    "git1": "#ff0000",
                    "gitBranchLabel1": "#00ff00"
                }
            }),
            "gitGraph\ncommit id:\"A\"\nbranch side\ncommit id:\"B\"\n",
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(document.descendants().any(|node| has_class(node, "label1")));
        assert!(document.descendants().any(|node| {
            has_class(node, "branch-label1")
                && node.descendants().any(|child| child.text() == Some("side"))
        }));
        assert!(
            document
                .descendants()
                .any(|node| { has_class(node, "B") && has_class(node, "commit1") })
        );
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        for class in [
            "branch-label",
            "commit",
            "commit-highlight",
            "label",
            "arrow",
        ] {
            assert!(css.contains(&format!(".{class}0{{")), "{theme}/{class}");
            assert_eq!(
                css.contains(&format!(".{class}1{{")),
                has_slot_one,
                "{theme}/{class}",
            );
            assert!(!css.contains(&format!(".{class}2{{")), "{theme}/{class}");
        }
    }
}

#[test]
fn redux_css_keeps_high_selectors_and_fractional_class_indices() {
    for (limit, expected_branch9, last_selector) in [(8.5, "1.5", 8), (65.5, "9", 65)] {
        let (_, _, svg) = inspect(&configured(
            json!({
                "theme": "redux-color",
                "themeVariables": {"THEME_COLOR_LIMIT": limit}
            }),
            &branch_history(9),
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(document.descendants().any(|node| {
            has_class(node, &format!("branch-label{expected_branch9}"))
                && node.descendants().any(|child| child.text() == Some("b9"))
        }));
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(css.contains(&format!(".label{last_selector}{{")));
        assert!(!css.contains(&format!(".label{}{{", last_selector + 1)));
    }
}
