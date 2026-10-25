use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};
use svgtypes::Transform;

fn process(svg: &str, pipeline: SvgPipeline) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    pipeline.process_to_string(svg, &session).unwrap()
}

fn coordinate(element: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    element.attribute(attribute).unwrap().parse().unwrap()
}

fn mapped(transform: Transform, x: f64, y: f64) -> (f64, f64) {
    (
        transform.a * x + transform.c * y + transform.e,
        transform.b * x + transform.d * y + transform.f,
    )
}

fn assert_point(actual: (f64, f64), expected: (f64, f64)) {
    assert!(
        (actual.0 - expected.0).abs() < 1e-8,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.1 - expected.1).abs() < 1e-8,
        "{actual:?} != {expected:?}"
    );
}

fn mapped_through_ancestors(element: roxmltree::Node<'_, '_>, mut point: (f64, f64)) -> (f64, f64) {
    if element.attribute("data-merman-foreignobject") == Some("fallback") {
        let instance = element
            .children()
            .find(|node| node.has_tag_name("use"))
            .unwrap();
        point.0 += coordinate(instance, "x");
        point.1 += coordinate(instance, "y");
    }
    for ancestor in element.ancestors().filter(|node| node.is_element()) {
        if let Some(raw) = ancestor.attribute("transform") {
            // Invalid SVG transform attributes are ignored as a whole.
            if let Ok(transform) = raw.parse::<Transform>() {
                point = mapped(transform, point.0, point.1);
            }
        }
    }
    point
}

fn content<'a, 'input>(overlay: roxmltree::Node<'a, 'input>) -> roxmltree::Node<'a, 'input> {
    let instance = overlay
        .children()
        .find(|node| node.has_tag_name("use"))
        .unwrap();
    let id = instance
        .attribute("href")
        .unwrap()
        .strip_prefix('#')
        .unwrap();
    overlay
        .document()
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap()
}

fn has_style(node: roxmltree::Node<'_, '_>, expected: &str) -> bool {
    let expected = expected.split_whitespace().collect::<String>();
    node.attribute("style")
        .unwrap()
        .split_whitespace()
        .collect::<String>()
        .contains(&expected)
}

#[test]
fn readable_fallback_preserves_transform_order_and_maps_background_and_text_together() {
    for (transform, expected) in [
        ("translate(10 20)", (35.0, 37.0)),
        (
            "translate(80 90) rotate(-90) translate(-20 -10)",
            (87.0, 85.0),
        ),
        ("rotate(90 10 20)", (13.0, 35.0)),
        ("rotate(&#x2d;90)", (17.0, -25.0)),
        ("translate(10 20) scale(2 3)", (60.0, 71.0)),
        ("scale(-2 3) translate(10 20)", (-70.0, 111.0)),
        ("matrix(2 1 -1 3 10 20)", (43.0, 96.0)),
        ("skewX(45) skewY(-45)", (17.0, -8.0)),
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g class="owner" transform='{transform}'><foreignObject x="5" y="7" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="background-color:#c0ffee"><p>Title</p></div></foreignObject></g></svg>"#
        );
        let output = process(&svg, SvgPipeline::readable());
        let document = roxmltree::Document::parse(&output).unwrap();
        let overlay = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        assert_eq!(overlay.parent().unwrap().attribute("class"), Some("owner"));
        assert_eq!(
            overlay.attribute("class"),
            Some("merman-foreignobject-fallback")
        );
        assert_eq!(
            overlay.attribute("data-merman-source-classes"),
            Some("owner labelBkg")
        );
        assert!(overlay.attribute("transform").is_none());
        let text = content(overlay)
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        let background = content(overlay)
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") != Some("none"))
            .unwrap();
        assert_point((coordinate(text, "x"), coordinate(text, "y")), (20.0, 10.0));
        assert_point(
            mapped_through_ancestors(overlay, (coordinate(text, "x"), coordinate(text, "y"))),
            expected,
        );
        assert_point(
            (coordinate(background, "x"), coordinate(background, "y")),
            (0.0, 0.0),
        );
        assert_eq!(coordinate(background, "width"), 40.0);
        assert_eq!(coordinate(background, "height"), 20.0);
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
    }
}

#[test]
fn root_non_group_ancestors_and_foreign_object_transforms_apply_once() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Diagram" viewBox="0 0 300 200" transform="translate(100 200)"><a class="owner" transform="translate(10 20)"><switch transform="scale(2 3)"><g transform="rotate(-90)"><foreignObject transform="translate(5 7)" x="3" y="4" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" class="nodeLabel"><p>Title</p></div></foreignObject></g></switch></a></svg>"#;
    let output = process(svg, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    assert_eq!(document.root_element().attribute("role"), Some("img"));
    assert_eq!(
        document.root_element().attribute("aria-label"),
        Some("Diagram")
    );
    let overlay = document
        .descendants()
        .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .unwrap();
    assert!(overlay.parent().unwrap().has_tag_name("g"));
    let local: Transform = overlay.attribute("transform").unwrap().parse().unwrap();
    assert_point(mapped(local, 23.0, 14.0), (28.0, 21.0));
    assert_point(
        mapped_through_ancestors(overlay, (20.0, 10.0)),
        (152.0, 136.0),
    );
    assert!(!output.contains("<foreignObject"));
    assert_eq!(
        overlay.attribute("data-merman-source-classes"),
        Some("owner nodeLabel")
    );
}

#[test]
fn nested_svg_viewports_preserve_aspect_ratio_and_parent_user_units() {
    for (aspect, expected) in [
        ("none", (80.0, 70.0)),
        ("xMidYMid meet", (110.0, 70.0)),
        ("xMaxYMin slice", (80.0, 80.0)),
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300" viewBox="0 0 400 300"><svg x="10%" y="20%" width="200" height="100" viewBox="10 20 100 100" preserveAspectRatio="{aspect}"><foreignObject x="10" y="20" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Title</div></foreignObject></svg></svg>"#
        );
        let output = process(&svg, SvgPipeline::resvg_safe());
        let document = roxmltree::Document::parse(&output).unwrap();
        let overlay = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        let viewport = overlay.parent().unwrap();
        assert!(viewport.has_tag_name("svg"));
        assert_eq!(viewport.attribute("x"), Some("10%"));
        assert_eq!(viewport.attribute("y"), Some("20%"));
        assert_eq!(viewport.attribute("viewBox"), Some("10 20 100 100"));
        assert_eq!(viewport.attribute("preserveAspectRatio"), Some(aspect));
        let text = content(overlay)
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_point((coordinate(text, "x"), coordinate(text, "y")), (20.0, 10.0));
        // Chromium getCTM evidence for this unchanged source viewport.
        let mapping: Transform = match aspect {
            "none" => "matrix(2 0 0 1 20 40)",
            "xMidYMid meet" => "matrix(1 0 0 1 80 40)",
            _ => "matrix(2 0 0 2 20 20)",
        }
        .parse()
        .unwrap();
        assert_point(mapped(mapping, 30.0, 30.0), expected);
    }
}

#[test]
fn transformed_switch_native_fallback_is_kept_without_a_generated_duplicate() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><clipPath id="clip"><rect width="100" height="100"/></clipPath><mask id="mask"><rect width="100" height="100" fill="white"/></mask></defs><g transform="translate(20 30) rotate(-90)"><switch id="native-label" transform="scale(2 3)" opacity="0.5" clip-path="url(#clip)" mask="url(#mask)" role="img" aria-label="Native title" requiredFeatures="unsupported" systemLanguage="other"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Native</div></foreignObject><text x="20" y="10">Native</text></switch></g></svg>"##;
    let output = process(svg, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    let text = document
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .count(),
        1
    );
    let group = text.parent().unwrap();
    assert_eq!(group.attribute("id"), Some("native-label"));
    assert_eq!(group.attribute("role"), Some("img"));
    assert_eq!(group.attribute("aria-label"), Some("Native title"));
    assert_eq!(group.attribute("opacity"), Some("0.5"));
    assert_eq!(group.attribute("clip-path"), Some("url(#clip)"));
    assert_eq!(group.attribute("mask"), Some("url(#mask)"));
    assert_eq!(group.attribute("transform"), Some("scale(2 3)"));
    assert_eq!(
        group.parent().unwrap().attribute("transform"),
        Some("translate(20 30) rotate(-90)")
    );
    assert!(group.attribute("requiredFeatures").is_none());
    assert!(group.attribute("systemLanguage").is_none());
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some("native-label"))
            .count(),
        1
    );
    assert!(!output.contains("data-merman-foreignobject=\"fallback\""));
    assert!(!output.contains("<switch"));
}

#[test]
fn swimlane_issue_200_export_keeps_lane_titles_rotated_and_centered() {
    let source = include_str!("../../../fixtures/swimlane/upstream_docs_basic.mmd");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let parity = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_string();
    let output = process(&parity, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    let reference = roxmltree::Document::parse(&parity).unwrap();
    for title in ["Customer", "Support", "Engineering"] {
        let source = reference
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .filter_map(|node| node.text())
                        .any(|text| text == title)
            })
            .unwrap();
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some(title))
            .unwrap();
        let definition = text
            .ancestors()
            .find(|node| node.attribute("id").is_some())
            .unwrap();
        let href = format!("#{}", definition.attribute("id").unwrap());
        let overlay = document
            .descendants()
            .find(|node| node.has_tag_name("use") && node.attribute("href") == Some(href.as_str()))
            .unwrap()
            .parent()
            .unwrap();
        assert!(overlay.attribute("transform").is_none());
        let source_transform: Transform = source
            .parent()
            .unwrap()
            .attribute("transform")
            .unwrap()
            .parse()
            .unwrap();
        assert!(source_transform.a.abs() < 1e-8 && source_transform.d.abs() < 1e-8);
        assert!((source_transform.b + 1.0).abs() < 1e-8 && (source_transform.c - 1.0).abs() < 1e-8);
        assert_point(
            mapped_through_ancestors(overlay, (coordinate(text, "x"), coordinate(text, "y"))),
            mapped_through_ancestors(
                source,
                (
                    coordinate(source, "width") / 2.0,
                    coordinate(source, "height") / 2.0,
                ),
            ),
        );
        assert!(
            overlay
                .attribute("data-merman-source-classes")
                .unwrap()
                .contains("swimlane-label")
        );
    }
    assert!(!output.contains("<foreignObject"));
}

#[test]
fn swimlane_long_html_edge_label_resvg_safe_keeps_all_wrapped_text_visible() {
    for (html, expected_text, expected_lines) in [
        (
            "First <b>review</b> the complete request before sending the final <i>reply</i>",
            "First review the complete request before sending the final reply",
            None,
        ),
        ("<b>Ready</b>", "Ready", Some(1)),
        ("First<br/>last", "First last", Some(2)),
        (
            "Supercalifragilisticexpialidocious",
            "Supercalifragilisticexpialidocious",
            Some(1),
        ),
    ] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 22px
  flowchart:
    wrappingWidth: 96
---
swimlane-beta LR
A -->|{html}| B
"#
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let artifact = family::prepare(
            parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let layout: merman_render::model::SwimlaneLayout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
        )
        .unwrap();
        let label_id = layout.edges[0].label_node_id.as_deref().unwrap();
        let parity = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .svg()
            .to_string();
        let output = process(&parity, SvgPipeline::resvg_safe());
        let document = roxmltree::Document::parse(&output).unwrap();
        let label = document
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("label edgeLabel")
                    && node.attribute("id") == Some(label_id)
            })
            .unwrap();
        let overlay = label
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        let definition = content(overlay);
        let lines = definition
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        if let Some(expected_lines) = expected_lines {
            assert_eq!(lines.len(), expected_lines, "{html}");
        } else {
            assert!(
                lines.len() > 1,
                "the exported long HTML edge label must actually wrap into multiple SVG text lines"
            );
        }
        let text = lines
            .iter()
            .map(|line| {
                line.descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            text.split_whitespace().collect::<Vec<_>>().join(" "),
            expected_text
        );
        let viewport = definition
            .children()
            .find(|node| node.has_tag_name("svg"))
            .unwrap();
        assert!(has_style(viewport, "overflow: visible"));
        let root_viewbox = document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        for line in lines {
            assert!(has_style(line, "font-size: 22px"));
            let y = coordinate(line, "y");
            assert!(y >= 11.0 && y + 11.0 <= coordinate(viewport, "height"));
            let (_, root_y) = mapped_through_ancestors(overlay, (coordinate(line, "x"), y));
            assert!(root_y - 11.0 >= root_viewbox[1]);
            assert!(root_y + 11.0 <= root_viewbox[1] + root_viewbox[3]);
        }
        assert!(!output.contains("<foreignObject"));
    }
}

#[test]
fn overflowing_fallback_transforms_fail_with_a_named_pipeline_error() {
    for transform in ["scale(1e308) scale(1e308)", "scale(1e308)"] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="{transform}"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Title</div></foreignObject></g></svg>"#
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = SvgPipeline::resvg_safe()
            .process_to_string(&svg, &session)
            .unwrap_err();
        assert!(
            matches!(error, merman_render::Error::SvgPostprocess { ref pass, .. } if pass == "foreign-object-fallback"),
            "{error}"
        );
    }
}

#[test]
fn invalid_transform_syntax_is_ignored_as_a_whole_before_resvg_sanitizing() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(50 60) rotate(nope)"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Title</div></foreignObject></g></svg>"#;
    let output = process(svg, SvgPipeline::readable());
    let document = roxmltree::Document::parse(&output).unwrap();
    let overlay = document
        .descendants()
        .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .unwrap();
    assert!(overlay.attribute("transform").is_none());
    let text = content(overlay)
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert_point((coordinate(text, "x"), coordinate(text, "y")), (20.0, 10.0));
}

#[test]
fn fallback_retains_source_paint_context_and_resolves_fo_paint_without_live_selectors() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>#html{opacity:0.4;clip-path:url(#clip);mask:url(#mask);visibility:hidden}.node text{fill:red!important;font-size:90px!important}.node rect{fill:red!important}</style><defs><clipPath id="clip" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath><mask id="mask" maskContentUnits="objectBoundingBox"><rect width="1" height="1" fill="white"/></mask></defs><g id="owner" class="node" opacity="0.5" display="none" transform="translate(20 30)"><rect x="100" width="10" height="20"/><foreignObject id="html" class="source-label" role="img" aria-label="A &amp; B" transform="rotate(90)" x="5" y="7" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="background-color:#c0ffee;color:#123456">Title</div></foreignObject></g></svg>"##;
    let output = process(svg, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    let overlay = document
        .descendants()
        .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .unwrap();
    let owner = overlay.parent().unwrap();
    assert_eq!(owner.attribute("id"), Some("owner"));
    assert_eq!(owner.attribute("opacity"), Some("0.5"));
    assert_eq!(owner.attribute("display"), Some("none"));
    assert!(
        owner
            .children()
            .any(|node| node.has_tag_name("rect") && node.attribute("x") == Some("100"))
    );
    let style = overlay.attribute("style").unwrap();
    for property in [
        "opacity: 0.4",
        "clip-path: url(#clip)",
        "mask: url(#mask)",
        "visibility: hidden",
    ] {
        assert!(has_style(overlay, property), "{style}");
    }
    assert_eq!(overlay.attribute("role"), Some("img"));
    assert_eq!(overlay.attribute("aria-label"), Some("A & B"));
    assert!(overlay.attribute("id").is_none());
    assert_eq!(
        overlay.attribute("class"),
        Some("merman-foreignobject-fallback")
    );
    assert!(
        overlay
            .attribute("data-merman-source-classes")
            .unwrap()
            .contains("source-label")
    );
    let text = content(overlay)
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert!(has_style(text, "fill: #123456"));
    assert!(has_style(text, "font-size: 16px"));
    let background = content(overlay)
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#c0ffee"))
        .unwrap();
    assert!(has_style(background, "fill: #c0ffee"));
    let viewport = content(overlay)
        .children()
        .find(|node| node.has_tag_name("svg"))
        .unwrap();
    assert_eq!(viewport.attribute("viewBox"), Some("0 0 40 20"));
    let instance = overlay
        .children()
        .find(|node| node.has_tag_name("use"))
        .unwrap();
    assert_eq!(
        (instance.attribute("x"), instance.attribute("y")),
        (Some("5"), Some("7"))
    );
    assert!(has_style(viewport, "overflow: hidden"));
    let bounds = viewport
        .children()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("none"))
        .unwrap();
    assert_eq!(bounds.attribute("x"), Some("0"));
    assert_eq!(bounds.attribute("width"), Some("40"));
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("clipPathUnits") == Some("objectBoundingBox"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("maskContentUnits") == Some("objectBoundingBox"))
    );
}

#[test]
fn fallback_preserves_nested_viewport_clipping_and_explicit_visible_overflow() {
    for (overflow, expected) in [("", "hidden"), ("overflow=\"visible\"", "visible")] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300"><svg id="viewport" x="10" y="20" width="100" height="50" viewBox="0 0 100 50" overflow="hidden"><foreignObject width="80" height="24" {overflow}><div xmlns="http://www.w3.org/1999/xhtml">Title</div></foreignObject></svg></svg>"#
        );
        let output = process(&svg, SvgPipeline::resvg_safe());
        let document = roxmltree::Document::parse(&output).unwrap();
        let overlay = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        assert_eq!(overlay.parent().unwrap().attribute("id"), Some("viewport"));
        assert_eq!(
            overlay.parent().unwrap().attribute("overflow"),
            Some("hidden")
        );
        let viewport = content(overlay)
            .children()
            .find(|node| node.has_tag_name("svg"))
            .unwrap();
        assert!(has_style(viewport, &format!("overflow: {expected}")));
    }
}

#[test]
fn duplicate_drop_preserves_ordinary_labels_without_proven_switch_origin() {
    for (native, fo_x, expected_fallbacks) in [
        (
            r#"<text x="60" y="10" text-anchor="middle" dominant-baseline="central">Same</text>"#,
            40,
            1,
        ),
        (
            r#"<text x="60" y="10" text-anchor="middle" dominant-baseline="central">Same</text>"#,
            80,
            1,
        ),
        (
            r#"<g><text x="60" y="10" text-anchor="middle" dominant-baseline="central">Same</text></g>"#,
            40,
            1,
        ),
        (
            r#"<text x="60" y="10" text-anchor="middle">Same</text>"#,
            40,
            1,
        ),
        (
            r#"<text x="60" y="10" transform="translate(100)" text-anchor="middle" dominant-baseline="central">Same</text>"#,
            40,
            1,
        ),
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g>{native}<foreignObject x="{fo_x}" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Same</div></foreignObject></g></svg>"#
        );
        let output = process(
            &svg,
            SvgPipeline::resvg_safe().with_drop_native_duplicate_fallbacks(true),
        );
        assert_eq!(
            output
                .matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            expected_fallbacks,
            "{output}"
        );
    }
}

#[test]
fn actual_root_ctm_and_measured_text_extent_are_checked_before_emission() {
    for (root, dimensions, label) in [
        (
            r#"transform="scale(1e308)""#,
            r#"width="40" height="20""#,
            "Title",
        ),
        (
            r#"width="1e308" height="1e308" viewBox="0 0 1 1""#,
            r#"width="40" height="20""#,
            "Title",
        ),
        (
            r#"transform="scale(1e308)""#,
            r#"width="1" height="1""#,
            "Title",
        ),
        (
            r#"transform="scale(1e308)""#,
            r#"width="1" height="1""#,
            "A<br/>B",
        ),
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" {root}><foreignObject {dimensions}><div xmlns="http://www.w3.org/1999/xhtml">{label}</div></foreignObject></svg>"#
        );
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = SvgPipeline::resvg_safe()
            .process_to_string(&svg, &session)
            .unwrap_err();
        assert!(
            matches!(error, merman_render::Error::SvgPostprocess { ref pass, .. } if pass == "foreign-object-fallback"),
            "{error}"
        );
    }
}

#[test]
fn native_duplicates_do_not_remove_foreign_object_background_or_visible_label() {
    for native_style in ["", "display:none", "visibility:hidden", "opacity:0"] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g><text x="20" y="10" text-anchor="middle" dominant-baseline="central" style="{native_style}">Same</text><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:red">Same</div></foreignObject></g></svg>"#
        );
        let output = process(
            &svg,
            SvgPipeline::resvg_safe().with_drop_native_duplicate_fallbacks(true),
        );
        let document = roxmltree::Document::parse(&output).unwrap();
        let overlay = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        let definition = content(overlay);
        assert!(
            definition
                .descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("red"))
        );
        assert!(
            definition
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("Same"))
        );
    }
}

#[test]
fn foreign_object_css_transform_wins_over_transform_attribute_without_double_application() {
    for stylesheet in [false, true] {
        let (style, inline) = if stylesheet {
            ("<style>#html{transform:matrix(1,0,0,1,80,60)}</style>", "")
        } else {
            ("", "style=\"transform:matrix(1,0,0,1,80,60)\"")
        };
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">{style}<foreignObject id="html" transform="translate(5 7)" width="40" height="20" {inline}><div xmlns="http://www.w3.org/1999/xhtml">Same</div></foreignObject></svg>"#
        );
        let output = process(&svg, SvgPipeline::resvg_safe());
        let document = roxmltree::Document::parse(&output).unwrap();
        let overlay = document
            .descendants()
            .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
            .unwrap();
        assert!(has_style(overlay, "transform: matrix(1,0,0,1,80,60)"));
        assert!(!has_style(overlay, "transform: matrix(1 0 0 1 5 7)"));
        assert_eq!(
            overlay
                .children()
                .filter(|node| node.has_tag_name("use"))
                .count(),
            1
        );
    }
}

#[test]
fn fallback_definitions_are_outside_source_node_css_and_generated_ids_do_not_collide() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.node rect{fill:blue!important;stroke:green!important;stroke-width:10!important}.node text{fill:blue!important}</style><g id="merman-foreignobject-label-0"/><g class="node" opacity="0.5"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:red">Same</div></foreignObject></g></svg>"#;
    let output = process(svg, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    let overlay = document
        .descendants()
        .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .unwrap();
    assert_eq!(overlay.parent().unwrap().attribute("class"), Some("node"));
    assert_eq!(overlay.parent().unwrap().attribute("opacity"), Some("0.5"));
    let definition = content(overlay);
    assert!(definition.parent().unwrap().has_tag_name("defs"));
    assert!(
        definition
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .has_tag_name("svg")
    );
    assert_ne!(
        definition.attribute("id"),
        Some("merman-foreignobject-label-0")
    );
    let mut ids = std::collections::HashSet::new();
    assert!(
        document
            .descendants()
            .filter_map(|node| node.attribute("id"))
            .all(|id| ids.insert(id))
    );
    assert!(
        definition
            .descendants()
            .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("red"))
    );
}

#[test]
fn foreign_object_css_matrix_is_checked_in_the_actual_root_coordinate_system() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" transform="scale(1e308)"><foreignObject width="1" height="1" style="transform:matrix(1,0,0,1,80,60)"><div xmlns="http://www.w3.org/1999/xhtml">Same</div></foreignObject></svg>"#;
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = SvgPipeline::resvg_safe()
        .process_to_string(svg, &session)
        .unwrap_err();
    assert!(
        matches!(error, merman_render::Error::SvgPostprocess { ref pass, .. } if pass == "foreign-object-fallback"),
        "{error}"
    );
}

#[test]
fn valid_css_matrix_can_override_an_overflowing_foreign_object_attribute() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="40" height="20" transform="scale(1e308)" style="transform:matrix(1,0,0,1,0,0)"><div xmlns="http://www.w3.org/1999/xhtml">Same</div></foreignObject></svg>"#;
    let output = process(svg, SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(&output).unwrap();
    let overlay = document
        .descendants()
        .find(|node| node.attribute("data-merman-foreignobject") == Some("fallback"))
        .unwrap();
    assert!(has_style(overlay, "transform:matrix(1,0,0,1,0,0)"));
    assert!(
        content(overlay)
            .descendants()
            .any(|node| node.has_tag_name("text") && node.text() == Some("Same"))
    );
}

#[test]
fn fallback_ids_follow_the_source_root_namespace_when_diagrams_share_a_document() {
    let mut identifiers = std::collections::HashSet::new();
    for root in ["first-diagram", "second-diagram"] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="{root}"><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml">Same</div></foreignObject></svg>"#
        );
        let output = process(&svg, SvgPipeline::resvg_safe());
        let document = roxmltree::Document::parse(&output).unwrap();
        let instance = document
            .descendants()
            .find(|node| node.has_tag_name("use"))
            .unwrap();
        let id = instance
            .attribute("href")
            .unwrap()
            .strip_prefix('#')
            .unwrap();
        assert!(id.starts_with(&format!("{root}-")));
        assert!(
            identifiers.insert(id.to_owned()),
            "separate diagram instances must not share label IDs"
        );
        let target = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        assert!(
            target
                .descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some("Same"))
        );
    }
}
