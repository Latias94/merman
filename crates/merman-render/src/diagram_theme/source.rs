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
    Design,
    CompleteSpec,
}

impl ThemeSourceDocument {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Design => "design",
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
    Design {
        styles: Vec<ThemeStyleSource>,
        canvas_base: Option<ThemeStyleSource>,
    },
}

impl ThemeWireSources {
    pub(super) fn for_design(design: &merman_theme_contract::ThemeDesignV1) -> Self {
        let escape = |target: &str| target.replace('~', "~0").replace('/', "~1");
        let styles = design
            .styles
            .keys()
            .map(|target| format!("/styles/{}", escape(target)))
            .chain(
                design
                    .rules
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format!("/rules/{index}/style")),
            )
            .chain(
                design
                    .palettes
                    .keys()
                    .map(|target| format!("/palettes/{}", escape(target))),
            )
            .map(ThemeStyleSource::authored)
            .collect();
        let canvas_base = design
            .canvas
            .as_ref()
            .filter(|canvas| canvas.base.is_some())
            .map(|_| ThemeStyleSource::authored("/canvas/base".to_owned()));
        Self::Design {
            styles,
            canvas_base,
        }
    }

    pub(super) fn document(&self) -> Option<ThemeSourceDocument> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeSourceDocument::CompleteSpec),
            Self::Design { .. } => Some(ThemeSourceDocument::Design),
        }
    }

    pub(super) fn take_style(&mut self, index: usize) -> Option<ThemeStyleSource> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeStyleSource::authored(format!("/styles/{index}"))),
            Self::Design { styles, .. } => styles.get_mut(index).map(std::mem::take),
        }
    }

    pub(super) fn take_canvas_base(&mut self) -> Option<ThemeStyleSource> {
        match self {
            Self::None => None,
            Self::CompleteSpec => Some(ThemeStyleSource::authored("/canvas/base".to_owned())),
            Self::Design { canvas_base, .. } => canvas_base.take(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_source_paths_follow_sorted_defaults_and_authored_rule_order() {
        let design = serde_json::from_value(serde_json::json!({
            "styles":{"z/z~":{},"a":{}},
            "rules":[{"target":"node","style":{}},{"target":"edge","style":{}}],
            "palettes":{"z/z~":["red"]},
            "canvas":{"base":"white"}
        }))
        .unwrap();
        let mut sources = ThemeWireSources::for_design(&design);
        assert_eq!(sources.document().unwrap().id(), "design");
        for (index, expected) in [
            "/styles/a",
            "/styles/z~1z~0",
            "/rules/0/style",
            "/rules/1/style",
            "/palettes/z~1z~0",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(sources.take_style(index).unwrap().paths, [expected]);
        }
        assert!(sources.take_style(5).is_none());
        assert_eq!(sources.take_canvas_base().unwrap().paths, ["/canvas/base"]);
        assert!(sources.take_canvas_base().is_none());
    }
}
