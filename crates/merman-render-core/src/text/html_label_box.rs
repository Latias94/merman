use quick_xml::{Reader, events::Event};

use crate::mermaid_style::{
    ParsedStyleDeclaration, parse_html_label_style_declaration,
    visit_style_declaration_boundaries_with_checkpoints,
};
use crate::text::trim_html_collapsible_ascii_whitespace;

#[derive(Clone, Copy, Debug)]
pub struct HtmlLabelBox {
    pub width: Option<f64>,
    pub height: Option<f64>,
}

#[derive(Clone, Copy)]
pub struct HtmlLabelWidth {
    pub outer: f64,
    pub wrap: Option<f64>,
}

impl HtmlLabelBox {
    pub fn width_in_label(self, wrapping_width: Option<f64>) -> Option<HtmlLabelWidth> {
        let authored = self.width?;
        let Some(limit) = wrapping_width.filter(|limit| limit.is_finite() && *limit > 0.0) else {
            return Some(HtmlLabelWidth {
                outer: authored,
                wrap: None,
            });
        };
        let initial = authored.min(layout_px(limit));
        Some(if initial == limit {
            HtmlLabelWidth {
                outer: authored,
                wrap: Some(authored),
            }
        } else {
            HtmlLabelWidth {
                outer: initial,
                wrap: None,
            }
        })
    }

    pub fn from_xhtml<E>(
        xhtml: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Self>, E> {
        let xhtml = trim_html_collapsible_ascii_whitespace(xhtml);
        let xhtml = xhtml
            .strip_prefix("<p></p>")
            .and_then(|content| content.strip_suffix("<p></p>"))
            .unwrap_or(xhtml);
        if !xhtml.starts_with("<div ") || !xhtml.ends_with("</div>") {
            return Ok(None);
        }
        let mut reader = Reader::from_str(xhtml);
        let Ok(Event::Start(root)) = reader.read_event() else {
            return Ok(None);
        };
        if root.name().as_ref() != "div" {
            return Ok(None);
        }
        let Ok(Some(style)) = root.try_get_attribute("style") else {
            return Ok(None);
        };
        let style = merman_model::text::decode_html_entities_to_unicode(&style.value);
        let Some(dimensions) = Self::from_style(&style, checkpoint)? else {
            return Ok(None);
        };

        let mut depth = 1usize;
        while depth > 0 {
            checkpoint()?;
            match reader.read_event() {
                Ok(Event::Start(_)) => depth += 1,
                Ok(Event::End(_)) => depth -= 1,
                Ok(Event::Eof) | Err(_) => return Ok(None),
                _ => {}
            }
        }
        Ok(matches!(reader.read_event(), Ok(Event::Eof)).then_some(dimensions))
    }

    fn from_style<E>(
        style: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Self>, E> {
        let mut width = InlineDeclaration::default();
        let mut height = InlineDeclaration::default();
        let mut display = InlineDeclaration::default();
        let mut complex_box = false;
        visit_style_declaration_boundaries_with_checkpoints(style, checkpoint, |boundary| {
            let Some(declaration) = parse_html_label_style_declaration(boundary.raw()) else {
                return Ok(true);
            };
            match declaration.property() {
                property @ ("width" | "height") => {
                    let slot = if property == "width" {
                        &mut width
                    } else {
                        &mut height
                    };
                    if let Ok(length) = declaration.html_box_length_px() {
                        slot.observe(length.map(layout_px), declaration.important());
                    }
                }
                "display" => {
                    if let Some(simple_block) = simple_block_display(&declaration) {
                        display.observe(Some(simple_block), declaration.important());
                    }
                }
                name if uses_other_box_layout(name) => {
                    complex_box = true;
                    return Ok(false);
                }
                _ => {}
            }
            Ok(true)
        })?;
        let width = width.value;
        let height = height.value;
        Ok(
            (!complex_box
                && display.value.unwrap_or(true)
                && (width.is_some() || height.is_some()))
            .then_some(Self { width, height }),
        )
    }
}

#[derive(Default)]
struct InlineDeclaration<T> {
    value: T,
    important: bool,
}

impl<T> InlineDeclaration<T> {
    fn observe(&mut self, value: T, important: bool) {
        if important || !self.important {
            *self = Self { value, important };
        }
    }
}

fn simple_block_display(declaration: &ParsedStyleDeclaration<'_>) -> Option<bool> {
    let simple = match declaration.analysis().single_ident() {
        Some("block") => true,
        Some(
            "none" | "contents" | "inline" | "flow-root" | "flex" | "grid" | "table" | "ruby"
            | "list-item" | "inline-block" | "inline-flex" | "inline-grid" | "inline-table"
            | "table-row" | "table-cell" | "inherit" | "initial" | "unset" | "revert"
            | "revert-layer",
        ) => false,
        None => false,
        Some(_) => return None,
    };
    Some(simple)
}

fn uses_other_box_layout(property: &str) -> bool {
    matches!(
        property,
        "all"
            | "aspect-ratio"
            | "box-sizing"
            | "float"
            | "position"
            | "transform"
            | "white-space"
            | "writing-mode"
    ) || [
        "min-",
        "max-",
        "inline-size",
        "block-size",
        "border",
        "margin",
        "padding",
    ]
    .iter()
    .any(|prefix| property.starts_with(prefix))
}

fn layout_px(length: f64) -> f64 {
    const LAYOUT_UNITS_PER_PX: f64 = 64.0;
    f64::from((length * LAYOUT_UNITS_PER_PX) as i32) / LAYOUT_UNITS_PER_PX
}
