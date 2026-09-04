mod render;

pub(super) use render::render_tree_view_diagram_svg_model;

pub(super) fn canonical_tree_view_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
) -> String {
    let scope = format!("#{}", super::sanitize_svg_id(&diagram_id.to_string()));
    let css = render::tree_view_css(effective_config);
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
