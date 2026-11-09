use indexmap::IndexMap;
use merman_model::families::block::BlockClassDef;
use std::sync::Arc;

use crate::mermaid_style::{self, CssFontSizeContext, TextStyleCascade};
use crate::resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::TextStyle;

/// Visible text, rather than retained DOM nodes, consumes base typography and label paint.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BlockTextParticipation {
    pub(crate) visible: bool,
    pub(crate) font_stack: bool,
    pub(crate) font_size: bool,
    pub(crate) unverified: bool,
}

impl BlockTextParticipation {
    pub(crate) fn merge(&mut self, other: Self) {
        self.visible |= other.visible;
        self.font_stack |= other.font_stack;
        self.font_size |= other.font_size;
        self.unverified |= other.unverified;
    }

    fn observe(&mut self, font: &FontState) {
        if !font.host_css && font.known[1] && font.style.font_size == 0.0 {
            return;
        }
        self.visible = true;
        self.font_stack |= font.inherits_family;
        self.font_size |= font.inherits_size;
        self.unverified |= font.host_css || font.known.iter().any(|known| !known);
    }
}

#[derive(Debug)]
pub(crate) struct BlockLabelTypography {
    pub(crate) style: Arc<TextStyle>,
    pub(crate) participation: BlockTextParticipation,
    pub(crate) measurement_xhtml: Option<String>,
    _retained_reservation: PreparedTextRetainedReservation,
    _projection_reservation: Option<PreparedTextRetainedReservation>,
}

#[derive(Clone)]
struct FontState {
    style: Arc<TextStyle>,
    known: [bool; 4],
    inherits_family: bool,
    inherits_size: bool,
    host_css: bool,
}

fn retained_font_string_bytes(style: &TextStyle) -> usize {
    [&style.font_family, &style.font_weight, &style.font_style]
        .into_iter()
        .filter_map(Option::as_ref)
        .map(String::capacity)
        .sum()
}

impl FontState {
    fn new(style: &TextStyle) -> Self {
        Self {
            style: Arc::new(style.clone()),
            known: [true; 4],
            inherits_family: true,
            inherits_size: true,
            host_css: false,
        }
    }

    fn element(&self, declarations: &[String], root_size: f64) -> Self {
        let mut next = self.clone();
        let context = CssFontSizeContext::new(self.style.font_size, root_size);
        let mut cascade = TextStyleCascade::new(&self.style, context);
        for (raw, declaration) in super::node_binding::source_style_winners(declarations) {
            cascade.observe_declaration(&declaration);
            let value = declaration.value();
            if declaration.inherits_property_value() {
                continue;
            }
            match declaration.property() {
                "font-family" => {
                    next.known[0] = mermaid_style::is_static_css_font_family_list(value);
                    next.inherits_family = false;
                }
                "font-size" => {
                    // Invalid CSS numbers do not win a browser declaration. Unknown host
                    // functions do win, but must not inherit a fallback zero as proof.
                    let owner = mermaid_style::css_font_size_declaration_ownership([raw]);
                    if owner == mermaid_style::CssFontSizeOwnership::Inherited {
                        continue;
                    }
                    let absolute = mermaid_style::resolve_mermaid_absolute_font_size_px(value)
                        .is_some()
                        || matches!(
                            value,
                            "initial"
                                | "medium"
                                | "small"
                                | "large"
                                | "x-small"
                                | "xx-small"
                                | "x-large"
                                | "xx-large"
                        );
                    next.known[1] = declaration.resolve_font_size_px(context).is_some()
                        && !declaration.analysis().depends_on_root_font_size()
                        && (absolute || self.known[1]);
                    if absolute {
                        next.inherits_size = false;
                    }
                }
                "font-weight" => {
                    next.known[2] = mermaid_style::is_supported_css_font_weight_value(value)
                        || value.eq_ignore_ascii_case("initial");
                }
                "font-style" => {
                    next.known[3] = mermaid_style::is_supported_css_font_style_value(value)
                        || value.eq_ignore_ascii_case("initial");
                }
                _ => {}
            }
        }
        next.style = match cascade.finish() {
            std::borrow::Cow::Borrowed(_) => Arc::clone(&self.style),
            std::borrow::Cow::Owned(style) => Arc::new(style),
        };
        // These are emitted by class CSS, outside Block's supported font longhands.
        // Retain uncertainty instead of treating a measurement fallback as a zero font.
        if declarations.iter().any(|raw| {
            mermaid_style::parse_style_declaration(raw)
                .is_some_and(|declaration| matches!(declaration.property(), "font" | "all"))
        }) {
            next.known = [false; 4];
        }
        next
    }
}

/// Resolve the fixed Block label wrappers, not arbitrary host selectors. Class CSS is
/// important and follows definition order; assigned inline fonts follow assignment order.
fn stylesheet_fonts(
    definitions: &IndexMap<String, BlockClassDef>,
    classes: &[&str],
    work: &OperationWorkMeter,
) -> Result<Vec<String>, OperationWorkError> {
    work.charge(classes.len())?;
    let mut matching = classes
        .iter()
        .filter_map(|class| definitions.get_full(*class))
        .collect::<Vec<_>>();
    matching.sort_unstable_by_key(|(index, _, _)| *index);
    matching.dedup_by_key(|(index, _, _)| *index);
    let mut declarations = Vec::new();
    for (_, _, definition) in matching {
        for raw in &definition.styles {
            work.charge(raw.len().saturating_add(1))?;
            if let Some(declaration) = mermaid_style::parse_style_declaration(raw)
                && (super::node_binding::is_font_key(declaration.property())
                    || matches!(declaration.property(), "font" | "all"))
            {
                declarations.push(format!(
                    "{}:{} !important",
                    declaration.property(),
                    declaration.value()
                ));
            }
        }
    }
    Ok(declarations)
}

impl BlockLabelTypography {
    #[allow(
        clippy::too_many_arguments,
        reason = "Block typography consumes one label's source, prepared XHTML, and operation budget."
    )]
    pub(crate) fn resolve(
        styles: &[String],
        classes: Option<&[String]>,
        definitions: &IndexMap<String, BlockClassDef>,
        base: &TextStyle,
        xhtml: Option<&str>,
        text: &str,
        key_bytes: usize,
        work: &std::sync::Arc<OperationWorkMeter>,
    ) -> Result<Self, OperationWorkError> {
        let mut node_classes = if let Some(classes) = classes {
            let mut names = vec!["node", "flowchart-label"];
            if classes.is_empty() {
                names.push("default");
            } else {
                names.extend(classes.iter().map(String::as_str));
            }
            names
        } else {
            vec!["edgeLabel"]
        };
        let terminal_class = if classes.is_some() {
            "nodeLabel"
        } else {
            "edgeLabel"
        };
        let mut matching_classes = node_classes.clone();
        matching_classes.extend(["block", "label", terminal_class]);
        if classes.is_none() && xhtml.is_some() {
            matching_classes.push("labelBkg");
        }
        if xhtml.is_none() {
            matching_classes.extend(["row", "text-outer-tspan"]);
        }
        // Visit only assigned and structural classes. Unrelated definitions neither cost
        // work per label nor inflate this operation's concurrent retained reservation.
        let mut string_bytes = [
            base.font_family.as_ref().map_or(0, String::capacity),
            base.font_weight.as_ref().map_or(0, String::capacity),
            base.font_style.as_ref().map_or(0, String::capacity),
        ];
        if xhtml.is_none() {
            // The emitted SVG word can replace both emphasis longhands with normal.
            string_bytes[1] = string_bytes[1].max("normal".len());
            string_bytes[2] = string_bytes[2].max("normal".len());
        }
        for raw in styles.iter().chain(
            matching_classes
                .iter()
                .filter_map(|class| definitions.get(*class))
                .flat_map(|definition| &definition.styles),
        ) {
            work.charge(raw.len().saturating_add(1))?;
            if let Some(declaration) = mermaid_style::parse_style_declaration(raw) {
                let index = match declaration.property() {
                    "font-family" => Some(0),
                    "font-weight" => Some(1),
                    "font-style" => Some(2),
                    _ => None,
                };
                if let Some(index) = index {
                    string_bytes[index] = string_bytes[index].max(declaration.value().len());
                }
            }
        }
        // XHTML already reserves the shared label/map slot. SVG-only labels reserve
        // that owner here; both forms account for only the font strings actually kept.
        let owner_bytes = std::mem::size_of::<Self>()
            + std::mem::size_of::<TextStyle>()
            + 2 * std::mem::size_of::<usize>()
            + if xhtml.is_some() { 0 } else { 192 + key_bytes };
        let retained = string_bytes
            .into_iter()
            .try_fold(owner_bytes, |bytes, length| bytes.checked_add(length))
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut reservation = work.reserve_prepared_text_retained_bytes(retained)?;
        let inline = classes.map_or_else(Vec::new, |classes| {
            super::node_binding::source_font_styles(styles, classes, definitions)
        });
        let root_size = base.font_size;
        let mut font = FontState::new(base)
            .element(&stylesheet_fonts(definitions, &["block"], work)?, root_size);
        // Block's .label rule sets the base family directly on the label g.
        Arc::make_mut(&mut font.style)
            .font_family
            .clone_from(&base.font_family);
        font.known[0] = true;
        font.inherits_family = true;
        let mut declarations = stylesheet_fonts(definitions, &node_classes, work)?;
        declarations.extend(inline.iter().cloned());
        font = font.element(&declarations, root_size);
        if xhtml.is_some() {
            font = font.element(&stylesheet_fonts(definitions, &["label"], work)?, root_size);
            font = font.element(&inline, root_size);
            node_classes.extend(["block", "label"]);
            if classes.is_none() {
                node_classes.push("labelBkg");
            }
            let mut declarations = stylesheet_fonts(definitions, &node_classes, work)?;
            declarations.extend(inline.iter().cloned());
            font = font.element(&declarations, root_size);
        } else {
            let mut declarations = stylesheet_fonts(definitions, &["label"], work)?;
            declarations.extend(inline.iter().cloned());
            font = font.element(&declarations, root_size);
            if let std::borrow::Cow::Owned(style) = crate::text::svg_markdown_word_style(
                &font.style,
                crate::text::MermaidMarkdownWordType::Normal,
            ) {
                font.style = Arc::new(style);
            }
            font.known[2..].fill(true);
            // The fixed outer row tspan owns its inner word's declarations. The
            // word's presentation weight/style reset is below important class CSS.
            font = font.element(
                &stylesheet_fonts(definitions, &["row", "text-outer-tspan"], work)?,
                root_size,
            );
        }
        let mut participation = BlockTextParticipation::default();
        let mut measurement_xhtml = None;
        let mut projection_reservation = None;
        if let Some(xhtml) = xhtml {
            work.charge(xhtml.len())?;
            let terminal_fonts = stylesheet_fonts(definitions, &[terminal_class], work)?;
            if xhtml.starts_with("<p>")
                && xhtml.ends_with("</p>")
                && !xhtml[3..xhtml.len() - 4].contains('<')
                && terminal_fonts.is_empty()
            {
                if !text.trim().is_empty() {
                    participation.observe(&font);
                }
                let style = font.style;
                reservation.reconcile_downward(owner_bytes + retained_font_string_bytes(&style));
                return Ok(Self {
                    style,
                    participation,
                    measurement_xhtml: None,
                    _retained_reservation: reservation,
                    _projection_reservation: None,
                });
            }
            let wrapped = format!("<block-label>{xhtml}</block-label>");
            if let Ok(document) = crate::xml_dom::Document::parse(&wrapped) {
                let root = document.root_element();
                let mut root_font = font.clone();
                root_font.host_css = root.descendants().any(|node| node.has_tag_name("style"));
                let mut parents = vec![(root.id(), root_font)];
                let mut observed_text = false;
                let mut edits = Vec::new();
                let mut projected_bytes = xhtml.len();
                // .nodeLabel/.edgeLabel are ancestors of retained spans as well as
                // parents of the top-level p. Project their fixed font rules once;
                // the shared XHTML measurement traversal resolves the actual fonts.
                node_classes.push(terminal_class);
                let span_fonts = stylesheet_fonts(definitions, &node_classes, work)?;
                for node in root.descendants().skip(1) {
                    work.charge(1)?;
                    while parents.last().is_some_and(|(id, _)| {
                        node.parent().is_some_and(|parent| parent.id() != *id)
                    }) {
                        parents.pop();
                    }
                    let Some((_, parent_font)) = parents.last() else {
                        break;
                    };
                    if node.is_element() {
                        let class_fonts = if node.has_tag_name("span") {
                            span_fonts.as_slice()
                        } else if node.parent() == Some(root) {
                            terminal_fonts.as_slice()
                        } else {
                            &[]
                        };
                        for raw in class_fonts {
                            work.charge(raw.len().saturating_add(1))?;
                        }
                        let mut declarations = Vec::new();
                        for property in ["font-family", "font-size", "font-weight", "font-style"] {
                            if let Some(value) =
                                crate::text::xhtml_font_default(node.tag_name().name(), property)
                            {
                                declarations.push(format!("{property}:{value}"));
                            }
                        }
                        declarations.extend(class_fonts.iter().cloned());
                        if let Some(style) = node.attribute("style") {
                            mermaid_style::visit_parsed_style_declarations(style, |declaration| {
                                declarations.push(format!(
                                    "{}:{}",
                                    declaration.property(),
                                    declaration.source_value()
                                ));
                            });
                        }
                        if !class_fonts.is_empty() {
                            let original = node.attribute("style").unwrap_or_default();
                            let additional = class_fonts
                                .iter()
                                .try_fold(original.len() + 32, |bytes, declaration| {
                                    bytes.checked_add(declaration.len() + 1)
                                })
                                .and_then(|bytes| bytes.checked_mul(6))
                                .ok_or_else(|| work.arithmetic_overflow())?;
                            let upper_bound = projected_bytes
                                .checked_add(additional)
                                .ok_or_else(|| work.arithmetic_overflow())?;
                            work.preflight_prepared_text_retained_bytes(upper_bound)?;
                            let style = format!("{};{original}", class_fonts.join(";"));
                            let escaped = crate::svg::escape_attr(&style);
                            let (range, replacement) = if let Some(attribute) =
                                node.attribute_node("style")
                            {
                                (attribute.range(), format!("style=\"{escaped}\""))
                            } else {
                                let start = node.range().start + 1;
                                let name_len = wrapped[start..]
                                    .bytes()
                                    .position(|byte| {
                                        byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>')
                                    })
                                    .expect("retained XML element name has a delimiter");
                                (
                                    start + name_len..start + name_len,
                                    format!(" style=\"{escaped}\""),
                                )
                            };
                            projected_bytes = projected_bytes
                                .checked_sub(range.len())
                                .and_then(|bytes| bytes.checked_add(replacement.len()))
                                .ok_or_else(|| work.arithmetic_overflow())?;
                            edits.push((range, replacement));
                        }
                        let mut child = parent_font.element(&declarations, root_size);
                        child.host_css |= node
                            .attribute("class")
                            .is_some_and(|class| !class.trim().is_empty())
                            || (node.has_tag_name("font")
                                && (node.has_attribute("face") || node.has_attribute("size")));
                        parents.push((node.id(), child));
                    } else if !node
                        .parent()
                        .is_some_and(|parent| parent.has_tag_name("style"))
                        && node.text().is_some_and(|text| !text.trim().is_empty())
                    {
                        observed_text = true;
                        participation.observe(parent_font);
                    }
                }
                if !observed_text {
                    let facts = crate::text::VisibleTextStyleFacts::from_xhtml_fragment(xhtml);
                    if facts.has_visible_runs() {
                        // Icon and replaced-content hosts may paint without a text node.
                        // They cannot certify a fallback zero or a measured font here.
                        participation.visible = true;
                        participation.unverified = true;
                    }
                }
                if !edits.is_empty() {
                    let mut reserved =
                        work.reserve_prepared_text_retained_bytes(projected_bytes)?;
                    let mut projected = String::with_capacity(projected_bytes);
                    let mut offset = "<block-label>".len();
                    for (range, replacement) in edits {
                        projected.push_str(&wrapped[offset..range.start]);
                        projected.push_str(&replacement);
                        offset = range.end;
                    }
                    projected.push_str(&wrapped[offset..wrapped.len() - "</block-label>".len()]);
                    debug_assert_eq!(projected.len(), projected_bytes);
                    reserved.reconcile_downward(projected.capacity());
                    measurement_xhtml = Some(projected);
                    projection_reservation = Some(reserved);
                }
            } else {
                participation.visible = !text.trim().is_empty();
                participation.unverified = participation.visible;
            }
        } else if !text.trim().is_empty() {
            participation.observe(&font);
        }
        reservation.reconcile_downward(owner_bytes + retained_font_string_bytes(&font.style));
        Ok(Self {
            style: font.style,
            participation,
            measurement_xhtml,
            _retained_reservation: reservation,
            _projection_reservation: projection_reservation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use std::sync::Arc;

    #[test]
    fn unchanged_deep_font_states_share_the_retained_long_family() {
        let base = TextStyle {
            font_family: Some("x".repeat(64 * 1024)),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let root = FontState::new(&base);
        let mut parents = vec![root.clone()];
        for _ in 0..2048 {
            let child = parents.last().unwrap().element(&[], base.font_size);
            assert!(Arc::ptr_eq(&child.style, &root.style));
            parents.push(child);
        }
        assert_eq!(Arc::strong_count(&root.style), parents.len() + 1);
    }

    #[test]
    fn unrelated_class_definitions_do_not_add_per_label_work_or_retained_bytes() {
        let base = TextStyle {
            font_family: Some("sans-serif".to_owned()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let measure = |definitions: &IndexMap<String, BlockClassDef>| {
            let work = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let label = BlockLabelTypography::resolve(
                &[],
                Some(&[]),
                definitions,
                &base,
                Some("<p>Alpha</p>"),
                "Alpha",
                1,
                &work,
            )
            .unwrap();
            let usage = (
                work.used(),
                work.prepared_text_retained_bytes(),
                work.prepared_text_retained_bytes_peak(),
            );
            assert_eq!(label.style.font_size, 16.0);
            drop(label);
            assert_eq!(work.prepared_text_retained_bytes(), 0);
            usage
        };
        let baseline = measure(&IndexMap::new());
        let mut definitions = IndexMap::new();
        for index in 0..256 {
            let mut definition = BlockClassDef::default();
            definition.id = format!("unused{index}");
            definition.styles = vec![format!("font-family:{}", "x".repeat(1024))];
            definitions.insert(definition.id.clone(), definition);
        }
        assert_eq!(measure(&definitions), baseline);
    }
}
