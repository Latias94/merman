use merman::{OperationControl, Renderer, analysis::Analyzer};

fn main() {
    let source = "flowchart LR\nA[Start] --> B[Done]\n";
    let semantic = Renderer::new()
        .parse_semantic(source, OperationControl::new())
        .unwrap();
    assert!(semantic.is_some());
    assert!(Analyzer::new().analyze(source).valid);
}
