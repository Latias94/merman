use merman_core::MermaidEngine;
use merman_core::presentation::SourcePresentationConfig;

#[allow(dead_code)]
pub fn assert_svg_accessibility(
    svg: &str,
    expected_title: Option<&str>,
    expected_description: Option<&str>,
) {
    let document = roxmltree::Document::parse(svg).expect("valid accessibility SVG");
    let root = document.root_element();
    for (attribute, tag, expected) in [
        ("aria-labelledby", "title", expected_title),
        ("aria-describedby", "desc", expected_description),
    ] {
        let nodes = root
            .children()
            .filter(|node| node.has_tag_name(tag))
            .collect::<Vec<_>>();
        if let Some(expected) = expected {
            assert_eq!(nodes.len(), 1, "expected one {tag} element");
            let node = nodes[0];
            assert_eq!(node.text(), Some(expected), "{tag} text");
            let id = node
                .attribute("id")
                .filter(|id| !id.is_empty())
                .expect("accessibility node must have a nonempty ID");
            assert_eq!(root.attribute(attribute), Some(id), "{attribute} target");
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.attribute("id") == Some(id))
                    .count(),
                1,
                "accessibility ID must identify one element"
            );
        } else {
            assert!(nodes.is_empty(), "unexpected {tag} element");
            assert!(
                root.attribute(attribute).is_none(),
                "unexpected {attribute}"
            );
        }
    }
}

pub fn legacy_init_theme_compat_config() -> SourcePresentationConfig {
    SourcePresentationConfig::from_value(serde_json::json!({
        "secure": [
            "secure",
            "securityLevel",
            "startOnLoad",
            "maxTextSize",
            "suppressErrorRendering",
            "maxEdges"
        ]
    }))
}

#[allow(dead_code)]
pub fn legacy_init_theme_compat_engine() -> MermaidEngine {
    MermaidEngine::new().with_site_config(legacy_init_theme_compat_config())
}

#[allow(dead_code)]
pub fn presentation_config(
    value: serde_json::Value,
) -> merman_model::presentation::PresentationConfig {
    presentation_config_from_config(merman_model::MermaidConfig::from_value(value))
}

#[allow(dead_code)]
pub fn presentation_config_from_config(
    config: merman_model::MermaidConfig,
) -> merman_model::presentation::PresentationConfig {
    merman_model::presentation::PresentationConfig::from_config_controlled(
        config,
        &merman_model::resources::InputResourcePolicy::for_profile(
            merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
        ),
        &merman_model::OperationControl::new(),
    )
    .unwrap()
    .unwrap()
}

#[allow(dead_code)]
pub fn install_parse_compatibility(
    theme: &merman_render::diagram_theme::DiagramTheme,
    engine: merman_core::MermaidEngine,
) -> merman_core::MermaidEngine {
    let plan = merman_core::presentation::ThemeCompatibilityPlan::from_recipe(
        theme.compatibility_recipe().clone(),
    );
    engine.with_theme_plan(&plan)
}
