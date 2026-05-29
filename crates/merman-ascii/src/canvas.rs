#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<char>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![' '; width.saturating_mul(height)],
        }
    }

    pub(crate) fn set(&mut self, x: usize, y: usize, ch: char) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.cells[y * self.width + x] = ch;
    }

    pub(crate) fn get(&self, x: usize, y: usize) -> Option<char> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.cells[y * self.width + x])
    }

    pub(crate) fn write_text(&mut self, x: usize, y: usize, text: &str) {
        for (offset, ch) in text.chars().enumerate() {
            self.set(x + offset, y, ch);
        }
    }

    pub(crate) fn finish(self) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row in self.cells.chunks(self.width) {
            for ch in row {
                out.push(*ch);
            }
            out.push('\n');
        }
        out
    }
}
