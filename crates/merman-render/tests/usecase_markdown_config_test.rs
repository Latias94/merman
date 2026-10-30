#![cfg(feature = "diagram-usecase")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(body: &str, width: usize, auto_wrap: bool) -> String {
    let source = format!(
        "---\nconfig:\n  layout: dagre\n  htmlLabels: true\n  markdownAutoWrap: {auto_wrap}\n  usecase:\n    wrappingWidth: {width}\n---\nusecase-beta\n{body}\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn label_geometry(svg: &str) -> Vec<(String, f64, f64, usize)> {
    let document = roxmltree::Document::parse(svg).expect("valid Usecase XHTML");
    document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .filter_map(|node| {
            let text: String = node
                .descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect();
            text.contains("MMMM").then(|| {
                (
                    text,
                    node.attribute("width").unwrap().parse().unwrap(),
                    node.attribute("height").unwrap().parse().unwrap(),
                    node.descendants()
                        .filter(|child| child.has_tag_name("br"))
                        .count(),
                )
            })
        })
        .collect()
}

#[test]
fn usecase_markdown_config_no_wrap_measures_the_nonbreaking_terminal() {
    for body in [
        "actor A(\"`MMMM MMMM MMMM MMMM`\")",
        "A(\"`MMMM MMMM MMMM MMMM`\")",
        "A(\"`**MMMM MMMM** MMMM MMMM`\")",
        "A[\"`MMMM MMMM MMMM MMMM`\"] <<Secure>>",
        "A[Target]\nnote for A \"`MMMM MMMM MMMM MMMM`\"",
    ] {
        let narrow = label_geometry(&render(body, 50, false));
        let wide = label_geometry(&render(body, 1000, false));
        assert_eq!(narrow.len(), 1, "{body}");
        assert_eq!(wide.len(), 1, "{body}");
        assert_eq!(narrow[0].0, wide[0].0);
        assert!(narrow[0].0.contains('\u{a0}'), "{body}: {narrow:?}");
        assert!(
            (narrow[0].1 - wide[0].1).abs() < 0.02,
            "{body}: {narrow:?} vs {wide:?}"
        );
        assert!(
            (narrow[0].2 - wide[0].2).abs() < 0.02,
            "{body}: {narrow:?} vs {wide:?}"
        );
    }
}

#[test]
fn usecase_markdown_config_no_wrap_preserves_authored_breaks() {
    let body = "actor A(\"`MMMM MMMM<br/>MMMM MMMM`\")";
    let narrow = label_geometry(&render(body, 50, false));
    let wide = label_geometry(&render(body, 1000, false));
    assert_eq!(narrow[0].3, 1);
    assert_eq!(narrow[0].0, "MMMM\u{a0}MMMMMMMM\u{a0}MMMM");
    assert!((narrow[0].1 - wide[0].1).abs() < 0.02);
    assert!((narrow[0].2 - wide[0].2).abs() < 0.02);
}

#[test]
fn usecase_markdown_config_also_prepares_boundary_and_relationship_terminals() {
    let text = "MMMM MMMM MMMM MMMM MMMM MMMM MMMM MMMM MMMM MMMM";
    for body in [
        format!("systemBoundary S(\"`{text}`\")\nA[Target]\nend"),
        format!("A[Source]\nB[Target]\nA -- \"`{text}`\" --> B"),
    ] {
        let unwrapped = label_geometry(&render(&body, 50, false));
        let wrapped = label_geometry(&render(&body, 50, true));
        assert_eq!(unwrapped.len(), 1, "{body}");
        assert_eq!(wrapped.len(), 1, "{body}");
        assert!(unwrapped[0].0.contains('\u{a0}'));
        assert_eq!(wrapped[0].0, text);
        assert!(
            unwrapped[0].1 > wrapped[0].1,
            "{body}: {unwrapped:?} vs {wrapped:?}"
        );
        assert!(
            unwrapped[0].2 < wrapped[0].2,
            "{body}: {unwrapped:?} vs {wrapped:?}"
        );
    }
}

#[test]
fn usecase_markdown_config_auto_wrap_and_plain_text_keep_their_break_opportunities() {
    for (body, auto_wrap) in [
        ("actor A(\"`MMMM MMMM MMMM MMMM`\")", true),
        ("actor A(\"MMMM MMMM MMMM MMMM\")", false),
    ] {
        let narrow = label_geometry(&render(body, 50, auto_wrap));
        let wide = label_geometry(&render(body, 1000, auto_wrap));
        assert_eq!(narrow[0].0, "MMMM MMMM MMMM MMMM");
        assert!(narrow[0].1 < wide[0].1, "{body}");
        assert!(narrow[0].2 > wide[0].2, "{body}");
    }
}

#[test]
fn usecase_markdown_config_prepared_html_obeys_the_retained_text_budget() {
    use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "usecase-beta\nactor A(\"`MMMM MMMM`\")\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let error = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .err()
        .expect("prepared Usecase HTML must reserve retained bytes");
    let merman_render::Error::ResourceLimitExceeded(error) = error else {
        panic!("expected retained-text resource error: {error:?}");
    };
    assert_eq!(
        error.limit,
        ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
    );
}
