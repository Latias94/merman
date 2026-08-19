//! Operation-local prepared text attached to one Sequence terminal SVG.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use merman_core::OperationPhase;

use crate::diagram_theme::{FontStyle, ThemeTextStyle};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitPhase};
use crate::text::{
    ParsedCssFontStack, PrepareTextRequest, PreparedTextLabelFamily, PreparedTextLabelId,
    PreparedTextLabelLedgerEntry, PreparedTextLayout, PreparedTextWrap, parse_css_font_stack,
};
use crate::{Error, Result};

#[derive(Debug)]
pub(crate) struct SequenceTextSidecar {
    prepared_text_layout: Option<PreparedTextLayout>,
    base_typography: ThemeTextStyle,
    role_typography: Arc<super::typography::SequenceTypographyPlan>,
    work_meter: Arc<OperationWorkMeter>,
    labels: RefCell<Vec<PreparedTextLabelLedgerEntry>>,
    retained_reservations: RefCell<Vec<PreparedTextRetainedReservation>>,
}

impl SequenceTextSidecar {
    pub(crate) fn new(
        prepared_text_layout: Option<&PreparedTextLayout>,
        role_typography: Arc<super::typography::SequenceTypographyPlan>,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        let base_typography = role_typography.base_prepared_typography();
        Self {
            prepared_text_layout: prepared_text_layout.cloned(),
            base_typography,
            role_typography,
            work_meter,
            labels: RefCell::new(Vec::new()),
            retained_reservations: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn bind_terminal_svg(&self, mut svg: String) -> Result<String> {
        let Some(layout) = self.prepared_text_layout.as_ref() else {
            return Ok(svg);
        };
        if !self.labels.borrow().is_empty() || !self.retained_reservations.borrow().is_empty() {
            return Err(sequence_text_error(
                "Sequence prepared text cannot be bound more than once",
            ));
        }

        let document = roxmltree::Document::parse(&svg)
            .map_err(|error| sequence_text_error(error.to_string()))?;
        let mut prepared = Vec::new();
        let mut edits = Vec::new();

        for text in document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
        {
            let fragments = text
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            if fragments.is_empty() {
                continue;
            }
            if fragments.len() != 1 {
                return Err(sequence_text_error(
                    "Sequence prepared text does not yet admit multi-fragment terminal labels",
                ));
            }
            if text.attribute("id").is_some() {
                return Err(sequence_text_error(
                    "Sequence prepared text requires an unclaimed terminal text id",
                ));
            }

            let role = terminal_text_role(text);
            let role_typography = role.map(|role| self.role_typography.role(role));
            let base_typography = role_typography
                .map(super::typography::SequenceResolvedTypography::prepared_typography)
                .unwrap_or(&self.base_typography);
            let (requested_typography, emitted_font_stack) =
                terminal_text_typography(base_typography, text)?;
            let typography = layout
                .admit_typography_with_css_font_stack(
                    &requested_typography,
                    emitted_font_stack
                        .as_ref()
                        .filter(|stack| stack.contains_named_family())
                        .or_else(|| {
                            role_typography
                                .map(super::typography::SequenceResolvedTypography::font_stack)
                        })
                        .or_else(|| Some(self.role_typography.inherited_font_stack())),
                )
                .map_err(Error::from)?;
            let request = PrepareTextRequest::new(fragments[0], typography.typography().clone())
                .with_family_normalized_projection()
                .with_wrap(PreparedTextWrap::SingleRun);
            let prepared_text = layout
                .prepare_text_with_work_meter(&request, &self.work_meter)
                .map_err(Error::from)?;
            let pending = prepared_text.label_ledger_entry().ok_or_else(|| {
                sequence_text_error("Sequence text backend omitted label evidence")
            })?;
            let key = u32::try_from(prepared.len()).map_err(|_| {
                sequence_text_error("Sequence prepared label identity overflowed u32")
            })?;
            let id = PreparedTextLabelId::new(PreparedTextLabelFamily::Sequence, key);
            let entry = pending.bind(id);
            let reservation = self
                .work_meter
                .reserve_prepared_text_retained_bytes(entry.retained_bytes())
                .map_err(Error::from)?;
            let style = typography.merge_emission_font_style(text.attribute("style"));
            edits.extend(text_start_tag_edits(&svg, text, id, &style)?);
            prepared.push((entry, reservation));
        }
        drop(document);

        if prepared.is_empty() {
            return Ok(svg);
        }
        svg = rebuild_svg_with_edits(&svg, &mut edits, &self.work_meter)?;
        let (entries, reservations): (Vec<_>, Vec<_>) = prepared.into_iter().unzip();
        *self.labels.borrow_mut() = entries;
        *self.retained_reservations.borrow_mut() = reservations;
        Ok(svg)
    }

    pub(crate) fn prepared_text_label_ledger(&self) -> Vec<PreparedTextLabelLedgerEntry> {
        self.labels.borrow().clone()
    }

    pub(crate) fn take_prepared_text_retained_reservations(
        &self,
    ) -> Vec<PreparedTextRetainedReservation> {
        std::mem::take(&mut *self.retained_reservations.borrow_mut())
    }
}

fn terminal_text_role(
    text: roxmltree::Node<'_, '_>,
) -> Option<super::typography::SequenceTypographyRole> {
    let classes = text.attribute("class").unwrap_or_default();
    let has_class = |expected: &str| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == expected)
    };
    if has_class("actor") || has_class("actor-box") || has_class("actor-man") {
        return Some(super::typography::SequenceTypographyRole::Actor);
    }
    // Sequence currently reserves the generic `text` class for box titles. Layout and terminal
    // emission both project those participant-group labels through ActorLabel typography.
    if has_class("text") {
        return Some(super::typography::SequenceTypographyRole::Actor);
    }
    if has_class("messageText") || has_class("sequenceNumber") {
        return Some(super::typography::SequenceTypographyRole::Message);
    }
    if has_class("noteText") {
        return Some(super::typography::SequenceTypographyRole::Note);
    }
    if has_class("loopText") || has_class("sectionTitle") || has_class("labelText") {
        return Some(super::typography::SequenceTypographyRole::Loop);
    }
    None
}

fn terminal_text_typography(
    base: &ThemeTextStyle,
    text: roxmltree::Node<'_, '_>,
) -> Result<(ThemeTextStyle, Option<ParsedCssFontStack>)> {
    let mut typography = base.clone();
    let mut font_stack = text.attribute("font-family").and_then(parse_css_font_stack);
    if let Some(value) = text.attribute("font-size") {
        typography = apply_font_size(typography, value)?;
    }
    if let Some(value) = text.attribute("font-weight") {
        typography = apply_font_weight(typography, value)?;
    }
    if let Some(value) = text.attribute("font-style") {
        typography = apply_font_style(typography, value)?;
    }
    for declaration in text.attribute("style").unwrap_or_default().split(';') {
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(declaration) else {
            continue;
        };
        match parsed.property().to_ascii_lowercase().as_str() {
            "font-family" => font_stack = parse_css_font_stack(parsed.value()),
            "font-size" => typography = apply_font_size(typography, parsed.value())?,
            "font-weight" => typography = apply_font_weight(typography, parsed.value())?,
            "font-style" => typography = apply_font_style(typography, parsed.value())?,
            _ => {}
        }
    }
    Ok((typography, font_stack))
}

fn apply_font_size(typography: ThemeTextStyle, value: &str) -> Result<ThemeTextStyle> {
    let declaration = format!("font-size:{value}");
    let parsed = crate::mermaid_style::parse_style_declaration(&declaration)
        .and_then(|parsed| parsed.svg_number_or_px())
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| sequence_text_error("Sequence terminal font size is unsupported"))?;
    typography
        .with_font_size_px(parsed as f32)
        .map_err(|error| sequence_text_error(error.to_string()))
}

fn apply_font_weight(typography: ThemeTextStyle, value: &str) -> Result<ThemeTextStyle> {
    let weight = match value.trim().to_ascii_lowercase().as_str() {
        "normal" => 400,
        "bold" => 700,
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| sequence_text_error("Sequence terminal font weight is unsupported"))?,
    };
    typography
        .with_font_weight(weight)
        .map_err(|error| sequence_text_error(error.to_string()))
}

fn apply_font_style(typography: ThemeTextStyle, value: &str) -> Result<ThemeTextStyle> {
    let style = match value.trim().to_ascii_lowercase().as_str() {
        "normal" => FontStyle::Normal,
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique,
        _ => {
            return Err(sequence_text_error(
                "Sequence terminal font style is unsupported",
            ));
        }
    };
    Ok(typography.with_font_style(style))
}

#[derive(Debug)]
struct TextEdit {
    range: Range<usize>,
    replacement: String,
}

fn text_start_tag_edits(
    svg: &str,
    text: roxmltree::Node<'_, '_>,
    id: PreparedTextLabelId,
    style: &str,
) -> Result<Vec<TextEdit>> {
    let node_range = text.range();
    let start_tag_end = find_start_tag_end(svg, node_range.clone())?;
    let id_replacement = format!(r#"id="{}""#, id.as_svg_id());
    let style_replacement = format!(r#"style="{}""#, escape_xml_attribute(style));
    if let Some(attribute) = text
        .attributes()
        .find(|attribute| attribute.name() == "style")
    {
        Ok(vec![
            TextEdit {
                range: attribute.range(),
                replacement: style_replacement,
            },
            TextEdit {
                range: start_tag_end..start_tag_end,
                replacement: format!(" {id_replacement}"),
            },
        ])
    } else {
        Ok(vec![TextEdit {
            range: start_tag_end..start_tag_end,
            replacement: format!(" {style_replacement} {id_replacement}"),
        }])
    }
}

fn find_start_tag_end(svg: &str, range: Range<usize>) -> Result<usize> {
    let range_start = range.start;
    let source = svg
        .get(range)
        .ok_or_else(|| sequence_text_error("Sequence text source range is invalid"))?;
    let mut quote = None;
    for (offset, character) in source.char_indices() {
        match (quote, character) {
            (Some(active), current) if active == current => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, '>') => return Ok(range_start + offset),
            _ => {}
        }
    }
    Err(sequence_text_error(
        "Sequence terminal text start tag is not closed",
    ))
}

fn rebuild_svg_with_edits(
    svg: &str,
    edits: &mut [TextEdit],
    work_meter: &OperationWorkMeter,
) -> Result<String> {
    edits.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then_with(|| left.range.end.cmp(&right.range.end))
    });

    let mut previous_end = 0usize;
    for edit in edits.iter() {
        if edit.range.start > edit.range.end
            || edit.range.end > svg.len()
            || !svg.is_char_boundary(edit.range.start)
            || !svg.is_char_boundary(edit.range.end)
        {
            return Err(sequence_text_error(
                "Sequence prepared text edit range is invalid",
            ));
        }
        if edit.range.start < previous_end {
            return Err(sequence_text_error(
                "Sequence prepared text edit ranges overlap",
            ));
        }
        previous_end = previous_end.max(edit.range.end);
    }

    let projected = edits.iter().try_fold(svg.len(), |len, edit| {
        len.checked_sub(edit.range.len())?
            .checked_add(edit.replacement.len())
    });
    let projected = projected
        .ok_or_else(|| sequence_text_error("Sequence prepared SVG size overflowed usize"))?;
    work_meter
        .policy()
        .check_svg_byte_count(projected, ResourceLimitPhase::SvgOutput)
        .map_err(Error::from)?;
    work_meter
        .checkpoint(OperationPhase::Emit)
        .map_err(Error::from)?;

    let mut rebuilt = String::new();
    rebuilt
        .try_reserve_exact(projected)
        .map_err(|_| sequence_text_error("Sequence prepared SVG allocation failed"))?;

    let mut cursor = 0usize;
    for edit in edits {
        work_meter
            .checkpoint(OperationPhase::Emit)
            .map_err(Error::from)?;
        rebuilt.push_str(&svg[cursor..edit.range.start]);
        rebuilt.push_str(&edit.replacement);
        cursor = edit.range.end;
    }
    work_meter
        .checkpoint(OperationPhase::Emit)
        .map_err(Error::from)?;
    rebuilt.push_str(&svg[cursor..]);
    debug_assert_eq!(rebuilt.len(), projected);
    Ok(rebuilt)
}

fn escape_xml_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn sequence_text_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("sequence-prepared-text", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    #[test]
    fn prepared_text_edits_rebuild_the_svg_in_source_order() {
        let source = "0123456789";
        let mut edits = vec![
            TextEdit {
                range: 8..10,
                replacement: "XY".to_string(),
            },
            TextEdit {
                range: 2..4,
                replacement: "ab".to_string(),
            },
            TextEdit {
                range: 6..6,
                replacement: "!".to_string(),
            },
        ];

        let rebuilt = rebuild_svg_with_edits(source, &mut edits, &work_meter())
            .expect("non-overlapping prepared-text edits");

        assert_eq!(rebuilt, "01ab45!67XY");
    }

    #[test]
    fn prepared_text_edits_reject_overlapping_source_ranges() {
        let source = "0123456789";
        let mut edits = vec![
            TextEdit {
                range: 2..6,
                replacement: "first".to_string(),
            },
            TextEdit {
                range: 4..8,
                replacement: "second".to_string(),
            },
        ];

        let error = rebuild_svg_with_edits(source, &mut edits, &work_meter())
            .expect_err("overlapping prepared-text edits must fail closed");

        assert!(error.to_string().contains("overlap"), "{error}");
    }
}
