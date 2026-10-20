use merman::diagram_theme::{
    DiagramFamilyId, DiagramThemeCompiler, ThemeDesignCompileError, ThemeDesignV1, ThemeRecipeV1,
    ThemeRuleFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1, ThemeSupportStateV1, ThemeTarget,
    describe_theme_support, materialize_theme_design,
};
use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};
use serde_json::json;

fn editable_design(fill: &str, background: &str) -> ThemeDesignV1 {
    serde_json::from_value(json!({
        "tokens":{
            "color.ink":{"type":"color","value":fill},
            "paint.body":{"type":"paint","value":{"kind":"solid","color":{"ref":"color.ink"}}}
        },
        "canvas":{"base":background},
        "styles":{
            "node":{"fill":{"ref":"paint.body"}},
            "actor":{"fill":{"ref":"paint.body"}}
        }
    }))
    .expect("editable theme design")
}

fn compile_design(design: ThemeDesignV1) -> merman::diagram_theme::DiagramTheme {
    DiagramThemeCompiler::new()
        .compile_recipe(ThemeRecipeV1::Design { design })
        .expect("compile design")
}

fn render_svg(
    renderer: &Renderer,
    source: &str,
    theme: &merman::diagram_theme::DiagramTheme,
) -> String {
    let output = renderer
        .render(
            RenderRequest::svg(
                source,
                OperationControl::new(),
                merman::SvgRequest {
                    options: merman::svg::SvgRenderOptions {
                        diagram_id: Some("editable-theme-witness".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .with_theme(theme.clone()),
        )
        .expect("render themed SVG");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("SVG diagram required");
    };
    output.svg().to_owned()
}

#[test]
fn editable_bindings_round_trip_and_remain_isolated_between_compiled_themes() {
    let light_design = editable_design("#12ab34", "#ffffff");
    let dark_design = editable_design("#3456de", "#111827");
    let light_json = light_design.canonical_json_bytes().unwrap();
    let dark_json = dark_design.canonical_json_bytes().unwrap();
    assert_eq!(
        serde_json::from_slice::<ThemeDesignV1>(&light_json).unwrap(),
        light_design
    );
    let light = compile_design(light_design.clone());
    let dark = compile_design(dark_design.clone());
    let renderer = Renderer::new();
    for source in [
        "flowchart LR\nA[Alpha] --> B[Beta]\n",
        "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
    ] {
        let first_light = render_svg(&renderer, source, &light);
        let first_dark = render_svg(&renderer, source, &dark);
        assert!(first_light.contains("#12ab34"), "{first_light}");
        assert!(first_dark.contains("#3456de"), "{first_dark}");
        assert_ne!(first_light, first_dark);
        assert_eq!(render_svg(&renderer, source, &light), first_light);
        assert_eq!(render_svg(&Renderer::new(), source, &dark), first_dark);
    }
    assert_eq!(light_design.canonical_json_bytes().unwrap(), light_json);
    assert_eq!(dark_design.canonical_json_bytes().unwrap(), dark_json);
}

#[test]
fn changing_one_shared_value_updates_all_bound_consumers_without_rewriting_the_design() {
    let mut design = editable_design("#123456", "#ffffff");
    let before = serde_json::to_value(&design).unwrap();
    design.tokens.get_mut("color.ink").unwrap().value =
        merman_theme_contract::ThemeDesignTokenValueV1::Color(
            merman_theme_contract::ThemeValueV1::Literal("#abcdef".into()),
        );
    let spec = serde_json::to_value(materialize_theme_design(&design).unwrap()).unwrap();
    let rules = spec["styles"].as_array().unwrap();
    assert_eq!(rules.len(), 2);
    assert!(
        rules
            .iter()
            .all(|rule| rule["style"]["fill"]["color"] == "#abcdef")
    );
    let after = serde_json::to_value(&design).unwrap();
    assert_eq!(
        before["styles"], after["styles"],
        "bindings survive materialization and token edits"
    );
}

#[test]
fn complete_snapshot_can_cold_start_without_the_editable_design() {
    let design = editable_design("#dc2626", "#ffffff");
    let spec = materialize_theme_design(&design).unwrap();
    let raw = serde_json::to_vec(&ThemeRecipeV1::CompleteSpec {
        complete_spec: spec,
    })
    .unwrap();
    drop(design);
    let recipe = serde_json::from_slice(&raw).unwrap();
    let cold = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
    let svg = render_svg(
        &Renderer::new(),
        "flowchart LR\nA[Alpha] --> B[Beta]\n",
        &cold,
    );
    assert!(svg.contains("fill:#dc2626"), "{svg}");
}

fn property<'a>(node: roxmltree::Node<'a, '_>, key: &str) -> Option<&'a str> {
    node.attribute("style")
        .and_then(|style| {
            style
                .split(';')
                .filter_map(|part| part.split_once(':'))
                .filter(|(name, _)| name.trim() == key)
                .map(|(_, value)| value.trim())
                .next_back()
        })
        .or_else(|| node.attribute(key))
        .map(|value| value.strip_suffix("!important").unwrap_or(value).trim())
}

#[test]
fn scoped_design_styles_and_palettes_reach_representative_family_writers() {
    let design = serde_json::from_value(json!({
        "tokens": {"size.task-radius":{"type":"length","value":{"px":7}}},
        "rules": [
            {"target":"edge","family":"class","style":{"stroke":{"paint":"#123456"}}},
            {"target":"task","family":"gantt","style":{"fill":"#345678","radius":{"ref":"size.task-radius"}}}
        ],
        "palettes": {"pie-slice":["#12ab34","#3456de"]}
    })).unwrap();
    let theme = compile_design(design);
    let renderer = Renderer::new();
    let class = render_svg(&renderer, "classDiagram\nA *-- B\n", &theme);
    let class = roxmltree::Document::parse(&class).unwrap();
    let relation = class
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert_eq!(property(relation, "stroke"), Some("#123456"));
    let marker_id = relation
        .attribute("marker-start")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let marker = class
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
        .and_then(|marker| marker.descendants().find(|node| node.has_tag_name("path")))
        .unwrap();
    assert_eq!(property(marker, "stroke"), Some("#123456"));
    assert_eq!(property(marker, "fill"), Some("#123456"));

    let gantt = render_svg(
        &renderer,
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d\n",
        &theme,
    );
    let gantt = roxmltree::Document::parse(&gantt).unwrap();
    let task = gantt
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|value| {
                    value.split_ascii_whitespace().any(|class| class == "task")
                })
        })
        .unwrap();
    assert_eq!(property(task, "fill"), Some("#345678"));
    assert_eq!(task.attribute("rx"), Some("7"));
    assert_eq!(task.attribute("ry"), Some("7"));

    let pie = render_svg(&renderer, "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n", &theme);
    let pie = roxmltree::Document::parse(&pie).unwrap();
    let slices = pie
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|value| {
                    value
                        .split_ascii_whitespace()
                        .any(|class| class == "pieCircle")
                })
        })
        .map(|node| node.attribute("fill").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(slices, ["#12ab34", "#3456de"]);
    let swatches = pie
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .parent()
                    .is_some_and(|parent| parent.attribute("class") == Some("legend"))
        })
        .collect::<Vec<_>>();
    assert_eq!(swatches.len(), 2);
    for (swatch, expected) in swatches
        .into_iter()
        .zip(["rgb(18, 171, 52)", "rgb(52, 86, 222)"])
    {
        assert_eq!(property(swatch, "fill"), Some(expected));
        assert_eq!(property(swatch, "stroke"), Some(expected));
    }
}

#[test]
fn rust_facade_exposes_shared_semantic_support_discovery() {
    let support = describe_theme_support(&ThemeSupportQueryV1::for_target(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Node.id(),
        ThemeRuleFacetV1::Radius,
    ));
    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(support.query().family_id(), "flowchart");
}

#[test]
fn rust_facade_preserves_materialization_and_compilation_error_sources() {
    use std::error::Error;
    let compiler = DiagramThemeCompiler::new();
    let missing: ThemeDesignV1 =
        serde_json::from_value(json!({"styles":{"node":{"fill":{"ref":"missing"}}}})).unwrap();
    let error = compiler
        .compile_recipe(ThemeRecipeV1::Design { design: missing })
        .unwrap_err();
    assert!(error.source().is_some());
    let ThemeDesignCompileError::Materialization(error) = error else {
        panic!("materialization error required");
    };
    assert_eq!(error.diagnostic().path(), "/styles/node/fill");

    let invalid_target =
        serde_json::from_value(json!({"styles":{"future-node":{"fill":"#123456"}}})).unwrap();
    let error = compiler
        .compile_recipe(ThemeRecipeV1::Design {
            design: invalid_target,
        })
        .unwrap_err();
    assert!(error.source().is_some());
    assert!(matches!(error, ThemeDesignCompileError::Compilation(_)));
}

#[test]
fn rust_facade_applies_caller_owned_input_admission_before_resolving_references() {
    let resources = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
        .unwrap();
    let compiler = DiagramThemeCompiler::new().with_resource_policy(resources);
    let design = editable_design("#123456", "#ffffff");
    let error = compiler
        .compile_recipe(ThemeRecipeV1::Design { design })
        .unwrap_err();
    let ThemeDesignCompileError::Materialization(error) = error else {
        panic!("input admission must precede compilation");
    };
    assert_eq!(
        error.diagnostic().limit_id(),
        Some("max_theme_encoded_bytes")
    );
    assert_eq!(error.diagnostic().actual(), Some(2));
    assert_eq!(error.diagnostic().max(), Some(1));
}
