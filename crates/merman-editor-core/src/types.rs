use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentUri(String);

impl DocumentUri {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DocumentUri {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DocumentUri {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentKind {
    Diagram,
    Markdown,
    Mdx,
}

impl DocumentKind {
    pub fn is_markdown(self) -> bool {
        matches!(self, Self::Markdown | Self::Mdx)
    }

    pub fn from_path(path: &str) -> Self {
        match path.rsplit_once('.') {
            Some((_, "md")) | Some((_, "markdown")) => Self::Markdown,
            Some((_, "mdx")) => Self::Mdx,
            _ => Self::Diagram,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

impl Position {
    pub fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

impl Default for Range {
    fn default() -> Self {
        Self::new(Position::new(0, 0), Position::new(0, 0))
    }
}
