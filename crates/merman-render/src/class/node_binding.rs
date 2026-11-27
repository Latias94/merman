use merman_model::families::class::{ClassDiagram, ClassMember, ClassNode};
use merman_model::text::parse_generic_types;
use rustc_hash::FxHashMap;
use std::borrow::Cow;

/// Retains the member's authored type boundaries until SVG Markdown has been tokenized.
pub(crate) fn class_svg_generic_member_source(member: &ClassMember) -> Option<String> {
    fn signature(member: &ClassMember, mut project: impl FnMut(&str) -> String) -> String {
        let mut text = member.visibility.clone();
        text.push_str(&project(&member.id));
        if member.member_type == "method" {
            text.push('(');
            text.push_str(&project(member.parameters.trim()));
            text.push(')');
            if !member.return_type.is_empty() {
                text.push_str(" : ");
                text.push_str(&project(&member.return_type));
            }
        }
        text
    }
    if ![&member.id, &member.parameters, &member.return_type]
        .iter()
        .any(|part| part.contains('~'))
    {
        return None;
    }
    let mut changed = false;
    let rendered = signature(member, |part| {
        if let Some(encoded) = encode_generic_angles(part) {
            changed = true;
            encoded
        } else {
            part.to_owned()
        }
    });
    // A typed model may supply its own display text instead of the parser's signature.
    if !changed || signature(member, parse_generic_types).trim() != member.display_text.trim() {
        return None;
    }
    Some(rendered.trim().to_owned())
}

fn encode_generic_angles(part: &str) -> Option<String> {
    if !part.contains('~') {
        return None;
    }
    let mut rendered = String::with_capacity(part.len());
    let mut depth = 0usize;
    let mut opened = false;
    let mut start = 0;
    // Keep the nesting depth across commas inside the type argument list.
    for (at, _) in part.match_indices('~') {
        let previous = part[..at].chars().next_back();
        let next = part[at + 1..].chars().next();
        let after_type_name = previous.is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        let before_argument = next.is_some_and(|ch| {
            ch.is_alphanumeric()
                || matches!(ch, '_' | '?' | '\'')
                || (depth == 0 && ch.is_whitespace())
        });
        let delimiter = if after_type_name && before_argument {
            depth += 1;
            opened = true;
            "&lt;"
        } else if depth > 0 {
            depth -= 1;
            "&gt;"
        } else {
            continue;
        };
        rendered.push_str(&part[start..at]);
        rendered.push_str(delimiter);
        start = at + 1;
    }
    if !opened || depth != 0 {
        return None;
    }
    rendered.push_str(&part[start..]);
    Some(rendered)
}

pub(super) fn class_node_for_svg_measurement(node: &ClassNode) -> Cow<'_, ClassNode> {
    let mut projected = Cow::Borrowed(node);
    for (rows, is_method) in [(&node.members, false), (&node.methods, true)] {
        for (index, member) in rows.iter().enumerate() {
            if let Some(source) = class_svg_generic_member_source(member) {
                let projected = projected.to_mut();
                let rows = if is_method {
                    &mut projected.methods
                } else {
                    &mut projected.members
                };
                // The measured box decodes display_text once before parsing Markdown.
                rows[index].display_text = source.replace('&', "&amp;");
            }
        }
    }
    projected
}

/// Source styles prepared once for each class. The renderer borrows shared defaults as needed.
#[derive(Debug)]
pub(crate) struct ClassNodeVisualPlan {
    nodes: FxHashMap<String, ClassNodeVisualBinding>,
}

impl ClassNodeVisualPlan {
    pub(crate) fn resolve(
        model: &ClassDiagram,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
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
        Self {
            fill_override: source.fill.map(str::to_owned),
            stroke_override: source.stroke.map(str::to_owned),
            stroke_width_override: source
                .stroke_width
                .map(|value| value.trim_end_matches("px").trim().to_owned()),
            stroke_dasharray_override: source.stroke_dasharray.map(str::to_owned),
            style: source.style_attr.trim().trim_end_matches(';').to_owned(),
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
}

pub(super) fn class_apply_inline_styles<'a>(node: &'a ClassNode) -> ClassInlineStyles<'a> {
    let mut style_attr = String::new();
    let mut svg_color: Option<&str> = None;
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;

    for raw in &node.styles {
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
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
            let meter = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
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
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
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
        let work = crate::resources::OperationWorkMeter::new_with_control(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        control.cancel();
        assert!(matches!(
            ClassNodeVisualPlan::resolve(&fixture_model(), &work),
            Err(crate::Error::Cancelled(_))
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
