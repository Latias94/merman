#![cfg(feature = "svg")]

use merman::resources::InputResourcePolicy;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn render(source: &str) -> String {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("ZenUML render must succeed");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("ZenUML must produce SVG");
    };
    svg.into_parts().0
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
fn advanced_zenuml_is_a_first_class_typed_render() {
    let renderer = Renderer::new();
    let semantic = renderer
        .prepare_semantic(ADVANCED, OperationControl::new())
        .expect("ZenUML preparation must succeed")
        .expect("ZenUML must be detected");
    assert_eq!(semantic.semantic_kind(), "zenuml");
    let output = semantic
        .render(merman::RenderTarget::Svg(SvgRequest::default()))
        .expect("ZenUML SVG must render");
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("ZenUML must produce SVG");
    };
    let svg = svg.svg();
    assert!(svg.contains("participant-group"), "{svg}");
    assert!(svg.contains("fragment-par"), "{svg}");
    assert!(svg.contains("fragment-separator"), "{svg}");
    assert!(svg.contains("OrderController"), "{svg}");
    assert!(svg.contains("#FFEBE6"), "{svg}");
    assert!(!svg.contains("foreignObject"), "{svg}");
}

#[test]
fn invalid_zenuml_does_not_poison_the_next_render() {
    let renderer = Renderer::new();
    let invalid = renderer.render(RenderRequest::svg(
        "zenuml\n@Starter(A)\nif( {\n",
        OperationControl::new(),
        SvgRequest::default(),
    ));
    assert!(
        invalid.is_err(),
        "invalid ZenUML must return a structured error"
    );
    let valid = renderer
        .render(RenderRequest::svg(
            "zenuml\nA->B: hello\n",
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("next render must not inherit parser state");
    let RenderOutput::Svg(Some(valid)) = valid else {
        panic!("valid ZenUML must render");
    };
    assert!(valid.svg().contains("hello"), "{}", valid.svg());
}

#[test]
fn zenuml_labels_are_xml_escaped_at_the_family_boundary() {
    let source = "zenuml\nA->B: <script>alert(1)</script>\n";
    let svg = render(source);
    assert!(!svg.contains("<script>"), "{svg}");
    assert!(svg.contains("&lt;script>"), "{svg}");
}

#[test]
fn zenuml_honors_the_shared_label_resource_budget_before_layout() {
    let renderer = Renderer::new();
    let resources = InputResourcePolicy::for_profile(
        merman::resources::ResourceProfile::UnboundedForTrustedInput,
    )
    .with_limit(
        merman::resources::InputResourceLimitId::MaxModelTextBytes,
        4,
    )
    .unwrap();
    let error = renderer
        .render(
            RenderRequest::svg(
                "zenuml\nA->B: a label beyond the budget\n",
                OperationControl::new(),
                SvgRequest::default(),
            )
            .with_resource_policy(resources),
        )
        .expect_err("ZenUML must honor the shared label budget");
    assert!(
        error.to_string().contains("max_model_text_bytes"),
        "{error}"
    );
}

#[test]
fn zenuml_honors_its_structural_resource_budget_before_layout() {
    let renderer = Renderer::new();
    let resources = InputResourcePolicy::for_profile(
        merman::resources::ResourceProfile::UnboundedForTrustedInput,
    )
    .with_limit(merman::resources::InputResourceLimitId::MaxModelItems, 1)
    .unwrap();
    let error = renderer
        .render(
            RenderRequest::svg(
                "zenuml\nA.call()\nB.call()\n",
                OperationControl::new(),
                SvgRequest::default(),
            )
            .with_resource_policy(resources),
        )
        .expect_err("ZenUML must honor its family-owned statement budget");
    assert!(error.to_string().contains("max_model_items"), "{error}");
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
