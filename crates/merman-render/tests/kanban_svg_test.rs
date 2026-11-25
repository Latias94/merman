use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};

fn kanban_root_font_size_css(svg: &str, diagram_id: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Kanban SVG");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Kanban stylesheet");
    css.split('}')
        .find(|rule| rule.starts_with(&format!("#{diagram_id}{{")))
        .expect("Kanban root CSS rule")
        .split(';')
        .find_map(|declaration| declaration.strip_prefix("font-size:"))
        .expect("Kanban root font size")
        .to_owned()
}

fn render_kanban_artifact(
    artifact: family::FamilyRenderArtifact,
) -> merman_render::Result<(KanbanDiagramLayout, String)> {
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["KanbanDiagram"].clone())
        .expect("Kanban layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("kanban-markdown".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned();
    Ok((layout, svg))
}

fn render_kanban_with_source_config(
    source: &str,
    config: serde_json::Value,
) -> (KanbanDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    render_kanban_artifact(family::prepare(&parsed, &LayoutOptions::default(), session).unwrap())
        .unwrap()
}

fn parse_layout_and_render(source: &str) -> (KanbanDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Kanban")
        .expect("detect Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("start deterministic render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Kanban layout");
    render_kanban_artifact(artifact).expect("render Kanban SVG")
}

fn try_render_kanban_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse resource-bound Kanban")
        .expect("detect resource-bound Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("start resource-bound Kanban render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("kanban-bounded".to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn kanban_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"kanban
  todo[Todo]
    task[Bounded output task]@{ ticket: MC-2038, assigned: 'Alice', priority: 'High' }
  done[Done]
    shipped[Shipped]
"#;
    let baseline = try_render_kanban_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Kanban baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Kanban fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Kanban SVG byte ceiling");
    let exact = try_render_kanban_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Kanban family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Kanban SVG byte ceiling");
    let error = try_render_kanban_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Kanban family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Kanban MaxSvgBytes rejection, got {error}");
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

#[test]
fn kanban_markdown_metrics_drive_canonical_layout_and_svg() {
    let (markdown_layout, markdown_svg) = parse_layout_and_render(
        "kanban\n  todo[Todo]\n    task[*aaaa aaaa aaaaaaa*]\n    next[Next]\n",
    );
    let (plain_layout, _) = parse_layout_and_render(
        "kanban\n  todo[Todo]\n    task[aaaa aaaa aaaaaaa]\n    next[Next]\n",
    );

    assert_eq!(
        markdown_layout.items[0].height,
        plain_layout.items[0].height
    );
    assert_eq!(
        markdown_layout.items[1].center_y,
        plain_layout.items[1].center_y
    );
    assert_eq!(
        markdown_layout.sections[0].rect_height,
        plain_layout.sections[0].rect_height
    );
    assert!(
        markdown_svg.contains("<p><em>aaaa aaaa aaaaaaa</em></p>"),
        "{markdown_svg}"
    );
}

#[test]
fn kanban_theme_variable_dark_mode_reaches_sections() {
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    for (root, variable, dark) in [
        (None, serde_json::json!(true), true),
        (Some(false), serde_json::json!(true), true),
        (Some(true), serde_json::json!(false), false),
        (None, serde_json::json!("false"), true),
        (None, serde_json::json!(0), false),
    ] {
        let mut config = serde_json::json!({
            "themeVariables": { "darkMode": variable, "cScale1": "#123456" }
        });
        if let Some(root) = root {
            config["darkMode"] = root.into();
        }
        let fill = if dark {
            "hsl(210, 65.3846153846%, 10.3921568627%)"
        } else {
            "hsl(210, 65.3846153846%, 30.3921568627%)"
        };
        for from_source in [false, true] {
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{source}"),
                    serde_json::json!({}),
                )
            } else {
                (source.to_owned(), config.clone())
            };
            let (_, svg) = render_kanban_with_source_config(&source, site);
            let document = roxmltree::Document::parse(&svg).expect("valid Kanban SVG");
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Kanban stylesheet");
            assert!(
                css.contains(&format!(".section-0 path{{fill:{fill};")),
                "section dark mode: config={config}, source={from_source}"
            );
        }
    }
}

#[test]
fn kanban_selected_dark_palette_keeps_its_independent_dark_mode_input() {
    // Upstream Dark selects a palette; it does not initialize themeVariables.darkMode=true.
    for dark in [false, true] {
        let mut config = serde_json::json!({
            "theme": "dark", "themeVariables": { "cScale1": "#123456" }
        });
        if dark {
            config["themeVariables"]["darkMode"] = true.into();
        }
        let (_, svg) =
            render_kanban_with_source_config("kanban\n  todo[Todo]\n    task[Task]\n", config);
        let document = roxmltree::Document::parse(&svg).expect("valid dark Kanban SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Kanban stylesheet");
        let fill = if dark {
            "hsl(210, 65.3846153846%, 10.3921568627%)"
        } else {
            "hsl(210, 65.3846153846%, 30.3921568627%)"
        };
        assert!(css.contains(&format!(".section-0 path{{fill:{fill};")));
    }
}

#[test]
fn kanban_duplicate_section_ids_do_not_replay_items() {
    let source = concat!(
        "kanban\n",
        "  lane[First lane]\n",
        "    first[First task]\n",
        "  lane[Second lane]\n",
        "    second[Second task]\n",
    );
    let (layout, svg) = parse_layout_and_render(source);

    assert_eq!(layout.sections.len(), 2);
    assert_eq!(layout.items.len(), 4);
    assert!(
        layout
            .items
            .iter()
            .all(|item| item.rx == 5.0 && item.ry == 5.0)
    );

    let document = roxmltree::Document::parse(&svg).expect("valid duplicate-section Kanban SVG");
    for id in ["kanban-markdown-first", "kanban-markdown-second"] {
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
                .count(),
            2,
            "each upstream model occurrence should be emitted exactly once"
        );
    }
}

#[test]
fn kanban_legacy_title_color_has_no_consumer_while_text_color_styles_labels() {
    let source = concat!(
        "---\ntitle: Metadata title\n---\n",
        "kanban\n  title Release\n    task[Task]\n",
    );
    let render = |title_color: &str, text_color: &str| {
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": {
                            "titleColor": title_color,
                            "textColor": text_color
                        }
                    }),
                ),
            )
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse ordinary configured Kanban")
            .expect("detect Kanban")
            .into_presentation();
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.titleColor"),
            Some(title_color),
            "the legacy config probe must reach the effective configuration"
        );
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin unthemed Kanban session");
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("prepare ordinary configured Kanban");
        render_kanban_artifact(artifact)
            .expect("render ordinary configured Kanban")
            .1
    };
    let first = render("#123456", "#abcdef");
    let second = render("#987654", "#abcdef");
    assert_eq!(first, second, "titleColor must not change any emitted SVG");
    let changed_text = render("#123456", "#fedcba");
    assert_ne!(first, changed_text, "textColor has real label consumers");
    for (svg, text_color) in [(&first, "#abcdef"), (&changed_text, "#fedcba")] {
        let document = roxmltree::Document::parse(svg).expect("valid configured Kanban SVG");
        let stylesheet = document
            .root_element()
            .children()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("root Kanban stylesheet");
        assert!(stylesheet.contains(&format!(
            "#kanban-markdown .cluster-label,#kanban-markdown .label{{color:{text_color};fill:{text_color};}}"
        )));
        let column_label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("visible column label");
        assert!(
            column_label
                .descendants()
                .any(|node| { node.is_text() && node.text() == Some("title Release") })
        );
        let card = document
            .descendants()
            .find(|node| node.attribute("id") == Some("kanban-markdown-task"))
            .expect("visible task card");
        assert!(
            card.descendants()
                .any(|node| { node.is_text() && node.text() == Some("Task") })
        );
        assert!(
            !document
                .descendants()
                .any(|node| { node.is_text() && node.text() == Some("Metadata title") })
        );
    }
}

#[test]
fn kanban_root_font_size_preserves_numeric_and_css_unit_fallbacks() {
    for (font_size, expected_css) in [
        (serde_json::json!(0), "0px"),
        (serde_json::json!(0.5), "0.5px"),
        (serde_json::json!(20), "20px"),
        (serde_json::json!("0px"), "16px"),
        (serde_json::json!("0.5px"), "16px"),
        (serde_json::json!("20px"), "16px"),
    ] {
        let source = format!(
            "---\nconfig: {}\n---\nkanban\n  todo[Todo]\n    task[Task]\n",
            serde_json::json!({
                "fontSize": font_size,
                "themeVariables": { "fontSize": "" }
            })
        );
        let (layout, svg) = parse_layout_and_render(&source);
        let expected_size: f64 = expected_css.strip_suffix("px").unwrap().parse().unwrap();
        assert_eq!(layout.items[0].height, 20.0 + 1.5 * expected_size);
        let document = roxmltree::Document::parse(&svg).expect("valid Kanban SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Kanban stylesheet");
        let root_rule = css
            .split('}')
            .find(|rule| rule.starts_with("#kanban-markdown{"))
            .expect("Kanban root CSS rule");
        assert!(
            root_rule.contains(&format!("font-size:{expected_css};")),
            "font_size={font_size}: {root_rule}"
        );
    }
}

#[test]
fn kanban_invalid_theme_size_falls_back_to_valid_root_or_default_without_a_theme() {
    for from_source in [false, true] {
        for (theme_size, root_size, expected_size) in [
            (serde_json::json!(-2), serde_json::json!(0), 0.0),
            (serde_json::json!("-2px"), serde_json::json!(0.5), 0.5),
            (serde_json::json!("bogus"), serde_json::json!(0.5), 0.5),
            (serde_json::json!(""), serde_json::json!(0.5), 0.5),
            (serde_json::json!(-2), serde_json::json!(-3), 16.0),
        ] {
            let config = serde_json::json!({
                "fontSize": root_size,
                "themeVariables": { "fontSize": theme_size }
            });
            let board = "kanban\n  todo[Todo]\n    task[Task]\n";
            let (source, engine) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{board}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    board.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .expect("prepare unthemed Kanban size fallback");
            let (layout, svg) = render_kanban_artifact(artifact).unwrap();
            assert_eq!(
                kanban_root_font_size_css(&svg, "kanban-markdown"),
                format!("{expected_size}px"),
                "source={from_source}, theme={theme_size}, root={root_size}"
            );
            assert_eq!(layout.items[0].height, 20.0 + 1.5 * expected_size);
        }
    }
}

#[test]
fn kanban_native_roles_reach_sections_tasks_and_status_markers() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2,
                "extends": "paper-cut",
                "roles": {
                    "surface.container": "#D9DBDD",
                    "surface.node": "#E2E4E6",
                    "status.danger": "#AA3549",
                    "status.warn": "#A45C28",
                    "status.info": "#26399D",
                    "status.success": "#287749"
                },
                "form": {"radius": 11, "container_radius": 13},
                "typography": {
                    "sans": ["Card Label", "sans-serif"],
                    "serif": ["Column Heading", "serif"],
                    "body_family": "sans",
                    "title_family": "serif",
                    "base_size_px": 23,
                    "title_weight": 700
                },
                "appearances": {"light": {"data": {"ink": ["#90251D", "#226982", "#664226", "#86517C"]}}}
            }"##,
            None,
        )
        .unwrap();
    let source = concat!(
        "kanban\n",
        "  todo[To do]\n",
        "    urgent[Urgent]@{priority: 'Very High'}\n",
        "    waiting[Waiting]@{priority: 'High'}\n",
        "  done[Done]\n",
        "    low[Low]@{priority: 'Low'}\n",
        "    complete[Complete]@{priority: 'Very Low'}\n",
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let (layout, svg) = render_kanban_artifact(
        family::prepare(&parsed, &LayoutOptions::default(), session).unwrap(),
    )
    .unwrap();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("font-family:\"Card Label\", sans-serif;font-size:23px"));
    assert!(css.contains("font-family:\"Column Heading\", serif;font-size:23px;font-weight:700"));
    assert!(css.contains(".cluster>rect{fill:#D9DBDD;"));
    assert!(css.contains(".node>rect{fill:#E2E4E6;"));
    assert_eq!(layout.sections[0].rx, 13.0);
    assert_eq!(layout.items[0].rx, 11.0);
    assert!(layout.items[0].height > parse_layout_and_render(source).0.items[0].height);
    let accents: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("kanban-column-accent"))
        .filter_map(|node| node.attribute("stroke"))
        .collect();
    assert_eq!(accents, ["#90251D", "#226982"]);
    let priorities: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("line") && node.attribute("stroke-width") == Some("4"))
        .filter_map(|node| node.attribute("stroke"))
        .collect();
    assert_eq!(priorities, ["#AA3549", "#A45C28", "#26399D", "#287749"]);
}
