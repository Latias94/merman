#![cfg(all(feature = "svg", feature = "diagram-sequence"))]

use merman::{
    OperationControl, PrepareSourceRequest, RenderOutput, RenderRequest, RenderTarget, Renderer,
    SvgRequest,
};

#[test]
fn direct_and_prepared_sequence_layouts_export_owned_native_drawing_geometry() {
    let source = "---\ntitle: Checkout\n---\n\
sequenceDiagram
    box Team
    participant A
    participant B
    end
    loop retry
        A->>+B: request
        alt accepted
            B-->>A: success
        else rejected
            B-->>-A: failure
        end
    end
";
    let renderer = Renderer::new();
    let prepared = renderer
        .prepare_source(PrepareSourceRequest::new(source, OperationControl::new()))
        .expect("sequence source should prepare")
        .expect("sequence diagram should be detected");
    let RenderOutput::LayoutJson(Some(from_prepared)) = renderer
        .render_prepared(
            &prepared,
            RenderTarget::LayoutJson(SvgRequest::default()),
            OperationControl::new(),
        )
        .expect("prepared sequence layout should export")
    else {
        panic!("expected prepared layout JSON");
    };
    drop(prepared);

    let RenderOutput::LayoutJson(Some(direct)) = renderer
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("direct sequence layout should export")
    else {
        panic!("expected direct layout JSON");
    };
    assert_eq!(direct.layout()["layout"], from_prepared.layout()["layout"]);

    let sequence = &from_prepared.layout()["layout"]["SequenceDiagram"];
    assert_eq!(
        sequence["frames"].as_array().expect("control frames").len(),
        2
    );
    assert_eq!(
        sequence["activations"]
            .as_array()
            .expect("activation bars")
            .len(),
        1
    );
    assert_eq!(
        sequence["boxes"]
            .as_array()
            .expect("participant boxes")
            .len(),
        1
    );
    assert_eq!(sequence["title"]["text"], "Checkout");
}
