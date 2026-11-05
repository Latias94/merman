#![cfg(feature = "diagram-flowchart")]

//! Generated ancestor class selectors own only the edge path terminals they actually match.

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderedFamilySvg};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

mod common;

const SOURCE_STYLES: &str = "stroke:#aabbcc,stroke-width:3px,stroke-dasharray:2 4";
const TYPED_STROKE: &str = "#123456";

fn theme(patches: impl IntoIterator<Item = ThemeStylePatch>) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                patches
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |rules, patch| {
                        rules.with_rule(ThemeRule::new(ThemeTarget::Edge, patch))
                    }),
            ),
        )
        .unwrap()
}

fn edge_theme() -> DiagramTheme {
    theme([
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid(TYPED_STROKE).unwrap()),
        ThemeStylePatch::default().with_stroke_width(8.0).unwrap(),
        ThemeStylePatch::default()
            .with_stroke_dasharray([9.0, 7.0])
            .unwrap(),
    ])
}

fn render(
    source: &str,
    theme: Option<&DiagramTheme>,
    html: bool,
    layout: &str,
) -> RenderedFamilySvg {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(json!({
            "layout": layout, "look": "classic", "htmlLabels": html,
            "flowchart": {"htmlLabels": html}
        })),
    );
    let engine = match theme {
        Some(theme) => common::install_parse_compatibility(theme, engine),
        None => engine,
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("structural-edge-theme".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
}

fn layouts() -> &'static [&'static str] {
    if cfg!(feature = "layout-elk") {
        &["dagre", "elk"]
    } else {
        &["dagre"]
    }
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn path<'a, 'input>(document: &'a roxmltree::Document<'input>) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .expect("real edge path terminal")
}

fn property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?
        .rsplit(';')
        .find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == property).then(|| {
                let value = value.trim();
                value.strip_suffix("!important").unwrap_or(value).trim()
            })
        })
}

fn assert_typed(path: roxmltree::Node<'_, '_>, applied: bool, case: &str) {
    let ancestors = path
        .ancestors()
        .filter(roxmltree::Node::is_element)
        .map(|node| {
            format!(
                "{}[class={:?}]",
                node.tag_name().name(),
                node.attribute("class")
            )
        })
        .collect::<Vec<_>>()
        .join(" <- ");
    for (name, value) in [
        ("stroke", TYPED_STROKE),
        ("stroke-width", "8px"),
        ("stroke-dasharray", "9 7"),
    ] {
        assert_eq!(
            property(path, name) == Some(value),
            applied,
            "{case}; property={name}, actual={:?}, edge={:?}, style={:?}, marker-end={:?}, ancestors={ancestors}",
            property(path, name),
            path.attribute("data-id"),
            path.attribute("style"),
            path.attribute("marker-end")
        );
    }
}

fn marker<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    path: roxmltree::Node<'_, '_>,
) -> roxmltree::Node<'a, 'input> {
    let id = path
        .attribute("marker-end")
        .and_then(|reference| reference.strip_prefix("url(#"))
        .and_then(|reference| reference.strip_suffix(')'))
        .expect("edge arrowhead reference");
    document
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
        .and_then(|node| node.descendants().find(|node| node.has_tag_name("path")))
        .expect("actual referenced arrowhead shape")
}

fn assert_evidence(rendered: RenderedFamilySvg, applied: usize, absent: usize, residual: usize) {
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), applied + absent + residual);
    assert_eq!(evidence.accounted_count(), evidence.required_count());
    assert_eq!(evidence.applied_count(), applied);
    assert_eq!(evidence.not_applicable_count(), absent);
    assert_eq!(evidence.theme_residual_count(), residual);
}

#[test]
fn structural_selectors_preserve_edge_source_ownership_and_marker_selection() {
    let theme = edge_theme();
    for layout in layouts() {
        for html in [false, true] {
            for class in ["root", "edgePaths", "edges", "nodes"] {
                let source = format!("flowchart LR\nA --> B\nclassDef {class} {SOURCE_STYLES}\n");
                let baseline = render(&source, None, html, layout);
                let rendered = render(&source, Some(&theme), html, layout);
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let baseline_document = roxmltree::Document::parse(baseline.svg()).unwrap();
                let path = path(&document);
                let parent = path.parent_element().expect("edge group");
                assert!(has_class(parent, "edgePaths"));
                let matches = if html {
                    has_class(parent, class)
                } else {
                    path.ancestors().skip(1).any(|node| has_class(node, class))
                };
                let suffix = if html { ">*" } else { " path" };
                assert!(rendered.svg().contains(&format!(" .{class}{suffix}{{")));
                assert_typed(
                    path,
                    !matches,
                    &format!(
                        "layout={layout}, html={html}, class={class}, source_matches={matches}"
                    ),
                );
                let arrowhead = marker(&document, path);
                if matches {
                    let original = self::path(&baseline_document);
                    // No source declarations are copied into the inline path or arrowhead.
                    assert_eq!(path.attribute("style"), original.attribute("style"));
                    assert_eq!(
                        path.attribute("marker-end"),
                        original.attribute("marker-end")
                    );
                    let baseline_marker = marker(&baseline_document, original);
                    for name in ["style", "stroke", "fill"] {
                        assert_eq!(arrowhead.attribute(name), baseline_marker.attribute(name));
                    }
                    assert!(
                        rendered.svg() == baseline.svg(),
                        "fully source-owned Edge rules must preserve the complete SVG: {layout}/{html}/{class}"
                    );
                } else {
                    assert_eq!(arrowhead.attribute("stroke"), Some(TYPED_STROKE));
                    assert_eq!(arrowhead.attribute("fill"), Some(TYPED_STROKE));
                }
                assert_evidence(
                    rendered,
                    if matches { 0 } else { 3 },
                    if matches { 3 } else { 0 },
                    0,
                );
            }
        }
    }
}

#[test]
fn structural_edge_owners_preserve_untyped_and_empty_theme_svg_bytes() {
    let empty_theme = theme([]);
    for layout in layouts() {
        for html in [false, true] {
            for class in ["root", "edgePaths", "edges", "nodes"] {
                for link_style in ["", "linkStyle default stroke:#112233,stroke-width:11px\n"] {
                    let source = format!(
                        "flowchart LR\nA --> B\nclassDef {class} stroke:#aabbcc,stroke-width:40px,stroke-dasharray:2 4\n{link_style}"
                    );
                    let untyped = render(&source, None, html, layout);
                    let empty = render(&source, Some(&empty_theme), html, layout);
                    assert!(
                        untyped.svg() == empty.svg(),
                        "structural ownership must not alter untyped marker colors or geometry: {layout}/{html}/{class}/{link_style}"
                    );
                }
            }
        }
    }
}

#[test]
fn structural_edge_ownership_is_per_property() {
    let theme = edge_theme();
    for (declaration, property_name, typed_value) in [
        ("stroke:#aabbcc", "stroke", TYPED_STROKE),
        ("stroke-width:0px", "stroke-width", "8px"),
        ("stroke-dasharray:none", "stroke-dasharray", "9 7"),
    ] {
        let source = format!("flowchart LR\nA --> B\nclassDef edgePaths {declaration}\n");
        let rendered = render(&source, Some(&theme), true, "dagre");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let path = path(&document);
        assert_ne!(property(path, property_name), Some(typed_value));
        for (other, value) in [
            ("stroke", TYPED_STROKE),
            ("stroke-width", "8px"),
            ("stroke-dasharray", "9 7"),
        ] {
            if other != property_name {
                assert_eq!(property(path, other), Some(value));
            }
        }
        if property_name == "stroke" {
            assert_ne!(
                marker(&document, path).attribute("stroke"),
                Some(TYPED_STROKE)
            );
        } else {
            assert_eq!(
                marker(&document, path).attribute("stroke"),
                Some(TYPED_STROKE)
            );
        }
        assert_evidence(rendered, 2, 1, 0);
    }
}

#[test]
fn nested_dagre_edges_only_inherit_matching_native_nodes_selectors() {
    let theme = edge_theme();
    let source = format!(
        "flowchart TB\nsubgraph Group[Group]\ndirection LR\nA --> B\nend\nC --> D\nclassDef nodes {SOURCE_STYLES}\n"
    );
    for html in [false, true] {
        let rendered = render(&source, Some(&theme), html, "dagre");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let mut nested = 0;
        let mut top = 0;
        for path in document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        {
            let in_nodes = path
                .ancestors()
                .skip(1)
                .any(|node| has_class(node, "nodes"));
            if in_nodes {
                nested += 1;
            } else {
                top += 1;
            }
            let owns = !html && in_nodes;
            assert_typed(
                path,
                !owns,
                &format!("layout=dagre, html={html}, class=nodes, nested={in_nodes}"),
            );
            assert_eq!(
                marker(&document, path).attribute("stroke") == Some(TYPED_STROKE),
                !owns
            );
        }
        assert!(
            nested > 0 && top > 0,
            "fixture must exercise real nested and top-level paths"
        );
        assert_evidence(rendered, 3, 0, 0);
    }
}

#[test]
fn structural_edge_ownership_uses_emitted_safe_css_and_catalog_order() {
    let theme =
        theme([ThemeStylePatch::default().with_stroke(CanvasPaint::solid(TYPED_STROKE).unwrap())]);
    for (definitions, applied, absent, residual) in [
        (
            "classDef root stroke:url(https://example.com/paint)\n",
            1,
            0,
            0,
        ),
        ("classDef root stroke:var(--edge-color)\n", 0, 1, 0),
        (
            "classDef root stroke:var(--edge-color)\nclassDef edgePaths stroke:#aabbcc\n",
            0,
            1,
            0,
        ),
        (
            "classDef edgePaths stroke:#aabbcc\nclassDef root stroke:var(--edge-color)\n",
            0,
            1,
            0,
        ),
    ] {
        let source = format!("flowchart LR\nA --> B\n{definitions}");
        let rendered = render(&source, Some(&theme), false, "dagre");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let path = path(&document);
        assert_eq!(
            property(path, "stroke") == Some(TYPED_STROKE),
            applied == 1,
            "{definitions}"
        );
        assert_eq!(
            marker(&document, path).attribute("stroke") == Some(TYPED_STROKE),
            applied == 1
        );
        assert_evidence(rendered, applied, absent, residual);
    }
}
