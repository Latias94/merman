#![cfg(all(
    feature = "diagram-flowchart",
    any(feature = "layout-dagre", feature = "layout-elk"),
    feature = "native-compact",
    feature = "svg"
))]

use merman::svg::{RenderResourcePolicy, ResourceLimitId, SvgOutputProfile};
#[cfg(feature = "layout-dagre")]
use merman::theme::{BuiltinPreset, ThemeSelection};
use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};

#[cfg(feature = "layout-dagre")]
const FLOWCHART_CONFIG: &str = r#"---
config:
  layout: dagre
  look: classic
  htmlLabels: false
  flowchart:
    htmlLabels: false
    curve: linear
---
"#;

#[cfg(feature = "layout-elk")]
const ELK_CONFIG: &str = r#"---
config:
  layout: elk
  look: classic
  htmlLabels: false
  flowchart:
    htmlLabels: false
  elk:
    lineHops: false
---
"#;

#[cfg(feature = "layout-dagre")]
fn render(body: &str, request: SvgRequest) -> Result<String, RenderError> {
    render_with_config(FLOWCHART_CONFIG, body, request)
}

fn render_with_config(
    config: &str,
    body: &str,
    mut request: SvgRequest,
) -> Result<String, RenderError> {
    let source = format!("{config}{body}");
    request.options.diagram_id = Some("compact-flow".into());
    let output = Renderer::new().render(RenderRequest::svg(
        &source,
        OperationControl::new(),
        request,
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("expected standalone SVG");
    };
    Ok(svg.svg().to_owned())
}

fn compact_request() -> SvgRequest {
    SvgRequest::default().with_output_profile(SvgOutputProfile::NativeCompact)
}

fn shaft_nodes<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    root.descendants().filter(|node| {
        node.has_tag_name("path") && !node.ancestors().any(|parent| parent.has_tag_name("marker"))
    })
}

fn shafts(root: roxmltree::Node<'_, '_>) -> Vec<Vec<f64>> {
    shaft_nodes(root)
        .map(|node| {
            let path = node.attribute("d").expect("expected a shaft path");
            assert!(path.starts_with('M') && path.contains('L'), "{path}");
            path.split(['M', 'L', ',', ' '])
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().expect("expected a shaft coordinate"))
                .collect()
        })
        .collect()
}

#[cfg(feature = "layout-dagre")]
#[test]
fn compact_flowchart_keeps_the_prepared_dagre_shafts() {
    let source = "flowchart TB\nA[Left lane] --> Z[Join here]\nB[Middle] --> Z\nC[Right] --> Z";
    let parity = render(source, SvgRequest::default()).unwrap();
    let compact = render(source, compact_request()).unwrap();
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let compact = roxmltree::Document::parse(&compact).unwrap();
    let root = compact.root_element();
    assert_eq!(root.attribute("id"), Some("compact-flow"));
    assert!(!root.descendants().any(|node| {
        node.has_tag_name("foreignObject")
            || node.has_tag_name("style")
            || node.attribute("data-points").is_some()
    }));
    for label in ["Left lane", "Middle", "Right", "Join here"] {
        assert!(
            root.descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some(label))
        );
    }

    let parity_shafts = shafts(parity.root_element());
    let compact_shafts = shafts(root);
    assert_eq!(parity_shafts.len(), 3, "expected all three parity shafts");
    assert_eq!(compact_shafts.len(), parity_shafts.len());
    assert!(parity_shafts.iter().any(|shaft| shaft.len() > 4));
    for shaft in parity_shafts {
        assert!(compact_shafts.contains(&shaft), "missing shaft {shaft:?}");
    }
}

#[cfg(feature = "layout-dagre")]
#[test]
fn compact_flowchart_keeps_the_native_label_weight() {
    let source = "flowchart LR\nA[Plan]";
    let theme = ThemeSelection::preset(BuiltinPreset::Monograph, None);
    let parity = render(
        source,
        SvgRequest::default().with_native_theme(theme.clone()),
    )
    .unwrap();
    let compact = render(source, compact_request().with_native_theme(theme)).unwrap();
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let compact = roxmltree::Document::parse(&compact).unwrap();

    assert!(parity.descendants().any(|node| {
        node.has_tag_name("style")
            && node
                .text()
                .is_some_and(|css| css.contains("#compact-flow{font-weight:600;}"))
    }));
    let label = compact
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Plan"))
        .expect("expected the compact label");
    assert_eq!(label.attribute("font-weight"), Some("600"));
}

#[cfg(feature = "layout-dagre")]
#[test]
fn compact_flowchart_refuses_an_edge_label_without_falling_back() {
    let source = "flowchart LR\nA[Plan] -->|Review| B[Ship]";
    let parity = render(source, SvgRequest::default()).unwrap();
    assert!(parity.contains("Review"));
    let error = render(source, compact_request()).unwrap_err();
    assert!(matches!(
        error,
        RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            ..
        })
    ));
}

#[cfg(feature = "layout-dagre")]
#[test]
fn compact_flowchart_refuses_interactive_nodes() {
    let source = "flowchart LR\nA[Plan] --> B[Ship]\nclick A href \"https://example.org/\"";
    let error = render(source, compact_request()).unwrap_err();
    assert!(matches!(
        error,
        RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            ..
        })
    ));
}

#[cfg(feature = "layout-dagre")]
#[test]
fn compact_flowchart_checks_the_exact_svg_byte_boundary() {
    let source = "flowchart LR\nA[Plan] --> B[Ship] --- C[Done]";
    let expected = render(source, compact_request()).unwrap();
    let render_capped = |maximum| {
        let mut request = compact_request();
        request.environment = request.environment.with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap(),
        );
        render(source, request)
    };
    assert_eq!(render_capped(expected.len()).unwrap(), expected);
    let RenderError::ResourceLimitExceeded(limit) = render_capped(expected.len() - 1).unwrap_err()
    else {
        panic!("expected the SVG byte limit");
    };
    assert_eq!(limit.id, "max_svg_bytes");
}

#[cfg(feature = "layout-elk")]
#[test]
fn compact_flowchart_elk_keeps_straight_shafts_and_marker_contacts() {
    fn marker<'a, 'input>(
        root: roxmltree::Node<'a, 'input>,
        shaft: roxmltree::Node<'a, 'input>,
    ) -> roxmltree::Node<'a, 'input> {
        let reference = shaft.attribute("marker-end").unwrap();
        let id = reference
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        root.descendants()
            .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
            .unwrap()
    }

    for direction in ["LR", "TB", "RL", "BT"] {
        let body = format!("flowchart {direction}\nA[Plan] --> B[Ship] --- C[Done]");
        let parity = render_with_config(ELK_CONFIG, &body, SvgRequest::default()).unwrap();
        let compact = render_with_config(ELK_CONFIG, &body, compact_request()).unwrap();
        let parity = roxmltree::Document::parse(&parity).unwrap();
        let compact = roxmltree::Document::parse(&compact).unwrap();
        let parity = parity.root_element();
        let compact = compact.root_element();
        let parity_shafts = shafts(parity);
        assert_eq!(parity_shafts.len(), 2, "{direction}");
        assert!(parity_shafts.iter().all(|shaft| shaft.len() == 4));
        assert_eq!(shafts(compact), parity_shafts, "{direction}");
        assert_eq!(compact.attribute("viewBox"), parity.attribute("viewBox"));
        assert!(!compact.descendants().any(|node| {
            node.has_tag_name("foreignObject") || node.attribute("data-points").is_some()
        }));

        let mut parity_edges = shaft_nodes(parity);
        let mut compact_edges = shaft_nodes(compact);
        let parity_marker = marker(parity, parity_edges.next().unwrap());
        let compact_marker = marker(compact, compact_edges.next().unwrap());
        for attr in [
            "viewBox",
            "refX",
            "refY",
            "markerUnits",
            "markerWidth",
            "markerHeight",
            "orient",
        ] {
            assert_eq!(
                compact_marker.attribute(attr),
                parity_marker.attribute(attr)
            );
        }
        for open in [parity_edges.next().unwrap(), compact_edges.next().unwrap()] {
            assert!(open.attribute("marker-start").is_none());
            assert!(open.attribute("marker-end").is_none());
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn compact_flowchart_elk_refuses_rounded_bends_line_hops_and_labels() {
    let line_hops = ELK_CONFIG.replace("lineHops: false", "lineHops: true");
    let bent = "flowchart TB\nA[Plan] --> Z[Ship]\nB[Check] --> Z\nC[Read] --> Z";
    let parity = render_with_config(ELK_CONFIG, bent, SvgRequest::default()).unwrap();
    let parity = roxmltree::Document::parse(&parity).unwrap();
    assert!(
        shaft_nodes(parity.root_element())
            .any(|shaft| shaft.attribute("d").is_some_and(|path| path.contains('Q')))
    );
    for (config, body, expected) in [
        (ELK_CONFIG, bent, "flowchart-elk-straight-painted-route"),
        (
            &line_hops,
            "flowchart LR\nA[Plan] --> B[Ship]",
            "flowchart-elk-line-hops",
        ),
        (
            ELK_CONFIG,
            "flowchart LR\nA[Plan] -->|Review| B[Ship]",
            "flowchart-edge-shape-style-or-label",
        ),
    ] {
        let error = render_with_config(config, body, compact_request()).unwrap_err();
        assert!(
            matches!(error, RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            reason,
        }) if reason == expected),
            "{error:?}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn compact_flowchart_elk_checks_exact_output_limits() {
    let body = "flowchart LR\nA[Plan] --> B[Ship] --- C[Done]";
    let expected = render_with_config(ELK_CONFIG, body, compact_request()).unwrap();
    let element_count = roxmltree::Document::parse(&expected)
        .unwrap()
        .descendants()
        .filter(roxmltree::Node::is_element)
        .count();
    for (limit, maximum, id) in [
        (
            ResourceLimitId::MaxSvgBytes,
            expected.len(),
            "max_svg_bytes",
        ),
        (
            ResourceLimitId::MaxSvgElements,
            element_count,
            "max_svg_elements",
        ),
    ] {
        let render_capped = |maximum| {
            let mut request = compact_request();
            request.environment = request.environment.with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(limit, maximum)
                    .unwrap(),
            );
            render_with_config(ELK_CONFIG, body, request)
        };
        assert_eq!(render_capped(maximum).unwrap(), expected);
        let RenderError::ResourceLimitExceeded(error) = render_capped(maximum - 1).unwrap_err()
        else {
            panic!("expected {id}");
        };
        assert_eq!(error.id, id);
    }
}
