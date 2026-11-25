#![cfg(any(
    feature = "diagram-gantt",
    feature = "diagram-journey",
    feature = "diagram-timeline"
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

fn theme() -> NativeTheme {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2,
                "appearances": {"light": {
                    "seed": {"canvas": "#FFFFFF", "ink": "#141414", "accent": "#224466"},
                    "data": {
                        "fill": ["#334455", "#556677", "#8899AA", "#BBCCDD"],
                        "ink": ["#882244", "#336633", "#334466", "#553388"],
                        "fill_ink": "#FDFCFA"
                    }
                }},
                "roles": {
                    "surface.node": "#F3E4D5", "surface.container": "#DDDDEE",
                    "stroke.border": "#333355", "stroke.connector": "#4C5760",
                    "stroke.guide": "#7B8691", "text.title": "#1B3047",
                    "status.info": "#2266AA", "status.warn": "#BC852C",
                    "status.danger": "#AA2233", "status.success": "#278858"
                },
                "typography": {"sans": ["Schedule Face", "sans-serif"], "base_size_px": 24},
                "form": {"radius": 7, "container_radius": 11, "chart": {"gridlines": true, "bar_radius": 6}}
            }"##,
            None,
        )
        .unwrap();
    NativeTheme::new(theme).with_background(Color::rgb(240, 230, 220))
}

fn render(source: &str, theme: Option<NativeTheme>) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("schedule".into()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )
    .unwrap()
    .svg()
    .to_owned()
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn native_theme_v2_gantt_layers_rows_and_preserves_task_states() {
    let source = concat!(
        "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\n",
        "todayMarker stroke:red,opacity:0.5\nsection First\n",
        "Default: ordinary, 2024-01-01, 4d\n",
        "Active: active, underway, 2024-01-02, 4d\n",
        "Done: done, completed, 2024-01-04, 4d\n",
        "Critical: crit, critical, 2024-01-08, 4d\n",
        "Active critical: crit, active, both, 2024-01-09, 4d\n",
        "section Second\nMilestone: milestone, release-marker, 2024-01-12, 0d\n"
    );
    let svg = render(source, Some(theme()));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let root = doc.root_element();
    let canvas = root.first_element_child().unwrap();
    assert_eq!(canvas.attribute("class"), Some("merman-theme-canvas"));
    assert_eq!(
        canvas.first_element_child().unwrap().attribute("fill"),
        Some("#F0E6DC")
    );
    let row = |class: &str| {
        doc.descendants()
            .find(|node| node.attribute("class") == Some(class))
            .unwrap()
    };
    assert_eq!(row("section section0").attribute("fill"), Some("#DDDDEE"));
    assert_eq!(row("section section1").attribute("fill"), Some("none"));
    assert!(row("section section0").range().start < row("exclude-range").range().start);
    assert!(row("exclude-range").range().start < row("grid").range().start);
    assert!(svg.contains(".grid .tick line{stroke:#7B8691;"));
    assert!(svg.contains(".exclude-range{fill:#7B8691;fill-opacity:0.28;"));

    let by_id = |id: &str| {
        doc.descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap()
    };
    for (id, fill) in [
        ("ordinary", "#334455"),
        ("underway", "#2266AA"),
        ("completed", "#DDDDEE"),
        ("critical", "#AA2233"),
        ("both", "#2266AA"),
        ("release-marker", "#1B3047"),
    ] {
        let task = by_id(&format!("schedule-{id}"));
        assert_eq!(task.attribute("fill"), Some(fill), "{id}");
        assert_eq!(task.attribute("rx"), Some("6"), "{id}");
    }
    assert_eq!(by_id("schedule-both").attribute("stroke"), Some("#AA2233"));
    assert!(
        by_id("schedule-release-marker")
            .attribute("transform")
            .is_some_and(|value| value.contains("rotate(45)"))
    );
    assert_eq!(
        by_id("schedule-ordinary-text").attribute("font-size"),
        Some("24")
    );
    assert!(
        row("today")
            .first_element_child()
            .unwrap()
            .attribute("style")
            .unwrap()
            .contains("stroke:red")
    );

    let parity = render(source, None);
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let parity_label = parity
        .descendants()
        .find(|node| node.attribute("id") == Some("schedule-ordinary-text"))
        .unwrap();
    assert_ne!(
        by_id("schedule-ordinary-text").attribute("y"),
        parity_label.attribute("y")
    );
    assert_eq!(parity_label.attribute("font-size"), Some("11"));
    assert!(
        !parity
            .descendants()
            .any(|node| node.attribute("class") == Some("merman-theme-canvas"))
    );

    let authored = format!(
        "---\nconfig:\n  gantt:\n    fontSize: 17\n  themeVariables:\n    taskBkgColor: '#654321'\n---\n{source}"
    );
    let authored = render(&authored, Some(theme()));
    let doc = roxmltree::Document::parse(&authored).unwrap();
    let task = |id| {
        doc.descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap()
    };
    assert_eq!(task("schedule-ordinary").attribute("fill"), Some("#334455"));
    assert_eq!(
        task("schedule-ordinary-text").attribute("font-size"),
        Some("17")
    );
    assert_eq!(task("schedule-underway").attribute("fill"), Some("#2266AA"));
}

#[cfg(feature = "diagram-timeline")]
#[test]
fn native_theme_v2_timeline_uses_unwrapped_ordinals_and_connector_paint() {
    for direction in ["LR", "TD"] {
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    THEME_COLOR_LIMIT: 1\n---\ntimeline {direction}\n  title Schedule\n  section First\n    Planning : An event\n  section Second\n    Delivery : A second event\n"
        );
        let svg = render(&source, Some(theme()));
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let cards: Vec<_> = doc
            .descendants()
            .filter(|node| node.attribute("class") == Some("node-bkg node-undefined"))
            .collect();
        let fills: Vec<_> = cards
            .iter()
            .map(|node| node.attribute("fill").unwrap())
            .collect();
        assert_eq!(
            fills,
            [
                "#334455", "#334455", "#F3E4D5", "#556677", "#556677", "#F3E4D5"
            ]
        );
        let radii: Vec<_> = cards
            .iter()
            .map(|node| node.attribute("rx").unwrap())
            .collect();
        assert_eq!(radii, ["11", "7", "7", "11", "7", "7"]);
        let accents: Vec<_> = doc
            .descendants()
            .filter(|node| node.attribute("class") == Some("timeline-event-accent"))
            .collect();
        assert_eq!(
            accents
                .iter()
                .map(|node| node.attribute("stroke").unwrap())
                .collect::<Vec<_>>(),
            ["#882244", "#336633"]
        );
        let marker = doc
            .descendants()
            .find(|node| node.attribute("id") == Some("schedule-arrowhead"))
            .unwrap();
        assert_eq!(
            marker.first_element_child().unwrap().attribute("fill"),
            Some("#4C5760")
        );
        let mut connectors = doc
            .descendants()
            .filter(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
            .peekable();
        assert!(connectors.peek().is_some());
        assert!(
            connectors.all(|node| node.attribute("stroke") == Some("#4C5760")
                && node.attribute("marker-end") == Some("url(#schedule-arrowhead)"))
        );
        let parity = render(&source, None);
        let parity = roxmltree::Document::parse(&parity).unwrap();
        assert_ne!(
            doc.root_element().attribute("viewBox"),
            parity.root_element().attribute("viewBox")
        );
        assert!(
            svg.contains("font-family:&quot;Schedule Face&quot;, sans-serif")
                || svg.contains("font-family:\"Schedule Face\", sans-serif")
        );

        let authored = source.replace("    THEME_COLOR_LIMIT: 1", "    THEME_COLOR_LIMIT: 1\n    cScale0: '#123456'\n    cScaleLabel0: '#ABCDEF'\n    cScaleInv0: '#765432'");
        assert_eq!(render(&authored, Some(theme())), svg);
    }
}

#[cfg(feature = "diagram-journey")]
#[test]
fn native_theme_v2_journey_preserves_score_actor_and_authored_label_styles() {
    let source = concat!(
        "journey\n  title Project\n  section Launch\n",
        "    Discover: 1: Ada Lovelace and the design team\n",
        "    Plan: 3: Grace Hopper\n    Ship: 5: Ada Lovelace and the design team, Grace Hopper\n"
    );
    let svg = render(source, Some(theme()));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let elements = |tag, class| {
        doc.descendants()
            .filter(move |node| node.has_tag_name(tag) && node.attribute("class") == Some(class))
    };
    let section = elements("rect", "journey-section section-type-0")
        .next()
        .unwrap();
    assert_eq!(section.attribute("fill"), Some("#334455"));
    assert_eq!(section.attribute("rx"), Some("11"));
    let tasks: Vec<_> = elements("rect", "task task-type-0").collect();
    assert_eq!(tasks.len(), 3);
    assert!(
        tasks
            .iter()
            .all(|node| node.attribute("fill") == Some("#F3E4D5")
                && node.attribute("rx") == Some("7"))
    );
    let faces: Vec<_> = elements("circle", "face")
        .map(|node| node.attribute("fill").unwrap())
        .collect();
    assert_eq!(faces, ["#AA2233", "#BC852C", "#278858"]);
    for (class, fill) in [("actor-0", "#882244"), ("actor-1", "#336633")] {
        let mut actors = elements("circle", class).peekable();
        assert!(actors.peek().is_some());
        assert!(actors.all(|node| node.attribute("fill") == Some(fill)));
    }
    let parity = render(source, None);
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let parity_task = parity
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("task task-type-0")
        })
        .unwrap();
    assert_ne!(tasks[0].attribute("x"), parity_task.attribute("x"));

    let authored = format!(
        "---\nconfig:\n  themeVariables:\n    textColor: '#FE00FE'\n    mainBkg: '#FE00FE'\n  journey:\n    taskFontFamily: Georgia\n    taskFontSize: 19\n    sectionFills: ['#543210']\n    sectionColours: ['#FEDCBA']\n    actorColours: ['#123456']\n---\n{source}"
    );
    // Mermaid appends authored arrays to the defaults. Draw the first authored slot in each list.
    let mut authored = authored;
    for index in 1..=7 {
        use std::fmt::Write;
        writeln!(
            authored,
            "  section Phase {index}\n    Task: 3: Actor {index}"
        )
        .unwrap();
    }
    let authored = render(&authored, Some(theme()));
    assert!(!authored.contains("#FE00FE"));
    let authored = roxmltree::Document::parse(&authored).unwrap();
    let section = authored
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class") == Some("journey-section section-type-7")
        })
        .unwrap();
    assert_eq!(section.attribute("fill"), Some("#543210"));
    assert!(
        authored
            .descendants()
            .any(|node| node.has_tag_name("circle")
                && node.attribute("class") == Some("actor-6")
                && node.attribute("fill") == Some("#123456"))
    );
    for tag in ["div", "text"] {
        assert!(
            authored.descendants().any(|node| node.has_tag_name(tag)
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("font-family: Georgia;")
                        && style.contains("font-size: 19px;")
                        && style.contains("#FEDCBA"))),
            "{tag} label retained source typography and color"
        );
    }
}
