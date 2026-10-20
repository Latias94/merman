#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
use std::fs;
#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
use std::path::PathBuf;

#[path = "../benches/support/json_identity.rs"]
mod json_identity;

#[test]
fn pipeline_json_identity_ignores_only_object_member_order() {
    use merman_core::ManagedSemanticJson;

    let first = ManagedSemanticJson::from_value(
        serde_json::from_str(r#"{"z":[{"b":2,"a":1},3],"a":"value"}"#).unwrap(),
    );
    let reordered = ManagedSemanticJson::from_value(
        serde_json::from_str(r#"{"a":"value","z":[{"a":1,"b":2},3]}"#).unwrap(),
    );
    assert_eq!(first, reordered);
    let mut original_bytes = Vec::new();
    first.write_json(&mut original_bytes).unwrap();
    let mut reordered_bytes = Vec::new();
    reordered.write_json(&mut reordered_bytes).unwrap();
    assert_ne!(original_bytes, reordered_bytes);

    let identity = json_identity::json_identity_bytes(&first);
    assert_eq!(identity, json_identity::json_identity_bytes(&reordered));
    for changed in [
        r#"{"z":[3,{"b":2,"a":1}],"a":"value"}"#,
        r#"{"z":[{"b":2,"a":1},3],"a":"other"}"#,
        r#"{"z":[{"b":2,"a":1},3]}"#,
    ] {
        let changed = ManagedSemanticJson::from_value(serde_json::from_str(changed).unwrap());
        assert_ne!(identity, json_identity::json_identity_bytes(&changed));
    }
    let mut unchanged_bytes = Vec::new();
    first.write_json(&mut unchanged_bytes).unwrap();
    assert_eq!(original_bytes, unchanged_bytes);
}

#[cfg(feature = "diagram-state")]
#[test]
fn pipeline_state_identity_preserves_repeated_parse_values() {
    let engine = merman_core::Engine::new();
    let projections = (0..8)
        .map(|_| {
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    include_str!("../benches/fixtures/state_medium.mmd"),
                    merman_core::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            parsed
                .model()
                .compatibility_json(parsed.metadata())
                .unwrap()
        })
        .collect::<Vec<_>>();
    for projection in &projections[1..] {
        assert_eq!(projections[0], *projection);
    }
    let identity = json_identity::json_identity_bytes(&projections[0]);
    for projection in &projections[1..] {
        assert!(
            identity == json_identity::json_identity_bytes(projection),
            "equal State projection values must have the same benchmark identity"
        );
    }
}

#[test]
fn pipeline_json_identity_preserves_deep_managed_export() {
    let mut value = serde_json::from_str(r#"{"z":1,"a":2}"#).unwrap();
    let depth = 2048;
    for _ in 0..depth {
        value = serde_json::Value::Array(vec![value]);
    }
    let value = merman_core::ManagedSemanticJson::from_value(value);
    let mut expected = vec![b'['; depth];
    expected.extend_from_slice(br#"{"a":2,"z":1}"#);
    expected.extend(std::iter::repeat_n(b']', depth));
    assert!(json_identity::json_identity_bytes(&value) == expected);
}

#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
fn assert_no_empty_style_elements(name: &str, svg: &str) {
    let mut cursor = 0;
    while let Some(rel_start) = svg[cursor..].find("<style") {
        let tag_start = cursor + rel_start;
        let Some(rel_tag_end) = svg[tag_start..].find('>') else {
            panic!("{name}: malformed style start tag");
        };
        let content_start = tag_start + rel_tag_end + 1;
        let Some(rel_close) = svg[content_start..].find("</style>") else {
            panic!("{name}: malformed style element");
        };
        let content_end = content_start + rel_close;
        assert!(
            !svg[content_start..content_end].trim().is_empty(),
            "{name}: rendered SVG contains an empty <style> element"
        );
        cursor = content_end + "</style>".len();
    }
}

#[test]
#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
fn pipeline_bench_fixtures_are_benchmarkable() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("fixtures");
    let mut fixtures = fs::read_dir(&fixtures_dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", fixtures_dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("read {} entry: {err}", fixtures_dir.display()))
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mmd"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(!fixtures.is_empty(), "no pipeline bench fixtures found");

    let engine = merman_core::Engine::new();
    let parse_options = merman_core::ParseOptions::strict();
    let layout = merman::svg::LayoutOptions::headless_svg_defaults();
    let environment = merman::SvgEnvironment::deterministic()
        .with_resource_policy(merman::svg::RenderResourcePolicy::unbounded_for_trusted_input());

    for path in fixtures {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("<invalid fixture name>")
            .to_string();
        let input = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{name}: read {}: {err}", path.display()));

        let metadata = engine
            .parse_metadata_sync(&input)
            .unwrap_or_else(|err| panic!("{name}: metadata parse failed: {err}"));

        engine
            .parse_diagram_with_type_sync(&metadata.diagram_type, &input, parse_options)
            .unwrap_or_else(|err| {
                panic!(
                    "{name}: known-type parse failed for {}: {err}",
                    metadata.diagram_type
                )
            })
            .unwrap_or_else(|| panic!("{name}: known-type parser returned no diagram"));

        let parsed = engine
            .parse_diagram_for_render_model_sync(&input, parse_options)
            .unwrap_or_else(|err| panic!("{name}: render-model parse failed: {err}"))
            .unwrap_or_else(|| panic!("{name}: render-model parser returned no diagram"));

        let svg_options = merman::svg::SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(&name)),
            ..Default::default()
        };
        let renderer = merman::Renderer::new()
            .with_engine(engine.clone())
            .with_resource_policy(merman_core::resources::InputResourcePolicy::for_profile(
                merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
            ));
        let layout_json = match renderer
            .render(merman::RenderRequest::layout_json(
                &input,
                merman::OperationControl::new(),
                merman::SvgRequest {
                    environment: environment.clone(),
                    layout: layout.clone(),
                    options: svg_options.clone(),
                    ..Default::default()
                },
            ))
            .unwrap_or_else(|err| panic!("{name}: layout JSON failed: {err}"))
        {
            merman::RenderOutput::LayoutJson(Some(output)) => output.layout().clone(),
            merman::RenderOutput::LayoutJson(None) => {
                panic!("{name}: layout JSON returned no diagram")
            }
            other => panic!("{name}: unexpected layout target output: {other:?}"),
        };
        assert!(
            layout_json.is_object(),
            "{name}: layout JSON must be an object"
        );
        let svg = match renderer
            .render(merman::RenderRequest::svg(
                &input,
                merman::OperationControl::new(),
                merman::SvgRequest {
                    environment: environment.clone(),
                    layout: layout.clone(),
                    options: svg_options,
                    ..Default::default()
                },
            ))
            .unwrap_or_else(|err| panic!("{name}: end-to-end SVG render failed: {err}"))
        {
            merman::RenderOutput::Svg(Some(output)) => output.svg().to_owned(),
            merman::RenderOutput::Svg(None) => panic!("{name}: render returned no SVG"),
            other => panic!("{name}: unexpected SVG target output: {other:?}"),
        };

        assert!(
            !svg.is_empty(),
            "{name}: render returned an empty SVG for {:?}",
            parsed.metadata().diagram_type
        );
        let document = roxmltree::Document::parse(&svg)
            .unwrap_or_else(|err| panic!("{name}: rendered SVG is invalid XML: {err}"));
        assert_eq!(
            document.root_element().tag_name().name(),
            "svg",
            "{name}: rendered output root must be svg"
        );
        assert!(
            document.descendants().any(|node| node.is_element()),
            "{name}: rendered SVG must contain elements"
        );
        assert_no_empty_style_elements(&name, &svg);
    }
}
