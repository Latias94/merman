use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn independent_treemap_presentation(
    values: &[f64],
    config: serde_json::Value,
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::families::treemap::{
        TreemapDiagramModel, TreemapDiagramRenderModel, TreemapNode,
    };
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;
    use merman_model::{MermaidConfig, OperationControl};

    let model = TreemapDiagramModel {
        root: TreemapNode {
            children: Some((0..values.len()).collect()),
            ..Default::default()
        },
        nodes: values
            .iter()
            .enumerate()
            .map(|(index, value)| TreemapNode {
                name: format!("Leaf{index}"),
                value: Some(serde_json::json!(value).into()),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let config = MermaidConfig::from_value(config);
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "treemap".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
        },
        RenderSemanticModel::Treemap(TreemapDiagramRenderModel::from_semantic_model(model)),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("independent Treemap construction is active")
    .expect("small Treemap model passes construction resource admission")
}

#[test]
fn treemap_direct_input_rejects_nonfinite_derived_values() {
    for (values, config, reason) in [
        (vec![1e308], serde_json::json!({}), "derived geometry"),
        (vec![1e308, 1e308], serde_json::json!({}), "aggregate value"),
        (
            vec![1.0],
            serde_json::json!({"treemap": {"nodeWidth": 1e308}}),
            "dimensions",
        ),
        (
            vec![1.0],
            serde_json::json!({"treemap": {"nodeHeight": 1e308}}),
            "dimensions",
        ),
    ] {
        let parsed = independent_treemap_presentation(&values, config);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&parsed, &LayoutOptions::default(), session)
            .err()
            .expect("finite inputs must not produce a successful nonfinite Treemap layout");
        assert!(
            matches!(
                error,
                merman_render::Error::InvalidModel { ref message } if message.contains(reason)
            ),
            "{error:?}"
        );
    }
}

#[test]
fn treemap_direct_input_preserves_finite_geometry_and_weight_proportions() {
    for values in [[3.0, 2.0], [1e150, 1e150]] {
        let parsed = independent_treemap_presentation(&values, serde_json::json!({}));
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("finite ordinary and large supported weights remain valid");
        let projection = artifact.layout_json().unwrap();
        let layout: merman_render::model::TreemapDiagramLayout =
            serde_json::from_value(projection["layout"]["TreemapDiagram"].clone()).unwrap();
        assert_eq!((layout.width, layout.height), (1000.0, 400.0));
        assert_eq!(layout.leaves.len(), 2);
        for (index, leaf) in layout.leaves.iter().enumerate() {
            assert_eq!(leaf.name, format!("Leaf{index}"));
            assert_eq!(leaf.value, values[index]);
            assert!(
                [leaf.x0, leaf.y0, leaf.x1, leaf.y1]
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
        let areas: Vec<_> = layout
            .leaves
            .iter()
            .map(|leaf| (leaf.x1 - leaf.x0) * (leaf.y1 - leaf.y0))
            .collect();
        assert!((areas[0] / areas[1] - values[0] / values[1]).abs() < 0.02);
    }
}

#[test]
fn treemap_direct_input_validation_preserves_operation_error_priority() {
    use merman_model::OperationControl;

    let parsed = independent_treemap_presentation(&[1e308, 1e308], serde_json::json!({}));
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let error = family::plan_render(&parsed, &session).err().unwrap();
    let merman_render::Error::ResourceLimitExceeded(first) = error else {
        panic!("expected model resource admission before Treemap validation, got {error:?}");
    };
    assert_eq!(first.limit, ResourceLimitId::MaxModelItems.as_str());
    control.cancel();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        merman_render::Error::ResourceLimitExceeded(replayed) if replayed == first
    ));
}

#[derive(Default)]
struct CountingTreemapHost {
    calls: AtomicUsize,
    observations: Mutex<Vec<(String, TextStyle)>>,
    distinguish_font_variants: bool,
}

impl CountingTreemapHost {
    fn width(&self, text: &str, style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .push((text.to_owned(), style.clone()));
        let variant_scale = if self.distinguish_font_variants
            && matches!(style.font_weight.as_deref(), Some("bold" | "700"))
        {
            2.0
        } else {
            1.0
        };
        text.chars().count() as f64 * style.font_size.max(1.0) * variant_scale
    }

    fn reset(&self) {
        self.calls.store(0, Ordering::Relaxed);
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .clear();
    }

    fn observed_styles(&self) -> Vec<TextStyle> {
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .iter()
            .map(|(_, style)| style.clone())
            .collect()
    }

    fn observed_styles_for_text(&self, text: &str) -> Vec<TextStyle> {
        self.observations
            .lock()
            .expect("text measurement recorder is not poisoned")
            .iter()
            .filter(|(observed_text, _)| observed_text == text)
            .map(|(_, style)| style.clone())
            .collect()
    }
}

impl HostTextMeasurer for CountingTreemapHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let width = self.width(request.text, request.style);
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure | TextMeasurementOperation::Wrapped => {
                HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size.max(1.0),
                    line_count: 1,
                })
            }
            TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxWidth => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxHeight => {
                HostTextMeasurement::Length(request.style.font_size.max(1.0))
            }
            _ => return Ok(None),
        }))
    }
}

fn counting_treemap_environment(host: Arc<CountingTreemapHost>) -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.treemap-host").expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host, TextMeasurementPhase::ALL),
    )
}

fn render_treemap_with_recorded_host(
    source: &str,
    host: Arc<CountingTreemapHost>,
) -> family::RenderedFamilySvg {
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables": {"fontFamily": "Cousine"}}),
            ),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = counting_treemap_environment(host).begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn font_size_px(tag: &str) -> Option<f64> {
    let (_, suffix) = tag.split_once("font-size:")?;
    let value = suffix.trim_start();
    let end = value.find("px")?;
    value[..end].trim().parse().ok()
}

fn css_declarations_for_selector_suffix<'a>(stylesheet: &'a str, suffix: &str) -> &'a str {
    stylesheet
        .split('}')
        .filter_map(|rule| rule.rsplit_once('{'))
        .find_map(|(selectors, declarations)| {
            selectors
                .split(',')
                .any(|selector| selector.trim().ends_with(suffix))
                .then_some(declarations)
        })
        .unwrap_or_else(|| panic!("missing CSS selector ending in `{suffix}`: {stylesheet}"))
}

fn text_tag_by_text<'a>(svg: &'a str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let end = svg.find(&needle).expect("expected text tag") + needle.len();
    let start = svg[..end].rfind("<text").expect("expected text tag start");
    &svg[start..end]
}

fn text_tag_by_class_and_text<'a>(svg: &'a str, class_name: &str, text: &str) -> &'a str {
    let needle = format!(">{text}</text>");
    let mut offset = 0;
    while let Some(rel_end) = svg[offset..].find(&needle) {
        let end = offset + rel_end + needle.len();
        let start = svg[..end].rfind("<text").expect("expected text tag start");
        let tag = &svg[start..end];
        if tag.contains(&format!(r#"class="{class_name}""#)) {
            return tag;
        }
        offset = end;
    }
    panic!("expected text tag with class {class_name} and text {text}");
}

fn contains_default_text_fill(tag: &str) -> bool {
    tag.contains("fill:#333")
        || tag.contains("fill: #333")
        || tag.contains("fill:rgb(51, 51, 51)")
        || tag.contains("fill: rgb(51, 51, 51)")
}

fn render_treemap_svg_and_config_from_fixture(fixture: &str) -> (String, serde_json::Value) {
    let path = workspace_root()
        .join("fixtures")
        .join("treemap")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_treemap_svg_and_config_from_source(&text)
}

fn render_treemap_svg_and_config_from_source(text: &str) -> (String, serde_json::Value) {
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let effective_config = parsed.metadata().effective_config.as_value().clone();

    let layout_options = LayoutOptions::default();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(&parsed, &layout_options, session).expect("layout ok");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned();

    (svg, effective_config)
}

fn render_treemap_svg_from_fixture(fixture: &str) -> String {
    render_treemap_svg_and_config_from_fixture(fixture).0
}

fn render_treemap_svg_from_source(text: &str) -> String {
    render_treemap_svg_and_config_from_source(text).0
}

#[test]
fn treemap_intrinsic_svg_size_follows_effective_family_configuration() {
    use merman_core::presentation::SourcePresentationConfig;

    for (site_value, source_value, responsive) in [
        (None, None, true),
        (Some(false), None, false),
        (Some(true), Some(false), false),
        (Some(false), Some(true), true),
    ] {
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            serde_json::json!({"useMaxWidth": false, "treemap": {"useMaxWidth": site_value}}),
        ));
        let prefix = source_value.map_or_else(String::new, |value| {
            format!("---\nconfig:\n  treemap:\n    useMaxWidth: {value}\n---\n")
        });
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                &format!("{prefix}treemap-beta\ntitle Sizes\n\"Root\"\n  \"Child\": 1\n"),
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        let view_box = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        if responsive {
            assert_eq!(root.attribute("width"), Some("100%"));
            assert!(root.attribute("height").is_none());
            assert!(root.attribute("style").unwrap().contains("max-width:"));
        } else {
            assert_eq!(
                root.attribute("width").unwrap().parse::<f64>().unwrap(),
                view_box[2]
            );
            assert_eq!(
                root.attribute("height").unwrap().parse::<f64>().unwrap(),
                view_box[3]
            );
            assert!(root.attribute("style").is_none());
        }
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("class") == Some("treemapLeaf"))
        );
        assert!(
            document
                .descendants()
                .any(|node| node.text() == Some("Sizes"))
        );
    }
}

fn try_render_treemap_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Treemap resource-bound session");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Treemap resource-bound fixture")
        .expect("detect Treemap resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn treemap_repeated_source_declarations_fit_the_actual_retained_budget() {
    let repeated = format!(
        "treemap\nclassDef accent {};\n\"Leaf\": 42:::accent\n",
        vec!["fill:red"; 1000].join(",")
    );
    let single = "treemap\nclassDef accent fill:red;\n\"Leaf\": 42:::accent\n";
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 65_536)
        .unwrap();
    let expected = try_render_treemap_svg_with_resource_policy(single, policy)
        .expect("single source declaration fits budget");
    let actual = try_render_treemap_svg_with_resource_policy(&repeated, policy)
        .expect("overwritten source declarations must not inflate final visual reservation");
    assert_eq!(
        actual, expected,
        "duplicate properties preserve the actual rendered SVG"
    );
    let document = roxmltree::Document::parse(&actual).expect("valid SVG");
    let leaf = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("treemapLeaf"))
        .expect("actual leaf rectangle");
    assert_eq!(leaf.attribute("style"), Some("fill:red !important"));
}

#[test]
fn treemap_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "treemap\n\"Section\"\n  \"Alpha\": 4\n  \"Beta\": 2\n";
    let baseline = try_render_treemap_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Treemap baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Treemap fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Treemap SVG byte ceiling");
    let exact = try_render_treemap_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Treemap family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Treemap SVG byte ceiling");
    let error = try_render_treemap_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Treemap family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Treemap MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

fn deep_treemap_chain(depth: usize) -> String {
    let mut input = String::from("treemap\n");
    for level in 0..depth {
        input.push_str(&" ".repeat(level));
        input.push('"');
        input.push_str(&format!("section{level}"));
        input.push_str("\"\n");
    }
    input.push_str(&" ".repeat(depth));
    input.push_str("\"leaf\": 1\n");
    input
}

#[test]
fn treemap_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_treemap_svg_from_source(
        r#"---
title: Frontmatter treemap
---
treemap-beta
"A": 1
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="treemapTitle" text-anchor="middle" dominant-baseline="middle">Frontmatter treemap</text>"#),
        "frontmatter title should render when the Treemap body has none: {frontmatter_svg}"
    );
    assert!(frontmatter_svg.contains(r#"transform="translate(0, 30)" class="treemapContainer""#));

    let body_svg = render_treemap_svg_from_source(
        r#"---
title: Frontmatter treemap
---
treemap-beta
title Body treemap
"A": 1
"#,
    );
    assert!(body_svg.contains(">Body treemap</text>"));
    assert!(!body_svg.contains(">Frontmatter treemap</text>"));
}

#[test]
fn treemap_classdef_typography_drives_measurement_and_final_leaf_styles() {
    let root_font = "TreemapRoot";
    let source = r#"treemap
title Root title
classDef sectionFont font-family:SectionFace,font-size:24px,font-weight:normal;
classDef leafFont font-family:"Leaf\,Face"\,serif,font-size:18px,font-style:italic;
"Section":::sectionFont
  "Leaf": 42:::leafFont
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables": {"fontFamily": "TreemapRoot"}}),
            ),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse class-styled Treemap")
        .expect("detect class-styled Treemap")
        .into_presentation();
    let session = counting_treemap_environment(Arc::clone(&host))
        .begin_session()
        .expect("begin Treemap session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare class-styled Treemap");
    host.reset();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render class-styled Treemap");
    let styles = host.observed_styles();
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some(root_font) && style.font_size == 14.0
    }));
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some("SectionFace")
            && style.font_size == 24.0
            && style.font_weight.as_deref() == Some("normal")
    }));
    assert!(styles.iter().any(|style| {
        style.font_family.as_deref() == Some(r#""Leaf,Face",serif"#)
            && style.font_size == 18.0
            && style.font_style.as_deref() == Some("italic")
    }));

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Treemap SVG");
    let section_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapSectionLabel")
                && node.text() == Some("Section")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap section label");
    assert!(
        section_style.contains("font-family:SectionFace !important"),
        "{section_style}"
    );
    assert!(
        section_style.contains("font-size:24px !important"),
        "{section_style}"
    );

    let leaf_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapLabel")
                && node.text() == Some("Leaf")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap leaf label");
    assert!(leaf_style.contains(r#"font-family:"Leaf,Face",serif !important"#));
    assert!(leaf_style.contains("font-style:italic !important"));
    assert_eq!(leaf_style.matches("font-size:").count(), 1, "{leaf_style}");
    assert!(
        !leaf_style.contains("font-size:18px !important"),
        "the final D3 font-size mutation must retire the authored leaf size: {leaf_style}"
    );

    let value_style = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("treemapValue")
                && node.text() == Some("42")
        })
        .and_then(|node| node.attribute("style"))
        .expect("styled Treemap leaf value");
    assert!(value_style.contains(r#"font-family:"Leaf,Face",serif !important"#));
    assert_eq!(
        value_style.matches("font-size:").count(),
        1,
        "{value_style}"
    );
}

#[test]
fn treemap_title_font_size_config_drives_measurement_and_final_css() {
    let source = r#"---
config:
  treemap:
    titleFontSize: 31.5px
---
treemap
title Config-sized title
"Leaf": 42
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables": {"fontFamily": "TreemapRoot"}}),
            ),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse config-sized Treemap title")
        .expect("detect config-sized Treemap title")
        .into_presentation();
    let session = counting_treemap_environment(Arc::clone(&host))
        .begin_session()
        .expect("begin Treemap session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare config-sized Treemap title");
    host.reset();

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render config-sized Treemap title");
    let title_styles = host.observed_styles_for_text("Config-sized title");
    assert_eq!(title_styles.len(), 2, "title width and height measurements");
    assert!(
        title_styles.iter().all(|style| {
            style.font_family.as_deref() == Some("TreemapRoot") && style.font_size == 31.5
        }),
        "title measurements must use the final configured CSS size: {title_styles:?}"
    );

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Treemap SVG");
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Treemap stylesheet");
    let title_rule = css_declarations_for_selector_suffix(style, ".treemapTitle");
    assert!(
        title_rule.contains("font-size:31.5px;"),
        "final Treemap title rule must preserve the measured size: {title_rule}"
    );
}

#[test]
fn treemap_root_font_size_is_shared_by_css_and_relative_text_measurements() {
    for (root_size, expected_root_px, expected_text_px, nested_style) in [
        ("0px", "0", 0.0, false),
        ("0.5px", "0.5", 50.0, false),
        ("0.0000000001px", "0.0000000001", 0.00000001, false),
        ("24pt", "32", 3200.0, false),
        ("0px", "0", 0.0, true),
        ("0.5px", "0.5", 50.0, true),
    ] {
        let mut config = serde_json::json!({
            "themeVariables": {"fontSize": root_size},
            "treemap": {"titleFontSize": "100em"}
        });
        if nested_style {
            config["themeVariables"]["treemap"] = serde_json::json!({"titleFontSize": "100em"});
            config["treemap"]["titleFontSize"] = serde_json::json!("1px");
        }
        let source = format!(
            "---\nconfig: {config}\n---\ntreemap\ntitle Scaled title\nclassDef scaled font-size:100em;\n\"Section\":::scaled\n  \"Leaf\": 42\n"
        );
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"themeVariables": {"fontFamily": "TreemapRoot"}}),
                ),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let host = Arc::new(CountingTreemapHost::default());
        let session = counting_treemap_environment(Arc::clone(&host))
            .begin_session()
            .unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        host.reset();
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("treemap-root-size".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        for selector in ["#treemap-root-size", "#treemap-root-size svg"] {
            assert!(
                css.contains(&format!(
                    "{selector}{{font-family:TreemapRoot;font-size:{expected_root_px}px;"
                )),
                "{root_size}: {css}",
            );
        }
        assert!(
            css_declarations_for_selector_suffix(css, ".treemapTitle").contains("font-size:100em;")
        );
        for text in ["Scaled title", "Section"] {
            let observed = host.observed_styles_for_text(text);
            assert!(!observed.is_empty(), "{root_size}/{text}");
            assert!(
                observed.iter().all(|style| {
                    style.font_family.as_deref() == Some("TreemapRoot")
                        && (style.font_size - expected_text_px).abs() < 1e-12
                }),
                "{root_size}/{text}: {observed:?}",
            );
        }
    }
}

#[test]
fn treemap_zero_sized_source_labels_stay_hidden() {
    for size in ["0", "0px", "0em", "0%", "-0.0pt"] {
        let source = format!(
            "treemap\nclassDef hidden font-size:{size};\n\"Section\":::hidden\n  \"Leaf\": 42:::hidden\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let host = Arc::new(CountingTreemapHost::default());
        let session = counting_treemap_environment(Arc::clone(&host))
            .begin_session()
            .unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        host.reset();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("zero size {size} must remain supported: {error}"));
        for text in ["Section", "Leaf"] {
            let styles = host.observed_styles_for_text(text);
            assert!(!styles.is_empty(), "{size}/{text}: missing measurement");
            assert!(
                styles.iter().all(|style| style.font_size == 0.0),
                "{size}/{text}: {styles:?}"
            );
        }
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for class in ["treemapLabel", "treemapValue"] {
            let node = document
                .descendants()
                .find(|node| node.attribute("class") == Some(class))
                .expect("the hidden leaf text terminal remains in the SVG");
            let style = node.attribute("style").unwrap();
            assert!(style.contains("display: none;"), "{size}/{class}: {style}");
            if class == "treemapLabel" {
                assert!(style.contains("font-size: 0px;"), "{size}: {style}");
            }
        }
        for class in ["treemapSectionLabel", "treemapSectionValue"] {
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("class") == Some(class)
                        && node.text().is_some()
                        && node.attribute("style") != Some("display: none;")
                })
                .expect("the zero-sized section text terminal remains in the SVG");
            assert!(
                node.attribute("style")
                    .unwrap()
                    .contains(&format!("font-size:{size} !important")),
                "{size}/{class}: {}",
                node.attribute("style").unwrap()
            );
        }
    }
}

#[test]
fn treemap_leaf_label_and_value_remain_visible_and_vertically_ordered() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_basic_spec.mmd");

    let needle = ">Item A1</text>";
    let end = svg.find(needle).expect("expected Item A1 label");
    let tag = text_tag_by_text(&svg, "Item A1");

    assert!(tag.contains(r#"class="treemapLabel""#));
    assert!(!tag.contains("display: none"));
    assert_eq!(
        font_size_px(tag),
        Some(34.0),
        "Treemap leaf fitting must use Mermaid's getComputedTextLength semantics"
    );

    let rest = &svg[(end + needle.len())..];
    let value_class = rest
        .find(r#"class="treemapValue""#)
        .expect("expected value tag");
    let value_start = rest[..value_class]
        .rfind("<text")
        .expect("expected value tag start");
    let value_end_rel = rest[value_start..]
        .find("</text>")
        .expect("expected value end");
    let value_tag = &rest[value_start..(value_start + value_end_rel + "</text>".len())];
    let label_y = attr_f64(tag, "y").expect("label y");
    let value_y = attr_f64(value_tag, "y").expect("value y");
    assert!(!value_tag.contains("display: none"));
    assert!(value_y > label_y, "value must be placed below its label");
    assert!(
        font_size_px(value_tag).expect("value font size")
            <= font_size_px(tag).expect("label font size")
    );
}

#[test]
fn treemap_huge_source_font_uses_cssom_scalar_and_bounds_fitting_work() {
    struct NarrowTextHost(Arc<AtomicUsize>);
    impl HostTextMeasurer for NarrowTextHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::ComputedLength {
                return Ok(None);
            }
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Some(HostTextMeasurement::Length(1.0)))
        }
    }
    let source = "treemap\nclassDef huge font-size:100000000000000000000px;\n\"Leaf\": 42:::huge\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.treemap-narrow").unwrap(),
        "1",
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(NarrowTextHost(Arc::clone(&calls))),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let label = text_tag_by_text(rendered.svg(), "Leaf");
    assert_eq!(font_size_px(label), Some(1.0), "{label}");
    assert!(label.contains("display: none"), "{label}");
    let value = text_tag_by_class_and_text(rendered.svg(), "treemapValue", "42");
    assert!(value.contains("font-size:100000000000000000000px !important"));
    assert!(value.contains("display: none"));
    assert_eq!(attr_f64(value, "y"), attr_f64(label, "y"));
    assert!(
        (1..128).contains(&calls.load(Ordering::Relaxed)),
        "CSSOM fitting remains bounded"
    );
}

#[test]
fn treemap_huge_overwide_source_font_skips_width_search_after_cssom_parse_int() {
    struct WideTextHost(Arc<AtomicUsize>);
    impl HostTextMeasurer for WideTextHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::ComputedLength {
                return Ok(None);
            }
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Some(HostTextMeasurement::Length(
                request.style.font_size.min(1_000_000.0),
            )))
        }
    }
    let source = "treemap\nclassDef huge font-size:100000000000000000000px;\n\"Leaf\": 42:::huge\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.treemap-wide").unwrap(),
        "1",
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let host: Arc<dyn HostTextMeasurer> = Arc::new(WideTextHost(Arc::clone(&calls)));
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host,
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("bounded width fitting must finish");
    let label = text_tag_by_text(rendered.svg(), "Leaf");
    assert_eq!(font_size_px(label), Some(1.0), "{label}");
    assert!(label.contains("display: none"), "{label}");
    let value = text_tag_by_class_and_text(rendered.svg(), "treemapValue", "42");
    assert!(value.contains("display: none"));
    assert_eq!(attr_f64(value, "y"), attr_f64(label, "y"));
    assert!(
        (1..128).contains(&calls.load(Ordering::Relaxed)),
        "CSSOM scalar 1 must skip the decrement search for huge source sizes"
    );
}

#[test]
fn treemap_rejects_value_format_outside_the_supported_d3_subset() {
    let source = r#"---
config:
  treemap:
    valueFormat: .2s
---
treemap
"Section"
  "Leaf": 1234
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Treemap with valueFormat")
        .expect("detect Treemap with valueFormat")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin Treemap session");
    let error = match family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Treemap")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unsupported D3 valueFormat must fail closed"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        merman_render::Error::InvalidModel { ref message }
            if message.contains("unsupported Treemap valueFormat")
    ));
}

#[test]
fn treemap_hierarchical_leaf_label_is_visible_with_positive_font_size() {
    let svg = render_treemap_svg_from_fixture("upstream_treemap_docs_hierarchical_spec.mmd");
    let tag = text_tag_by_text(&svg, "Accessories");

    assert!(!tag.contains("display: none"), "{tag}");
    assert!(font_size_px(tag).is_some_and(|size| size > 0.0), "{tag}");
}

#[test]
fn treemap_dark_complex_example_uses_readable_label_colors() {
    let (svg, effective_config) = render_treemap_svg_and_config_from_fixture(
        "upstream_cypress_treemap_spec_9_should_handle_a_complex_example_with_multiple_features_016.mmd",
    );
    let theme = effective_config
        .get("theme")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let label_text_color = effective_config
        .pointer("/themeVariables/labelTextColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");
    let scale_label_color = effective_config
        .pointer("/themeVariables/scaleLabelColor")
        .and_then(|v| v.as_str())
        .unwrap_or("<missing>");

    let engineering_tag = text_tag_by_text(&svg, "Engineering");
    assert!(
        engineering_tag.contains("fill:lightgrey") || engineering_tag.contains("fill: lightgrey"),
        "expected Engineering section label to use lightgrey like upstream, got {engineering_tag}; theme={theme}; labelTextColor={label_text_color}; scaleLabelColor={scale_label_color}"
    );

    let frontend_tag = text_tag_by_text(&svg, "Frontend");
    assert!(
        frontend_tag.contains("fill:lightgrey") || frontend_tag.contains("fill: lightgrey"),
        "expected Frontend leaf label to use lightgrey like upstream, got {frontend_tag}"
    );
}

#[test]
fn treemap_single_leaf_label_uses_readable_fill_over_transparent_cell() {
    let svg = render_treemap_svg_from_source(
        r#"treemap
"Item" : 123.45
"#,
    );

    assert!(
        svg.contains(r#"class="treemapLeaf" fill="transparent""#),
        "expected single top-level leaf to preserve Mermaid's transparent cell fill: {svg}"
    );

    let label_tag = text_tag_by_text(&svg, "Item");
    assert!(
        contains_default_text_fill(label_tag),
        "single-leaf label must remain visible on the white root background: {label_tag}"
    );
    assert!(
        !label_tag.contains("fill:#ffffff")
            && !label_tag.contains("fill: #ffffff")
            && !label_tag.contains("fill: rgb(255, 255, 255)"),
        "single-leaf label should not keep upstream's white-on-transparent fill: {label_tag}"
    );

    let value_tag = text_tag_by_class_and_text(&svg, "treemapValue", "123.45");
    assert!(
        contains_default_text_fill(value_tag),
        "single-leaf value must remain visible on the white root background: {value_tag}"
    );
}

#[test]
fn treemap_classdef_bare_label_style_token_renders_error_like_mermaid_parser() {
    let source = r#"treemap
classDef c fill:#ff0000, stroke:rgb(1\,2\,3), color;
"Root":::c
  "Leaf": 1000.00:::c
"#;

    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            source,
            ParseOptions {
                suppress_errors: true,
            },
        )
        .expect("parse returns suppressed error")
        .expect("diagram detected")
        .into_presentation();

    assert_eq!(parsed.metadata().diagram_type, "error");

    let layout_options = LayoutOptions::default();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(&parsed, &layout_options, session).expect("layout ok");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned();

    assert!(
        svg.contains(r#"aria-roledescription="error""#) && svg.contains("Syntax error in text"),
        "expected Mermaid parser-compatible error SVG for invalid classDef style token: {svg}"
    );
}

#[test]
fn treemap_deep_chain_projects_and_renders_flat_model() {
    const DEPTH: usize = 1200;
    let source = deep_treemap_chain(DEPTH);

    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();

    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("flat deep Treemap layout");
    let projection = artifact
        .layout_json()
        .expect("managed deep semantic projection");
    assert_eq!(
        projection["semantic"]["nodes"].as_array().unwrap().len(),
        DEPTH + 1
    );
    let mut exported = Vec::new();
    projection.write_json(&mut exported).unwrap();
    assert!(serde_json::to_vec(&projection).is_err());
    drop(projection.clone());
    drop(projection);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("deep Treemap SVG");
    roxmltree::Document::parse(rendered.svg()).expect("well-formed deep Treemap SVG");
}

#[test]
fn treemap_svg_uses_the_session_measurement_route() {
    let source = r#"treemap
title Routed title
"Section"
  "Measured leaf alpha": 42
  "Measured leaf bravo": 42
  "Measured leaf charlie": 42
  "Measured leaf delta": 42
  "Measured leaf echo": 42
  "Measured leaf foxtrot": 42
  "Measured leaf golf": 42
  "Measured leaf hotel": 42
"#;
    let host = Arc::new(CountingTreemapHost::default());
    let session = counting_treemap_environment(Arc::clone(&host))
        .begin_session()
        .expect("begin render session");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    host.calls.store(0, Ordering::Relaxed);

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render SVG");
    let completion = rendered.into_completion();
    assert_eq!(completion.report().family_id(), DiagramFamilyId::TREEMAP);
    let (host_svg, host_report) = completion.into_output_and_report();

    assert!(
        host.calls.load(Ordering::Relaxed) > 0,
        "Treemap must not bypass the session with a family-local deterministic measurer"
    );
    assert!(
        host_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().phase == TextMeasurementPhase::ComputedLength
                    && entry.provenance().operation == TextMeasurementOperation::ComputedLength
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Treemap fitting checks must use the exact getComputedTextLength operation"
    );
    assert!(
        host_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().operation == TextMeasurementOperation::RawBBoxHeight
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Treemap title bounds must route SVG bbox height through the session"
    );

    let parity_parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let parity_session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parity_artifact =
        family::prepare(&parity_parsed, &LayoutOptions::default(), parity_session)
            .expect("parity layout");
    let parity_svg = parity_artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("parity SVG")
        .svg()
        .to_owned();
    assert_ne!(
        host_svg, parity_svg,
        "host metrics must change observable geometry"
    );
}

#[test]
fn treemap_value_text_preserves_zero_ending_scientific_exponents() {
    let svg = render_treemap_svg_from_source("treemap-beta\n\"Tiny\": 0.00000000012\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let value = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("treemapValue"))
        .expect("visible treemap value");
    assert_eq!(value.text(), Some("1.2e-10"));
}

#[test]
fn treemap_invalid_final_font_sizes_use_generated_sizes_after_property_folding() {
    let body = "\"Section\":::probe\n  \"Leaf\": 42:::probe\n";
    let control_host = Arc::new(CountingTreemapHost::default());
    let control = render_treemap_with_recorded_host(
        &format!("treemap\n{}", body.replace(":::probe", "")),
        Arc::clone(&control_host),
    );
    let control_document = roxmltree::Document::parse(control.svg()).unwrap();
    let control_text: Vec<_> = control_document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            (
                node.attribute("class"),
                node.text(),
                node.attribute("x"),
                node.attribute("y"),
            )
        })
        .collect();

    for invalid in ["-1px", "-10%", "potato"] {
        for preceding in ["", "font-size:48px,"] {
            let source = format!("treemap\nclassDef probe {preceding}font-size:{invalid};\n{body}");
            let host = Arc::new(CountingTreemapHost::default());
            let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
            assert_eq!(
                serde_json::to_value(host.observed_styles()).unwrap(),
                serde_json::to_value(control_host.observed_styles()).unwrap(),
                "{preceding}{invalid}: measurement must follow the final folded CSS"
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let text: Vec<_> = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| {
                    (
                        node.attribute("class"),
                        node.text(),
                        node.attribute("x"),
                        node.attribute("y"),
                    )
                })
                .collect();
            assert_eq!(text, control_text, "{preceding}{invalid}");
            let section =
                text_tag_by_class_and_text(rendered.svg(), "treemapSectionLabel", "Section");
            assert!(section.contains(&format!("font-size:{invalid} !important")));
            assert!(!section.contains("font-size:48px"));
            for (class, text) in [("treemapLabel", "Leaf"), ("treemapValue", "42")] {
                let actual = text_tag_by_class_and_text(rendered.svg(), class, text);
                let expected = text_tag_by_class_and_text(control.svg(), class, text);
                assert_eq!(font_size_px(actual), font_size_px(expected));
                assert_eq!(actual.matches("font-size:").count(), 1);
                assert!(!actual.contains("font-size:48px"));
            }
        }
    }
}

#[test]
fn treemap_unitless_svg_sizes_match_pixel_sizes_during_fitting() {
    let render = |size: &str| {
        let host = Arc::new(CountingTreemapHost::default());
        let source = format!(
            "treemap\nclassDef probe font-size:{size};\n\"Section\":::probe\n  \"Leaf\": 42:::probe\n"
        );
        let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let text = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| {
                (
                    node.attribute("class").map(str::to_owned),
                    node.text().map(str::to_owned),
                    node.attribute("x").map(str::to_owned),
                    node.attribute("y").map(str::to_owned),
                )
            })
            .collect::<Vec<_>>();
        for (class, value) in [("treemapLabel", "Leaf"), ("treemapValue", "42")] {
            let tag = text_tag_by_class_and_text(rendered.svg(), class, value);
            assert!(font_size_px(tag).is_some_and(|size| size > 0.0));
        }
        (text, serde_json::to_value(host.observed_styles()).unwrap())
    };
    assert_eq!(render("24"), render("24px"));
}

#[test]
fn treemap_leaf_fitting_separates_cssom_scalars_from_used_measurement_sizes() {
    for (size, used_px, label_px, value_px, hidden) in [
        ("24", 24.0, 24.0, 14.0, false),
        ("24px", 24.0, 24.0, 14.0, false),
        ("24pt", 32.0, 24.0, 14.0, false),
        (r"24p\74", 32.0, 24.0, 14.0, false),
        ("+24px", 24.0, 24.0, 14.0, false),
        ("24PX", 24.0, 24.0, 14.0, false),
        ("32px", 32.0, 32.0, 19.0, false),
        ("1e2px", 100.0, 100.0, 28.0, false),
        ("1e6px", 1e6, 1.0, 1e6, true),
        ("1e12px", 1e12, 1.0, 1e12, true),
        ("1e20px", 1e20, 1.0, 1e20, true),
        ("100000000000000000000px", 1e20, 1.0, 1e20, true),
        ("0.000001px", 1e-6, 1.0, 1e-6, true),
        ("1e-6px", 1e-6, 1.0, 1e-6, true),
        ("150%", 24.0, 150.0, 28.0, false),
        ("2em", 32.0, 2.0, 32.0, true),
        ("0.5px", 0.5, 0.0, 0.5, true),
        (".5px", 0.5, 0.0, 0.5, true),
        ("0", 0.0, 0.0, 0.0, true),
        ("-0.0pt", 0.0, 0.0, 0.0, true),
    ] {
        let source = format!("treemap\nclassDef probe font-size:{size};\n\"Leaf\": 42:::probe\n");
        let host = Arc::new(CountingTreemapHost::default());
        let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
        let measured = host.observed_styles_for_text("Leaf");
        assert!(!measured.is_empty(), "{size}: missing initial measurement");
        assert_eq!(measured[0].font_size, used_px, "{size}: initial used size");
        assert_eq!(
            measured.last().unwrap().font_size,
            label_px,
            "{size}: final used size"
        );
        assert!(
            measured
                .iter()
                .all(|style| style.font_size.is_finite() && style.font_size >= 0.0)
        );

        let label = text_tag_by_class_and_text(rendered.svg(), "treemapLabel", "Leaf");
        let value = text_tag_by_class_and_text(rendered.svg(), "treemapValue", "42");
        assert_eq!(font_size_px(label), Some(label_px), "{size}: {label}");
        assert_eq!(
            label.contains("; display: none;"),
            hidden,
            "{size}: {label}"
        );
        assert_eq!(
            value.contains("; display: none;"),
            hidden,
            "{size}: {value}"
        );
        assert_eq!(label.matches("font-size:").count(), 1, "{size}: {label}");
        let label_y = attr_f64(label, "y").unwrap();
        let value_y = attr_f64(value, "y").unwrap();
        assert!(label_y.is_finite() && value_y.is_finite(), "{size}");
        if hidden {
            assert!(
                value.contains(&format!("font-size:{size} !important")),
                "{size}: {value}"
            );
            assert_eq!(
                font_size_px(value),
                Some(28.0),
                "{size}: generated value size is retained"
            );
            assert_eq!(
                value_y, label_y,
                "{size}: hidden value keeps the source placeholder"
            );
            assert!(
                host.observed_styles_for_text("42").is_empty(),
                "{size}: hidden value is not measured"
            );
        } else {
            assert_eq!(font_size_px(value), Some(value_px), "{size}: {value}");
            assert_eq!(value.matches("font-size:").count(), 1, "{size}: {value}");
            assert_eq!(
                value_y,
                label_y + label_px / 2.0 + 2.0,
                "{size}: value follows final inline size"
            );
            let measured_values = host.observed_styles_for_text("42");
            assert!(
                !measured_values.is_empty(),
                "{size}: missing value measurement"
            );
            assert!(
                measured_values
                    .iter()
                    .all(|style| style.font_size == value_px),
                "{size}: {measured_values:?}"
            );
        }
        assert!(!rendered.svg().contains("NaN"), "{size}");
    }
}

#[test]
fn treemap_leaf_width_and_height_fitting_use_authored_scalar_after_first_measurement() {
    for (size, width, height, used_px, final_px) in [
        ("24pt", 12.0, 40.0, 32.0, 23.0),
        ("24pt", 100.0, 8.1, 32.0, 16.0),
        ("150%", 13.0, 40.0, 24.0, 150.0),
    ] {
        let source = format!(
            "---\nconfig:\n  treemap:\n    nodeWidth: {width}\n    nodeHeight: {height}\n---\n\
             treemap\nclassDef probe font-size:{size};\n\"Leaf\": 42:::probe\n"
        );
        let host = Arc::new(CountingTreemapHost::default());
        let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
        let label = text_tag_by_class_and_text(rendered.svg(), "treemapLabel", "Leaf");
        let measured = host.observed_styles_for_text("Leaf");
        assert_eq!(measured[0].font_size, used_px, "{size}/{width}/{height}");
        assert_eq!(
            font_size_px(label),
            Some(final_px),
            "{size}/{width}/{height}: {label}"
        );
        assert!(
            measured.iter().skip(1).all(|style| {
                style.font_size == style.font_size.trunc() && style.font_size < 24.0
            }) || size == "150%",
            "{size}/{width}/{height}: {measured:?}"
        );
        if size == "150%" {
            assert!(
                label.contains("display: none"),
                "the final check must measure 150px: {label}"
            );
            let value = text_tag_by_class_and_text(rendered.svg(), "treemapValue", "42");
            assert!(value.contains("font-size:150% !important") && value.contains("display: none"));
        } else {
            assert!(!label.contains("display: none"), "{label}");
        }
    }
}

#[test]
fn treemap_leaf_fitting_observes_only_the_final_folded_font_size_property() {
    for size in [
        "24pt", "2em", "150%", "0.5px", "inherit", "initial", "medium",
    ] {
        let render = |preceding: &str| {
            let source = format!(
                "---\nconfig:\n  treemap:\n    showValues: false\n---\n\
                 treemap\nclassDef probe {preceding}font-size:{size};\n\"Leaf\": 42:::probe\n"
            );
            let host = Arc::new(CountingTreemapHost::default());
            let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
            let label =
                text_tag_by_class_and_text(rendered.svg(), "treemapLabel", "Leaf").to_owned();
            (
                label,
                serde_json::to_value(host.observed_styles_for_text("Leaf")).unwrap(),
            )
        };
        let control = render("");
        for preceding in ["font-size:48px,", "font-size:potato,", "font-size:inherit,"] {
            assert_eq!(render(preceding), control, "{preceding}{size}");
        }
    }
}

#[test]
fn treemap_leaf_keywords_keep_css_ownership_and_bound_invalid_upstream_value_geometry() {
    for (size, used_px) in [
        ("inherit", 16.0),
        ("unset", 16.0),
        ("initial", 16.0),
        ("medium", 16.0),
        ("large", 18.0),
        ("larger", 19.2),
    ] {
        for show_values in [false, true] {
            let source = format!(
                "---\nconfig:\n  treemap:\n    showValues: {show_values}\n---\n\
                 treemap\nclassDef probe font-size:{size};\n\"Leaf\": 42:::probe\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let host = Arc::new(CountingTreemapHost::default());
            let session = counting_treemap_environment(Arc::clone(&host))
                .begin_session()
                .unwrap();
            let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
            host.reset();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let label = text_tag_by_class_and_text(rendered.svg(), "treemapLabel", "Leaf");
            assert!(
                label.contains("font-size: 38px;")
                    && label.contains(&format!("font-size:{size} !important")),
                "{size}: {label}"
            );
            assert!(!label.contains("display: none"), "{size}: {label}");
            for style in host.observed_styles_for_text("Leaf") {
                assert!(
                    (style.font_size - used_px).abs() < 1e-12,
                    "{size}: {style:?}"
                );
            }
            let pixel_source = source.replace(
                &format!("font-size:{size}"),
                &format!("font-size:{used_px}px"),
            );
            let pixel_host = Arc::new(CountingTreemapHost::default());
            let pixel_control =
                render_treemap_with_recorded_host(&pixel_source, Arc::clone(&pixel_host));
            let pixel_label =
                text_tag_by_class_and_text(pixel_control.svg(), "treemapLabel", "Leaf");
            assert_eq!(font_size_px(pixel_label), Some(used_px.trunc()), "{size}");
            assert_eq!(attr_f64(label, "y"), attr_f64(pixel_label, "y"), "{size}");
            assert!(
                (pixel_host.observed_styles_for_text("Leaf")[0].font_size - used_px).abs() < 1e-12
            );
            if show_values {
                let value = text_tag_by_class_and_text(rendered.svg(), "treemapValue", "42");
                assert!(
                    value.contains("font-size: 28px;")
                        && value.contains(&format!("font-size:{size} !important")),
                    "{size}: {value}"
                );
                assert_eq!(
                    attr_f64(value, "y"),
                    None,
                    "{size}: invalid upstream y is a residual, not a new position"
                );
                let pixel_value =
                    text_tag_by_class_and_text(pixel_control.svg(), "treemapValue", "42");
                assert!(
                    attr_f64(pixel_value, "y").is_some_and(f64::is_finite),
                    "{size}"
                );
                let measured = host.observed_styles_for_text("42");
                assert!(
                    !measured.is_empty(),
                    "{size}: retained value style is measured"
                );
                assert!(
                    measured
                        .iter()
                        .all(|style| (style.font_size - used_px).abs() < 1e-12),
                    "{size}: {measured:?}"
                );
            }
            assert!(!rendered.svg().contains("NaN"), "{size}");
        }
    }
}

#[test]
fn treemap_initial_font_resets_match_normal_measurement_and_keep_raw_suffixes() {
    let name = "i".repeat(40);
    let render = |weight: &str, style: &str| {
        // A weight-sensitive host exposes wrong requests even for monospace fixture fonts.
        let host = Arc::new(CountingTreemapHost {
            distinguish_font_variants: true,
            ..Default::default()
        });
        let source = format!(
            "---\nconfig:\n  treemap:\n    nodeWidth: 20\n---\ntreemap\n\
             classDef probe font-weight:{weight},font-style:{style};\n\
             \"{name}\":::probe\n  \"Leaf\": 42\n"
        );
        let rendered = render_treemap_with_recorded_host(&source, Arc::clone(&host));
        (rendered, host)
    };
    let (initial, host) = render("initial", "initial");
    let (normal, _) = render("normal", "normal");
    let (bold, _) = render("bold", "normal");
    let visible_section_label = |svg: &str| {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("treemapSectionLabel") && node.text().is_some()
            })
            .unwrap()
            .text()
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        visible_section_label(initial.svg()),
        visible_section_label(normal.svg())
    );
    assert_ne!(
        visible_section_label(initial.svg()),
        visible_section_label(bold.svg()),
        "the production truncation control must distinguish bold metrics"
    );
    let styles = host.observed_styles_for_text(&name);
    assert!(!styles.is_empty());
    assert!(styles.iter().all(|style| {
        style.font_family.as_deref() == Some("Cousine")
            && style.font_weight.as_deref() == Some("400")
            && style.font_style.as_deref() == Some("normal")
    }));
    let document = roxmltree::Document::parse(initial.svg()).unwrap();
    for class in ["treemapSectionLabel", "treemapSectionValue"] {
        let text = document
            .descendants()
            .find(|node| {
                node.attribute("class") == Some(class)
                    && node.attribute("style") != Some("display: none;")
            })
            .unwrap();
        let suffix = text.attribute("style").unwrap();
        assert!(suffix.contains("font-weight:initial !important"));
        assert!(suffix.contains("font-style:initial !important"));
    }
}

#[test]
fn treemap_nested_style_options_override_root_aliases_in_the_production_stylesheet() {
    let config = serde_json::json!({
        "treemap": {
            "titleColor": "#010101", "labelColor": "#020202", "valueColor": "#030303",
            "titleFontSize": "1px", "labelFontSize": "2px", "valueFontSize": "3px",
            "sectionStrokeColor": "#040404", "sectionStrokeWidth": 1,
            "sectionFillColor": "#050505", "leafStrokeColor": "#060606",
            "leafStrokeWidth": 2, "leafFillColor": "#070707"
        },
        "themeVariables": {"treemap": {
            "titleColor": "#111111", "labelColor": "#222222", "valueColor": "#333333",
            "titleFontSize": "14px", "labelFontSize": "13px", "valueFontSize": "11px",
            "sectionStrokeColor": "#444444", "sectionStrokeWidth": 4,
            "sectionFillColor": "#555555", "leafStrokeColor": "#666666",
            "leafStrokeWidth": 5, "leafFillColor": "#777777"
        }}
    });
    let svg = render_treemap_svg_from_source(&format!(
        "---\nconfig: {config}\n---\ntreemap\ntitle Options\n\"Leaf\": 1\n"
    ));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    for (selector, expected) in [
        (".treemapTitle", "fill:#111111;font-size:14px;"),
        (".treemapLabel", "fill:#222222;font-size:13px;"),
        (".treemapValue", "fill:#333333;font-size:11px;"),
        (
            ".treemapNode.section",
            "stroke:#444444;stroke-width:4;fill:#555555;",
        ),
        (
            ".treemapNode.leaf",
            "stroke:#666666;stroke-width:5;fill:#777777;",
        ),
    ] {
        assert_eq!(
            css_declarations_for_selector_suffix(css, selector),
            expected
        );
    }
}

#[allow(dead_code)]
mod common;

#[test]
fn treemap_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "treemap-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\n\"Root\"\n  \"Leaf\": 1\n"
        );
        let svg = render_treemap_svg_from_source(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}

#[test]
fn native_theme_v2_treemap_preserves_class_styles_and_uses_canvas_separators() {
    use merman_render::theme::{Color, NativeTheme, ThemeCompiler};

    let source = r#"treemap
title Allocation
classDef chosen fill:#123456,color:#abcdef;
"First"
  "Alpha": 30:::chosen
"Second"
  "Beta": 20
"#;
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2,
                "extends": "studio",
                "appearances": {"light": {"data": {
                    "fill": ["#475569", "#334155", "#526377", "#3D4B60"],
                    "ink": ["#F8FAFC", "#E2E8F0", "#F1F5F9", "#F8FAFC"],
                    "pattern_encoding": true
                }}},
                "typography": {"sans": ["Treemap Native", "sans-serif"], "base_size_px": 20},
                "form": {"radius": 7}
            }"##,
            None,
        )
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(
            NativeTheme::new(theme).with_background(Color::rgb(240, 230, 220)),
        )
        .unwrap();
    let rendered = family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();
    let document = roxmltree::Document::parse(svg).unwrap();
    let leaves = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treemapLeaf"))
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), 2);
    assert!(leaves.iter().all(|node| {
        node.attribute("stroke") == Some("#F0E6DC") && node.attribute("rx") == Some("7")
    }));
    assert!(
        leaves
            .iter()
            .any(|node| node.attribute("style") == Some("fill:#123456 !important"))
    );
    assert!(
        leaves
            .iter()
            .any(|node| node.attribute("fill") == Some("url(#treemap-treemap-pattern-1)"))
    );
    let label = text_tag_by_class_and_text(svg, "treemapLabel", "Alpha");
    assert!(label.contains("fill:#abcdef !important"), "{label}");
    assert!(svg.contains("font-family:\"Treemap Native\", sans-serif;"));
    assert!(svg.contains("font-size:27px;"));
}
