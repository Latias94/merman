use merman::svg::{HeadlessRenderer, RenderExecutionPath};

#[test]
fn rendered_document_keeps_svg_resources_and_operation_evidence_correlated() {
    let document = HeadlessRenderer::new()
        .render_document_sync("info")
        .unwrap()
        .expect("info diagram");

    assert!(document.svg_text().starts_with("<svg"));
    assert!(document.resource_closure().is_closed());
    assert_eq!(
        document.report().execution_path(),
        RenderExecutionPath::HeadlessOperationTyped
    );
    assert_eq!(
        document.font_catalog().fingerprint(),
        document.report().font_catalog_fingerprint()
    );
    assert_eq!(
        document.font_source_policy(),
        document.report().font_source_policy()
    );
    assert_eq!(
        document.resource_fingerprint(),
        document.svg().resource_fingerprint()
    );
}
