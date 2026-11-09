#![cfg(all(feature = "native-fonts", feature = "diagram-git-graph"))]

use merman::svg::SvgRenderOptions;
use merman::svg::export::{NativeFontContext, NativeFontOptions};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgEnvironment, SvgOutput, SvgRequest,
};
use serde_json::{Value, json};

fn context() -> NativeFontContext {
    NativeFontOptions::default()
        .with_font_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../merman-export/tests/fonts/Cousine-Regular.ttf"),
        )
        .load(&OperationControl::new())
        .unwrap()
}

fn renderer(mut config: Value) -> Renderer {
    config["themeVariables"]["fontFamily"] = json!("Cousine");
    Renderer::new().with_presentation_engine(
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(config)),
    )
}

fn request(context: &NativeFontContext) -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context.clone())
            .unwrap(),
        options: SvgRenderOptions {
            diagram_id: Some("git-native-size".to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn svg(output: RenderOutput) -> SvgOutput {
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected GitGraph SVG");
    };
    output
}

fn geometry(svg: &str) -> Vec<(String, Vec<(String, String)>)> {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.is_element())
        .map(|node| {
            (
                node.tag_name().name().to_owned(),
                node.attributes()
                    .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
}

#[test]
fn native_gitgraph_absolute_role_sizes_have_identical_final_geometry() {
    let context = context();
    let source = "gitGraph\ncommit id:\"MMMMMMMM\" tag:\"MMMMMMMM\"\n";
    for property in ["commitLabelFontSize", "tagLabelFontSize"] {
        let render = |size: &str| {
            svg(renderer(json!({
                "gitGraph": {"showBranches": false, "rotateCommitLabel": false},
                "themeVariables": {property: size}
            }))
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                request(&context),
            ))
            .unwrap_or_else(|error| panic!("{property}/{size}: {error}")))
        };
        let expected = render("32px");
        let expected = geometry(expected.svg());
        assert_ne!(geometry(render("10px").svg()), expected, "{property}");
        for size in ["24pt", r"24p\74 !IMPORTANT"] {
            assert_eq!(geometry(render(size).svg()), expected, "{property}/{size}");
        }
    }
}

#[test]
fn native_gitgraph_rejects_only_consumed_unresolved_role_sizes() {
    let context = context();
    for (property, source, show_commit_label) in [
        ("commitLabelFontSize", "gitGraph\ncommit id:\"A\"\n", true),
        (
            "tagLabelFontSize",
            "gitGraph\ncommit id:\"A\" tag:\"v1\"\n",
            false,
        ),
    ] {
        for size in ["calc(0px)", "1rem", "var(--role-size)", "1e-999px"] {
            let error = renderer(json!({
                "gitGraph": {"showBranches": false, "showCommitLabel": show_commit_label},
                "themeVariables": {property: size}
            }))
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                request(&context),
            ))
            .expect_err("unresolved consumed role must fail before measurement");
            assert!(
                error
                    .to_string()
                    .contains(&format!("themeVariables.{property} (unresolved CSS value)")),
                "{error}"
            );
        }
    }
    for (source, show_commit_label, commit_size, tag_size) in [
        ("gitGraph\n", true, "calc(0px)", "1rem"),
        ("gitGraph\ncommit id:\"A\"\n", false, "calc(0px)", "1rem"),
        (
            "gitGraph\ncommit id:\"A\" tag:\"   \"\n",
            false,
            "calc(0px)",
            "1rem",
        ),
        (
            "gitGraph\ncommit id:\"A\" tag:\"v1\"\n",
            true,
            "0rem",
            "0em",
        ),
        (
            "gitGraph\ncommit id:\"A\" tag:\"v1\"\n",
            true,
            "inherit",
            "150%",
        ),
    ] {
        svg(renderer(json!({
            "gitGraph": {"showBranches": false, "showCommitLabel": show_commit_label},
            "themeVariables": {"fontSize": "0px", "commitLabelFontSize": commit_size, "tagLabelFontSize": tag_size}
        })).render(RenderRequest::svg(source, OperationControl::new(), request(&context)))
            .unwrap_or_else(|error| panic!("unused/known-zero role: {source:?}: {error}")));
    }
}

#[test]
fn native_gitgraph_invalid_role_sizes_inherit_the_actual_base_font() {
    let context = context();
    for (property, source, show_commit_label) in [
        (
            "commitLabelFontSize",
            "gitGraph\ncommit id:\"MMMMMMMM\"\n",
            true,
        ),
        (
            "tagLabelFontSize",
            "gitGraph\ncommit id:\"A\" tag:\"MMMMMMMM\"\n",
            false,
        ),
    ] {
        for base in ["0px", "29px"] {
            let render = |size: &str| {
                svg(renderer(json!({
                    "gitGraph": {"showBranches": false, "showCommitLabel": show_commit_label, "rotateCommitLabel": false},
                    "themeVariables": {"fontSize": base, property: size}
                })).render(RenderRequest::svg(source, OperationControl::new(), request(&context)))
                    .unwrap_or_else(|error| panic!("{property}/{base}/{size}: {error}")))
            };
            let inherited = geometry(render("inherit").svg());
            let zero = geometry(render("0px").svg());
            if base == "29px" {
                assert_ne!(
                    inherited, zero,
                    "{property}: fixture must distinguish inherited and zero sizes"
                );
            }
            for size in ["auto", "bogus", "12fr", "0fr", "0futureunit", "0"] {
                let rendered = render(size);
                let expected = if size == "0" { &zero } else { &inherited };
                assert_eq!(
                    &geometry(rendered.svg()),
                    expected,
                    "{property}/{base}/{size}"
                );
            }
        }
    }
}

#[test]
fn native_gitgraph_space_only_tags_guard_raw_svg_geometry() {
    let context = context();
    let render = |tag: &str, size: &str| {
        let source = format!("gitGraph\ncommit id:\"A\" tag:\"{tag}\"\n");
        renderer(json!({
            "gitGraph": {"showBranches": false, "showCommitLabel": false},
            "themeVariables": {"fontSize": "29px", "commitLabelFontSize": "calc(0px)", "tagLabelFontSize": size}
        })).render(RenderRequest::svg(&source, OperationControl::new(), request(&context)))
    };
    for tag in ["\u{00a0}", "\u{2003}"] {
        for size in ["1rem", "calc(0px)"] {
            let error = render(tag, size).expect_err("Unicode spaces consume raw SVG width");
            assert!(
                error
                    .to_string()
                    .contains("themeVariables.tagLabelFontSize (unresolved CSS value)"),
                "{tag:?}/{size}: {error}"
            );
        }
        let zero = svg(render(tag, "0px").unwrap());
        let document = roxmltree::Document::parse(zero.svg()).unwrap();
        assert!(
            document.descendants().any(
                |node| node.attribute("class") == Some("tag-label") && node.text() == Some(tag)
            )
        );
        assert_ne!(
            geometry(svg(render(tag, "16px").unwrap()).svg()),
            geometry(zero.svg()),
            "{tag:?}: space width must affect geometry"
        );
        for size in ["0", "0em", "0rem"] {
            let rendered =
                svg(render(tag, size).unwrap_or_else(|error| panic!("{tag:?}/{size}: {error}")));
            assert_eq!(
                geometry(rendered.svg()),
                geometry(zero.svg()),
                "{tag:?}/{size}"
            );
        }
    }
    for tag in ["", "   "] {
        for size in ["1rem", "calc(0px)"] {
            svg(render(tag, size)
                .unwrap_or_else(|error| panic!("collapsed/empty tag {tag:?}/{size}: {error}")));
        }
    }
}
