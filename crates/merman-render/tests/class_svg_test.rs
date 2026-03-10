use merman_core::{Engine, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    SvgRenderOptions, render_class_diagram_v2_debug_svg, render_class_diagram_v2_svg,
};
use merman_render::{LayoutOptions, layout_parsed};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn class_debug_svg_renders_terminal_labels() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_debug_svg(&layout, &SvgRenderOptions::default());
    assert!(svg.contains("<svg"));
    assert!(
        svg.contains("terminal-label-box"),
        "expected terminal label boxes in debug svg"
    );
}

#[test]
fn class_svg_generic_title_uses_upstream_max_width_override() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_cypress_classdiagram_v3_spec_12_should_render_a_simple_class_diagram_with_generic_types_021.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains("max-width: 166px"),
        "expected generic class title to keep Mermaid-matching max-width"
    );
    assert!(
        svg.contains("max-width: 170px") && svg.contains("max-width: 323px"),
        "expected generic member/method rows to keep Mermaid-matching max-widths"
    );
}

#[test]
fn class_svg_namespaces_and_relation_labels_keep_upstream_geometry() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_namespaces_and_generics.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"id="Company.Project" data-look="classic"><rect x="395.7421875" y="208" width="396.1640625" height="220" style="fill:none !important;stroke:black !important"/>"#),
        "expected Company.Project cluster geometry to match Mermaid"
    );
    assert!(
        svg.contains(r#"<foreignObject width="62.078125" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"><p>manages</p></span></div></foreignObject>"#),
        "expected relation label width for `manages` to match Mermaid"
    );
}

#[test]
fn class_svg_nested_namespace_subgraphs_keep_mermaid_wrapper_structure() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("stress_class_comments_inside_namespaces_024.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_comments_inside_namespaces_024".to_string()),
            ..Default::default()
        },
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"<g class="root" transform="translate(-8,0)"><g class="clusters">"#),
        "expected nested namespace wrapper to keep Mermaid's -8px root translation"
    );
    assert!(
        svg.contains(r#"</g><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes">"#),
        "expected nested namespace wrapper placeholders to keep Mermaid order"
    );
    assert!(
        svg.contains(r#"max-width: 114px; text-align: center;"><span class="nodeLabel markdown-node-label" style=""><p>Outer.Foo</p>"#),
        "expected qualified namespace reference title to keep Mermaid max-width"
    );
}

#[test]
fn class_svg_multiple_namespace_subgraphs_keep_local_root_offsets() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("stress_class_nested_namespaces_many_levels_021.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("stress_class_nested_namespaces_many_levels_021".to_string()),
            ..Default::default()
        },
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"<g class="root" transform="translate(-8,0)"><g class="clusters">"#),
        "expected first namespace root to keep Mermaid's left margin wrapper"
    );
    assert!(
        svg.contains(r#"<g class="root" transform="translate(160."#),
        "expected later namespace roots to keep Mermaid's local wrapper offsets"
    );
    assert!(
        svg.contains(r#"id="Root.B.B1""#)
            && svg.contains(r#"x="8" y="8" width="127.203125" height="288""#),
        "expected second namespace cluster geometry to be localized inside its wrapper"
    );
}

#[test]
fn class_svg_long_relation_labels_wrap_to_mermaid_html_cap() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("stress_class_long_labels_wrapping_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"<foreignObject width="200" height="72">"#)
            && svg.contains("display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: 200px;"),
        "expected long class relation labels to wrap at Mermaid's 200px HTML cap"
    );
}

#[test]
fn class_svg_annotations_and_comment_rows_keep_mermaid_html_caps() {
    let fixtures: &[(&str, &[&str])] = &[
        (
            "upstream_annotations_in_brackets_spec.mmd",
            &[
                "max-width: 102px",
                "max-width: 116px",
                "max-width: 120px",
                "max-width: 81px",
            ],
        ),
        (
            "stress_class_comments_and_spacing_005.mmd",
            &["max-width: 177px"],
        ),
        (
            "stress_class_interfaces_and_abstracts_007.mmd",
            &["max-width: 103px", "max-width: 135px", "max-width: 129px"],
        ),
        (
            "stress_class_member_separators_and_annotations_009.mmd",
            &["max-width: 146px", "max-width: 233px", "max-width: 218px"],
        ),
        (
            "stress_class_enums_and_interfaces_mix_023.mmd",
            &["max-width: 89px", "max-width: 134px", "max-width: 147px"],
        ),
        (
            "stress_class_styles_classdef_and_inline_010.mmd",
            &["max-width: 92px", "max-width: 89px", "max-width: 102px"],
        ),
        (
            "stress_class_styles_multiple_classdef_016.mmd",
            &[
                "max-width: 89px",
                "max-width: 76px",
                "max-width: 72px",
                "max-width: 194px",
                "max-width: 271px",
                "max-width: 226px",
            ],
        ),
    ];

    for (fixture, expected_caps) in fixtures {
        let path = workspace_root()
            .join("fixtures")
            .join("class")
            .join(fixture);
        let text = std::fs::read_to_string(&path).expect("fixture");

        let engine = Engine::new();
        let parsed =
            futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .expect("parse ok")
                .expect("diagram detected");

        let layout_opts = LayoutOptions::headless_svg_defaults();
        let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
        let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
            panic!("expected ClassDiagramV2 layout");
        };

        let svg = render_class_diagram_v2_svg(
            layout,
            &out.semantic,
            &out.meta.effective_config,
            out.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &SvgRenderOptions::default(),
        )
        .expect("svg render ok");

        for expected_cap in expected_caps.iter().copied() {
            assert!(
                svg.contains(expected_cap),
                "expected {fixture} to contain Mermaid-matching HTML cap {expected_cap}"
            );
        }
    }
}

#[test]
fn class_svg_annotation_width_overrides_drive_html_node_bounds() {
    let fixtures: &[(&str, &[&str])] = &[
        (
            "upstream_annotations_in_brackets_spec.mmd",
            &[
                r#"id="classId-Class1-0" transform="translate(72.1171875, 92)""#,
                r#"<path d="M-64.1171875 -84 L64.1171875 -84 L64.1171875 84 L-64.1171875 84""#,
            ],
        ),
        (
            "stress_class_interfaces_and_abstracts_007.mmd",
            &[
                r#"id="classId-IService-0" transform="translate(61.171875, 83)""#,
                r#"<path d="M-53.171875 -54 L53.171875 -54 L53.171875 54 L-53.171875 54""#,
            ],
        ),
        (
            "stress_class_member_separators_and_annotations_009.mmd",
            &[
                r#"id="classId-Data-0" transform="translate(145.48828125, 292)""#,
                r#"<path d="M-137.48828125 -108 L137.48828125 -108 L137.48828125 108 L-137.48828125 108""#,
            ],
        ),
        (
            "stress_class_enums_and_interfaces_mix_023.mmd",
            &[
                r#"id="classId-Status-0" transform="translate(485.59765625, 104)""#,
                r#"<path d="M-76.28515625 -96 L76.28515625 -96 L76.28515625 96 L-76.28515625 96""#,
            ],
        ),
    ];

    for (fixture, expected_snippets) in fixtures {
        let path = workspace_root()
            .join("fixtures")
            .join("class")
            .join(fixture);
        let text = std::fs::read_to_string(&path).expect("fixture");

        let engine = Engine::new();
        let parsed =
            futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .expect("parse ok")
                .expect("diagram detected");

        let layout_opts = LayoutOptions::headless_svg_defaults();
        let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
        let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
            panic!("expected ClassDiagramV2 layout");
        };

        let svg = render_class_diagram_v2_svg(
            layout,
            &out.semantic,
            &out.meta.effective_config,
            out.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &SvgRenderOptions::default(),
        )
        .expect("svg render ok");

        for expected in expected_snippets.iter().copied() {
            assert!(
                svg.contains(expected),
                "expected {fixture} to keep Mermaid annotation-driven node geometry: {expected}"
            );
        }
    }
}

#[test]
fn class_svg_cardinality_terminals_keep_mermaid_sizes_and_offsets() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"<foreignObject style="width: 36px; height: 12px;">"#)
            && svg.contains(r#"<span class="edgeLabel">many</span>"#),
        "expected `many` cardinality terminal to keep Mermaid width sizing"
    );
}

#[test]
fn class_svg_edge_labels_precede_terminals_in_edge_labels_group() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("stress_class_parallel_edges_and_cardinality_004.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    let edge_labels_start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("edgeLabels group");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes group after edge labels");
    let section = &svg[edge_labels_start..nodes_start];
    let last_label = section
        .rfind(r#"<g class="edgeLabel""#)
        .expect("edgeLabel group present");
    let first_terminal = section
        .find(r#"<g class="edgeTerminals""#)
        .expect("edge terminal group present");

    assert!(
        last_label < first_terminal,
        "expected Mermaid-style edgeLabels ordering: all edgeLabel groups before edgeTerminals"
    );
}

#[test]
fn class_svg_terminal_groups_keep_upstream_dom_order_for_mixed_cardinalities() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    let edge_labels_start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("edgeLabels group");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes group after edge labels");
    let section = &svg[edge_labels_start..nodes_start];

    let first_terminal = section
        .find(r#"<g class="edgeTerminals" transform="translate(680.59375, 109.5)">"#)
        .expect("first terminal present");
    let second_terminal = section
        .find(r#"<g class="edgeTerminals" transform="translate(964.71875, 143.5)">"#)
        .expect("second terminal present");
    let third_terminal = section
        .find(r#"<g class="edgeTerminals" transform="translate(705.59375, 143.5)">"#)
        .expect("third terminal present");

    assert!(
        first_terminal < second_terminal && second_terminal < third_terminal,
        "expected mixed cardinality terminals to keep Mermaid DOM order"
    );
}

#[test]
fn class_svg_single_char_title_keeps_upstream_html_max_width() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("stress_class_many_relations_labels_020.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    let e_idx = svg
        .find("<p>E</p>")
        .expect("single-character title present");
    let e_section = &svg[e_idx.saturating_sub(260)..(e_idx + 120).min(svg.len())];

    assert!(
        e_section.contains("max-width: 60px"),
        "expected single-character title `E` to keep Mermaid's 60px max-width"
    );
}

#[test]
fn class_svg_relation_titles_decode_entities_once() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::default();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains(r#"<p>&lt; owns</p>"#),
        "expected relation title entities to render exactly once"
    );
    assert!(
        !svg.contains("&amp;lt; owns"),
        "expected relation title entities to avoid double escaping"
    );
}

#[test]
fn class_svg_relation_only_generic_nodes_keep_type_suffix() {
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_cypress_classdiagram_v3_spec_8_should_render_a_simple_class_diagram_with_generic_class_and_re_016.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let out = layout_parsed(&parsed, &LayoutOptions::default()).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    let layout_opts = LayoutOptions::default();
    let svg = render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok");

    assert!(
        svg.contains("Class01&lt;T")
            && svg.contains("Class03&lt;T")
            && svg.contains("Class04&lt;T"),
        "expected relation-only generic classes to keep Mermaid-matching type suffixes"
    );
}
