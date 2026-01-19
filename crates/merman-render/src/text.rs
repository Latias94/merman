use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    SvgLike,
    HtmlLike,
}

impl Default for WrapMode {
    fn default() -> Self {
        Self::SvgLike
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 16.0,
            font_weight: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
}

pub trait TextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics;

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let _ = max_width;
        let _ = wrap_mode;
        self.measure(text, style)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeterministicTextMeasurer {
    pub char_width_factor: f64,
    pub line_height_factor: f64,
}

impl DeterministicTextMeasurer {
    fn replace_br_variants(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            if text[i..].starts_with("<br") {
                let next = text[i + 3..].chars().next();
                let is_valid = match next {
                    None => true,
                    Some(ch) => ch.is_whitespace() || ch == '/' || ch == '>',
                };
                if is_valid {
                    if let Some(rel_end) = text[i..].find('>') {
                        i += rel_end + 1;
                        out.push('\n');
                        continue;
                    }
                }
            }

            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    pub fn normalized_text_lines(text: &str) -> Vec<String> {
        let t = Self::replace_br_variants(text);
        let out = t.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();
        if out.is_empty() {
            return vec!["".to_string()];
        }
        out
    }

    pub(crate) fn split_line_to_words(text: &str) -> Vec<String> {
        // Mirrors Mermaid's `splitLineToWords` fallback behavior when `Intl.Segmenter` is absent:
        // split by spaces, then re-add the spaces as separate tokens (preserving multiple spaces).
        let parts = text.split(' ').collect::<Vec<_>>();
        let mut out: Vec<String> = Vec::new();
        for part in parts {
            if !part.is_empty() {
                out.push(part.to_string());
            }
            out.push(" ".to_string());
        }
        while out.last().is_some_and(|s| s == " ") {
            out.pop();
        }
        out
    }

    fn wrap_line(line: &str, max_chars: usize, break_long_words: bool) -> Vec<String> {
        if max_chars == 0 {
            return vec![line.to_string()];
        }

        let mut tokens = std::collections::VecDeque::from(Self::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            if candidate.chars().count() <= max_chars {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            // `tok` itself does not fit on an empty line.
            if tok == " " {
                continue;
            }
            if !break_long_words {
                out.push(tok);
            } else {
                // Split it by characters (Mermaid SVG text mode behavior).
                let tok_chars = tok.chars().collect::<Vec<_>>();
                let head: String = tok_chars.iter().take(max_chars.max(1)).collect();
                let tail: String = tok_chars.iter().skip(max_chars.max(1)).collect();
                out.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }
}

impl TextMeasurer for DeterministicTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::SvgLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let uses_heuristic_widths = self.char_width_factor == 0.0;
        let char_width_factor = if uses_heuristic_widths {
            match wrap_mode {
                WrapMode::SvgLike => 0.6,
                WrapMode::HtmlLike => 0.5,
            }
        } else {
            self.char_width_factor
        };
        let default_line_height_factor = match wrap_mode {
            WrapMode::SvgLike => 1.1,
            WrapMode::HtmlLike => 1.5,
        };
        let line_height_factor = if self.line_height_factor == 0.0 {
            default_line_height_factor
        } else {
            self.line_height_factor
        };

        let font_size = style.font_size.max(1.0);
        let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);
        let break_long_words = wrap_mode == WrapMode::SvgLike;

        let mut lines = Vec::new();
        for line in Self::normalized_text_lines(text) {
            if let Some(w) = max_width {
                let char_px = font_size * char_width_factor;
                let max_chars = ((w / char_px).floor() as isize).max(1) as usize;
                lines.extend(Self::wrap_line(&line, max_chars, break_long_words));
            } else {
                lines.push(line);
            }
        }

        let mut width: f64 = 0.0;
        for line in &lines {
            let w = if uses_heuristic_widths {
                estimate_line_width_px(line, font_size)
            } else {
                line.chars().count() as f64 * font_size * char_width_factor
            };
            width = width.max(w);
        }
        // Mermaid HTML labels use `max-width` and can visually overflow for long words, but their
        // layout width is effectively clamped to the max width. Mirror this to avoid explosive
        // headless widths when `htmlLabels=true`.
        if wrap_mode == WrapMode::HtmlLike {
            if let Some(w) = max_width {
                width = width.min(w);
            }
        }
        let height = lines.len() as f64 * font_size * line_height_factor;
        TextMetrics {
            width,
            height,
            line_count: lines.len(),
        }
    }
}

pub fn wrap_text_lines_px(
    text: &str,
    style: &TextStyle,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
) -> Vec<String> {
    let font_size = style.font_size.max(1.0);
    let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);
    let break_long_words = wrap_mode == WrapMode::SvgLike;

    fn split_token_to_width_px(tok: &str, max_width_px: f64, font_size: f64) -> (String, String) {
        let max_em = max_width_px / font_size;
        let mut em = 0.0;
        let chars = tok.chars().collect::<Vec<_>>();
        let mut split_at = 0usize;
        for (idx, ch) in chars.iter().enumerate() {
            em += estimate_char_width_em(*ch);
            if em > max_em && idx > 0 {
                break;
            }
            split_at = idx + 1;
            if em >= max_em {
                break;
            }
        }
        if split_at == 0 {
            split_at = 1.min(chars.len());
        }
        let head = chars.iter().take(split_at).collect::<String>();
        let tail = chars.iter().skip(split_at).collect::<String>();
        (head, tail)
    }

    fn wrap_line_to_width_px(
        line: &str,
        max_width_px: f64,
        font_size: f64,
        break_long_words: bool,
    ) -> Vec<String> {
        let mut tokens =
            std::collections::VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_trimmed = candidate.trim_end();
            if estimate_line_width_px(candidate_trimmed, font_size) <= max_width_px {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            if tok == " " {
                continue;
            }

            if !break_long_words {
                out.push(tok);
            } else {
                let (head, tail) = split_token_to_width_px(&tok, max_width_px, font_size);
                out.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for line in DeterministicTextMeasurer::normalized_text_lines(text) {
        if let Some(w) = max_width_px {
            lines.extend(wrap_line_to_width_px(&line, w, font_size, break_long_words));
        } else {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        vec!["".to_string()]
    } else {
        lines
    }
}

fn estimate_line_width_px(line: &str, font_size: f64) -> f64 {
    let mut em = 0.0;
    for ch in line.chars() {
        em += estimate_char_width_em(ch);
    }
    em * font_size
}

fn estimate_char_width_em(ch: char) -> f64 {
    if ch == ' ' {
        return 0.33;
    }
    if ch == '\t' {
        return 0.66;
    }
    if ch == '_' || ch == '-' {
        return 0.33;
    }
    if matches!(ch, '.' | ',' | ':' | ';') {
        return 0.28;
    }
    if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
        return 0.33;
    }
    if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
        return 0.45;
    }
    if ch.is_ascii_digit() {
        return 0.56;
    }
    if ch.is_ascii_uppercase() {
        return match ch {
            'I' => 0.30,
            'W' => 0.85,
            _ => 0.60,
        };
    }
    if ch.is_ascii_lowercase() {
        return match ch {
            'i' | 'l' => 0.28,
            'm' | 'w' => 0.78,
            'k' | 'y' => 0.55,
            _ => 0.43,
        };
    }
    // Punctuation/symbols/unicode: approximate.
    0.60
}
