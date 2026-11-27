#[cfg(test)]
mod tests {

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn shoal_mermaid_html_edge_label_inset_matches_layout_and_preserves_wrapping() {
        for theme in ["default", "dark"] {
            let source = format!(
                "---\nconfig:\n  layout: shoal\n  theme: {theme}\n  htmlLabels: true\n---\n\
                 flowchart LR\nA near@-->|{}| B\n\
                 B long@-->|A long explanation with several words that should wrap across multiple lines| C\n\
                 C --> D",
                "i".repeat(43)
            );
            let parsed = merman_core::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let artifact = crate::family::prepare(
                parsed.presentation(),
                &crate::LayoutOptions::default(),
                session,
            )
            .unwrap();
            let layout = artifact.layout_json().unwrap();
            let svg = artifact
                .render_svg(&Default::default(), &Default::default())
                .unwrap();
            let doc = crate::xml_dom::Document::parse(svg.svg()).unwrap();
            let labels = layout["layout"]["FlowchartV2"]["edges"].as_array().unwrap();
            for (id, wrapping) in [("near", "nowrap"), ("long", "break-spaces")] {
                let label = labels.iter().find(|edge| edge["id"] == id).unwrap();
                let width = label["label"]["width"].as_f64().unwrap();
                let box_node = doc
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("foreignObject")
                            && node.parent_element().unwrap().attribute("data-id") == Some(id)
                    })
                    .unwrap();
                assert!(width > 200.0 && width <= 208.0, "{theme} {id} {width}");
                assert!(
                    (width - box_node.attribute("width").unwrap().parse::<f64>().unwrap()).abs()
                        < 1e-9
                );
                let style = box_node
                    .first_element_child()
                    .unwrap()
                    .attribute("style")
                    .unwrap();
                assert!(style.contains("padding-inline: 4px;"));
                assert!(style.contains(&format!("white-space: {wrapping};")));
            }
            let empty = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("foreignObject")
                        && node.parent_element().unwrap().attribute("data-id") == Some("L_C_D_0")
                })
                .unwrap();
            assert_eq!(empty.attribute("width"), Some("0"));
            assert_eq!(empty.attribute("height"), Some("0"));
            assert!(
                !empty
                    .first_element_child()
                    .unwrap()
                    .attribute("style")
                    .unwrap()
                    .contains("padding-inline")
            );
        }
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn auto_backplates_composite_nested_container_paint_for_each_label() {
        let source = "---\nconfig:\n  layout: shoal\n---\nflowchart LR
subgraph Outer
    subgraph Inner
        A inner@-->|inner text| B
        B next@-->|second inner text| C
    end
    D outer@-->|outer text| E
end
X canvas@-->|canvas text| Y
style Outer fill:#523A60
style Inner fill:#88CCCC80,fill-opacity:0.5";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap()
            .with_canvas_background(crate::theme::Color::parse("#F1EEE3").unwrap());
        let svg = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let doc = crate::xml_dom::Document::parse(svg.svg()).unwrap();
        for (id, color) in [
            ("inner", "#605F7B"),
            ("next", "#605F7B"),
            ("outer", "#523A60"),
            ("canvas", "#F1EEE3"),
        ] {
            let label = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap();
            let plate = label
                .parent_element()
                .unwrap()
                .prev_sibling_element()
                .unwrap();
            assert_eq!(
                plate.attribute("class"),
                Some("merman-edge-label-background")
            );
            assert_eq!(plate.attribute("fill"), Some(color));
        }
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn adaptive_edge_label_ink_ignores_background_color_and_styles_svg_markdown_labels() {
        let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\n\
            subgraph Dark[Dark Zone]\n\
                A plain@-->|Plain text| B\n\
                B md@-->|\"`**Bold** edge`\"| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 background-color:transparent\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let svg = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = svg.svg();
        assert!(
            !out.contains("background-fill:"),
            "background-color must never be rewritten into background-fill: {out}"
        );
        let doc = crate::xml_dom::Document::parse(out).unwrap();
        for id in ["plain", "md"] {
            let label = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap_or_else(|| panic!("missing edge label {id}"));
            let text = label
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap_or_else(|| panic!("missing SVG <text> for {id}"));
            let style = text.attribute("style").unwrap_or("");
            assert!(
                style.contains("fill:#FFFFFF"),
                "expected adaptive white fill on {id} over dark subgraph, got style={style:?}"
            );
        }
    }

    #[cfg(all(feature = "diagram-swimlane", feature = "layout-shoal"))]
    #[test]
    fn swimlane_svg_and_html_edge_labels_apply_adaptive_ink_and_preserve_hex_styles() {
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();

        let svg_source = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nswimlane-beta LR\n\
            subgraph Dark[Dark Lane]\n\
                A -->|Route| B\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(svg_source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme.clone()))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let doc = crate::xml_dom::Document::parse(rendered.svg()).unwrap();
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-et") == Some("edge-label"))
            .expect("swimlane edge-label group");
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("swimlane SVG edge label text");
        assert!(
            text.attribute("style")
                .unwrap_or("")
                .contains("fill:#FFFFFF"),
            "swimlane SVG-text edge label must set fill:#FFFFFF directly on <text>: {:?}",
            text.attribute("style")
        );

        let html_source = "---\nconfig:\n  htmlLabels: true\n---\nswimlane-beta LR\n\
            subgraph Dark[Dark Lane]\n\
                A -->|Styled| B\n\
                B -->|Adaptive| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 fill:#333;\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(html_source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = rendered.svg();
        assert!(
            !out.contains("&amp;333;"),
            "hex color followed by `;` in swimlane style must not be Mermaid-entity decoded: {out}"
        );
        assert!(
            !out.contains("color:#FFFFFF !important;;"),
            "swimlane HTML div style must not emit double semicolons: {out}"
        );
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn transparent_edge_label_bg_case_insensitive_color_and_hex_edge_style_escaping() {
        let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n---\nflowchart LR\n\
            subgraph Dark[Dark Zone]\n\
                A auto@-->|Auto Ink| B\n\
                B authored@-->|Authored Upper| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 stroke:#333;\n\
            linkStyle 1 COLOR:yellow\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"transparent"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = rendered.svg();
        assert!(
            !out.contains("merman-edge-label-background"),
            "transparent edge_label_bg must not emit backplate rects: {out}"
        );
        assert!(
            !out.contains("&amp;333;"),
            "edge path style `#333;` must not be Mermaid-entity corrupted: {out}"
        );
        let doc = crate::xml_dom::Document::parse(out).unwrap();
        let auto_span = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some("auto")
            })
            .and_then(|label| {
                label.descendants().find(|node| {
                    node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel")
                })
            })
            .expect("auto edge label span");
        assert!(
            auto_span
                .attribute("style")
                .unwrap_or("")
                .contains("color:#FFFFFF !important"),
            "transparent edge_label_bg must still select adaptive white ink over dark cluster: {:?}",
            auto_span.attribute("style")
        );
        let authored_span = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some("authored")
            })
            .and_then(|label| {
                label.descendants().find(|node| {
                    node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel")
                })
            })
            .expect("authored edge label span");
        assert!(
            !authored_span
                .attribute("style")
                .unwrap_or("")
                .contains("color:#FFFFFF"),
            "uppercase COLOR:yellow must suppress adaptive ink override: {:?}",
            authored_span.attribute("style")
        );
    }
}
