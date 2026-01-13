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
}

impl TextMeasurer for DeterministicTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
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

        let lines = Self::normalized_text_lines(text);
        let font_size = style.font_size.max(1.0);
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
