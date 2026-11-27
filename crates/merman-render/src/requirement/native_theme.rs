#[cfg(all(test, feature = "layout-dagre"))]
mod tests {
    use merman_core::{MermaidEngine, ParseOptions};

    use crate::environment::RenderEnvironment;
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use crate::theme::{NativeTheme, ThemeCompiler};
    use crate::{LayoutOptions, family};

    #[test]
    fn native_theme_v2_requirement_uses_header_and_risk_roles() {
        let theme = ThemeCompiler::new().compile_json(br##"{
            "schema_version":2,"extends":"paper",
            "roles":{"surface.node":"#EEEEEE","surface.emphasis":"#345678","status.danger":"#7A1616","stroke.connector":"#123456"},
            "typography":{"sans":["Requirement Body","sans-serif"],"base_size_px":18},
            "form":{"radius":7,"edge_style":"straight"}
        }"##, None).unwrap();
        let environment = RenderEnvironment::deterministic();
        for html in [false, true] {
            let source = format!(
                "---\nconfig:\n  htmlLabels: {html}\n---\nrequirementDiagram\nrequirement secured {{\n  id: 1\n  text: A requirement\n  risk: high\n  verifymethod: analysis\n}}\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let prepared = family::prepare(
                parsed.presentation(),
                &LayoutOptions::default(),
                environment
                    .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                    .unwrap(),
            )
            .unwrap();
            let render = prepared
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let dom = roxmltree::Document::parse(render.svg()).unwrap();
            assert!(dom.descendants().any(|node| {
                node.attribute("class") == Some("requirement-native-box")
                    && node.attribute("rx") == Some("7")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains("fill:#EEEEEE"))
            }));
            assert!(dom.descendants().any(|node| node.attribute("class")
                == Some("requirement-native-header")
                && node.attribute("style") == Some("fill:#345678;stroke:none")));
            let risk = dom
                .descendants()
                .find(|node| {
                    node.is_text() && node.text().is_some_and(|text| text.contains("Risk:"))
                })
                .unwrap();
            assert!(risk.ancestors().any(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("#7A1616"))
            }));
        }
    }
}
