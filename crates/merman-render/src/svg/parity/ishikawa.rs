mod render;

pub(super) use render::render_ishikawa_diagram_svg;

pub(super) fn canonical_ishikawa_css(
    diagram_id: impl Copy + std::fmt::Display,
    font_size_px: f64,
    effective_config: &serde_json::Value,
) -> String {
    let scope = format!("#{}", super::sanitize_svg_id(&diagram_id.to_string()));
    let css = render::ishikawa_css(font_size_px, effective_config);
    let mut scoped = String::with_capacity(css.len() + scope.len() * 8);
    for rule in css.split_inclusive('}') {
        let Some((selectors, declarations)) = rule.split_once('{') else {
            scoped.push_str(rule);
            continue;
        };
        for (index, selector) in selectors.split(',').enumerate() {
            if index > 0 {
                scoped.push(',');
            }
            scoped.push_str(&scope);
            scoped.push(' ');
            scoped.push_str(selector.trim());
        }
        scoped.push('{');
        scoped.push_str(declarations);
    }
    scoped
}
