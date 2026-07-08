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
            Some((_, ext)) if merman_analysis::markdown::is_mdx_extension(ext) => Self::Mdx,
            Some((_, ext)) if merman_analysis::markdown::is_markdown_extension(ext) => {
                Self::Markdown
            }
            _ => Self::Diagram,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentKind;

    #[test]
    fn document_kind_from_path_treats_markdown_extensions_case_insensitively() {
        assert_eq!(
            DocumentKind::from_path("/tmp/README.MD"),
            DocumentKind::Markdown
        );
        assert_eq!(
            DocumentKind::from_path("/tmp/Guide.Markdown"),
            DocumentKind::Markdown
        );
        assert_eq!(DocumentKind::from_path("/tmp/Story.MDX"), DocumentKind::Mdx);
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
