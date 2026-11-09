//! Bounded terminal-font inheritance for prepared XHTML labels.

use std::collections::HashMap;
use std::sync::Arc;

use super::{InlineTextRun, TextMeasurer, TextMetrics, TextStyle, WrapMode};
use crate::mermaid_style::{CssFontSizeContext, TextStyleCascade, visit_parsed_style_declarations};

pub(super) fn same_font(left: Option<&TextStyle>, right: Option<&TextStyle>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            std::ptr::eq(left, right)
                || (left.font_size == right.font_size
                    && left.font_family == right.font_family
                    && left.font_weight == right.font_weight
                    && left.font_style == right.font_style)
        }
        _ => false,
    }
}

pub(super) struct TerminalFonts {
    root_font_size: f64,
    // The cached ancestry maximum makes text-range struts independent of stack depth.
    stack: Vec<(String, Arc<TextStyle>, f64)>,
    open_indices: HashMap<String, Vec<usize>>,
    has_font_declarations: bool,
    line_sizes: Vec<f64>,
    #[cfg(test)]
    stack_work: FontStackWork,
}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
struct FontStackWork {
    close_lookups: usize,
    pushes: usize,
    drains: usize,
}

impl TerminalFonts {
    pub(super) fn new(style: &TextStyle, root_font_size: f64) -> Self {
        Self {
            root_font_size,
            stack: vec![(String::new(), Arc::new(style.clone()), style.font_size)],
            open_indices: HashMap::new(),
            has_font_declarations: false,
            line_sizes: vec![style.font_size],
            #[cfg(test)]
            stack_work: FontStackWork::default(),
        }
    }

    pub(super) fn current(&self) -> &Arc<TextStyle> {
        &self
            .stack
            .last()
            .expect("terminal font root remains present")
            .1
    }

    pub(super) fn current_ancestor_strut_font_size(&self) -> f64 {
        let (_, style, maximum) = self
            .stack
            .last()
            .expect("terminal font root remains present");
        // The terminal run and operation root already own their own line-height probes/floors.
        // Carry only an intermediate ancestor that they cannot otherwise represent.
        if *maximum > style.font_size && *maximum > self.stack[0].1.font_size {
            *maximum
        } else {
            0.0
        }
    }

    pub(super) fn has_font_declarations(&self) -> bool {
        self.has_font_declarations
    }

    pub(super) fn line_sizes(&self) -> &[f64] {
        &self.line_sizes
    }

    pub(super) fn observe_tag(&mut self, name: &str, tag: &str, closing: bool) {
        if closing {
            #[cfg(test)]
            {
                self.stack_work.close_lookups += 1;
            }
            if let Some(index) = self
                .open_indices
                .get(name)
                .and_then(|indices| indices.last().copied())
            {
                // Only suffixes are removed, so surviving stack indices stay valid. Each open
                // element is drained once; unmatched closes never scan the ancestry stack.
                for (element, _, _) in self.stack.drain(index..).rev() {
                    let indices = self
                        .open_indices
                        .get_mut(&element)
                        .expect("each open element has an index");
                    indices.pop().expect("each open element has an index");
                    if indices.is_empty() {
                        self.open_indices.remove(&element);
                    }
                    #[cfg(test)]
                    {
                        self.stack_work.drains += 1;
                    }
                }
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
        for property in ["font-weight", "font-style", "font-family", "font-size"] {
            if let Some(value) = crate::text::xhtml_font_default(name, property) {
                cascade.observe(property, value, false);
            }
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
        let resolved = match cascade.finish() {
            std::borrow::Cow::Borrowed(_) => Arc::clone(parent),
            std::borrow::Cow::Owned(style) => Arc::new(style),
        };
        self.has_font_declarations |= has_font_declarations || matches!(name, "big" | "small");
        if !tag.trim_end().ends_with('/')
            && !matches!(
                name,
                "area"
                    | "base"
                    | "br"
                    | "col"
                    | "embed"
                    | "hr"
                    | "img"
                    | "input"
                    | "link"
                    | "meta"
                    | "param"
                    | "source"
                    | "track"
                    | "wbr"
            )
        {
            self.open_indices
                .entry(name.to_owned())
                .or_default()
                .push(self.stack.len());
            let maximum = self
                .stack
                .last()
                .expect("terminal font root remains present")
                .2
                .max(resolved.font_size);
            self.stack.push((name.to_owned(), resolved, maximum));
            #[cfg(test)]
            {
                self.stack_work.pushes += 1;
            }
        }
    }

    pub(super) fn new_line(&mut self) {
        let break_font_size = self
            .stack
            .last()
            .expect("terminal font root remains present")
            .2;
        if let Some(size) = self.line_sizes.last_mut() {
            *size = size.max(break_font_size);
        }
        self.line_sizes.push(self.stack[0].1.font_size);
    }

    pub(super) fn line_height_px(&self, index: usize, base: &TextStyle) -> f64 {
        self.line_sizes
            .get(index)
            .copied()
            .unwrap_or(base.font_size)
            .max(0.0)
            * 1.5
    }

    pub(super) fn measure<M: TextMeasurer + ?Sized>(
        &self,
        measurer: &M,
        runs_by_line: &[Vec<InlineTextRun>],
        base: &TextStyle,
        explicit_line_boxes: usize,
    ) -> TextMetrics {
        let width = runs_by_line
            .iter()
            .map(|runs| {
                super::measure_inline_runs_width_px(measurer, runs, base, WrapMode::HtmlLike, false)
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
                        let height = if run.replaced_width_px.is_some() {
                            style.font_size.max(0.0) * 1.5
                        } else {
                            // Use the routed terminal measurement for host-specific vertical metrics.
                            // The parent strut supplies the floor without rounding up host precision.
                            measurer.measure_html_line_height_px(&run.text, style)
                        };
                        height.max(run.ancestor_strut_height_px())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_strut_cache_restores_live_ancestry_without_marking_empty_lines() {
        let style = TextStyle {
            font_size: 0.0,
            ..TextStyle::default()
        };
        let mut fonts = TerminalFonts::new(&style, 16.0);
        fonts.observe_tag("span", "span style='font-size:64px'", false);
        fonts.observe_tag("span", "span style='font-size:0px'", false);
        assert_eq!(fonts.current_ancestor_strut_font_size(), 64.0);
        assert_eq!(fonts.line_sizes(), &[0.0], "openings alone create no strut");
        fonts.observe_tag("img", "img style='font-size:128px'", false);
        assert_eq!(fonts.current_ancestor_strut_font_size(), 64.0);
        fonts.new_line();
        assert_eq!(fonts.line_sizes(), &[64.0, 0.0]);
        fonts.observe_tag("span", "/span", true);
        fonts.observe_tag("span", "/span", true);
        assert_eq!(fonts.current_ancestor_strut_font_size(), 0.0);
        assert_eq!(fonts.current().font_size, 0.0);
        fonts.observe_tag("span", "span style='font-size:10px'", false);
        assert_eq!(fonts.current_ancestor_strut_font_size(), 0.0);
        assert_eq!(fonts.current().font_size, 10.0);
    }

    #[test]
    fn unmatched_close_work_is_linear_in_open_depth_and_close_count() {
        let style = TextStyle {
            font_family: Some("Inter, sans-serif".to_owned()),
            font_size: 16.0,
            ..TextStyle::default()
        };
        for count in [0, 1, 32, 512, 4096] {
            let mut fonts = TerminalFonts::new(&style, 16.0);
            let root = Arc::clone(fonts.current());
            for _ in 0..count {
                fonts.observe_tag("span", "span", false);
            }
            for _ in 0..count {
                fonts.observe_tag("b", "/b", true);
            }
            assert_eq!(
                fonts.stack_work,
                FontStackWork {
                    close_lookups: count,
                    pushes: count,
                    drains: 0,
                }
            );
            assert_eq!(fonts.stack.len(), count + 1);
            assert!(Arc::ptr_eq(fonts.current(), &root));
            for _ in 0..count {
                fonts.observe_tag("span", "/span", true);
            }
            assert_eq!(
                fonts.stack_work,
                FontStackWork {
                    close_lookups: count * 2,
                    pushes: count,
                    drains: count,
                }
            );
            assert_eq!(fonts.stack.len(), 1);
            assert!(fonts.open_indices.is_empty());
            assert!(Arc::ptr_eq(fonts.current(), &root));
        }
    }

    #[test]
    fn ancestor_closures_roll_back_each_tag_index_and_restore_parent_fonts() {
        let style = TextStyle {
            font_size: 16.0,
            font_weight: Some("400".to_owned()),
            ..TextStyle::default()
        };
        let mut fonts = TerminalFonts::new(&style, 16.0);
        let root = Arc::clone(fonts.current());
        fonts.observe_tag("span", "span style='font-weight:100'", false);
        let outer = Arc::clone(fonts.current());
        fonts.observe_tag("strong", "strong", false);
        fonts.observe_tag("em", "em", false);
        fonts.observe_tag("strong", "strong", false);
        assert_eq!(fonts.current().font_weight.as_deref(), Some("700"));
        fonts.observe_tag("em", "/em", true);
        assert_eq!(fonts.stack.len(), 3);
        assert_eq!(fonts.open_indices.get("strong"), Some(&vec![2]));
        assert!(!fonts.open_indices.contains_key("em"));
        assert_eq!(fonts.current().font_weight.as_deref(), Some("400"));
        assert_eq!(fonts.current().font_style, None);
        fonts.observe_tag("strong", "/strong", true);
        assert!(Arc::ptr_eq(fonts.current(), &outer));
        assert!(!fonts.open_indices.contains_key("strong"));

        // Reusing drained indices must not leave an old descendant in the name index.
        fonts.observe_tag("strong", "strong", false);
        fonts.observe_tag("span", "span", false);
        fonts.observe_tag("span", "/span", true);
        assert_eq!(fonts.current().font_weight.as_deref(), Some("400"));
        assert_eq!(fonts.open_indices.get("span"), Some(&vec![1]));
        fonts.observe_tag("span", "/span", true);
        assert_eq!(fonts.stack.len(), 1);
        assert!(fonts.open_indices.is_empty());
        assert!(Arc::ptr_eq(fonts.current(), &root));
        assert_eq!(fonts.stack_work.pushes, fonts.stack_work.drains);
    }

    #[test]
    fn authored_initial_resets_override_primitive_fonts_without_changing_the_parent() {
        let style = TextStyle {
            font_family: Some("Inter, sans-serif".to_owned()),
            font_size: 24.0,
            font_weight: Some("600".to_owned()),
            font_style: Some("oblique".to_owned()),
        };
        for (name, tag, weight, font_style) in [
            (
                "strong",
                "strong style='font-weight:initial'",
                "400",
                "oblique",
            ),
            ("em", "em style='font-style:initial'", "600", "normal"),
            (
                "b",
                "b style='font-weight:initial!important;font-weight:900'",
                "400",
                "oblique",
            ),
            (
                "i",
                "i style='font-style:initial!important;font-style:italic'",
                "600",
                "normal",
            ),
        ] {
            let mut fonts = TerminalFonts::new(&style, 16.0);
            let root = Arc::clone(fonts.current());
            fonts.observe_tag(name, tag, false);
            assert_eq!(
                fonts.current().font_weight.as_deref(),
                Some(weight),
                "{tag}"
            );
            assert_eq!(
                fonts.current().font_style.as_deref(),
                Some(font_style),
                "{tag}"
            );
            assert_eq!(fonts.current().font_family, style.font_family);
            assert_eq!(fonts.current().font_size, style.font_size);
            fonts.observe_tag(name, name, true);
            assert!(Arc::ptr_eq(fonts.current(), &root));
        }
    }
}
