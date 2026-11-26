//! Rich HTML measurement and numeric planning for decoded label rows.

use super::line_break::html_break_spaces_segments;
use super::measurement::InlineHtmlMeasurementCarrier;
use super::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, ceil_to_1_64_px,
    is_html_collapsible_ascii_whitespace, round_to_1_64_px, trim_html_collapsible_ascii_whitespace,
    wrap,
};

mod inline_plan;
mod types;
mod xhtml_fonts;

pub use inline_plan::style_requests_bold_font_weight;
use inline_plan::{
    InlineHtmlLineBoxes, InlineHtmlWrap, InlineTextRun, explicit_inline_html_line_boxes,
    finish_inline_html_layout, inline_text_style, measure_inline_run_width_px, same_font,
};
pub use types::{MermaidMarkdownWordType, SvgTextEntityMode};

/// Preserves structured HTML whitespace and measurement requests with an explicit root size.
pub fn measure_html_with_document_root(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    root_font_size: f64,
) -> TextMetrics {
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        InlineHtmlMeasurementCarrier::for_measurer(measurer),
        fragment,
        style,
        InlineHtmlWrap::Fit(max_width),
        WrapMode::HtmlLike,
        Some(xhtml_fonts::TerminalFonts::new(style, root_font_size)),
    )
}

pub fn measure_html_with_authored_width(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    wrap_width: Option<f64>,
) -> TextMetrics {
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        InlineHtmlMeasurementCarrier::for_measurer(measurer),
        fragment,
        style,
        wrap_width.map_or(InlineHtmlWrap::Fit(None), InlineHtmlWrap::BreakSpaces),
        WrapMode::HtmlLike,
        Some(xhtml_fonts::TerminalFonts::new(style, style.font_size)),
    )
}

#[test]
fn html_document_root_context_preserves_plain_label_host_requests() {
    use std::sync::Mutex;

    #[derive(Default)]
    struct Requests(Mutex<Vec<(&'static str, String)>>);

    impl TextMeasurer for Requests {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(("measure", text.to_owned()));
            super::DeterministicTextMeasurer::default().measure(text, style)
        }

        fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.0.lock().unwrap().push(("html-width", text.to_owned()));
            super::DeterministicTextMeasurer::default().measure_html_line_width_px(text, style)
        }
    }

    let style = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };
    for html in ["<p>A  B</p>", "<p>A<br/>B</p>", "<p>A&nbsp;B</p>"] {
        let previous = Requests::default();
        let explicit_root = Requests::default();
        let expected = measure_html_with_inline_styles(
            &previous,
            html,
            &style,
            Some(27.0),
            WrapMode::HtmlLike,
        );
        let actual =
            measure_html_with_document_root(&explicit_root, html, &style, Some(27.0), 16.0);
        assert_eq!(
            (actual.width, actual.height, actual.line_count),
            (expected.width, expected.height, expected.line_count),
            "{html}"
        );
        assert_eq!(
            *explicit_root.0.lock().unwrap(),
            *previous.0.lock().unwrap(),
            "{html}"
        );
    }
}

// This input has already passed through the lightweight HTML parser. Do not use the generic text
// normalizer here: its Unicode trim would erase a final NBSP-only line before inline measurement.
fn normalized_preparsed_html_text_lines(text: &str) -> Vec<String> {
    let mut out = text.split('\n').map(str::to_string).collect::<Vec<_>>();

    while out.len() > 1
        && out
            .last()
            .is_some_and(|line| trim_html_collapsible_ascii_whitespace(line).is_empty())
    {
        out.pop();
    }

    if out.is_empty() {
        vec![String::new()]
    } else {
        out
    }
}

fn push_inline_text_char(
    runs: &mut Vec<InlineTextRun>,
    ch: char,
    bold: bool,
    italic: bool,
    code: bool,
) {
    push_inline_text_char_with_terminal_style(runs, ch, bold, italic, code, None, 0.0);
}

fn push_inline_text_char_with_terminal_style(
    runs: &mut Vec<InlineTextRun>,
    ch: char,
    bold: bool,
    italic: bool,
    code: bool,
    terminal_style: Option<&std::sync::Arc<TextStyle>>,
    ancestor_strut_font_size: f64,
) {
    if let Some(run) = runs.last_mut().filter(|run| {
        run.replaced_width_px.is_none()
            && run.bold == bold
            && run.italic == italic
            && run.code == code
            && same_font(
                run.terminal_style.as_deref(),
                terminal_style.map(|s| s.as_ref()),
            )
    }) {
        let start = run.text.len();
        run.text.push(ch);
        run.record_ancestor_strut(start, ancestor_strut_font_size);
    } else {
        let mut run = InlineTextRun {
            text: ch.to_string(),
            bold,
            italic,
            code,
            terminal_style: terminal_style.cloned(),
            replaced_width_px: None,
            ancestor_struts: Vec::new(),
        };
        run.record_ancestor_strut(0, ancestor_strut_font_size);
        runs.push(run);
    }
}

/// Matches createText's explicit word attributes, not the enclosing text's inheritance.
pub fn svg_markdown_word_style(
    base: &TextStyle,
    kind: MermaidMarkdownWordType,
) -> std::borrow::Cow<'_, TextStyle> {
    let bold = kind == MermaidMarkdownWordType::Strong;
    let italic = kind == MermaidMarkdownWordType::Em;
    let weight_matches = match base.font_weight.as_deref() {
        None | Some("normal" | "400") => !bold,
        Some("bold" | "700") => bold,
        _ => false,
    };
    let style_matches = match base.font_style.as_deref() {
        None | Some("normal") => !italic,
        Some("italic") => italic,
        _ => false,
    };
    if weight_matches && style_matches {
        return std::borrow::Cow::Borrowed(base);
    }
    let mut style = base.clone();
    style.font_weight = Some(if bold { "700" } else { "normal" }.to_owned());
    style.font_style = Some(if italic { "italic" } else { "normal" }.to_owned());
    std::borrow::Cow::Owned(style)
}

fn measure_inline_runs_width_px<M: TextMeasurer + ?Sized>(
    measurer: &M,
    runs: &[InlineTextRun],
    style: &TextStyle,
    wrap_mode: WrapMode,
    svg_advance: bool,
) -> f64 {
    runs.iter()
        .filter(|run| !run.text.is_empty())
        .take_while(|_| !measurer.cancellation_requested())
        .map(|run| {
            if let Some(width) = run.replaced_width_px {
                return width;
            }
            let run_style = if let Some(terminal) = &run.terminal_style {
                std::borrow::Cow::Borrowed(terminal.as_ref())
            } else if svg_advance && wrap_mode != WrapMode::HtmlLike {
                let kind = if run.bold {
                    MermaidMarkdownWordType::Strong
                } else if run.italic {
                    MermaidMarkdownWordType::Em
                } else {
                    MermaidMarkdownWordType::Normal
                };
                svg_markdown_word_style(style, kind)
            } else {
                std::borrow::Cow::Owned(inline_text_style(style, run.bold, run.italic, run.code))
            };
            measure_inline_run_width_px(measurer, &run.text, &run_style, wrap_mode, svg_advance)
        })
        .sum()
}

/// Measures inline HTML using inherited fonts, element defaults, and supported font longhands.
///
/// This compatibility entry point has no document-root context: `rem` uses `style.font_size`.
/// Family-specific prepared labels can supply a separate root size; callers needing portable
/// geometry should prefer pixel or parent-relative sizes and a matching host font measurer.
pub fn measure_html_with_inline_styles(
    measurer: &dyn TextMeasurer,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    // Keep the public `TextMeasurer` surface request-oriented. A whole-operation override would
    // let an opaque custom measurer bypass the established callback order; only the unnameable
    // routed carrier may select the built-in streaming implementation.
    let carrier = InlineHtmlMeasurementCarrier::for_measurer(measurer);
    measure_html_with_inline_styles_with_carrier(
        measurer, carrier, html, style, max_width, wrap_mode,
    )
}

/// Consumes markup after `<`, leaving literal comparison signs in the text stream.
///
/// This only finds boundaries: the existing label logic still owns tag names, fonts, and icons.
/// An empty tag represents a skipped comment or an unfinished tag discarded at EOF.
fn take_inline_html_tag(input: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let mut lookahead = input.clone();
    let first = lookahead.next()?;
    let name_start = if first == '/' {
        lookahead.next()?
    } else {
        first
    };
    if matches!(first, '!' | '?') || (first == '/' && !name_start.is_ascii_alphabetic()) {
        let is_comment =
            first == '!' && lookahead.next() == Some('-') && lookahead.next() == Some('-');
        if is_comment {
            // Consume `!--`; quotes and tag-like text in comments have no attribute semantics.
            input.next();
            input.next();
            input.next();
            let mut at_start = true;
            let mut hyphens = 0_u8;
            let mut end_bang = false;
            for ch in input.by_ref() {
                if ch == '>' && (at_start || hyphens == 2 || end_bang) {
                    break;
                }
                end_bang = ch == '!' && hyphens == 2;
                hyphens = if ch == '-' { (hyphens + 1).min(2) } else { 0 };
                at_start = at_start && ch == '-' && hyphens == 1;
            }
        } else {
            // Bogus declarations, end tags, and processing instructions ignore attribute quotes.
            for ch in input.by_ref() {
                if ch == '>' {
                    break;
                }
            }
        }
        return Some(String::new());
    }

    if !name_start.is_ascii_alphabetic() {
        return None;
    }

    #[derive(Clone, Copy)]
    enum Boundary {
        TagName,
        BeforeAttribute,
        AttributeName,
        AfterAttribute,
        BeforeValue,
        UnquotedValue,
        QuotedValue(char),
    }
    use Boundary::*;

    let mut tag = String::new();
    if first == '/' {
        tag.push(input.next()?);
    }
    let mut boundary = TagName;
    for ch in input.by_ref() {
        if ch == '>' && !matches!(boundary, QuotedValue(_)) {
            return Some(tag);
        }
        tag.push(ch);
        boundary = match boundary {
            QuotedValue(quote) if ch == quote => BeforeAttribute,
            QuotedValue(_) => boundary,
            TagName | BeforeAttribute if is_html_collapsible_ascii_whitespace(ch) || ch == '/' => {
                BeforeAttribute
            }
            BeforeAttribute => AttributeName,
            AttributeName | AfterAttribute if ch == '=' => BeforeValue,
            AttributeName if is_html_collapsible_ascii_whitespace(ch) => AfterAttribute,
            AttributeName | AfterAttribute if ch == '/' => BeforeAttribute,
            AfterAttribute if !is_html_collapsible_ascii_whitespace(ch) => AttributeName,
            BeforeValue if is_html_collapsible_ascii_whitespace(ch) => BeforeValue,
            BeforeValue if matches!(ch, '"' | '\'') => QuotedValue(ch),
            BeforeValue => UnquotedValue,
            UnquotedValue if is_html_collapsible_ascii_whitespace(ch) => BeforeAttribute,
            _ => boundary,
        };
    }
    Some(String::new())
}

fn measure_html_with_inline_styles_with_carrier<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    html: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> TextMetrics {
    measure_html_with_inline_styles_and_terminal_fonts(
        measurer,
        carrier,
        html,
        style,
        InlineHtmlWrap::Fit(max_width),
        wrap_mode,
        (wrap_mode == WrapMode::HtmlLike)
            .then(|| xhtml_fonts::TerminalFonts::new(style, style.font_size)),
    )
}

#[allow(clippy::too_many_arguments)]
fn measure_html_with_inline_styles_and_terminal_fonts<M: TextMeasurer + ?Sized>(
    measurer: &M,
    carrier: InlineHtmlMeasurementCarrier,
    html: &str,
    style: &TextStyle,
    wrapping: InlineHtmlWrap,
    wrap_mode: WrapMode,
    mut terminal_fonts: Option<xhtml_fonts::TerminalFonts>,
) -> TextMetrics {
    let max_width = wrapping.max_width();
    fn html_tag_class_attr(tag: &str) -> Option<String> {
        let lower = tag.to_ascii_lowercase();
        let idx = lower.find("class=")?;
        let rest = tag[idx + 6..].trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }

        let mut it = rest.chars();
        let _ = it.next();
        let mut value = String::new();
        for ch in it {
            if ch == quote {
                break;
            }
            value.push(ch);
        }

        Some(value)
    }

    fn fontawesome_icon_width_px(tag: &str, font_size: f64) -> Option<f64> {
        let class_attr = html_tag_class_attr(tag)?;
        let mut prefix: Option<&str> = None;
        let mut icon: Option<&str> = None;

        for token in class_attr.split_ascii_whitespace() {
            if matches!(token, "fa" | "fab" | "fak" | "fal" | "far" | "fas") {
                prefix = Some(token);
                continue;
            }
            if let Some(name) = token.strip_prefix("fa-") {
                icon = Some(name);
            }
        }

        let prefix = prefix?;
        let icon = icon?;
        let advance_em = match (prefix, icon) {
            ("fa" | "fab" | "fak" | "fal" | "far" | "fas", _) => 1.25,
            _ => return None,
        };

        Some(round_to_1_64_px(font_size.max(0.0) * advance_em))
    }

    // Mermaid supports inline FontAwesome icons via `<i class="fa fa-..."></i>` inside HTML
    // labels. Upstream layout is computed with FontAwesome CSS available, while exported SVGs
    // keep only the empty `<i>` element. Model the layout-time glyph advance explicitly.
    let mut plain = String::new();
    let mut icon_width_px_by_line: Vec<f64> = vec![0.0];
    let mut icon_on_line: Vec<bool> = vec![false];
    let mut image_on_line: Vec<bool> = vec![false];
    let mut inline_runs_by_line: Vec<Vec<InlineTextRun>> = vec![Vec::new()];
    let mut break_spaces_runs_by_line: Option<Vec<Vec<InlineTextRun>>> = None;
    let mut strong_depth: usize = 0;
    let mut em_depth: usize = 0;
    let mut code_depth: usize = 0;
    let mut fa_icon_depth: usize = 0;
    let mut inline_replaced_boundary = false;

    let html = html.replace("\r\n", "\n");
    let mut it = html.chars().peekable();
    let mut entity_reference = String::new();
    while let Some(ch) = it.next() {
        if ch == '<'
            && let Some(tag) = take_inline_html_tag(&mut it)
        {
            let tag = trim_html_collapsible_ascii_whitespace(&tag);
            let tag_lower = tag.to_ascii_lowercase();
            let tag_trim = trim_html_collapsible_ascii_whitespace(&tag_lower);
            if tag_trim.starts_with('!') || tag_trim.starts_with('?') {
                continue;
            }
            let is_closing = tag_trim.starts_with('/');
            let name = tag_trim
                .trim_start_matches('/')
                .trim_end_matches('/')
                .split(is_html_collapsible_ascii_whitespace)
                .find(|part| !part.is_empty())
                .unwrap_or("");

            if let Some(fonts) = terminal_fonts.as_mut() {
                fonts.observe_tag(name, tag, is_closing);
            }

            let fontawesome_icon_width = if name == "i" && !is_closing {
                fontawesome_icon_width_px(
                    tag,
                    terminal_fonts
                        .as_ref()
                        .map_or(style.font_size, |fonts| fonts.current().font_size),
                )
            } else {
                None
            };

            match name {
                "strong" | "b" => {
                    if is_closing {
                        strong_depth = strong_depth.saturating_sub(1);
                    } else {
                        strong_depth += 1;
                    }
                }
                "em" | "i" => {
                    if is_closing {
                        if name == "i" && fa_icon_depth > 0 {
                            fa_icon_depth = fa_icon_depth.saturating_sub(1);
                        } else {
                            em_depth = em_depth.saturating_sub(1);
                        }
                    } else if let Some(icon_w) = fontawesome_icon_width {
                        let line_idx = icon_width_px_by_line.len().saturating_sub(1);
                        icon_width_px_by_line[line_idx] += icon_w;
                        if let Some(slot) = icon_on_line.get_mut(line_idx) {
                            *slot = true;
                        }
                        let mut icon =
                            terminal_fonts
                                .as_ref()
                                .map_or_else(InlineTextRun::default, |fonts| {
                                    InlineTextRun {
                                        // U+FFFC indexes an inline replaced box for Unicode line breaking.
                                        // Its fixed advance is never measured as a text glyph.
                                        text: "\u{fffc}".to_owned(),
                                        terminal_style: Some(std::sync::Arc::clone(
                                            fonts.current(),
                                        )),
                                        replaced_width_px: Some(icon_w),
                                        ..InlineTextRun::default()
                                    }
                                });
                        if let Some(fonts) = terminal_fonts.as_ref() {
                            icon.record_ancestor_strut(0, fonts.current_ancestor_strut_font_size());
                        }
                        if let Some(runs) = inline_runs_by_line.get_mut(line_idx) {
                            runs.push(icon.clone());
                        }
                        if let Some(runs) = break_spaces_runs_by_line
                            .as_mut()
                            .and_then(|lines| lines.last_mut())
                        {
                            runs.push(icon);
                        }
                        inline_replaced_boundary = true;
                        fa_icon_depth += 1;
                    } else {
                        em_depth += 1;
                    }
                }
                "code" => {
                    if is_closing {
                        code_depth = code_depth.saturating_sub(1);
                    } else {
                        code_depth += 1;
                    }
                }
                "img" if !is_closing => {
                    if terminal_fonts.is_some()
                        && let Some(runs) = inline_runs_by_line.last_mut()
                    {
                        runs.push(InlineTextRun::default());
                    }
                    if let Some(slot) = image_on_line.last_mut() {
                        *slot = true;
                    }
                    inline_replaced_boundary = true;
                }
                "br" => {
                    if let Some(fonts) = terminal_fonts.as_mut() {
                        fonts.new_line();
                    }
                    plain.push('\n');
                    icon_width_px_by_line.push(0.0);
                    icon_on_line.push(false);
                    image_on_line.push(false);
                    inline_runs_by_line.push(Vec::new());
                    if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                        lines.push(Vec::new());
                    }
                    inline_replaced_boundary = false;
                }
                "p" | "div" | "li" | "tr" | "ul" | "ol" if is_closing => {
                    // A block end starts the next line only when the current line has content.
                    // Reusing the `<br>` branch here would append a phantom row after
                    // `<p><br></p>` and make blank-only labels impossible to count exactly.
                    if !plain.ends_with('\n') && !plain.is_empty() {
                        if let Some(fonts) = terminal_fonts.as_mut() {
                            fonts.new_line();
                        }
                        plain.push('\n');
                        icon_width_px_by_line.push(0.0);
                        icon_on_line.push(false);
                        image_on_line.push(false);
                        inline_runs_by_line.push(Vec::new());
                        if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                            lines.push(Vec::new());
                        }
                    }
                    inline_replaced_boundary = false;
                }
                _ => {}
            }
            continue;
        }

        let push_char = |decoded: char,
                         plain: &mut String,
                         icon_on_line: &mut Vec<bool>,
                         inline_runs_by_line: &mut Vec<Vec<InlineTextRun>>,
                         break_spaces_runs_by_line: &mut Option<Vec<Vec<InlineTextRun>>>,
                         inline_replaced_boundary: &mut bool| {
            if wrap_mode == WrapMode::HtmlLike
                && is_html_collapsible_ascii_whitespace(decoded)
                && break_spaces_runs_by_line.is_none()
            {
                // The browser decodes entities before it decides whether fixed-width
                // `break-spaces` layout is needed. Lazily fork the exact-text projection at the
                // first decoded whitespace so literal and entity-produced whitespace share the
                // same semantics without duplicating every ordinary label up front.
                *break_spaces_runs_by_line = Some(inline_runs_by_line.clone());
            }
            if let Some(lines) = break_spaces_runs_by_line.as_mut() {
                if decoded == '\n' || decoded == '\r' {
                    lines.push(Vec::new());
                } else if let Some(runs) = lines.last_mut() {
                    push_inline_text_char_with_terminal_style(
                        runs,
                        decoded,
                        strong_depth > 0,
                        em_depth > 0,
                        code_depth > 0,
                        terminal_fonts.as_ref().map(|fonts| fonts.current()),
                        terminal_fonts
                            .as_ref()
                            .map_or(0.0, |fonts| fonts.current_ancestor_strut_font_size()),
                    );
                }
            }
            let decoded = if is_html_collapsible_ascii_whitespace(decoded) {
                ' '
            } else {
                decoded
            };
            if decoded == ' ' {
                let line_has_visual = icon_on_line.last().copied().unwrap_or(false)
                    || image_on_line.last().copied().unwrap_or(false);
                if (plain.ends_with(' ') && !*inline_replaced_boundary)
                    || (plain.ends_with('\n') && !line_has_visual)
                    || (plain.is_empty() && !line_has_visual)
                {
                    return;
                }
            }
            plain.push(decoded);
            if let Some(runs) = inline_runs_by_line.last_mut() {
                push_inline_text_char_with_terminal_style(
                    runs,
                    decoded,
                    strong_depth > 0,
                    em_depth > 0,
                    code_depth > 0,
                    terminal_fonts.as_ref().map(|fonts| fonts.current()),
                    terminal_fonts
                        .as_ref()
                        .map_or(0.0, |fonts| fonts.current_ancestor_strut_font_size()),
                );
            }
            *inline_replaced_boundary = false;
        };

        if ch == '&' {
            entity_reference.clear();
            entity_reference.push('&');
            let mut saw_semicolon = false;
            while let Some(&c) = it.peek() {
                if c == ';' {
                    it.next();
                    saw_semicolon = true;
                    break;
                }
                if c == '<'
                    || c == '&'
                    || is_html_collapsible_ascii_whitespace(c)
                    || entity_reference.len() > 65
                {
                    break;
                }
                entity_reference.push(c);
                it.next();
            }
            if saw_semicolon {
                entity_reference.push(';');
            }
            let decoded = merman_model::text::decode_html_entities_to_unicode(&entity_reference);
            for decoded in decoded.chars() {
                push_char(
                    decoded,
                    &mut plain,
                    &mut icon_on_line,
                    &mut inline_runs_by_line,
                    &mut break_spaces_runs_by_line,
                    &mut inline_replaced_boundary,
                );
            }
            continue;
        }

        push_char(
            ch,
            &mut plain,
            &mut icon_on_line,
            &mut inline_runs_by_line,
            &mut break_spaces_runs_by_line,
            &mut inline_replaced_boundary,
        );
    }

    if let Some(lines) = break_spaces_runs_by_line.as_mut() {
        while lines.len() > 1
            && lines
                .last()
                .is_some_and(|runs| runs.iter().all(|run| run.text.is_empty()))
        {
            lines.pop();
        }
    }

    // Keep whitespace adjacent to inline icons: in HTML it becomes significant when it separates
    // text from an inline-block `<i>` (for both `<i> text` and `text <i>`). Otherwise discard only
    // collapsible ASCII boundary whitespace. U+00A0 remains visible at either boundary.
    let plain = if icon_on_line.iter().any(|v| *v) {
        plain.trim_end_matches('\n').to_string()
    } else {
        plain
            .trim_end_matches(is_html_collapsible_ascii_whitespace)
            .to_string()
    };
    let has_inline_icons = icon_on_line.iter().any(|has_icon| *has_icon);

    if let Some(fonts) = terminal_fonts {
        // The eight-style planner represents ordinary emphasis and code without retaining an
        // arbitrary font stack. Keep that fast path only when every terminal font agrees with it;
        // nested bolder, light/heavy parents, and oblique-to-italic overrides need resolved runs.
        let requires_terminal_styles = fonts.has_font_declarations()
            || inline_runs_by_line.iter().flatten().any(|run| {
                !run.text.is_empty()
                    && run.replaced_width_px.is_none()
                    && !same_font(
                        run.terminal_style.as_deref(),
                        Some(&inline_text_style(style, run.bold, run.italic, run.code)),
                    )
            });
        if requires_terminal_styles {
            debug_assert!(wrap_mode == WrapMode::HtmlLike);
            let explicit_line_boxes =
                explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);
            let lines = normalized_preparsed_html_text_lines(&plain);
            inline_runs_by_line.resize_with(lines.len().max(explicit_line_boxes), Vec::new);
            // Keep collapsed tails out of styled widths; empty runs mark replaced elements.
            for runs in &mut inline_runs_by_line {
                for run in runs.iter_mut().rev() {
                    if run.text.is_empty() {
                        break;
                    }
                    let end = run
                        .text
                        .trim_end_matches(is_html_collapsible_ascii_whitespace)
                        .len();
                    run.text.truncate(end);
                    if end > 0 {
                        break;
                    }
                }
            }
            if !carrier.is_builtin() {
                measurer.measure_wrapped(
                    trim_html_collapsible_ascii_whitespace(&plain),
                    style,
                    max_width,
                    wrap_mode,
                );
            }
            if max_width.is_some() {
                return finish_inline_html_layout(
                    measurer,
                    carrier,
                    &inline_runs_by_line,
                    break_spaces_runs_by_line.as_deref(),
                    style,
                    wrapping,
                    InlineHtmlLineBoxes {
                        explicit_count: explicit_line_boxes,
                        font_sizes: Some(fonts.line_sizes()),
                    },
                );
            }
            return fonts.measure(measurer, &inline_runs_by_line, style, explicit_line_boxes);
        }
        // Representable markup retains the existing callback and primitive-font fast path.
        for run in inline_runs_by_line.iter_mut().flatten() {
            run.terminal_style = None;
            if run.replaced_width_px.take().is_some() {
                run.text.clear();
            }
        }
        if let Some(lines) = break_spaces_runs_by_line.as_mut() {
            for run in lines.iter_mut().flatten() {
                run.terminal_style = None;
                if run.replaced_width_px.take().is_some() {
                    run.text.clear();
                }
            }
        }
    }

    if wrap_mode == WrapMode::HtmlLike && !has_inline_icons && carrier.is_builtin() {
        let explicit_line_boxes =
            explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);
        let mut lines = normalized_preparsed_html_text_lines(&plain);
        if lines.is_empty() {
            lines.push(String::new());
        }
        inline_runs_by_line.resize_with(lines.len(), Vec::new);

        // The pinned Mermaid `createText.ts:addHtmlSpan` first measures decoded HTML with
        // `display: table-cell`, `white-space: nowrap`, and `max-width`. When the browser reports
        // exactly that maximum width, it switches to table layout with `white-space: break-spaces`
        // and a fixed width before measuring again. The built-in planner preserves that
        // natural-width activation, min-content expansion, and greedy line count directly;
        // exact browser shaping and `getBoundingClientRect()` floats remain a bounded residual.
        // Consequently the discarded plain-label and per-run premeasurement passes are not part
        // of this operation and need not call the built-in backend.
        return finish_inline_html_layout(
            measurer,
            carrier,
            &inline_runs_by_line,
            break_spaces_runs_by_line.as_deref(),
            style,
            wrapping,
            InlineHtmlLineBoxes {
                explicit_count: explicit_line_boxes,
                font_sizes: None,
            },
        );
    }

    // Keep this request before all styled probes for arbitrary and host measurers. Their callbacks
    // are observable, including stateful return values and the exact position of a failure.
    let base_text = trim_html_collapsible_ascii_whitespace(&plain);
    let base = measurer.measure_wrapped(base_text, style, max_width, wrap_mode);
    let explicit_line_boxes = explicit_inline_html_line_boxes(&inline_runs_by_line, &image_on_line);

    let mut lines = normalized_preparsed_html_text_lines(&plain);
    if lines.is_empty() {
        lines.push(String::new());
    }
    icon_width_px_by_line.resize(lines.len(), 0.0);
    icon_on_line.resize(lines.len(), false);
    inline_runs_by_line.resize_with(lines.len(), Vec::new);
    let styled_text_width_px_by_line = inline_runs_by_line
        .iter()
        .map(|runs| measure_inline_runs_width_px(measurer, runs, style, wrap_mode, false))
        .collect::<Vec<_>>();
    let inline_width_px_by_line = styled_text_width_px_by_line
        .iter()
        .zip(&icon_width_px_by_line)
        .map(|(text_width, icon_width)| text_width + icon_width)
        .collect::<Vec<_>>();

    if wrap_mode == WrapMode::HtmlLike && !has_inline_icons {
        return finish_inline_html_layout(
            measurer,
            carrier,
            &inline_runs_by_line,
            break_spaces_runs_by_line.as_deref(),
            style,
            wrapping,
            InlineHtmlLineBoxes {
                explicit_count: explicit_line_boxes,
                font_sizes: None,
            },
        );
    }

    let icon_start_wrap = if wrap_mode == WrapMode::HtmlLike {
        max_width
            .filter(|w| w.is_finite() && *w > 0.0)
            .and_then(|w| {
                let mut extra_lines = 0usize;
                let mut wrapped_width: f64 = 0.0;
                let mut has_width_override = false;

                for (idx, line) in lines.iter().enumerate() {
                    if !icon_on_line[idx] || !line.starts_with(is_html_collapsible_ascii_whitespace)
                    {
                        continue;
                    }
                    let text = trim_html_collapsible_ascii_whitespace(line);
                    if text.is_empty() {
                        continue;
                    }

                    let segments = html_break_spaces_segments(text);
                    let text_width = styled_text_width_px_by_line[idx];
                    let first_segment = segments.first().copied().unwrap_or(text);
                    let first_segment_width = measurer
                        .measure_wrapped(first_segment, style, None, wrap_mode)
                        .width;
                    if first_segment_width + icon_width_px_by_line[idx] > w {
                        extra_lines += 1;
                        has_width_override = true;
                        for segment in segments {
                            if segment.is_empty() {
                                continue;
                            }
                            wrapped_width = wrapped_width.max(
                                measurer
                                    .measure_wrapped(segment, style, None, wrap_mode)
                                    .width,
                            );
                        }
                    } else if text_width <= w && inline_width_px_by_line[idx] > w {
                        extra_lines += 1;
                        has_width_override = true;
                        wrapped_width = wrapped_width.max(w);
                    } else if inline_width_px_by_line[idx] > w {
                        has_width_override = true;
                        let mut segment_width: f64 = 0.0;
                        for segment in segments {
                            if segment.is_empty() {
                                continue;
                            }
                            segment_width = segment_width.max(
                                measurer
                                    .measure_wrapped(segment, style, None, wrap_mode)
                                    .width,
                            );
                        }
                        wrapped_width = wrapped_width.max(segment_width.max(w));
                    }
                }

                (has_width_override || extra_lines > 0).then_some((wrapped_width, extra_lines))
            })
    } else {
        None
    };

    let inline_style_extra_wrap_lines = if wrap_mode == WrapMode::HtmlLike {
        max_width
            .filter(|w| w.is_finite() && *w > 0.0)
            .map(|w| {
                lines
                    .iter()
                    .enumerate()
                    .filter(|(idx, line)| {
                        let text = trim_html_collapsible_ascii_whitespace(line);
                        if text.is_empty()
                            || icon_on_line.get(*idx).copied().unwrap_or(false)
                            || !text.chars().any(is_html_collapsible_ascii_whitespace)
                        {
                            return false;
                        }
                        let raw_width =
                            measurer.measure_wrapped(text, style, None, wrap_mode).width;
                        raw_width <= w
                            && styled_text_width_px_by_line
                                .get(*idx)
                                .copied()
                                .unwrap_or(raw_width)
                                > w
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    let max_line_width = inline_width_px_by_line
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);

    // Mermaid's upstream baselines land on a 1/64px lattice. For SVG-label measurement, the
    // underlying `getBBox()` numbers can hit exact `.5/64` ties; use ties-to-even rounding to
    // match the lattice choices observed in upstream class SVG fixtures.
    let mut width = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
            wrap::round_to_1_64_px_ties_to_even(max_line_width)
        }
        WrapMode::HtmlLike => round_to_1_64_px(max_line_width),
    };
    if wrap_mode == WrapMode::HtmlLike
        && let Some(w) = max_width.filter(|w| w.is_finite() && *w > 0.0)
    {
        let raw_w = max_line_width;
        let needs_wrap = raw_w > w;
        if needs_wrap {
            // When wrapping is active, the DOM-driven width behavior is governed by the
            // wrapped layout, not the unwrapped per-line extents. Reuse the wrapped baseline
            // width (without bold deltas) so we don't over-inflate `foreignObject width="..."`
            // from unwrapped lines.
            //
            // The underlying measurer is still responsible for modeling any min-content
            // expansion beyond `max-width`.
            width = icon_start_wrap
                .map(|(icon_width, _)| icon_width)
                .unwrap_or(base.width)
                .max(w);
        } else {
            width = width.min(w);
        }
    }

    let icon_only_extra_lines = if trim_html_collapsible_ascii_whitespace(&plain).is_empty() {
        0
    } else {
        lines
            .iter()
            .enumerate()
            .filter(|(idx, line)| {
                trim_html_collapsible_ascii_whitespace(line).is_empty()
                    && icon_on_line.get(*idx).copied().unwrap_or(false)
                    && icon_width_px_by_line.get(*idx).copied().unwrap_or(0.0) > 0.0
            })
            .count()
    };

    if icon_only_extra_lines > 0 {
        // DOM measurement keeps an inline icon-only line as a normal 1.5em line box and rounds the
        // resulting max line width upward on the 1/64px lattice.
        width = ceil_to_1_64_px(width);
    }

    let (mut height, mut line_count) = if let Some((_, extra_lines)) = icon_start_wrap {
        (
            base.height + extra_lines as f64 * style.font_size.max(0.0) * 1.5,
            base.line_count + extra_lines,
        )
    } else {
        (base.height, base.line_count)
    };
    if icon_only_extra_lines > 0 {
        height += icon_only_extra_lines as f64 * style.font_size.max(0.0) * 1.5;
        line_count += icon_only_extra_lines;
    }
    if inline_style_extra_wrap_lines > 0 {
        height += inline_style_extra_wrap_lines as f64 * style.font_size.max(0.0) * 1.5;
        line_count += inline_style_extra_wrap_lines;
    }

    TextMetrics {
        width,
        height,
        line_count,
    }
}

pub fn markdown_word_line_plain_text_and_width_px(
    measurer: &dyn TextMeasurer,
    words: &[(String, MermaidMarkdownWordType)],
    style: &TextStyle,
    wrap_mode: WrapMode,
    entity_mode: SvgTextEntityMode,
) -> (String, f64) {
    if measurer.cancellation_requested() {
        return (String::new(), 0.0);
    }
    let mut plain = String::new();
    let mut runs = Vec::new();

    for (word_idx, (word, ty)) in words.iter().enumerate() {
        if measurer.cancellation_requested() {
            return (String::new(), 0.0);
        }
        let visible_word = match wrap_mode {
            WrapMode::HtmlLike => merman_model::text::decode_html_entities_to_unicode(word),
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => entity_mode.visible_word(word),
        };
        let bold = *ty == MermaidMarkdownWordType::Strong;
        let italic = *ty == MermaidMarkdownWordType::Em;

        if word_idx > 0 {
            plain.push(' ');
            // Each SVG word tspan owns its leading space and its font attributes.
            let svg = wrap_mode != WrapMode::HtmlLike;
            push_inline_text_char(&mut runs, ' ', svg && bold, svg && italic, false);
        }
        for ch in visible_word.chars() {
            plain.push(ch);
            push_inline_text_char(&mut runs, ch, bold, italic, false);
        }
    }

    let width = measure_inline_runs_width_px(measurer, &runs, style, wrap_mode, true);
    (plain, width)
}
