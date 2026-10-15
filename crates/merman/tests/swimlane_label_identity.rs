#![cfg(all(feature = "svg", feature = "diagram-swimlane"))]

use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

#[test]
fn swimlane_synthetic_edge_labels_preserve_colliding_user_nodes() {
    let source = "swimlane-beta LR\nedge-label-occurrence-00000000000000000000[Keep me]\nedge-label-occurrence-00000000000000000000-1[Keep suffix]\nX L_A_B_0@--x|first owner| Y\nA -->|second owner| B\n";
    let output = Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("valid user identifiers must not be replaced by synthetic edge-label nodes");
    let RenderOutput::Svg(Some(rendered)) = output else {
        panic!("expected a Swimlane SVG");
    };
    let svg = rendered.svg();
    for label in ["Keep me", "Keep suffix", "first owner", "second owner"] {
        assert!(svg.contains(label), "missing independent terminal {label}");
    }
    assert_eq!(svg.matches("first owner").count(), 1);
    assert_eq!(svg.matches("second owner").count(), 1);
}
