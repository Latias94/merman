//! Terminal fonts and line boxes for sanitized XHTML labels.

use std::sync::Arc;

use super::{
    HtmlBreakKind, InlineBreakPlan, InlineLinePlan, InlineTextRun, TextMeasurer, TextMetrics,
    TextStyle, WrapMode, html_line_text_with_break, same_inline_style,
};
use crate::mermaid_style::{CssFontSizeContext, TextStyleCascade, visit_parsed_style_declarations};

fn run_line_height_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    text: &str,
    style: &TextStyle,
) -> f64 {
    // The routed host can supply taller font extents; inherited unitless line-height:1.5 is
    // still the CSS floor for every resolved font, independently of the diagram family.
    measurer
        .measure_html_line_height_px(text, style)
        .max(style.font_size.max(1.0) * 1.5)
}

pub(super) fn measure_line_height_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    base: &TextStyle,
) -> f64 {
    runs.iter()
        .filter(|run| !run.text.is_empty())
        .filter_map(|run| {
            let style = run.terminal_style.as_deref()?;
            Some(run_line_height_px(measurer, &run.text, style).max(base.font_size.max(1.0) * 1.5))
        })
        .fold(0.0_f64, f64::max)
}

pub(super) fn measure_fragments_line_height_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    breaks: &InlineBreakPlan,
    line: InlineLinePlan,
    selected_break: HtmlBreakKind,
    base: &TextStyle,
) -> f64 {
    let mut height = 0.0_f64;
    let mut index = line.fragment_start;
    while index < line.fragment_end {
        let first = breaks.fragments[index];
        let run = &runs[first.run_index];
        let mut end = index + 1;
        while end < line.fragment_end
            && same_inline_style(run, &runs[breaks.fragments[end].run_index])
        {
            end += 1;
        }
        if let Some(style) = run.terminal_style.as_deref() {
            // Probe the selected line's complete style group, including a conditional hyphen.
            // Unstyled callers issue no new requests through this font-specific path.
            let last = breaks.fragments[end - 1];
            let text = html_line_text_with_break(
                &breaks.source[first.start..last.end],
                end == line.fragment_end && selected_break == HtmlBreakKind::SoftHyphen,
            );
            height = height
                .max(run_line_height_px(measurer, &text, style))
                .max(base.font_size.max(1.0) * 1.5);
        }
        index = end;
    }
    height
}

pub(super) fn same_font(left: Option<&TextStyle>, right: Option<&TextStyle>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.font_size == right.font_size
                && left.font_family == right.font_family
                && left.font_weight == right.font_weight
                && left.font_style == right.font_style
        }
        _ => false,
    }
}

pub(super) struct TerminalFonts {
    root_font_size: f64,
    stack: Vec<(String, Arc<TextStyle>)>,
    has_font_declarations: bool,
    line_sizes: Vec<f64>,
}

impl TerminalFonts {
    #[cfg(any(
        feature = "diagram-block",
        feature = "diagram-class",
        feature = "diagram-er"
    ))]
    pub(super) fn new(style: &TextStyle, root_font_size: f64) -> Self {
        Self {
            root_font_size,
            stack: vec![(String::new(), Arc::new(style.clone()))],
            has_font_declarations: false,
            line_sizes: vec![style.font_size],
        }
    }

    pub(super) fn current(&self) -> &Arc<TextStyle> {
        &self
            .stack
            .last()
            .expect("terminal font root remains present")
            .1
    }

    pub(super) fn has_font_declarations(&self) -> bool {
        self.has_font_declarations
    }

    pub(super) fn observe_tag(&mut self, name: &str, tag: &str, closing: bool) {
        if closing {
            if let Some(index) = self.stack.iter().rposition(|(element, _)| element == name) {
                self.stack.truncate(index.max(1));
            }
            return;
        }
        let parent = self.current();
        let mut cascade = TextStyleCascade::new(
            parent,
            CssFontSizeContext::new(parent.font_size, self.root_font_size),
        );
        // Element primitives precede authored declarations. Explicit inherit reads the parent,
        // so font-weight:normal/inherit on strong can override its intrinsic emphasis.
        match name {
            "strong" | "b" if !super::style_requests_bold_font_weight(parent) => {
                cascade.observe("font-weight", "700", false);
            }
            "em" | "i" if !super::style_requests_italic_font_style(parent) => {
                cascade.observe("font-style", "italic", false);
            }
            "code" => cascade.observe("font-family", "monospace", false),
            _ => {}
        }
        let opening = quick_xml::events::BytesStart::from_content(tag, name.len());
        let mut has_font_declarations = false;
        if let Some(attribute) = opening
            .attributes()
            .with_checks(false)
            .filter_map(Result::ok)
            .find(|attribute| attribute.key.as_ref().eq_ignore_ascii_case("style"))
        {
            let declarations =
                merman_model::text::decode_html_entities_to_unicode(attribute.value.as_ref());
            visit_parsed_style_declarations(&declarations, |declaration| {
                if matches!(
                    declaration.property(),
                    "font-size" | "font-family" | "font-weight" | "font-style"
                ) {
                    has_font_declarations = true;
                    cascade.observe_declaration(&declaration);
                }
            });
        }
        let resolved = Arc::new(cascade.finish().into_owned());
        self.has_font_declarations |= has_font_declarations;
        if !tag.trim_end().ends_with('/') {
            self.stack.push((name.to_owned(), resolved));
        }
    }

    pub(super) fn new_line(&mut self) {
        self.line_sizes.push(self.current().font_size);
    }

    pub(super) fn line_height_px(&self, index: usize, base: &TextStyle) -> f64 {
        self.line_sizes
            .get(index)
            .copied()
            .unwrap_or(base.font_size)
            .max(1.0)
            * 1.5
    }

    pub(super) fn measure<M: TextMeasurer + ?Sized>(
        &self,
        measurer: &M,
        runs_by_line: &[Vec<InlineTextRun>],
        icon_widths: &[f64],
        base: &TextStyle,
        explicit_line_boxes: usize,
    ) -> TextMetrics {
        let width = runs_by_line
            .iter()
            .enumerate()
            .map(|(index, runs)| {
                super::measure_inline_runs_width_px(measurer, runs, base, WrapMode::HtmlLike, false)
                    + icon_widths.get(index).copied().unwrap_or(0.0)
            })
            .fold(0.0_f64, f64::max);
        let line_count = runs_by_line.len().max(explicit_line_boxes);
        let height = (0..line_count)
            .map(|index| {
                let strut = self.line_height_px(index, base);
                runs_by_line
                    .get(index)
                    .into_iter()
                    .flatten()
                    .filter(|run| !run.text.is_empty())
                    .map(|run| {
                        let style = run.terminal_style.as_deref().unwrap_or(base);
                        run_line_height_px(measurer, &run.text, style)
                    })
                    .fold(strut, f64::max)
            })
            .sum();
        TextMetrics {
            width: super::round_to_1_64_px(width),
            height,
            line_count,
        }
    }
}
