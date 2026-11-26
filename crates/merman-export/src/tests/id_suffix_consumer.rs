fn path_fill(svg: &str, id: &str) -> (usvg::Paint, f32) {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
    let usvg::Node::Path(path) = tree.node_by_id(id).unwrap() else {
        panic!("expected path")
    };
    let fill = path.fill().unwrap();
    (fill.paint().clone(), fill.opacity().get())
}

#[test]
fn resvg_safe_retains_hollow_markers_and_selector_specificity() {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="100" height="100"><style>
#diagram [id$="-aggregationEnd"],#diagram .aggregation{fill:transparent!important;stroke:black}
#diagram [id$="-extensionStart"]{fill:transparent!important}
#diagram [id$="-compositionEnd"]{fill:black!important}
#diagram [id$="-arrowhead"] path{fill:red!important}
#diagram [id$="-barbEnd"]{fill:blue!important}
#diagram [id$="-customEnd"]{fill:red!important}
#diagram .override{fill:blue!important}
</style><path id="diagram-aggregationEnd" class="aggregation" d="M0 0H10V10Z"/>
<path id="diagram-aggregationEnd-margin" class="aggregation" d="M20 0H30V10Z"/>
<path id="diagram-extensionStart" d="M0 20H10V30Z"/>
<path id="diagram-compositionEnd" d="M20 20H30V30Z"/>
<g id="diagram-arrowhead"><path id="arrow-paint" d="M40 0H50V10Z"/></g>
<path id="diagram-barbEnd" d="M40 20H50V30Z"/>
<path id="diagram-customEnd" class="override" d="M60 20H70V30Z"/>
</svg>"##;
    // Prove the transformed SVG repairs inheritance and suffix-only rules
    // while retaining the later class override at equal specificity.
    let safe = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
    for id in [
        "diagram-aggregationEnd",
        "diagram-aggregationEnd-margin",
        "diagram-extensionStart",
    ] {
        assert_eq!(path_fill(safe.as_str(), id).1, 0.0, "{id}");
    }
    assert_eq!(path_fill(safe.as_str(), "diagram-compositionEnd").1, 1.0);
    for (id, color) in [
        ("arrow-paint", usvg::Color::new_rgb(255, 0, 0)),
        ("diagram-barbEnd", usvg::Color::new_rgb(0, 0, 255)),
        ("diagram-customEnd", usvg::Color::new_rgb(0, 0, 255)),
    ] {
        assert_eq!(
            path_fill(safe.as_str(), id),
            (usvg::Paint::Color(color), 1.0),
            "{id}"
        );
    }
    assert_eq!(
        merman_render::svg::finalize_resvg_svg(safe.as_str(), &session).unwrap(),
        safe
    );
}
