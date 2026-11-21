#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman::svg::{SvgGeneration, SvgPipeline, SvgPostprocessContext, SvgPostprocessor};
use merman::{
    MermaidEngine, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgEnvironment, SvgRequest,
};
use std::borrow::Cow;

fn renderer() -> Renderer {
    Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        SourcePresentationConfig::from_value(serde_json::json!({
            "theme": "redux-color",
            "look": "classic",
            "securityLevel": "loose",
            "themeVariables": {
                "borderColorArray": ["#112233", "#223344", "#334455", "#445566"],
                "bkgColorArray": ["#ddeeff", "#ccddee"]
            }
        })),
    ))
}

fn static_pipeline() -> SvgPipeline {
    SvgPipeline::parity().with_generation(SvgGeneration::Static)
}

fn render_svg(
    renderer: &Renderer,
    source: &str,
    request: SvgRequest,
) -> Result<String, RenderError> {
    let result = renderer.render(RenderRequest::svg(source, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(output)) = result else {
        panic!("expected SVG");
    };
    Ok(output.svg().to_owned())
}

fn stylesheet(svg: &str) -> (&str, &str, &str) {
    let (root, rest) = svg.split_once("<style>").expect("generated stylesheet");
    let (css, body) = rest.split_once("</style>").expect("complete stylesheet");
    (root, css, body)
}

fn assert_projection(full: &str, optimized: &str, expected: &[(&str, usize)]) {
    let (full_root, full_css, full_body) = stylesheet(full);
    let (static_root, static_css, static_body) = stylesheet(optimized);
    assert_eq!(
        static_root, full_root,
        "root attributes and accessible text"
    );
    assert_eq!(static_body, full_body, "serialized scene");

    let document = roxmltree::Document::parse(optimized).expect("parse static SVG");
    let actual: std::collections::BTreeMap<_, _> = document
        .descendants()
        .filter_map(|node| Some((node.attribute("data-id")?, node.attribute("data-color-id")?)))
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (id, slot) in expected {
        assert_eq!(actual[id], format!("color-{slot}"), "{id}");
    }

    let mut omitted = Vec::new();
    for slot in 0..4 {
        let selector = format!(r#"[data-color-id="color-{slot}"]"#);
        assert!(full_css.contains(&selector), "complete slot {slot}");
        if expected.iter().any(|(_, used)| *used == slot) {
            assert!(static_css.contains(&selector), "used slot {slot}");
        } else {
            assert!(!static_css.contains(&selector), "unused slot {slot}");
            omitted.push(selector);
        }
    }
    let expected_css: String = full_css
        .split_inclusive('}')
        .filter(|rule| !omitted.iter().any(|selector| rule.contains(selector)))
        .collect();
    assert_eq!(
        static_css, expected_css,
        "stylesheet order and declarations"
    );
}

fn render_projection(source: &str, expected: &[(&str, usize)]) -> String {
    let renderer = renderer();
    let full = render_svg(&renderer, source, SvgRequest::default()).unwrap();
    let optimized = render_svg(
        &renderer,
        source,
        SvgRequest {
            pipeline: Some(static_pipeline()),
            ..SvgRequest::default()
        },
    )
    .unwrap();
    assert_projection(&full, &optimized, expected);
    optimized
}

#[test]
fn static_flowchart_without_containers_omits_the_palette() {
    render_projection("flowchart LR\nA --> B\n", &[]);
}

#[test]
fn static_flowchart_preserves_custom_palette_paint_and_document_content() {
    let svg = render_projection(
        r#"flowchart LR
accTitle: Build and ship
accDescr: Build then publish
subgraph Group
  A[Build]:::own
end
A --> B[Publish]
classDef own fill:#abcdef,stroke:#fedcba
click A href "https://example.org/guide" "Guide"
"#,
        &[("Group", 0)],
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let root = document.root_element();
    for (attribute, tag, text) in [
        ("aria-labelledby", "title", "Build and ship"),
        ("aria-describedby", "desc", "Build then publish"),
    ] {
        let target = root
            .attribute(attribute)
            .expect("accessible name reference");
        let node = root
            .children()
            .find(|node| node.has_tag_name(tag) && node.attribute("id") == Some(target))
            .unwrap();
        assert_eq!(node.text(), Some(text));
    }
    assert!(root.attribute("viewBox").is_some());
    assert!(document.descendants().any(|node| {
        node.has_tag_name("a")
            && node.attributes().any(|attribute| {
                attribute.name() == "href" && attribute.value() == "https://example.org/guide"
            })
    }));
    let (_, css, _) = stylesheet(&svg);
    assert!(css.contains(
        r#"[data-color-id="color-0"].cluster:not(.swimlane) rect{stroke:#112233;fill:#ddeeff;}"#
    ));
    assert!(css.contains(
        ".own>*{fill:rgb(171, 205, 239)!important;stroke:rgb(254, 220, 186)!important;}"
    ));
}

#[test]
fn static_flowchart_uses_the_collapsed_wrapper_slot_and_omits_hidden_groups() {
    let svg = render_projection(
        "flowchart TB\nsubgraph Outer\nsubgraph Child\nA --> B\nend\nend\nC --> A\nOuter@{view: collapsed}\n",
        &[("Outer", 0)],
    );
    let (_, css, body) = stylesheet(&svg);
    assert!(body.contains("collapsed-group"));
    assert!(!body.contains(r#"data-id="Child""#));
    assert!(css.contains(r#"[data-color-id="color-0"].node .collapsed-group"#));
}

#[test]
fn static_flowchart_uses_the_slots_of_both_nested_groups() {
    render_projection(
        "flowchart TB\nsubgraph Outer\nsubgraph Inner\nA --> B\nend\nend\n",
        &[("Outer", 0), ("Inner", 1)],
    );
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn static_swimlane_keeps_nested_group_and_synthetic_lane_slots() {
    let svg = render_projection(
        include_str!("../../../fixtures/swimlane/local_nested_mixed_lane_security.mmd"),
        &[("Outer", 0), ("Inner", 1), ("__swimlane_default__", 2)],
    );
    assert!(stylesheet(&svg).1.contains(
        r#"[data-color-id="color-2"].swimlane.cluster rect.swimlane-body{stroke:#334455;fill:#ddeeff;}"#,
    ));
}

#[test]
fn static_generation_keeps_the_full_draft_for_opaque_postprocessors() {
    struct ReassignSlot;

    impl SvgPostprocessor for ReassignSlot {
        fn name(&self) -> &'static str {
            "test-reassign-palette-slot"
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            _: &SvgPostprocessContext<'_>,
        ) -> merman::svg::RenderResult<Cow<'a, str>> {
            let (root, css, body) = stylesheet(&svg);
            assert!(css.contains(r#"[data-color-id="color-3"]"#));
            let body = body.replace(r#"data-color-id="color-0""#, r#"data-color-id="color-3""#);
            Ok(Cow::Owned(format!("{root}<style>{css}</style>{body}")))
        }
    }

    let svg = render_svg(
        &renderer(),
        "flowchart LR\nsubgraph Group\nA --> B\nend\n",
        SvgRequest {
            pipeline: Some(static_pipeline().with_postprocessor(ReassignSlot)),
            ..SvgRequest::default()
        },
    )
    .unwrap();
    assert!(stylesheet(&svg).2.contains(r#"data-color-id="color-3""#));
}

#[test]
fn static_generation_keeps_registry_and_browser_inline_styles_complete() {
    let renderer = renderer();
    let registry = merman::svg::IconRegistry::from_packs([merman::svg::IconPack::new(
        br#"{"prefix":"test","icons":{"square":{"body":"<path d=\"M0 0H16V16H0z\"/>"}}}"#,
    )])
    .unwrap();
    let source = "flowchart LR\nA@{icon: \"test:square\", label: \"A\"}\nA --> B\n";
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic().with_icon_registry(registry),
        ..SvgRequest::default()
    };
    let full = render_svg(&renderer, source, request.clone()).unwrap();
    let optimized = render_svg(
        &renderer,
        source,
        SvgRequest {
            pipeline: Some(static_pipeline()),
            ..request
        },
    )
    .unwrap();
    assert!(full.contains("M0 0H16V16H0z"));
    assert_eq!(optimized, full);

    let browser_svg = |pipeline: SvgPipeline| {
        render_svg(
            &renderer,
            "flowchart LR\nA --> B\n",
            SvgRequest {
                pipeline: Some(pipeline.with_browser_inline_contract("browser")),
                ..SvgRequest::default()
            },
        )
        .unwrap()
    };
    let browser = browser_svg(static_pipeline());
    assert_eq!(browser, browser_svg(SvgPipeline::parity()));
    let document = roxmltree::Document::parse(&browser).unwrap();
    let style = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    assert!(style.contains(r#"[data-color-id="color-3"]"#));
}

#[test]
fn static_svg_admits_the_requested_serialized_bytes() {
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let renderer = renderer();
    let source = "flowchart LR\nsubgraph Group\nA --> B\nend\n";
    let request = SvgRequest {
        pipeline: Some(static_pipeline()),
        ..SvgRequest::default()
    };
    let len = render_svg(&renderer, source, request.clone())
        .unwrap()
        .len();
    let environment = |maximum| {
        SvgEnvironment::deterministic().with_resource_policy(
            RenderResourcePolicy::interactive()
                .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap(),
        )
    };
    let exact = SvgRequest {
        environment: environment(len),
        ..request.clone()
    };
    assert_eq!(
        render_svg(&renderer, source, exact.clone()).unwrap().len(),
        len
    );
    for request in [
        SvgRequest {
            pipeline: None,
            ..exact
        },
        SvgRequest {
            environment: environment(len - 1),
            ..request
        },
    ] {
        let error = render_svg(&renderer, source, request).unwrap_err();
        assert!(
            matches!(error, RenderError::ResourceLimitExceeded(limit) if limit.id == "max_svg_bytes")
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn encoded_rust_targets_default_to_static_generation() {
    use merman::RenderTarget;
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let renderer = renderer();
    let source = "flowchart LR\nA --> B\n";
    let full_len = render_svg(&renderer, source, SvgRequest::default())
        .unwrap()
        .len();
    let environment = SvgEnvironment::deterministic().with_resource_policy(
        RenderResourcePolicy::interactive()
            .with_limit(ResourceLimitId::MaxSvgBytes, full_len - 1)
            .unwrap(),
    );
    let targets: &[fn(SvgRequest) -> RenderTarget] = &[
        #[cfg(feature = "png")]
        |svg| {
            RenderTarget::Png(merman::PngRequest {
                svg,
                options: Default::default(),
            })
        },
        #[cfg(feature = "jpeg")]
        |svg| {
            RenderTarget::Jpeg(merman::JpegRequest {
                svg,
                options: Default::default(),
            })
        },
        #[cfg(feature = "pdf")]
        |svg| {
            RenderTarget::Pdf(merman::PdfRequest {
                svg,
                options: Default::default(),
            })
        },
    ];
    for target in targets {
        let render = |pipeline| {
            renderer.render(RenderRequest::new(
                source,
                target(SvgRequest {
                    environment: environment.clone(),
                    pipeline,
                    ..SvgRequest::default()
                }),
                OperationControl::new(),
            ))
        };
        render(None).expect("encoded target should fit with static generation");
        let error = render(Some(
            SvgPipeline::resvg_safe().with_generation(SvgGeneration::Full),
        ))
        .unwrap_err();
        assert!(
            matches!(error, RenderError::ResourceLimitExceeded(limit) if limit.id == "max_svg_bytes")
        );
    }
}
