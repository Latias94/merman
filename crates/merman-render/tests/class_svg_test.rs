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

fn render_class_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_opts = LayoutOptions::headless_svg_defaults();
    let out = layout_parsed(&parsed, &layout_opts).expect("layout ok");
    let LayoutDiagram::ClassDiagramV2(layout) = &out.layout else {
        panic!("expected ClassDiagramV2 layout");
    };

    render_class_diagram_v2_svg(
        layout,
        &out.semantic,
        &out.meta.effective_config,
        out.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &SvgRenderOptions::default(),
    )
    .expect("svg render ok")
}

#[test]
fn class_svg_dotted_namespace_titles_use_hierarchical_segment_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Company.Project.Module {
  class User
}
"#,
    );

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected default hierarchical namespace labels to use path segments"
    );
    assert!(
        !svg.contains("<p>Company.Project.Module</p>"),
        "default hierarchical mode should not render the full dotted id as the leaf label"
    );
}

#[test]
fn class_svg_scopes_text_color_for_html_labels() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
"#,
    );

    assert!(
        svg.contains(r#"#merman p{margin:0;}"#),
        "expected class SVG to reset HTML label paragraph margins"
    );
    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#131300;}"#),
        "expected class SVG to make HTML labels self-contained instead of inheriting host page color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#131300;}"#),
        "expected class SVG text labels to get an explicit fill color"
    );
}

#[test]
fn class_svg_honors_configured_class_text_color() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"themeVariables": {"classText": "#123456"}}}%%
classDiagram
    class Animal
"##,
    );

    assert!(
        svg.contains(r#"#merman .nodeLabel,#merman .edgeLabel{color:#123456;}"#),
        "expected classText theme variable to drive HTML label color"
    );
    assert!(
        svg.contains(r#"#merman .label text{fill:#123456;}"#),
        "expected classText theme variable to drive SVG text fill"
    );
}

#[test]
fn class_svg_uses_configured_look_in_dom_attributes() {
    let svg = render_class_svg_from_text(
        r#"%%{init: {"look": "neo"}}%%
classDiagram
namespace Zoo {
  class Animal
  class Keeper
}
Animal --> Keeper
"#,
    );

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected class SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured class look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn class_svg_namespace_clusters_keep_theme_fill() {
    let svg = render_class_svg_from_text(
        r#"classDiagram
namespace Platform {
  class Api
}
namespace Platform.FFI {
  class Bridge
}
namespace Platform.Core {
  class Engine
}
"#,
    );

    assert!(
        svg.contains(r#"#merman .cluster rect{fill:#ffffde;stroke:#aaaa33;stroke-width:1px;}"#),
        "expected class namespace cluster CSS to provide the upstream yellow fill: {svg}"
    );
    assert!(
        !svg.contains(r#"style="fill:none !important;stroke:black !important""#),
        "namespace cluster rects must not override the theme fill with transparent inline CSS: {svg}"
    );
}

#[test]
fn class_svg_honors_numeric_stroke_width_theme_css() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"themeVariables": {"mainBkg": "#112233", "nodeBorder": "#445566", "lineColor": "#778899", "strokeWidth": 7}}}%%
classDiagram
    Animal <|-- Dog
    class Animal
    class Dog
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#112233;stroke:#445566;stroke-width:7}"#
        ),
        "expected numeric strokeWidth to drive Class node shape CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .divider{stroke:#445566;stroke-width:1;}"#),
        "expected nodeBorder to drive Class divider CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:7;fill:none;}"#),
        "expected numeric strokeWidth to drive Class relation CSS: {svg}"
    );
    assert!(
        !svg.contains(r#"#merman .relation{stroke:#778899;stroke-width:1;fill:none;}"#),
        "Class relation CSS must not drop numeric strokeWidth overrides: {svg}"
    );
}

#[test]
fn class_svg_honors_configured_note_theme_colors() {
    for html_labels in [true, false] {
        let svg = render_class_svg_from_text(&format!(
            r##"%%{{init: {{"htmlLabels": {html_labels}, "themeVariables": {{"noteBkgColor": "#112233", "noteBorderColor": "#445566", "noteTextColor": "#778899"}}}}}}%%
classDiagram
    class Animal
    note for Animal "hello"
"##
        ));

        assert!(
            svg.contains(
                r##"fill="#112233" style="fill:#112233 !important;stroke:#445566 !important""##
            ),
            "expected configured noteBkgColor/noteBorderColor in note body for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(r##"stroke="#445566" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill:#112233 !important;stroke:#445566 !important""##),
            "expected configured noteBorderColor in note rough stroke for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            svg.contains(
                r#"#merman .noteLabel .nodeLabel,#merman .noteLabel .edgeLabel{color:#778899;}"#
            ),
            "expected noteTextColor CSS for htmlLabels={html_labels}: {svg}"
        );
        assert!(
            !svg.contains(
                r##"fill="#fff5ad" style="fill:#fff5ad !important;stroke:#aaaa33 !important""##
            ),
            "note shape must not ignore configured colors for htmlLabels={html_labels}: {svg}"
        );
    }
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
fn class_svg_namespaces_use_11_15_hierarchical_labels_and_keep_relation_label() {
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

    assert!(svg.contains(r#"id="merman-Company" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project" data-look="classic""#));
    assert!(svg.contains(r#"id="merman-Company.Project.Module" data-look="classic""#));
    assert!(
        svg.contains("<p>Company</p>")
            && svg.contains("<p>Project</p>")
            && svg.contains("<p>Module</p>"),
        "expected dotted namespace labels to use Mermaid 11.15 path segments"
    );
    assert!(
        svg.contains("<p>manages</p>"),
        "expected relation label text to survive hierarchical namespace rendering"
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
        svg.contains(r#"<g class="root" transform="translate(-8, 0)"><g class="clusters">"#),
        "expected nested namespace wrapper to keep Mermaid's -8px root translation"
    );
    assert!(
        svg.contains(r#"</g><g class="edgeLabels"></g><g class="edgePaths"></g><g class="nodes">"#),
        "expected nested namespace wrapper placeholders to keep Mermaid order"
    );
    assert!(
        svg.contains(r#"max-width: 114px; text-align: center;"><span class="nodeLabel markdown-node-label" style=""><p>Outer.Foo</p>"#),
        "expected qualified namespace reference title to keep Mermaid max-width"
    );
}

#[test]
fn class_svg_multiple_dotted_namespace_subgraphs_use_segment_labels() {
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

    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.A" data-look="classic""#
    ));
    assert!(svg.contains(
        r#"id="stress_class_nested_namespaces_many_levels_021-Root.B.B1" data-look="classic""#
    ));
    assert!(
        svg.contains("<p>A</p>") && svg.contains("<p>B1</p>"),
        "expected rendered dotted namespace clusters to use path-segment labels"
    );
    assert!(
        svg.contains("<p>Root.A.A1</p>") && svg.contains("<p>Root.B.B1.B1a</p>"),
        "expected qualified relation facade class labels to remain visible"
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
                "max-width: 197px",
                "max-width: 276px",
                "max-width: 229px",
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
                r#"id="merman-classId-Class1-0" data-look="classic" transform="translate(72.1171875, 92)""#,
                r#"<path d="M-64.1171875 -84 L64.1171875 -84 L64.1171875 84 L-64.1171875 84""#,
            ],
        ),
        (
            "stress_class_interfaces_and_abstracts_007.mmd",
            &[
                r#"id="merman-classId-IService-0" data-look="classic" transform="translate(61.171875, 83)""#,
                r#"<path d="M-53.171875 -54 L53.171875 -54 L53.171875 54 L-53.171875 54""#,
            ],
        ),
        (
            "stress_class_member_separators_and_annotations_009.mmd",
            &[
                r#"id="merman-classId-Data-0" data-look="classic" transform="translate(145.48828125, 292)""#,
                r#"<path d="M-137.48828125 -108 L137.48828125 -108 L137.48828125 108 L-137.48828125 108""#,
            ],
        ),
        (
            "stress_class_enums_and_interfaces_mix_023.mmd",
            &[
                r#"id="merman-classId-Status-0" data-look="classic" transform="translate(485.59765625, 104)""#,
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
        svg.contains(r#"<foreignObject width="36" height="12">"#)
            && svg.contains(r#"<span class="edgeLabel"><p>many</p></span>"#),
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

#[test]
fn class_svg_preserves_numeric_theme_font_size_css_spelling() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"fontSize": 10, "themeVariables": {"fontSize": 24}, "htmlLabels": false} }%%
classDiagram
  class FontSizeSvgProbe {
    +veryLongMethodNameToForceMeasurement()
  }
"##,
    );

    assert!(
        svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24;fill:"#
        ),
        "numeric themeVariables.fontSize should be emitted like Mermaid's raw CSS value"
    );
    assert!(
        !svg.contains(
            r#"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:"#
        ),
        "numeric themeVariables.fontSize must not be rewritten as a px string"
    );
}

#[test]
fn class_svg_px_string_theme_font_size_uses_mermaid_svg_label_wrapping() {
    let svg = render_class_svg_from_text(
        r##"%%{init: {"theme": "base", "fontSize": 10, "themeVariables": {"fontSize": "24px"}, "htmlLabels": false} }%%
classDiagram
  class Foo {
    +veryLongMemberNameToWrapTheLayoutProbe: String
    +anotherVeryLongMemberNameToWrapTheLayoutProbe: String
    +thirdVeryLongMemberNameToWrapTheLayoutProbe: String
  }
"##,
    );

    assert!(
        svg.contains(
            r#"Probe:</tspan><tspan font-style="normal" class="text-inner-tspan" font-weight="normal"> String</tspan>"#
        ),
        "expected Mermaid-like native SVG wrapping to keep the type suffix on the second row: {svg}"
    );
    assert!(
        !svg.contains(
            r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">String</tspan></tspan>"#
        ),
        "type suffix should not be forced onto a standalone third row: {svg}"
    );
}
