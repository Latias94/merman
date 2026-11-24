use indexmap::IndexMap;
use merman_model::families::block::BlockClassDef;

use crate::mermaid_style::{self, CssFontSizeContext, TextStyleCascade};
use crate::resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::TextStyle;

#[derive(Debug)]
pub(crate) struct BlockLabelTypography {
    pub(crate) style: TextStyle,
    pub(crate) measurement_xhtml: Option<String>,
    _retained_reservation: PreparedTextRetainedReservation,
    _projection_reservation: Option<PreparedTextRetainedReservation>,
}

fn retained_font_string_bytes(style: &TextStyle) -> usize {
    [&style.font_family, &style.font_weight, &style.font_style]
        .into_iter()
        .filter_map(Option::as_ref)
        .map(String::capacity)
        .sum()
}

fn apply_font_declarations(style: &mut TextStyle, declarations: &[String], root_size: f64) {
    let context = CssFontSizeContext::new(style.font_size, root_size);
    let mut cascade = TextStyleCascade::new(style, context);
    for (_, declaration) in super::node_binding::source_style_winners(declarations) {
        cascade.observe_declaration(&declaration);
    }
    if let std::borrow::Cow::Owned(resolved) = cascade.finish() {
        *style = resolved;
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
    pub(crate) fn resolve(
        styles: &[String],
        classes: Option<&[String]>,
        definitions: &IndexMap<String, BlockClassDef>,
        base: &TextStyle,
        xhtml: Option<&str>,
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
        let owner_bytes =
            std::mem::size_of::<Self>() + if xhtml.is_some() { 0 } else { 192 + key_bytes };
        let retained = string_bytes
            .into_iter()
            .try_fold(owner_bytes, |bytes, length| bytes.checked_add(length))
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut reservation = work.reserve_prepared_text_retained_bytes(retained)?;
        let inline = classes.map_or_else(Vec::new, |classes| {
            super::node_binding::source_font_styles(styles, classes, definitions)
        });
        let root_size = if xhtml.is_some() {
            super::HTML_DOCUMENT_ROOT_FONT_SIZE_PX
        } else {
            base.font_size
        };
        let mut font = base.clone();
        apply_font_declarations(
            &mut font,
            &stylesheet_fonts(definitions, &["block"], work)?,
            root_size,
        );
        // Block's .label rule sets the base family directly on the label g.
        font.font_family.clone_from(&base.font_family);
        let mut declarations = stylesheet_fonts(definitions, &node_classes, work)?;
        declarations.extend(inline.iter().cloned());
        apply_font_declarations(&mut font, &declarations, root_size);
        if xhtml.is_some() {
            apply_font_declarations(
                &mut font,
                &stylesheet_fonts(definitions, &["label"], work)?,
                root_size,
            );
            apply_font_declarations(&mut font, &inline, root_size);
            node_classes.extend(["block", "label"]);
            if classes.is_none() {
                node_classes.push("labelBkg");
            }
            let mut declarations = stylesheet_fonts(definitions, &node_classes, work)?;
            declarations.extend(inline.iter().cloned());
            apply_font_declarations(&mut font, &declarations, root_size);
        } else {
            let mut declarations = stylesheet_fonts(definitions, &["label"], work)?;
            declarations.extend(inline.iter().cloned());
            apply_font_declarations(&mut font, &declarations, root_size);
            if let std::borrow::Cow::Owned(style) = crate::text::svg_markdown_word_style(
                &font,
                crate::text::MermaidMarkdownWordType::Normal,
            ) {
                font = style;
            }
            // The fixed outer row tspan owns its inner word's declarations. The
            // word's presentation weight/style reset is below important class CSS.
            apply_font_declarations(
                &mut font,
                &stylesheet_fonts(definitions, &["row", "text-outer-tspan"], work)?,
                root_size,
            );
        }
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
                reservation.reconcile_downward(owner_bytes + retained_font_string_bytes(&font));
                return Ok(Self {
                    style: font,
                    measurement_xhtml: None,
                    _retained_reservation: reservation,
                    _projection_reservation: None,
                });
            }
            let wrapped = format!("<block-label>{xhtml}</block-label>");
            if let Ok(document) = crate::xml_dom::Document::parse(&wrapped) {
                let root = document.root_element();
                let mut edits = Vec::new();
                let mut projected_bytes = xhtml.len();
                // .nodeLabel/.edgeLabel are ancestors of retained spans as well as
                // parents of the top-level p. Project their fixed font rules once;
                // the shared XHTML measurement traversal resolves the actual fonts.
                node_classes.push(terminal_class);
                let span_fonts = stylesheet_fonts(definitions, &node_classes, work)?;
                for node in root.descendants().skip(1) {
                    work.charge(1)?;
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
            }
        }
        reservation.reconcile_downward(owner_bytes + retained_font_string_bytes(&font));
        Ok(Self {
            style: font,
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
    fn prepared_html_rem_fonts_use_document_root() {
        for font_size in [14.0, 24.0] {
            for html in [false, true] {
                for node in [false, true] {
                    let work = Arc::new(OperationWorkMeter::new(
                        RenderResourcePolicy::unbounded_for_trusted_input(),
                    ));
                    let base = TextStyle {
                        font_size,
                        ..TextStyle::default()
                    };
                    let mut definitions = IndexMap::new();
                    let styles = if node {
                        vec!["font-size:2rem".to_owned()]
                    } else {
                        let mut definition = BlockClassDef::default();
                        definition.id = "edgeLabel".to_owned();
                        definition.styles = vec!["font-size:2rem".to_owned()];
                        definitions.insert(definition.id.clone(), definition);
                        Vec::new()
                    };
                    let prepared = BlockLabelTypography::resolve(
                        &styles,
                        node.then_some([].as_slice()),
                        &definitions,
                        &base,
                        html.then_some("<p>Alpha</p>"),
                        1,
                        &work,
                    )
                    .unwrap();
                    assert_eq!(
                        prepared.style.font_size,
                        if html { 32.0 } else { font_size * 2.0 },
                        "diagram font {font_size}, HTML {html}, node {node}"
                    );
                }
            }
        }
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
