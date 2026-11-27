use super::SourceConfigPath;
use crate::{MermaidConfig, SourceSpan};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(test)]
use std::mem::size_of;

/// Mermaid source construct that supplied one configuration key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceConfigOrigin {
    Frontmatter,
    Directive { directive_index: usize },
}

/// One admitted source declaration and the exact construct that supplied its assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAppearanceDeclaration {
    value: String,
    origin: SourceConfigOrigin,
    construct_span: SourceSpan,
}

impl SourceAppearanceDeclaration {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn origin(&self) -> SourceConfigOrigin {
        self.origin
    }

    pub const fn construct_span(&self) -> SourceSpan {
        self.construct_span
    }
}

/// Three authored declarations, independent from theme evaluation and completed SVG styling.
/// Unknown configuration must not be presented as a known absence of source declarations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SourceAppearanceSummary {
    #[default]
    Unknown,
    Known {
        theme: Option<SourceAppearanceDeclaration>,
        font: Option<SourceAppearanceDeclaration>,
        look: Option<SourceAppearanceDeclaration>,
    },
}

impl SourceAppearanceSummary {
    pub fn estimated_owned_heap_bytes(&self) -> usize {
        match self {
            Self::Unknown => 0,
            Self::Known { theme, font, look } => [theme, font, look]
                .into_iter()
                .flatten()
                .fold(0usize, |bytes, declaration| {
                    bytes.saturating_add(declaration.value.capacity())
                }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceAppearanceOwner {
    pub(super) origin: SourceConfigOrigin,
    pub(super) construct_span: SourceSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SourceAppearanceCapture {
    owners: BTreeMap<(&'static str, &'static str), SourceAppearanceOwner>,
}

impl SourceAppearanceCapture {
    fn paths() -> impl Iterator<Item = (&'static str, &'static str)> {
        [
            ("", "theme"),
            ("", "look"),
            ("", "fontFamily"),
            ("themeVariables", "fontFamily"),
        ]
        .into_iter()
        .chain(
            merman_core_preprocess::frontmatter::frontmatter_config_namespaces()
                .iter()
                .flat_map(|namespace| [(*namespace, "theme"), (*namespace, "look")]),
        )
    }

    fn affected(path: &[&str], namespace: &str, key: &str) -> bool {
        match path {
            [] => true,
            [root] if namespace.is_empty() => *root == key,
            [root] => *root == namespace,
            [root, leaf] => *root == namespace && *leaf == key,
            _ => false,
        }
    }

    pub(super) fn record(&mut self, path: &[&str], owner: SourceAppearanceOwner) {
        if path.len() > 2 {
            return;
        }
        for (namespace, key) in Self::paths() {
            if Self::affected(path, namespace, key) {
                self.owners.insert((namespace, key), owner);
            }
        }
    }

    pub(super) fn merge(
        &mut self,
        target: &mut MermaidConfig,
        incoming: &MermaidConfig,
        incoming_capture: &Self,
    ) {
        target.merge_with_mutation_observer(incoming.as_value(), &mut |path| {
            if path.len() > 2 {
                return;
            }
            for (namespace, key) in Self::paths() {
                if Self::affected(path, namespace, key) {
                    if let Some(owner) = incoming_capture.owners.get(&(namespace, key)) {
                        self.owners.insert((namespace, key), *owner);
                    } else {
                        self.owners.remove(&(namespace, key));
                    }
                }
            }
        });
    }

    pub(super) fn map_spans(&mut self, source: &super::PreprocessedSource) -> bool {
        let mut complete = true;
        for owner in self.owners.values_mut() {
            if let Some(span) = source.try_map_span(owner.construct_span) {
                owner.construct_span = span;
            } else {
                complete = false;
            }
        }
        complete
    }

    pub(super) fn normalize_font_alias(&mut self, config: &MermaidConfig) {
        if crate::config::legacy_font_family_for_mirroring(config.as_value()).is_some()
            && let Some(owner) = self.owners.get(&("", "fontFamily")).copied()
        {
            self.owners.insert(("themeVariables", "fontFamily"), owner);
        }
    }

    fn declaration(
        &self,
        config: &MermaidConfig,
        namespace: &'static str,
        key: &'static str,
    ) -> Option<SourceAppearanceDeclaration> {
        let root = config.as_value();
        let value = if namespace.is_empty() {
            root.get(key)
        } else {
            root.get(namespace)?.get(key)
        };
        let value = value?.as_str()?;
        let owner = self.owners.get(&(namespace, key))?;
        Some(SourceAppearanceDeclaration {
            value: value.to_owned(),
            origin: owner.origin,
            construct_span: owner.construct_span,
        })
    }
}

/// Source-backed evidence for one Mermaid directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDirectiveEvidence {
    keyword: String,
    full_span: SourceSpan,
    keyword_span: SourceSpan,
    order: usize,
    complete: bool,
    rewrite_safe: bool,
}

impl SourceDirectiveEvidence {
    pub(super) fn new(
        keyword: String,
        full_span: SourceSpan,
        keyword_span: SourceSpan,
        order: usize,
        complete: bool,
        rewrite_safe: bool,
    ) -> Self {
        Self {
            keyword,
            full_span,
            keyword_span,
            order,
            complete,
            rewrite_safe,
        }
    }

    pub fn keyword(&self) -> &str {
        &self.keyword
    }

    pub const fn full_span(&self) -> SourceSpan {
        self.full_span
    }

    pub const fn keyword_span(&self) -> SourceSpan {
        self.keyword_span
    }

    pub const fn order(&self) -> usize {
        self.order
    }

    pub const fn complete(&self) -> bool {
        self.complete
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    #[cfg(test)]
    fn estimated_owned_heap_bytes(&self) -> usize {
        self.keyword.capacity()
    }
}

/// Source-backed evidence for one source-addressable configuration key accepted by the owning
/// YAML or JSON5 parser.
///
/// A parser may accept a key whose decoded name cannot be mapped back to one contiguous source
/// span (for example, an escaped JSON5 key). Such keys remain in the parsed configuration value
/// but are intentionally absent from this collection rather than receiving a guessed span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfigKeyEvidence {
    origin: SourceConfigOrigin,
    path: SourceConfigPath,
    span: SourceSpan,
    order: usize,
    rewrite_safe: bool,
}

impl SourceConfigKeyEvidence {
    pub(super) fn new(
        origin: SourceConfigOrigin,
        path: SourceConfigPath,
        span: SourceSpan,
        order: usize,
        rewrite_safe: bool,
    ) -> Self {
        Self {
            origin,
            path,
            span,
            order,
            rewrite_safe,
        }
    }

    pub const fn origin(&self) -> SourceConfigOrigin {
        self.origin
    }

    /// Returns the decoded path segments in source order.
    ///
    /// The iterator owns only a temporary index of borrowed segments; iterating paths never adds
    /// retained storage to the captured evidence.
    pub fn path_segments(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator + '_ {
        self.path.segments().into_iter()
    }

    pub fn matches_path(&self, expected: &[&str]) -> bool {
        self.path.matches(expected)
    }

    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub const fn order(&self) -> usize {
        self.order
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }
}

/// Source-backed frontmatter boundary and rewrite-safety evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontmatterSourceEvidence {
    full_span: SourceSpan,
    body_span: SourceSpan,
    indent: String,
    has_config: bool,
    rewrite_safe: bool,
    fields: Option<MermaidConfig>,
    insert_order: Option<super::rewrite_order::FrontmatterInsertOrder>,
}

impl FrontmatterSourceEvidence {
    pub(super) fn new(
        full_span: SourceSpan,
        body_span: SourceSpan,
        indent: String,
        has_config: bool,
        rewrite_safe: bool,
        fields: Option<MermaidConfig>,
        insert_order: Option<super::rewrite_order::FrontmatterInsertOrder>,
    ) -> Self {
        Self {
            full_span,
            body_span,
            indent,
            has_config,
            rewrite_safe,
            fields,
            insert_order,
        }
    }

    pub const fn full_span(&self) -> SourceSpan {
        self.full_span
    }

    pub const fn body_span(&self) -> SourceSpan {
        self.body_span
    }

    pub fn indent(&self) -> &str {
        &self.indent
    }

    pub const fn has_config(&self) -> bool {
        self.has_config
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    /// Parsed top-level frontmatter fields retained from the owning preprocessing operation.
    pub fn fields(&self) -> Option<&MermaidConfig> {
        self.fields.as_ref()
    }

    #[doc(hidden)]
    pub fn insert_order(&self) -> Option<&super::rewrite_order::FrontmatterInsertOrder> {
        self.insert_order.as_ref()
    }

    #[cfg(test)]
    fn estimated_owned_heap_bytes(&self) -> usize {
        self.indent
            .capacity()
            .saturating_add(
                self.fields
                    .as_ref()
                    .map_or(0, MermaidConfig::estimated_owned_heap_bytes),
            )
            .saturating_add(self.insert_order.as_ref().map_or(
                0,
                super::rewrite_order::FrontmatterInsertOrder::estimated_owned_heap_bytes,
            ))
    }
}

/// Compact source-configuration evidence retained by an editor parse operation.
///
/// The evidence owns no copy of the Mermaid source and no YAML/JSON5 syntax tree. Paths and spans
/// are emitted by the same parser operation that materializes Mermaid configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConfigEvidence {
    frontmatter: Option<FrontmatterSourceEvidence>,
    directives: Vec<SourceDirectiveEvidence>,
    keys: Vec<SourceConfigKeyEvidence>,
    config_insert_span: SourceSpan,
    legacy_init_namespace: Option<&'static str>,
    rewrite_safe: bool,
    appearance: SourceAppearanceSummary,
    pub(super) appearance_capture: SourceAppearanceCapture,
    pub(super) appearance_complete: bool,
}

impl Default for SourceConfigEvidence {
    fn default() -> Self {
        Self::empty()
    }
}

impl SourceConfigEvidence {
    pub(super) fn empty() -> Self {
        Self {
            frontmatter: None,
            directives: Vec::new(),
            keys: Vec::new(),
            config_insert_span: SourceSpan::new(0, 0),
            legacy_init_namespace: None,
            rewrite_safe: true,
            appearance: SourceAppearanceSummary::Unknown,
            appearance_capture: SourceAppearanceCapture::default(),
            appearance_complete: false,
        }
    }

    pub fn frontmatter(&self) -> Option<&FrontmatterSourceEvidence> {
        self.frontmatter.as_ref()
    }

    #[doc(hidden)]
    pub const fn legacy_init_namespace(&self) -> Option<&'static str> {
        self.legacy_init_namespace
    }

    pub(super) fn set_legacy_init_namespace(&mut self, namespace: Option<&'static str>) {
        self.legacy_init_namespace = namespace;
    }

    pub fn directives(&self) -> &[SourceDirectiveEvidence] {
        &self.directives
    }

    pub fn keys(&self) -> &[SourceConfigKeyEvidence] {
        &self.keys
    }

    pub fn appearance(&self) -> &SourceAppearanceSummary {
        &self.appearance
    }

    pub(crate) fn freeze_appearance(
        &mut self,
        diagram_type: &str,
        admitted_source: &MermaidConfig,
    ) {
        let capture = std::mem::take(&mut self.appearance_capture);
        if !self.appearance_complete {
            return;
        }
        let namespace = crate::family::config_namespace_for_diagram_type(diagram_type);
        let declaration = |key| {
            namespace
                .and_then(|namespace| capture.declaration(admitted_source, namespace, key))
                .or_else(|| capture.declaration(admitted_source, "", key))
        };
        self.appearance = SourceAppearanceSummary::Known {
            theme: declaration("theme"),
            font: capture
                .declaration(admitted_source, "themeVariables", "fontFamily")
                .or_else(|| capture.declaration(admitted_source, "", "fontFamily")),
            look: declaration("look"),
        };
    }

    /// Original-source insertion point for a new frontmatter `config` block.
    pub const fn config_insert_span(&self) -> SourceSpan {
        self.config_insert_span
    }

    pub const fn rewrite_safe(&self) -> bool {
        self.rewrite_safe
    }

    /// Estimated owned heap storage, excluding the source text that the spans address.
    #[cfg(test)]
    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        let directive_storage = self
            .directives
            .capacity()
            .saturating_mul(size_of::<SourceDirectiveEvidence>());
        let key_storage = self
            .keys
            .capacity()
            .saturating_mul(size_of::<SourceConfigKeyEvidence>());
        let directive_payload = self
            .directives
            .iter()
            .map(SourceDirectiveEvidence::estimated_owned_heap_bytes)
            .fold(0usize, usize::saturating_add);
        let key_payload = SourceConfigPath::estimated_paths_owned_heap_bytes(
            self.keys.iter().map(|key| &key.path),
        );
        let frontmatter = self
            .frontmatter
            .as_ref()
            .map_or(0, FrontmatterSourceEvidence::estimated_owned_heap_bytes);
        directive_storage
            .saturating_add(key_storage)
            .saturating_add(directive_payload)
            .saturating_add(key_payload)
            .saturating_add(frontmatter)
            .saturating_add(self.appearance.estimated_owned_heap_bytes())
    }

    pub(super) fn set_frontmatter(&mut self, frontmatter: FrontmatterSourceEvidence) {
        // Adding a missing `config` block is an insertion and preserves the existing YAML bytes.
        // Parser losslessness is required only when an existing config forces full frontmatter
        // materialization and replacement.
        if frontmatter.has_config {
            self.rewrite_safe &= frontmatter.rewrite_safe;
        }
        self.frontmatter = Some(frontmatter);
    }

    pub(super) fn set_config_insert_span(&mut self, span: SourceSpan) {
        self.config_insert_span = span;
    }

    pub(super) fn push_directive(&mut self, directive: SourceDirectiveEvidence) -> usize {
        let index = self.directives.len();
        if matches!(directive.keyword(), "init" | "initialize") {
            self.rewrite_safe &= directive.rewrite_safe;
        }
        self.directives.push(directive);
        index
    }

    pub(super) fn push_key(&mut self, key: SourceConfigKeyEvidence) {
        self.keys.push(key);
    }

    pub(super) fn mark_rewrite_unsafe(&mut self) {
        self.rewrite_safe = false;
    }
}
