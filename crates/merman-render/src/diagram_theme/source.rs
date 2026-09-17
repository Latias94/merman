use std::collections::BTreeMap;

use super::ThemeTarget;

/// Diagnostic locations only; these never participate in visual recipe identity.
#[derive(Debug, Clone, Default)]
pub(crate) struct ThemeSourceMap {
    document: Option<ThemeSourceDocument>,
    rules: Vec<ThemeStyleSource>,
    palettes: BTreeMap<ThemeTarget, ThemeStyleSource>,
    canvas_base: Option<ThemeStyleSource>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThemeStyleSource {
    pub(crate) paths: Vec<String>,
    pub(crate) generated: bool,
}

impl ThemeStyleSource {
    pub(super) fn authored(path: String) -> Self {
        Self {
            paths: vec![path],
            generated: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeSourceDocument {
    Definition,
    CompleteSpec,
}

impl ThemeSourceDocument {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::CompleteSpec => "complete_spec",
        }
    }
}

impl ThemeSourceMap {
    pub(super) fn new(document: Option<ThemeSourceDocument>) -> Self {
        Self {
            document,
            ..Self::default()
        }
    }

    pub(crate) fn document(&self) -> Option<ThemeSourceDocument> {
        self.document
    }

    pub(crate) fn rule(&self, index: usize) -> Option<&ThemeStyleSource> {
        self.rules.get(index)
    }

    pub(crate) fn palette(&self, target: ThemeTarget) -> Option<&ThemeStyleSource> {
        self.palettes.get(&target)
    }

    pub(crate) fn canvas_base(&self) -> Option<&ThemeStyleSource> {
        self.canvas_base.as_ref()
    }

    pub(super) fn push_rule(&mut self, source: Option<ThemeStyleSource>) {
        if let Some(source) = source {
            self.rules.push(source);
        }
    }

    pub(super) fn insert_palette(&mut self, target: ThemeTarget, source: Option<ThemeStyleSource>) {
        if let Some(source) = source {
            self.palettes.insert(target, source);
        }
    }

    pub(super) fn set_canvas_base(&mut self, source: Option<ThemeStyleSource>) {
        self.canvas_base = source;
    }
}

/// Sources in the wire's mixed style-entry order, consumed alongside decoding.
#[derive(Debug)]
pub(super) enum ThemeWireSources {
    None,
    CompleteSpec,
    Definition {
        styles: Vec<ThemeStyleSource>,
        canvas_base: ThemeStyleSource,
    },
}

impl ThemeWireSources {
    pub(super) fn document(&self) -> Option<ThemeSourceDocument> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeSourceDocument::CompleteSpec),
            Self::Definition { .. } => Some(ThemeSourceDocument::Definition),
        }
    }

    pub(super) fn take_style(&mut self, index: usize) -> Option<ThemeStyleSource> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeStyleSource::authored(format!("/styles/{index}"))),
            Self::Definition { styles, .. } => Some(std::mem::take(&mut styles[index])),
        }
    }

    pub(super) fn take_canvas_base(&mut self) -> Option<ThemeStyleSource> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeStyleSource::authored("/canvas/base".to_owned())),
            Self::Definition { canvas_base, .. } => Some(std::mem::take(canvas_base)),
        }
    }
}
