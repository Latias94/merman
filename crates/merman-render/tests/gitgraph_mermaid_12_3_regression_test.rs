use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::git_graph::GitGraphDiagramModel;
use merman_model::presentation::RenderSemanticModel;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::GitGraphDiagramLayout;
use merman_render::svg::{
    SvgDebugOptions, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgRenderOptions,
};
use serde_json::{Value, json};

fn inspect(source: &str) -> (GitGraphDiagramModel, GitGraphDiagramLayout, String) {
    inspect_with_pipeline(source, None)
}

fn inspect_with_pipeline(
    source: &str,
    pipeline: Option<&SvgPipeline>,
) -> (GitGraphDiagramModel, GitGraphDiagramLayout, String) {
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
    let svg = match pipeline {
        Some(pipeline) => rendered
            .finalize_resvg(pipeline)
            .expect("finalize GitGraph for resvg")
            .svg()
            .as_str()
            .to_string(),
        None => rendered.svg().to_string(),
    };
    (model, layout, svg)
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
fn viewport_uses_gitgraph_max_width_and_padding_after_measuring_rotated_labels() {
    for direction in ["LR", "TB", "BT"] {
        let mut unpadded = None::<Vec<f64>>;
        for padding in [0.0, -2.5, 2.5, 25.0] {
            for use_max_width in [true, false] {
                let (_, layout, svg) = inspect(&configured(
                    json!({
                        "state": {"useMaxWidth": !use_max_width},
                        "gitGraph": {
                            "useMaxWidth": use_max_width,
                            "diagramPadding": padding
                        }
                    }),
                    &format!(
                        "gitGraph {direction}:\ncommit id:\"root\"\nbranch feature\ncommit id:\"work\"\ncheckout main\nmerge feature id:\"merged\"\n"
                    ),
                ));
                assert_eq!(layout.diagram_padding, padding);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let root = document.root_element();
                let view_box = root
                    .attribute("viewBox")
                    .unwrap()
                    .split_ascii_whitespace()
                    .map(|number| number.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                if use_max_width {
                    assert_eq!(root.attribute("width"), Some("100%"));
                    assert_eq!(root.attribute("height"), None);
                    assert_eq!(
                        root.attribute("style"),
                        Some(format!("max-width: {}px;", view_box[2]).as_str()),
                    );
                } else {
                    for (attribute, expected) in [("width", view_box[2]), ("height", view_box[3])] {
                        assert_eq!(
                            root.attribute(attribute).unwrap().parse::<f64>().unwrap(),
                            expected,
                        );
                    }
                    assert_eq!(root.attribute("style"), None);
                }
                if let Some(unpadded) = &unpadded {
                    for (index, delta) in [-padding, -padding, 2.0 * padding, 2.0 * padding]
                        .into_iter()
                        .enumerate()
                    {
                        assert!((view_box[index] - unpadded[index] - delta).abs() < 1e-8);
                    }
                } else {
                    unpadded = Some(view_box);
                }
            }
        }
    }
}

#[test]
fn signed_title_margin_reaches_the_svg_title_and_intrinsic_viewport() {
    for margin in [-12.5, 0.0, 40.0] {
        let source = format!(
            "---\ntitle: Release history\nconfig: {}\n---\ngitGraph\ncommit id:\"root\"\n",
            json!({"gitGraph": {"titleTopMargin": margin, "useMaxWidth": false}}),
        );
        let (_, _, svg) = inspect(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        let title = root
            .children()
            .find(|node| has_class(*node, "gitTitleText"))
            .unwrap();
        assert_eq!(
            title.attribute("y").unwrap().parse::<f64>().unwrap(),
            -margin
        );
        assert_eq!(title.text(), Some("Release history"));
        let bounds = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|number| number.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!(bounds[1] < -margin && -margin < bounds[1] + bounds[3]);
        assert_eq!(
            root.attribute("height").unwrap().parse::<f64>().unwrap(),
            bounds[3]
        );
    }
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
fn ordinary_bt_forks_use_the_destination_side_and_sweep_from_mermaid() {
    for (order, sweep, radius_sign) in [(1, 1, 1.0), (0, 0, -1.0)] {
        let (model, layout, svg) = inspect(&format!(
            "gitGraph BT:\nbranch late order:{order}\ncommit id:\"B\"\nbranch early\ncommit id:\"C\"\n"
        ));
        assert_eq!(model.commits[1].parents, ["B"]);
        assert!(!layout.parallel_commits);
        assert_eq!(layout.arrows.len(), 1);
        let from = layout
            .commits
            .iter()
            .find(|commit| commit.id == "B")
            .unwrap();
        let to = layout
            .commits
            .iter()
            .find(|commit| commit.id == "C")
            .unwrap();
        assert_eq!(from.x > to.x, order == 1);
        let expected = format!(
            "M {} {} L {} {} A 20 20, 0, 0, {sweep}, {} {} L {} {}",
            from.x,
            from.y,
            to.x + radius_sign * 20.0,
            from.y,
            to.x,
            from.y - 20.0,
            to.x,
            to.y,
        );
        assert_eq!(layout.arrows[0].d, expected);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let paths = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && has_class(*node, "arrow"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].attribute("d"), Some(expected.as_str()));
    }
}

#[test]
fn branch_display_rows_preserve_quoted_checkout_and_merge_identity() {
    let cases: &[(&str, &str, &[&str])] = &[
        (" left\n right ", " left\n right ", &["left", "right"]),
        (r" left\n right ", " left\n right ", &["left", "right"]),
        (r" left\\n right ", r" left\n right ", &["left", "right"]),
        ("left<br/>right", "left<br>right", &["left", "right"]),
        ("left<BR >right", "left<br>right", &["left", "right"]),
        ("left\n\nright", "left\n\nright", &["left", "", "right"]),
        ("left right", "left right", &["left right"]),
    ];
    for direction in ["LR", "TB", "BT"] {
        for &(authored, identity, expected_rows) in cases {
            let (model, layout, svg) = inspect(&configured(
                json!({"htmlLabels": false}),
                &format!(
                    "gitGraph {direction}:\ncommit id:\"root\"\nbranch \"{authored}\"\ncommit id:\"work\"\ncheckout main\nmerge \"{authored}\" id:\"merged\"\ncheckout \"{authored}\"\n"
                ),
            ));
            assert_eq!(model.branches[1].name, identity);
            assert_eq!(model.current_branch, identity);
            assert_eq!(model.commits[1].branch, identity);
            assert_eq!(model.commits[2].parents, ["root", "work"]);
            assert_eq!(layout.branches[1].name, identity);
            assert_eq!(
                layout
                    .commits
                    .iter()
                    .find(|commit| commit.id == "work")
                    .unwrap()
                    .branch,
                identity
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = document
                .descendants()
                .find(|node| has_class(*node, "branch-label1"))
                .unwrap();
            let rows = label
                .descendants()
                .filter(|node| node.has_tag_name("tspan") && has_class(*node, "row"))
                .collect::<Vec<_>>();
            assert_eq!(
                rows.iter()
                    .map(|row| row.text().unwrap_or(""))
                    .collect::<Vec<_>>(),
                expected_rows,
                "{direction}/{authored:?}",
            );
            for row in rows {
                assert_eq!(
                    row.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
                    Some("preserve")
                );
                assert_eq!(row.attribute("x"), Some("0"));
                assert_eq!(row.attribute("dy"), Some("1em"));
            }
        }
    }
}

#[test]
fn branch_row_geometry_and_resvg_baselines_keep_single_line_controls_and_theme_paint() {
    fn number(node: roxmltree::Node<'_, '_>, attr: &str) -> f64 {
        node.attribute(attr).unwrap().parse().unwrap()
    }

    fn translate_y(node: roxmltree::Node<'_, '_>) -> f64 {
        node.attribute("transform").map_or(0.0, |transform| {
            transform
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .nth(1)
                .unwrap()
                .parse()
                .unwrap()
        })
    }

    let pipeline = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        ..SvgOutputPolicy::default()
    }
    .pipeline();
    for direction in ["LR", "TB", "BT"] {
        for theme in ["default", "redux"] {
            let render = |name: &str| {
                inspect_with_pipeline(
                    &configured(
                        json!({"theme": theme}),
                        &format!("gitGraph {direction}:\nbranch \"{name}\"\ncommit id:\"work\"\n"),
                    ),
                    Some(&pipeline),
                )
            };
            let (_, control_layout, control_safe) = render("wide branch name");
            let control = roxmltree::Document::parse(&control_safe).unwrap();
            let control_bkg = control
                .descendants()
                .find(|node| has_class(*node, "branchLabelBkg") && has_class(*node, "label1"))
                .unwrap();
            let control_css = control
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text();
            for (name, row_count) in [
                (" wide branch name ", 1),
                ("short\nwide branch name", 2),
                ("short\n\nwide branch name", 3),
                ("\nshort\nwide branch name\n", 4),
            ] {
                let (_, layout, safe) = render(name);
                assert_eq!(
                    layout.branches[1].bbox_width,
                    control_layout.branches[1].bbox_width
                );
                let document = roxmltree::Document::parse(&safe).unwrap();
                let bkg = document
                    .descendants()
                    .find(|node| has_class(*node, "branchLabelBkg") && has_class(*node, "label1"))
                    .unwrap();
                assert_eq!(number(bkg, "width"), number(control_bkg, "width"));
                assert_eq!(
                    number(bkg, "height") > number(control_bkg, "height"),
                    row_count > 1
                );
                assert_eq!(
                    document
                        .descendants()
                        .find(|node| node.has_tag_name("style"))
                        .unwrap()
                        .text(),
                    control_css
                );
                assert_eq!(bkg.attribute("style"), control_bkg.attribute("style"));
                let group = document
                    .descendants()
                    .find(|node| has_class(*node, "branch-label1"))
                    .unwrap();
                let text = group
                    .children()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap();
                let rows = text
                    .children()
                    .filter(|node| node.has_tag_name("tspan"))
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), row_count);
                assert_eq!(text.attribute("dominant-baseline"), Some("central"));
                let center = number(bkg, "y") + translate_y(bkg) + number(bkg, "height") / 2.0;
                assert!((number(text, "y") + translate_y(group) - center).abs() < 1e-6);
                let painted = rows
                    .iter()
                    .filter(|row| row.text().is_some_and(|text| !text.is_empty()))
                    .collect::<Vec<_>>();
                let first_dy = painted[0].attribute("dy").unwrap();
                if row_count == 1 {
                    assert_eq!(first_dy, "0");
                    assert_eq!(
                        layout.branches[1].bbox_height,
                        control_layout.branches[1].bbox_height
                    );
                } else {
                    assert_eq!(
                        first_dy.strip_suffix("em").unwrap().parse::<f64>().unwrap(),
                        -(painted.len() as f64 - 1.0) / 2.0
                    );
                    assert!(
                        painted[1..]
                            .iter()
                            .all(|row| row.attribute("dy") == Some("1em"))
                    );
                    assert!(
                        rows.iter()
                            .filter(|row| row.text().is_none())
                            .all(|row| row.attribute("dy") == Some("1em"))
                    );
                }
                assert!(
                    rows.iter()
                        .all(|row| row.attribute("fill").is_none()
                            && row.attribute("style").is_none())
                );
            }
        }
    }
}

#[test]
fn decoded_commit_ids_and_tags_remain_direct_text_instead_of_branch_rows() {
    for (authored, expected) in [
        (r"first\nsecond", "first\nsecond"),
        (r"first\\nsecond", r"first\nsecond"),
    ] {
        let (model, layout, svg) = inspect(&format!(
            "gitGraph\ncommit id:\"{authored}\" tag:\"{authored}\"\n"
        ));
        assert_eq!(model.commits[0].id, expected);
        assert_eq!(layout.commits[0].id, expected);
        assert_eq!(model.commits[0].tags, [expected]);
        let document = roxmltree::Document::parse(&svg).unwrap();
        for class in ["commit-label", "tag-label"] {
            let label = document
                .descendants()
                .find(|node| has_class(*node, class))
                .unwrap();
            assert_eq!(label.text(), Some(expected));
            assert!(!label.children().any(|node| node.has_tag_name("tspan")));
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
fn wider_sibling_tags_do_not_move_each_tags_own_horizontal_center() {
    let history = concat!(
        "gitGraph\ncommit id:\"root\"\nbranch feature\ncommit id:\"F\"\n",
        "checkout main\nmerge feature id:\"M\" tag:\"short\"",
    );
    for theme in ["default", "redux"] {
        for font_size in ["10px", "24px", "150%"] {
            let config = json!({
                "theme": theme,
                "themeVariables": {
                    "fontFamily": "monospace", "tagLabelFontSize": font_size
                }
            });
            let (_, _, single) = inspect(&configured(config.clone(), &format!("{history}\n")));
            let (_, _, stacked) = inspect(&configured(
                config,
                &format!("{history} tag:\"a much wider release label\"\n"),
            ));
            let single_document = roxmltree::Document::parse(&single).unwrap();
            let stacked_document = roxmltree::Document::parse(&stacked).unwrap();
            let single_tag = single_document
                .descendants()
                .find(|node| has_class(*node, "tag-label"))
                .unwrap();
            let stacked_tags = stacked_document
                .descendants()
                .filter(|node| has_class(*node, "tag-label"))
                .collect::<Vec<_>>();
            assert_eq!(stacked_tags.len(), 2);
            assert_eq!(stacked_tags[0].text(), Some("a much wider release label"));
            assert_eq!(stacked_tags[1].text(), Some("short"));
            // drawCommitTags centers each text using its own getBBox().width. Only the
            // shared tag polygons use the widest sibling's width.
            assert_eq!(
                stacked_tags[1].attribute("x"),
                single_tag.attribute("x"),
                "{theme}/{font_size}"
            );
            let long_x = stacked_tags[0]
                .attribute("x")
                .unwrap()
                .parse::<f64>()
                .unwrap();
            let short_x = stacked_tags[1]
                .attribute("x")
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!(long_x < short_x, "{theme}/{font_size}");
        }
    }
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
