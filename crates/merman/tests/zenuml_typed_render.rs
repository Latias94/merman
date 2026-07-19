#![cfg(feature = "render")]

use merman::render::{HeadlessRenderer, RenderResourceLimits};

fn render(source: &str) -> String {
    HeadlessRenderer::new()
        .render_svg_sync(source)
        .expect("ZenUML render must succeed")
        .expect("ZenUML must produce SVG")
}

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
    assert!(svg.contains("fragment-separator"), "{svg}");
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

#[test]
fn zenuml_svg_uses_statement_specific_components_without_bottom_participants() {
    let svg = render("zenuml\n@Starter(A)\nA->B: async\nA.self()\nnew C\nA-->B: returned\n");

    assert!(svg.contains(r#"<g class="message""#), "{svg}");
    assert!(svg.contains(r#"<g class="message self-call""#), "{svg}");
    assert!(svg.contains(r#"<g class="creation""#), "{svg}");
    assert!(svg.contains(r#"<g class="return""#), "{svg}");
    assert!(!svg.contains("participant-bottom"), "{svg}");
}

#[test]
fn zenuml_self_return_uses_the_companion_circular_return_component() {
    let svg = render("zenuml\n@Starter(A)\nA-->\n");

    assert!(svg.contains(r#"class="return return-self""#), "{svg}");
    assert!(svg.contains(r#"class="return-icon""#), "{svg}");
    assert!(!svg.contains(r#"class="return-line""#), "{svg}");
}

#[test]
fn zenuml_participant_types_and_emoji_render_as_assets_not_source_prefixes() {
    let svg = render(
        "zenuml\n@Actor Client\n@Boundary Boundary\n@EC2 Service\n@Lambda Worker\n@AzureFunction Function\nClient->[rocket]Boundary.call()\n",
    );

    for icon in ["actor", "boundary", "ec2", "lambda", "azurefunction"] {
        assert!(
            svg.contains(&format!(r#"data-icon="{icon}""#)),
            "missing {icon}: {svg}"
        );
    }
    assert!(svg.contains("\u{1f680}"), "{svg}");
    assert!(!svg.contains("@EC2"), "{svg}");
    assert!(!svg.contains("[rocket]"), "{svg}");
}

#[test]
fn zenuml_comments_render_markdown_and_channel_specific_styles() {
    let svg = render("zenuml\n// <red> (bold) [italic, rocket] **important** `code`\nA.call()\n");

    assert!(
        svg.contains(r#"class="comment-text" data-statement="zenuml-statement-0" style="fill:red;font-style:italic""#),
        "{svg}"
    );
    assert!(svg.contains(r#"font-weight="bold""#), "{svg}");
    assert!(svg.contains("important"), "{svg}");
    assert!(svg.contains("code"), "{svg}");
    assert!(!svg.contains("`code`"), "{svg}");
    assert!(svg.contains(r#"class="message-label""#), "{svg}");
    assert!(
        svg.contains(r#"style="font-style:italic;font-weight:bold""#),
        "{svg}"
    );
}

#[test]
fn zenuml_fragments_render_fixed_headers_conditions_and_branch_labels() {
    let svg = render(
        "zenuml\nif(primary) {\n  A->B: first\n} else if(secondary) {\n  A->B: second\n} else {\n  A->B: third\n}\n",
    );

    assert!(svg.contains(r#"class="fragment fragment-alt""#), "{svg}");
    assert!(
        svg.contains(r#"class="fragment-label">Alt</text>"#),
        "{svg}"
    );
    assert!(svg.contains("primary"), "{svg}");
    assert!(svg.contains("secondary"), "{svg}");
    assert!(svg.contains("[else]"), "{svg}");
}
