use super::*;

#[test]
fn metadata_reads_only_the_root_svg_attributes() {
    let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" style="content: 'max-width: 9000px'; max-width: 400px"><g viewBox="0 0 9000 9000"/><text>viewBox=&quot;0 0 8000 8000&quot;</text></svg>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

    assert!(!metadata.has_view_box);
    assert_eq!(metadata.max_width_px, Some(400.0));
}

#[test]
fn metadata_uses_svg_number_and_css_token_grammar() {
    let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1.5-2,41.5,120 trailing" style="max-width: 400px nonsense"/>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

    assert!(metadata.has_view_box);
    assert_eq!(metadata.max_width_px, None);
}

#[test]
fn metadata_ignores_unknown_namespaces_and_uses_first_usvg_projection() {
    let metadata = parse_root_svg_metadata(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="urn:ignored" xmlns:s="http://www.w3.org/2000/svg" i:viewBox="0 0 9000 9000" s:viewBox="invalid" viewBox="0 0 20 10" i:style="max-width: 9000px" s:style="max-width: 200px" style="max-width: 400px"/>"#,
            &OperationControl::new(),
        )
        .expect("root SVG metadata");

    assert!(
        !metadata.has_view_box,
        "the first usvg-projected viewBox is invalid, so a later alias must not replace it"
    );
    assert_eq!(
        metadata.max_width_px,
        Some(400.0),
        "usvg consumes only the unbound style declaration list"
    );
}
