#[cfg(feature = "svg")]
#[test]
fn c4_renderer_uses_typed_render_path() {
    let input = r#"
C4Context
title Typed C4
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"#;

    let output = merman::Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(merman::RenderRequest::svg(
            input,
            merman::OperationControl::new(),
            merman::SvgRequest {
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("typed_c4".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("render svg");
    let merman::RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram not detected");
    };

    assert!(svg.svg().contains("typed_c4"));
    assert!(svg.svg().contains("c4"));
}

#[cfg(feature = "diagram-c4")]
#[test]
fn c4_semantic_facade_preserves_nested_and_sibling_contexts() {
    let source = r#"C4Context
Boundary(outer, "Outer") {
  System(before, "Before")
  Boundary(inner, "Inner") {
    System(inside, "Inside")
  }
  System(after, "After")
}
Boundary(sibling, "Sibling") {
  System(other, "Other")
}
System(root, "Root")
"#;
    let artifact = merman::Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .prepare_semantic(source, merman::OperationControl::new())
        .expect("prepare typed C4 semantics")
        .expect("C4 diagram detected");
    assert_eq!(artifact.semantic_kind(), "c4");
    let model = artifact
        .compatibility_json()
        .expect("project C4 compatibility JSON");

    assert_eq!(model["boundaries"][2]["parentBoundary"], "outer");
    assert_eq!(model["boundaries"][3]["parentBoundary"], "global");
    for (index, parent) in ["outer", "inner", "outer", "sibling", "global"]
        .iter()
        .enumerate()
    {
        assert_eq!(model["shapes"][index]["parentBoundary"], *parent);
    }
}

#[cfg(feature = "diagram-c4")]
#[test]
fn c4_semantic_facade_rejects_partial_boundary_then_remains_usable() {
    let renderer = merman::Renderer::new().with_parse_options(merman::ParseOptions::strict());
    let source = r#"C4Context
Boundary(open, "Open") {
  Boundary(completed, "Completed") {
    System(inside, "Inside")
  }
"#;
    let error = renderer
        .prepare_semantic(source, merman::OperationControl::new())
        .expect_err("completed inner boundary cannot commit an unclosed outer boundary");
    assert!(error.to_string().contains("expected '}' before end"));

    let artifact = renderer
        .prepare_semantic(
            "C4Context\nSystem(after, \"After\")\n",
            merman::OperationControl::new(),
        )
        .expect("renderer remains usable after malformed boundary")
        .expect("C4 diagram detected");
    let model = artifact
        .compatibility_json()
        .expect("project subsequent C4 model");
    assert_eq!(model["boundaries"].as_array().unwrap().len(), 1);
    assert_eq!(model["shapes"][0]["parentBoundary"], "global");
}
