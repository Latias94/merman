use serde::{Deserialize, Serialize};

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
    ) -> TextMetrics {
        let _ = max_width;
        self.measure(text, style)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeterministicTextMeasurer {
    pub char_width_factor: f64,
    pub line_height_factor: f64,
}

impl DeterministicTextMeasurer {
    pub fn normalized_text_lines(text: &str) -> Vec<String> {
        let t = text
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("<br>", "\n");
        let out = t.split('\n').map(|s| s.to_string()).collect::<Vec<_>>();
        if out.is_empty() {
            return vec!["".to_string()];
        }
        out
    }

    fn split_line_to_words(text: &str) -> Vec<String> {
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

    fn wrap_line(line: &str, max_chars: usize) -> Vec<String> {
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

            // `tok` itself does not fit on an empty line; split it by characters.
            if tok == " " {
                continue;
            }
            let tok_chars = tok.chars().collect::<Vec<_>>();
            let head: String = tok_chars.iter().take(max_chars.max(1)).collect();
            let tail: String = tok_chars.iter().skip(max_chars.max(1)).collect();
            out.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
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
        self.measure_wrapped(text, style, None)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
    ) -> TextMetrics {
        let char_width_factor = if self.char_width_factor == 0.0 {
            0.6
        } else {
            self.char_width_factor
        };
        let line_height_factor = if self.line_height_factor == 0.0 {
            1.2
        } else {
            self.line_height_factor
        };

        let font_size = style.font_size.max(1.0);
        let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);

        let mut lines = Vec::new();
        for line in Self::normalized_text_lines(text) {
            if let Some(w) = max_width {
                let char_px = font_size * char_width_factor;
                let max_chars = ((w / char_px).floor() as isize).max(1) as usize;
                lines.extend(Self::wrap_line(&line, max_chars));
            } else {
                lines.push(line);
            }
        }

        let mut max_chars = 0usize;
        for line in &lines {
            max_chars = max_chars.max(line.chars().count());
        }

        let width = max_chars as f64 * font_size * char_width_factor;
        let height = lines.len() as f64 * font_size * line_height_factor;
        TextMetrics {
            width,
            height,
            line_count: lines.len(),
        }
    }
}
