use merman_model::families::class::{ClassDiagram, ClassMember, ClassNode};
use merman_model::text::{GenericTypesPlan, is_ecmascript_whitespace};
use rustc_hash::FxHashMap;

/// Keeps generated type delimiters distinct from authored Markdown and HTML.
pub(super) fn class_generic_member_source(
    member: &ClassMember,
    angles: [&str; 2],
) -> Option<String> {
    fn signature(member: &ClassMember, mut append: impl FnMut(&mut String, &str)) -> String {
        let mut text = member.visibility.clone();
        append(&mut text, &member.id);
        if member.member_type == "method" {
            text.push('(');
            append(
                &mut text,
                member.parameters.trim_matches(is_ecmascript_whitespace),
            );
            text.push(')');
            if !member.return_type.is_empty() {
                text.push_str(" : ");
                append(&mut text, &member.return_type);
            }
        }
        text
    }

    if ![&member.id, &member.parameters, &member.return_type]
        .into_iter()
        .any(|part| part.contains('~'))
    {
        return None;
    }
    let mut closed = false;
    let canonical = signature(member, |text, part| {
        let plan = GenericTypesPlan::new(part);
        closed |= plan.has_closed_groups();
        plan.visit(|fragment| text.push_str(fragment));
    });
    if !closed {
        return None;
    }
    let trim = |text: String| text.trim_matches(is_ecmascript_whitespace).to_owned();
    let canonical = trim(canonical);
    // Typed callers may supply independent display text. Also recognize signatures saved
    // before the parser began retaining closed nested types.
    let display = member.display_text.trim_matches(is_ecmascript_whitespace);
    if canonical != display
        && trim(signature(member, |text, part| {
            GenericTypesPlan::visit_legacy(part, |fragment| text.push_str(fragment));
        })) != display
    {
        return None;
    }
    if angles == ["<", ">"] {
        return Some(canonical);
    }
    Some(trim(signature(member, |text, part| {
        GenericTypesPlan::new(part).visit_with_angles(angles, |fragment| text.push_str(fragment));
    })))
}

/// Source styles prepared once for each class. The renderer borrows shared defaults as needed.
#[derive(Debug)]
pub struct ClassNodeVisualPlan {
    nodes: FxHashMap<String, ClassNodeVisualBinding>,
}

impl ClassNodeVisualPlan {
    pub fn resolve(
        model: &ClassDiagram,
        work: &merman_render_core::resources::OperationWorkMeter,
    ) -> merman_render_core::Result<Self> {
        let mut nodes =
            FxHashMap::with_capacity_and_hasher(model.classes().len(), Default::default());
        for (id, node) in model.classes() {
            work.checkpoint(merman_model::OperationPhase::Layout)?;
            nodes.insert(id.to_owned(), ClassNodeVisualBinding::lower(node.node));
        }
        Ok(Self { nodes })
    }

    pub(crate) fn node(&self, id: &str) -> &ClassNodeVisualBinding {
        self.nodes
            .get(id)
            .expect("prepared Class node visual binding")
    }
}

#[derive(Debug)]
pub(crate) struct ClassNodeVisualBinding {
    fill_override: Option<String>,
    stroke_override: Option<String>,
    stroke_width_override: Option<String>,
    stroke_dasharray_override: Option<String>,
    pub(crate) style: String,
    pub(crate) opacity_style: String,
    pub(crate) fill_opacity: Option<String>,
    pub(crate) rough_outline_style: String,
    pub(crate) svg_label_style: String,
}

impl ClassNodeVisualBinding {
    pub(crate) fn fill<'a>(&'a self, defaults: &'a super::ClassCssThemeBinding) -> &'a str {
        self.fill_override
            .as_deref()
            .unwrap_or(&defaults.node_default_fill)
    }

    pub(crate) fn stroke<'a>(&'a self, defaults: &'a super::ClassCssThemeBinding) -> &'a str {
        self.stroke_override
            .as_deref()
            .unwrap_or(&defaults.node_default_stroke)
    }

    pub(crate) fn stroke_width<'a>(&'a self, default: &'a str) -> &'a str {
        self.stroke_width_override.as_deref().unwrap_or(default)
    }

    pub(crate) fn has_fill_override(&self) -> bool {
        self.fill_override.is_some()
    }

    pub(crate) fn guide_stroke<'a>(&'a self, guide: &'a str) -> &'a str {
        self.stroke_override.as_deref().unwrap_or(guide)
    }

    pub(crate) fn stroke_dasharray(&self) -> &str {
        self.stroke_dasharray_override.as_deref().unwrap_or("0 0")
    }

    fn lower(node: &ClassNode) -> Self {
        let source = class_apply_inline_styles(node);
        let mut rough_outline_style = String::new();
        merman_render_core::mermaid_style::visit_parsed_style_declarations(
            &source.style_attr,
            |declaration| {
                if declaration.property().starts_with("stroke") {
                    if !rough_outline_style.is_empty() {
                        rough_outline_style.push(';');
                    }
                    rough_outline_style.push_str(declaration.property_css());
                    rough_outline_style.push(':');
                    rough_outline_style.push_str(declaration.source_value());
                }
            },
        );
        Self {
            fill_override: source.fill.map(str::to_owned),
            stroke_override: source.stroke.map(str::to_owned),
            stroke_width_override: source
                .stroke_width
                .map(|value| value.trim_end_matches("px").trim().to_owned()),
            stroke_dasharray_override: source.stroke_dasharray.map(str::to_owned),
            style: source.style_attr.trim().trim_end_matches(';').to_owned(),
            opacity_style: source.opacity_style,
            fill_opacity: source.fill_opacity.map(str::to_owned),
            rough_outline_style,
            svg_label_style: source
                .svg_color
                .map_or_else(String::new, |color| format!("color:{color};fill:{color}")),
        }
    }
}

pub(super) struct ClassInlineStyles<'a> {
    pub style_attr: String,
    pub svg_color: Option<&'a str>,
    pub fill: Option<&'a str>,
    pub stroke: Option<&'a str>,
    pub stroke_width: Option<&'a str>,
    pub stroke_dasharray: Option<&'a str>,
    pub opacity_style: String,
    pub fill_opacity: Option<&'a str>,
}

pub(super) fn class_apply_inline_styles<'a>(node: &'a ClassNode) -> ClassInlineStyles<'a> {
    let mut style_attr = String::new();
    let mut svg_color: Option<&str> = None;
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;
    let mut opacity_style = String::new();
    let mut fill_opacity = None;

    for raw in &node.styles {
        let Some(parsed) = merman_render_core::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
        if parsed.property() == "opacity" {
            if !opacity_style.is_empty() {
                opacity_style.push(';');
            }
            opacity_style.push_str("opacity:");
            opacity_style.push_str(parsed.source_value());
            continue;
        }
        if !style_attr.is_empty() {
            style_attr.push(';');
        }
        style_attr.push_str(parsed.property_css());
        style_attr.push(':');
        style_attr.push_str(parsed.source_value());

        match parsed.property() {
            "color" if svg_color.is_none() => svg_color = Some(parsed.value()),
            "fill" => fill = Some(parsed.value()),
            "stroke" => stroke = Some(parsed.value()),
            "stroke-width" => stroke_width = Some(parsed.value()),
            "stroke-dasharray" => stroke_dasharray = Some(parsed.value()),
            "fill-opacity" => fill_opacity = Some(parsed.value()),
            _ => {}
        }
    }

    ClassInlineStyles {
        style_attr,
        svg_color,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
        opacity_style,
        fill_opacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model() -> ClassDiagram {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass A\nclass B\n",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        model.clone()
    }

    #[test]
    fn long_shared_defaults_are_borrowed_without_node_payload_growth() {
        let model = fixture_model();
        for padding in [0, 1024, 4096] {
            let fill = format!("rgb({}1,2,3)", " ".repeat(padding));
            let stroke = format!("rgb({}4,5,6)", " ".repeat(padding));
            let config = crate::test_support::presentation_config(serde_json::json!({
                "themeVariables": {"mainBkg": fill, "nodeBorder": stroke}
            }));
            let typography = crate::class::ClassTextThemePlan::resolve(None, &config);
            let defaults = typography.css_binding();
            let meter = merman_render_core::resources::OperationWorkMeter::new(
                merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let visual = ClassNodeVisualPlan::resolve(&model, &meter).unwrap();
            assert_eq!(defaults.node_default_fill, fill);
            assert_eq!(defaults.node_default_stroke, stroke);
            for binding in visual.nodes.values() {
                assert!(binding.fill_override.is_none());
                assert!(binding.stroke_override.is_none());
                assert!(binding.stroke_width_override.is_none());
                assert!(binding.stroke_dasharray_override.is_none());
                assert!(std::ptr::eq(
                    binding.fill(defaults),
                    defaults.node_default_fill.as_str(),
                ));
                assert!(std::ptr::eq(
                    binding.stroke(defaults),
                    defaults.node_default_stroke.as_str(),
                ));
            }
        }
    }

    #[test]
    fn explicit_source_values_equal_to_defaults_remain_owned() {
        let mut model = fixture_model().into_model();
        let config = merman_model::presentation::PresentationConfig::default();
        let typography = crate::class::ClassTextThemePlan::resolve(None, &config);
        let defaults = typography.css_binding();
        model.classes.get_mut("A").unwrap().styles = vec![
            format!("fill:{}", defaults.node_default_fill),
            format!("stroke:{}", defaults.node_default_stroke),
        ];
        let meter = merman_render_core::resources::OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let model = ClassDiagram::new(model, config.as_config());
        let visual = ClassNodeVisualPlan::resolve(&model, &meter).unwrap();
        let source = visual.node("A");
        let inherited = visual.node("B");
        assert_eq!(source.fill(defaults), inherited.fill(defaults));
        assert_eq!(source.stroke(defaults), inherited.stroke(defaults));
        assert_eq!(
            source.fill_override.as_deref(),
            Some(defaults.node_default_fill.as_str()),
        );
        assert_eq!(
            source.stroke_override.as_deref(),
            Some(defaults.node_default_stroke.as_str()),
        );
        assert!(inherited.fill_override.is_none());
        assert!(inherited.stroke_override.is_none());
    }

    #[test]
    fn visual_preparation_observes_cancellation() {
        let control = merman_model::OperationControl::new();
        let work = merman_render_core::resources::OperationWorkMeter::new_with_control(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        control.cancel();
        assert!(matches!(
            ClassNodeVisualPlan::resolve(&fixture_model(), &work),
            Err(merman_render_core::Error::Cancelled(_))
        ));
    }

    #[test]
    fn authored_browser_tokens_remain_in_the_visual_binding() {
        let node: ClassNode = serde_json::from_value(serde_json::json!({
            "id": "A", "label": "A", "text": "A", "domId": "A",
            "styles": [
                "fill:var(--external) !important", "stroke-width:7px",
                "stroke-dasharray:4 2"
            ]
        }))
        .unwrap();
        let binding = ClassNodeVisualBinding::lower(&node);
        assert_eq!(binding.fill_override.as_deref(), Some("var(--external)"));
        assert_eq!(binding.stroke_width("1.3"), "7");
        assert_eq!(binding.stroke_dasharray(), "4 2");
        assert!(binding.style.contains("fill:var(--external) !important"));
    }
}
