#![cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    feature = "native-compact",
    feature = "svg"
))]

use merman::svg::{RenderResourcePolicy, ResourceLimitId, SvgOutputProfile};
use merman::theme::{BuiltinPreset, ThemeSelection};
use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};

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

fn render(body: &str, mut request: SvgRequest) -> Result<String, RenderError> {
    let source = format!("{FLOWCHART_CONFIG}{body}");
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

fn shafts(root: roxmltree::Node<'_, '_>) -> Vec<Vec<f64>> {
    root.descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && !node.ancestors().any(|parent| parent.has_tag_name("marker"))
        })
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
