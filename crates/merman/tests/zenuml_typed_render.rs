#![cfg(feature = "render")]

use merman::render::{HeadlessRenderer, RenderResourceLimits};

const ADVANCED: &str = r#"zenuml
title Order Service
@Actor Client #FFEBE6
@Boundary OrderController #0747A6
@EC2 <<BFF>> OrderService #E3FCEF
group BusinessService {
  @Lambda PurchaseService
  @AzureFunction InvoiceService
}
@Starter(Client)
// `POST /orders`
OrderController.post(payload) {
  OrderService.create(payload) {
    order = new Order(payload)
    if(order != null) {
      par {
        PurchaseService.createPO(order)
        InvoiceService.createInvoice(order)
      }
    }
  }
}
"#;

#[test]
fn advanced_zenuml_is_a_first_class_typed_headless_render() {
    let renderer = HeadlessRenderer::new();
    let prepared = renderer
        .prepare_render_sync(ADVANCED)
        .expect("ZenUML preparation must succeed")
        .expect("ZenUML must be detected");
    assert_eq!(prepared.family_kind().as_str(), "zenuml");
    let svg = prepared
        .render_svg(&Default::default())
        .expect("ZenUML SVG must render");
    assert!(svg.contains("participant-group"), "{svg}");
    assert!(svg.contains("fragment-par"), "{svg}");
    assert!(svg.contains("OrderController"), "{svg}");
    assert!(svg.contains("#FFEBE6"), "{svg}");
    assert!(!svg.contains("foreignObject"), "{svg}");
}

#[test]
fn invalid_zenuml_does_not_poison_the_next_render() {
    let renderer = HeadlessRenderer::new();
    let invalid = renderer.render_svg_sync("zenuml\n@Starter(A)\nif( {\n");
    assert!(
        invalid.is_err(),
        "invalid ZenUML must return a structured error"
    );
    let valid = renderer
        .render_svg_sync("zenuml\nA->B: hello\n")
        .expect("next render must not inherit parser state")
        .expect("valid ZenUML must render");
    assert!(valid.contains("hello"), "{valid}");
}

#[test]
fn zenuml_labels_are_xml_escaped_at_the_family_boundary() {
    let source = "zenuml\nA->B: <script>alert(1)</script>\n";
    let svg = HeadlessRenderer::new()
        .render_svg_sync(source)
        .expect("source should render")
        .expect("ZenUML SVG should exist");
    assert!(!svg.contains("<script>"), "{svg}");
    assert!(svg.contains("&lt;script>"), "{svg}");
}

#[test]
fn zenuml_honors_the_shared_label_resource_budget_before_layout() {
    let renderer = HeadlessRenderer::new().with_resource_limits(RenderResourceLimits {
        max_label_bytes: Some(4),
        ..RenderResourceLimits::unbounded_for_trusted_input()
    });
    let error = renderer
        .render_svg_sync("zenuml\nA->B: a label beyond the budget\n")
        .expect_err("ZenUML must honor the shared label budget");
    assert!(error.to_string().contains("max_label_bytes"), "{error}");
}

#[test]
fn zenuml_honors_its_structural_resource_budget_before_layout() {
    let renderer = HeadlessRenderer::new().with_resource_limits(RenderResourceLimits {
        max_zenuml_statements: Some(1),
        ..RenderResourceLimits::unbounded_for_trusted_input()
    });
    let error = renderer
        .render_svg_sync("zenuml\nA.call()\nB.call()\n")
        .expect_err("ZenUML must honor its family-owned statement budget");
    assert!(
        error.to_string().contains("max_zenuml_statements"),
        "{error}"
    );
}
