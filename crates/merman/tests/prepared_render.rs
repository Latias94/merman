use merman::render::{
    LayoutDiagram, LayoutOptions, PreparedRender, SvgRenderOptions, prepare_render_sync,
    prepare_semantic_sync, render_svg_sync,
};
use merman::{ParseMetadata, ParseOptions, RenderSemanticModel};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static INFO_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static INFO_PARSER: OnceLock<merman::diagram::RenderSemanticParser> = OnceLock::new();

fn counting_info_parser(
    code: &str,
    meta: &ParseMetadata,
) -> merman_core::Result<RenderSemanticModel> {
    INFO_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    INFO_PARSER
        .get()
        .expect("the original Info parser should be installed")(code, meta)
}

fn assert_info_artifact(prepared: &PreparedRender) {
    assert_eq!(prepared.metadata().diagram_type, "info");
    assert_eq!(prepared.semantic_kind(), "info");
    assert!(matches!(
        prepared.layout(),
        LayoutDiagram::InfoDiagram(layout) if !layout.version.is_empty()
    ));
}

#[test]
fn staged_render_exposes_one_typed_parse_and_layout_before_svg() {
    let mut engine = merman::Engine::new();
    let original_parser = engine
        .render_diagram_registry()
        .get("info")
        .expect("Info should have a typed render parser");
    if let Some(installed) = INFO_PARSER.get() {
        assert_eq!(*installed as usize, original_parser as usize);
    } else {
        INFO_PARSER
            .set(original_parser)
            .expect("the original Info parser should only be installed once");
    }
    engine
        .render_diagram_registry_mut()
        .insert("info", counting_info_parser);
    INFO_PARSE_CALLS.store(0, Ordering::SeqCst);

    let layout_options = LayoutOptions::headless_svg_defaults();
    let semantic = prepare_semantic_sync(&engine, "info", ParseOptions::strict(), &layout_options)
        .unwrap()
        .expect("Info should prepare typed semantics");

    assert_eq!(semantic.metadata().diagram_type, "info");
    assert_eq!(semantic.semantic_kind(), "info");
    assert_eq!(INFO_PARSE_CALLS.load(Ordering::SeqCst), 1);

    let prepared = semantic
        .continue_layout()
        .expect("Info semantics should produce a typed layout");

    assert_info_artifact(&prepared);
    assert_eq!(INFO_PARSE_CALLS.load(Ordering::SeqCst), 1);

    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("prepared-info".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert!(svg.contains(r#"id="prepared-info""#), "{svg}");
    assert_eq!(INFO_PARSE_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn flowchart_elk_admission_can_skip_after_parse_and_before_layout() {
    let source = r#"---
config:
  layout: elk
---
flowchart TD
A --> B
"#;
    let engine = merman::Engine::new();
    let mut layout_options = LayoutOptions::headless_svg_defaults();
    layout_options.resource_limits.max_flowchart_nodes = Some(0);
    let semantic = prepare_semantic_sync(&engine, source, ParseOptions::strict(), &layout_options)
        .unwrap()
        .expect("Flowchart should prepare typed semantics before ELK layout admission");

    assert_eq!(semantic.metadata().diagram_type, "flowchart-v2");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("elk")
    );
    assert_eq!(semantic.semantic_kind(), "flowchart");

    // Admission can drop the semantic stage here without starting layout.
    drop(semantic);

    let semantic = prepare_semantic_sync(&engine, source, ParseOptions::strict(), &layout_options)
        .unwrap()
        .expect("the same source should prepare typed semantics again");
    let error = match semantic.continue_layout() {
        Ok(_) => panic!("layout should enforce the configured model limit"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains("max_flowchart_nodes") || message.contains("flowchart-v2"),
        "{message}"
    );
}

#[test]
fn high_level_render_matches_the_prepared_artifact_path() {
    let engine = merman::Engine::new();
    let source = "flowchart TD\nA[Prepared] --> B[Rendered]";
    let parse_options = ParseOptions::strict();
    let layout_options = LayoutOptions::headless_svg_defaults();
    let svg_options = SvgRenderOptions {
        diagram_id: Some("prepared-flowchart".to_string()),
        ..Default::default()
    };

    let prepared = prepare_render_sync(&engine, source, parse_options, &layout_options)
        .unwrap()
        .expect("Flowchart should prepare a render artifact");
    assert_eq!(prepared.metadata().diagram_type, "flowchart-v2");
    assert_eq!(prepared.semantic_kind(), "flowchart");
    assert!(matches!(prepared.layout(), LayoutDiagram::FlowchartV2(_)));

    let prepared_svg = prepared.render_svg(&svg_options).unwrap();
    let high_level_svg = render_svg_sync(
        &engine,
        source,
        parse_options,
        &layout_options,
        &svg_options,
    )
    .unwrap()
    .expect("Flowchart should render through the high-level helper");

    assert_eq!(prepared_svg, high_level_svg);
}
