use merman_core::{Engine, ParseOptions};
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

#[test]
fn eventmodeling_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  eventmodeling:
    padding: 24
    useMaxWidth: true
  themeVariables:
    emRelationStroke: '#135790'
    emCommandFill: '#DDEEFF'
    emCommandStroke: '#336699'
    textColor: '#111111'
---
eventmodeling
tf 01 ui Web.ShopCart
tf 02 cmd Cart.AddItem ->> 01 { sku: "SKU-1" }
tf 03 evt Cart.ItemAdded ->> 02 [[ItemAddedData]]
rf 04 rmo Cart.Summary
tf 05 evt Cart.CheckedOut

data ItemAddedData {
  sku: "SKU-1"
  quantity: 1
}
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "eventmodeling");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("eventmodeling-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(svg.contains(r#"aria-roledescription="eventmodeling""#));
    assert!(svg.contains(r#"<g class="eventmodeling">"#));
    assert!(svg.contains(r#"class="eventModeling-swimlane""#));
    assert!(svg.contains(r#"class="eventModeling-relation""#));
    assert!(svg.contains(r#"class="eventModeling-box""#));
    assert!(svg.contains(r#"eventModeling-reset-box"#));
    assert!(svg.contains(r#"id="eventmodeling-arrow-eventmodeling-test""#));
    assert!(svg.contains(r##"stroke="#135790""##));
    assert!(svg.contains(r##"fill="#DDEEFF""##));
}
