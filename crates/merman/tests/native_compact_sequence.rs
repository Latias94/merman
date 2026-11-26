#![cfg(all(feature = "diagram-sequence", feature = "svg"))]

use merman::svg::SvgOutputProfile;
use merman::{
    OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgOutput, SvgRequest,
};

const SEQUENCE: &str = r#"---
config:
  look: classic
  theme: default
---
sequenceDiagram
    participant Client
    participant Server
    Client->>Server: Send request
    Note over Server: Plain note
"#;

fn standalone(source: &str, request: SvgRequest) -> Result<SvgOutput, RenderError> {
    let output =
        Renderer::new().render(RenderRequest::svg(source, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("expected a standalone SVG");
    };
    Ok(svg)
}

fn compact_request() -> SvgRequest {
    SvgRequest::default().with_output_profile(SvgOutputProfile::NativeCompact)
}

fn compact_unsupported(error: RenderError) -> &'static str {
    let RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile { profile, reason }) =
        error
    else {
        panic!("expected an unsupported output profile, got {error:?}");
    };
    assert_eq!(profile, SvgOutputProfile::NativeCompact);
    assert!(!reason.is_empty());
    reason
}

#[test]
fn default_sequence_svg_matches_explicit_mermaid_parity() {
    let default = standalone(SEQUENCE, SvgRequest::default()).unwrap();
    let parity = standalone(
        SEQUENCE,
        SvgRequest::default().with_output_profile(SvgOutputProfile::MermaidV12Parity),
    )
    .unwrap();
    assert_eq!(default.svg(), parity.svg());
    assert!(parity.svg().contains("<style"));
}

#[cfg(feature = "native-compact")]
fn positive_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.attribute(name)
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|value| value.is_finite() && value > 0.0)
}

#[cfg(feature = "native-compact")]
#[test]
fn compact_sequence_has_visible_svg_and_locally_scoped_markers() {
    for diagram_id in ["compact-client", "compact-server"] {
        let mut request = compact_request();
        request.options.diagram_id = Some(diagram_id.to_owned());
        let svg = standalone(SEQUENCE, request).unwrap();
        let document = roxmltree::Document::parse(svg.svg()).unwrap();
        let root = document.root_element();
        assert!(root.has_tag_name(("http://www.w3.org/2000/svg", "svg")));
        assert_eq!(root.attribute("id"), Some(diagram_id));
        assert!(
            !root
                .descendants()
                .any(|node| node.has_tag_name("foreignObject") || node.has_tag_name("style"))
        );

        let find_text = |label: &str| {
            root.descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some(label))
                .unwrap_or_else(|| panic!("missing SVG text {label}"))
        };
        for label in ["Client", "Server", "Plain note"] {
            let rect = find_text(label)
                .parent()
                .unwrap()
                .children()
                .find(|child| child.has_tag_name("rect"))
                .unwrap_or_else(|| panic!("missing rectangle for {label}"));
            assert!(positive_attribute(rect, "width") && positive_attribute(rect, "height"));
            assert!(rect.attribute("fill").is_some_and(|fill| fill != "none"));
        }
        find_text("Send request");

        let shaft = root
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("marker-end").is_some())
            .expect("expected the message shaft and its end marker");
        assert!(positive_attribute(shaft, "stroke-width"));
        let path = shaft.attribute("d").unwrap();
        assert!(path.starts_with('M') && path.contains('L'));
        let coordinates: Vec<f64> = path
            .split(['M', 'L', ',', ' '])
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().unwrap())
            .collect();
        let [start_x, start_y, end_x, end_y] = coordinates.as_slice() else {
            panic!("expected one straight message segment, got {path}");
        };
        assert!(start_x < end_x);
        assert_eq!(start_y, end_y);

        let marker_id = shaft
            .attribute("marker-end")
            .and_then(|url| url.strip_prefix("url(#"))
            .and_then(|url| url.strip_suffix(')'))
            .expect("expected a same-document marker reference");
        assert!(marker_id.starts_with(&format!("{diagram_id}-")));
        assert!(root.descendants().any(|node| {
            node.has_tag_name("marker")
                && node.attribute("id") == Some(marker_id)
                && node
                    .parent()
                    .is_some_and(|parent| parent.has_tag_name("defs"))
        }));
    }
}

#[test]
fn compact_sequence_rejects_instead_of_falling_back_to_parity() {
    let (source, reason_fragment) = if cfg!(feature = "native-compact") {
        (
            "---\nconfig:\n  look: classic\n  theme: default\n---\nsequenceDiagram\n    Client->>Client: Self message\n",
            Some("self-message"),
        )
    } else {
        (SEQUENCE, None)
    };
    let error = standalone(source, compact_request())
        .map(|_| ())
        .expect_err("compact must not return parity SVG");
    let reason = compact_unsupported(error);
    if let Some(fragment) = reason_fragment {
        assert!(reason.contains(fragment), "{reason}");
    }
}

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    not(feature = "layout-dagre")
))]
#[test]
fn compact_rejects_an_unsupported_family_before_checking_its_layout_backend() {
    let source = "---\nconfig:\n  layout: dagre\n---\nflowchart LR\n    A --> B";
    let parity_error = standalone(source, SvgRequest::default())
        .map(|_| ())
        .unwrap_err();
    assert!(matches!(
        parity_error,
        RenderError::Svg(merman_render::Error::MissingCapability {
            capability: merman_render::RenderCapability::LayoutDagre,
            ..
        })
    ));
    let error = standalone(source, compact_request())
        .map(|_| ())
        .unwrap_err();
    assert!(compact_unsupported(error).contains("diagram family"));
}

#[test]
fn compact_sequence_rejects_non_standalone_or_custom_pipeline() {
    let error = Renderer::new()
        .render(RenderRequest::document(
            SEQUENCE,
            OperationControl::new(),
            compact_request(),
        ))
        .map(|_| ())
        .expect_err("compact is not a graphical document export target");
    let reason = compact_unsupported(error);
    assert!(reason.contains("standalone"), "{reason}");

    #[cfg(feature = "native-compact")]
    {
        use merman::svg::{StripForeignObjectPostprocessor, SvgPipeline};

        let mut request = compact_request();
        request.pipeline =
            Some(SvgPipeline::parity().with_postprocessor(StripForeignObjectPostprocessor));
        let error = standalone(SEQUENCE, request)
            .map(|_| ())
            .expect_err("compact does not admit a custom SVG postprocessor");
        compact_unsupported(error);
    }
}

#[cfg(feature = "native-compact")]
#[test]
fn compact_sequence_admits_exact_svg_bytes_but_rejects_one_less() {
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let expected = standalone(SEQUENCE, compact_request()).unwrap();
    let bytes = expected.svg().len();
    let with_limit = |limit| {
        let mut request = compact_request();
        request.environment = request.environment.with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, limit)
                .unwrap(),
        );
        standalone(SEQUENCE, request)
    };

    assert_eq!(with_limit(bytes).unwrap().svg(), expected.svg());
    let error = with_limit(bytes - 1)
        .map(|_| ())
        .expect_err("the actual byte budget is one byte short");
    let RenderError::ResourceLimitExceeded(limit) = error else {
        panic!("expected the SVG byte limit, got {error:?}");
    };
    assert_eq!(limit.id, "max_svg_bytes");
    assert_eq!(limit.maximum, (bytes - 1) as u64);
    assert!(limit.actual > limit.maximum);
}
