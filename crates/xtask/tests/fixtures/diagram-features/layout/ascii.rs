use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

fn main() {
    let source = "flowchart LR\nA[Start] --> B[Done]\n";
    let output = Renderer::new()
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            Default::default(),
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(output)) = output else {
        panic!("expected ASCII output")
    };
    assert!(output.text.contains("Start") && output.text.contains("Done"));
}
