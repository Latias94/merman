use super::css::StyleDeclarationScanner;
use crate::svg::pipeline::{
    SvgTagScanner, checkpoint_loop, end_tag_name, find_tag_end_with_checkpoints,
    find_with_checkpoints, start_tag_name, trim_with_checkpoints,
};
use crate::text::TextStyle;
use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, StyleSheetParser, Token,
};
use merman_core::theme_color::ThemeColor;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_FONT_FAMILY: &str = "trebuchet ms,verdana,arial,sans-serif";
const DEFAULT_FILL: &str = "#333";
const DEFAULT_COLOR: &str = "#000";
// Fallback text is a bounded adapter, so pathological CSS magnitudes must not enter output or
// overflow relative-unit and line-height calculations.
const MAX_CSS_NUMERIC_VALUE: f64 = 1_000_000.0;
pub(super) const MAX_SELECTOR_STYLESHEET_BYTES: usize = 2 * 1024 * 1024;
const MAX_SELECTOR_BYTES: usize = 1024 * 1024;
const MAX_SELECTOR_QUALIFIED_RULES: usize = 8192;
const MAX_SELECTOR_BRANCHES: usize = 16_384;
const MAX_SELECTOR_RULES: usize = 16_384;
const MAX_SELECTOR_COMPONENTS: usize = 65_536;
pub(super) const MAX_SELECTOR_COMPONENTS_PER_BRANCH: usize = 64;
const MAX_SELECTOR_DECLARATIONS: usize = 32_768;
pub(super) const MAX_SELECTOR_DECLARATIONS_PER_RULE: usize = 256;
pub(super) const MAX_SELECTOR_POSTINGS: usize = 8192;
pub(super) const MAX_UNIVERSAL_POSTINGS: usize = 4096;
// Matching work is byte-weighted so repeated class/attribute string scans cannot hide behind a
// small selector-component count. Keep enough headroom for normal multi-label Mermaid diagrams.
pub(super) const MAX_SELECTOR_MATCH_WORK: usize = 16 * 1024 * 1024;
pub(super) const MAX_SELECTOR_ANCESTRY_DEPTH: usize = crate::resources::MAX_RESVG_TREE_DEPTH;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum Namespace {
    Svg,
    Xhtml,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceElement {
    pub(super) namespace: Namespace,
    pub(super) local_name: String,
    pub(super) id: Option<String>,
    pub(super) classes: Arc<HashSet<String>>,
    attributes: Arc<HashMap<String, Option<String>>>,
    pub(super) inline: Vec<Declaration>,
    pub(super) presentation: Vec<Declaration>,
    selector_weight: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceAttribute {
    name: String,
    value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Declaration {
    pub(super) property: String,
    pub(super) value: String,
    pub(super) important: bool,
    pub(super) order: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ResolvedStyle {
    pub(super) font_size: f64,
    pub(super) font_family: String,
    pub(super) font_weight: Option<String>,
    pub(super) font_style: Option<String>,
    line_height: LineHeight,
    pub(super) fill: String,
    color: Option<String>,
    pub(super) background_color: Option<String>,
    background_color_specified: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum LineHeight {
    Normal,
    Multiplier(f64),
    AbsolutePx(f64),
}

impl LineHeight {
    fn pixels(&self, font_size: f64) -> f64 {
        match self {
            Self::Normal => font_size * 1.5,
            Self::Multiplier(value) => font_size * value,
            Self::AbsolutePx(value) => *value,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedFallbackTypography {
    pub(super) font_size: f64,
    pub(super) font_family: String,
    pub(super) font_weight: Option<String>,
    pub(super) font_style: Option<String>,
    pub(super) line_height: f64,
    pub(super) fill: String,
    pub(super) label_background: Option<String>,
}

impl ResolvedFallbackTypography {
    pub(super) fn text_style(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.font_family.clone()),
            font_size: self.font_size,
            font_weight: self.font_weight.clone(),
            font_style: self.font_style.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Specificity(u32, u32, u32);

#[derive(Clone, Debug)]
struct Compound {
    // A selector without an explicit type is namespace-neutral: `.label` can
    // match either the SVG source context or XHTML inside `foreignObject`.
    namespace: Option<Namespace>,
    local_name: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attributes: Vec<AttributeSelector>,
}

#[derive(Clone, Debug)]
struct AttributeSelector {
    name: String,
    matcher: AttributeMatcher,
}

#[derive(Clone, Debug)]
enum AttributeMatcher {
    Exists,
    Exact(String),
    ContainsToken(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
}

#[derive(Clone, Debug)]
struct Branch {
    compounds: Vec<Compound>,
    combinators: Vec<Combinator>,
    specificity: Specificity,
    component_count: usize,
    selector_weight: usize,
}

#[derive(Clone, Debug)]
struct Rule {
    branch: Branch,
    declarations: Arc<[Declaration]>,
    declaration_match_weight: usize,
    source_order: usize,
}

#[derive(Debug, Default)]
struct SelectorBudget {
    stylesheet_bytes: usize,
    selector_bytes: usize,
    qualified_rules: usize,
    branches: usize,
    rules: usize,
    components: usize,
    declarations: usize,
    postings: usize,
    match_work: usize,
    admission_exhausted: bool,
    postings_exhausted: bool,
    matching_exhausted: bool,
}

fn checked_charge<E>(
    used: &mut usize,
    exhausted: &mut bool,
    additional: usize,
    maximum: usize,
    selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<bool, E> {
    if *exhausted {
        return Ok(false);
    }
    let actual = used.checked_add(additional).unwrap_or(usize::MAX);
    if actual > maximum {
        *exhausted = true;
        selector_limit(actual, maximum)?;
        return Ok(false);
    }
    *used = actual;
    Ok(true)
}

impl SelectorBudget {
    fn reject_admission<E>(
        &mut self,
        actual: usize,
        maximum: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        self.admission_exhausted = true;
        selector_limit(actual, maximum)?;
        Ok(false)
    }

    fn charge_stylesheet_bytes<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.stylesheet_bytes,
            &mut self.admission_exhausted,
            additional,
            MAX_SELECTOR_STYLESHEET_BYTES,
            selector_limit,
        )
    }

    fn charge_selector_bytes<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.selector_bytes,
            &mut self.admission_exhausted,
            additional,
            MAX_SELECTOR_BYTES,
            selector_limit,
        )
    }

    fn charge_qualified_rule<E>(
        &mut self,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.qualified_rules,
            &mut self.admission_exhausted,
            1,
            MAX_SELECTOR_QUALIFIED_RULES,
            selector_limit,
        )
    }

    fn charge_branch<E>(
        &mut self,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.branches,
            &mut self.admission_exhausted,
            1,
            MAX_SELECTOR_BRANCHES,
            selector_limit,
        )
    }

    fn charge_rules<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.rules,
            &mut self.admission_exhausted,
            additional,
            MAX_SELECTOR_RULES,
            selector_limit,
        )
    }

    fn charge_components<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.components,
            &mut self.admission_exhausted,
            additional,
            MAX_SELECTOR_COMPONENTS,
            selector_limit,
        )
    }

    fn charge_declarations<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.declarations,
            &mut self.admission_exhausted,
            additional,
            MAX_SELECTOR_DECLARATIONS,
            selector_limit,
        )
    }

    fn charge_posting_group<E>(
        &mut self,
        additional: usize,
        universal_additional: usize,
        current_universal: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.postings_exhausted {
            return Ok(false);
        }
        let actual = self.postings.checked_add(additional).unwrap_or(usize::MAX);
        if actual > MAX_SELECTOR_POSTINGS {
            self.postings_exhausted = true;
            selector_limit(actual, MAX_SELECTOR_POSTINGS)?;
            return Ok(false);
        }
        let universal_actual = current_universal
            .checked_add(universal_additional)
            .unwrap_or(usize::MAX);
        if universal_actual > MAX_UNIVERSAL_POSTINGS {
            self.postings_exhausted = true;
            selector_limit(universal_actual, MAX_UNIVERSAL_POSTINGS)?;
            return Ok(false);
        }
        self.postings = actual;
        Ok(true)
    }

    fn charge_match_work<E>(
        &mut self,
        additional: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        checked_charge(
            &mut self.match_work,
            &mut self.matching_exhausted,
            additional,
            MAX_SELECTOR_MATCH_WORK,
            selector_limit,
        )
    }

    fn check_ancestry_depth<E>(
        &mut self,
        actual: usize,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.matching_exhausted {
            return Ok(false);
        }
        if actual > MAX_SELECTOR_ANCESTRY_DEPTH {
            self.matching_exhausted = true;
            selector_limit(actual, MAX_SELECTOR_ANCESTRY_DEPTH)?;
            return Ok(false);
        }
        Ok(true)
    }
}

#[derive(Debug)]
pub(super) struct CascadeIndex {
    rules: Vec<Rule>,
    id_postings: HashMap<String, Vec<usize>>,
    class_postings: HashMap<String, Vec<usize>>,
    type_postings: HashMap<String, Vec<usize>>,
    attribute_postings: HashMap<String, Vec<usize>>,
    universal_postings: Vec<usize>,
    budget: SelectorBudget,
}

impl CascadeIndex {
    fn with_postings<E>(
        rules: Vec<Rule>,
        budget: SelectorBudget,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut index = Self {
            rules,
            id_postings: HashMap::new(),
            class_postings: HashMap::new(),
            type_postings: HashMap::new(),
            attribute_postings: HashMap::new(),
            universal_postings: Vec::new(),
            budget,
        };
        // Rules produced by one selector list share a source-order value. Admit each such
        // group atomically so a posting cap cannot retain only part of a CSS selector list.
        let mut retained_rules = 0usize;
        let mut group_start = 0usize;
        while group_start < index.rules.len() {
            let source_order = index.rules[group_start].source_order;
            let mut group_end = group_start + 1;
            while group_end < index.rules.len()
                && index.rules[group_end].source_order == source_order
            {
                group_end += 1;
            }
            checkpoint_loop(group_start, checkpoint)?;

            let universal_additional = (group_start..group_end)
                .filter(|rule_index| {
                    let target = index.rules[*rule_index]
                        .branch
                        .compounds
                        .last()
                        .expect("admitted selector branches have a rightmost compound");
                    target.id.is_none()
                        && target.classes.is_empty()
                        && target.attributes.is_empty()
                        && target.local_name.is_none()
                })
                .count();
            if !index.budget.charge_posting_group(
                group_end - group_start,
                universal_additional,
                index.universal_postings.len(),
                selector_limit,
            )? {
                break;
            }

            for rule_index in group_start..group_end {
                checkpoint_loop(rule_index, checkpoint)?;
                let target = index.rules[rule_index]
                    .branch
                    .compounds
                    .last()
                    .expect("admitted selector branches have a rightmost compound");
                if let Some(id) = &target.id {
                    index
                        .id_postings
                        .entry(id.clone())
                        .or_default()
                        .push(rule_index);
                } else if let Some(class) = target.classes.first() {
                    index
                        .class_postings
                        .entry(class.clone())
                        .or_default()
                        .push(rule_index);
                } else if let Some(attribute) = target.attributes.first() {
                    index
                        .attribute_postings
                        .entry(attribute.name.clone())
                        .or_default()
                        .push(rule_index);
                } else if let Some(local_name) = &target.local_name {
                    index
                        .type_postings
                        .entry(local_name.clone())
                        .or_default()
                        .push(rule_index);
                } else {
                    // A universal rightmost selector cannot be narrowed by source-element
                    // postings, so keep it in the bounded universal bucket.
                    index.universal_postings.push(rule_index);
                }
            }
            retained_rules = group_end;
            group_start = group_end;
        }
        index.rules.truncate(retained_rules);
        checkpoint()?;
        Ok(index)
    }

    fn candidate_rule_indices(&self, element: &SourceElement) -> Vec<usize> {
        if self.budget.matching_exhausted {
            return Vec::new();
        }
        // Each branch is assigned exactly one primary rightmost posting when
        // indexed, so these buckets are disjoint and need no per-label
        // de-duplication. Candidate order is irrelevant because the cascade
        // compares the retained source-order tuple.
        let mut candidates = self.universal_postings.clone();
        if let Some(id) = &element.id
            && let Some(postings) = self.id_postings.get(id)
        {
            candidates.extend(postings);
        }
        for class in element.classes.iter() {
            if let Some(postings) = self.class_postings.get(class) {
                candidates.extend(postings);
            }
        }
        if let Some(postings) = self
            .type_postings
            .get(&element.local_name.to_ascii_lowercase())
        {
            candidates.extend(postings);
        }
        for attribute_name in element.attributes.keys() {
            if let Some(postings) = self.attribute_postings.get(attribute_name) {
                candidates.extend(postings);
            }
        }
        candidates
    }

    pub(super) fn new<E>(
        svg: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut rules = Vec::new();
        let mut source_order = 0usize;
        let mut budget = SelectorBudget::default();
        let mut scanner = SvgTagScanner::new(svg);
        let mut style_index = 0usize;
        while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
            checkpoint_loop(style_index, checkpoint)?;
            style_index = style_index.saturating_add(1);
            if start_tag_name(tag.raw()) != Some("style") || tag.is_self_closing() {
                continue;
            }
            let content_start = scanner.cursor();
            let mut close_tag = None;
            while let Some(candidate) = scanner.next_with_checkpoints(checkpoint)? {
                if end_tag_name(candidate.raw()) == Some("style") {
                    close_tag = Some(candidate);
                    break;
                }
            }
            let Some(close_tag) = close_tag else { break };
            let css = &svg[content_start..close_tag.start()];
            if !budget.charge_stylesheet_bytes(css.len(), selector_limit)?
                || !parse_stylesheet(
                    css,
                    &mut rules,
                    &mut source_order,
                    &mut budget,
                    checkpoint,
                    selector_limit,
                )?
            {
                break;
            }
        }
        checkpoint()?;
        Self::with_postings(rules, budget, checkpoint, selector_limit)
    }

    pub(super) fn source_element<E>(
        tag: &str,
        namespace: Namespace,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<SourceElement, E> {
        let source_name = start_tag_name(tag).unwrap_or("div");
        let local_name = if namespace == Namespace::Xhtml {
            source_name.to_ascii_lowercase()
        } else {
            source_name.to_string()
        };
        let attributes = parse_tag_attributes(tag, checkpoint)?;
        let id = attribute_value(&attributes, "id").map(str::to_owned);
        let classes = attribute_value(&attributes, "class")
            .map(|value| value.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default();
        let inline = parse_declarations(
            attribute_value(&attributes, "style").unwrap_or_default(),
            checkpoint,
        )?;
        let mut presentation = Vec::new();
        for property in [
            "font-size",
            "font-family",
            "font-weight",
            "font-style",
            "line-height",
            "fill",
            "color",
            "background-color",
        ] {
            if let Some(value) = attribute_value(&attributes, property) {
                presentation.push(Declaration {
                    property: property.to_string(),
                    value: value.to_string(),
                    important: false,
                    order: 0,
                });
            }
        }
        let attributes = attributes
            .into_iter()
            .map(|attribute| (attribute.name, attribute.value))
            .collect::<HashMap<_, _>>();
        let selector_weight =
            source_selector_weight(&local_name, id.as_deref(), &classes, &attributes);
        checkpoint()?;
        Ok(SourceElement {
            namespace,
            local_name,
            id,
            classes: Arc::new(classes),
            attributes: Arc::new(attributes),
            inline,
            presentation,
            selector_weight,
        })
    }

    pub(super) fn resolve_path<E>(
        &mut self,
        path: &[SourceElement],
        inherited: Option<&ResolvedStyle>,
        root_font_size: f64,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<ResolvedStyle, E> {
        let element = path.last().expect("source path is non-empty");
        let parent = inherited.cloned().unwrap_or_else(default_style);
        let mut specified: HashMap<String, Specified> = HashMap::new();
        let may_collect_candidates = self
            .budget
            .check_ancestry_depth(path.len(), selector_limit)?
            && self
                .budget
                .charge_match_work(element.selector_weight, selector_limit)?;
        let mut candidate_rule_indices = if may_collect_candidates {
            self.candidate_rule_indices(element)
        } else {
            Vec::new()
        };
        let path_source_weight = path
            .iter()
            .try_fold(0usize, |total, element| {
                total.checked_add(element.selector_weight)
            })
            .unwrap_or(usize::MAX);
        let match_work = candidate_rule_indices
            .iter()
            .fold(0usize, |total, rule_index| {
                let rule = &self.rules[*rule_index];
                let branch_work = rule
                    .branch
                    .selector_weight
                    .checked_mul(path.len())
                    .and_then(|work| {
                        rule.branch
                            .component_count
                            .checked_mul(path_source_weight)
                            .and_then(|source_work| work.checked_add(source_work))
                    })
                    .and_then(|work| work.checked_add(rule.declaration_match_weight))
                    .unwrap_or(usize::MAX);
                total.checked_add(branch_work).unwrap_or(usize::MAX)
            });
        if !self.budget.charge_match_work(match_work, selector_limit)? {
            candidate_rule_indices.clear();
        }
        for (candidate_index, rule_index) in candidate_rule_indices.into_iter().enumerate() {
            checkpoint_loop(candidate_index, checkpoint)?;
            let rule = &self.rules[rule_index];
            if !matches_branch(&rule.branch, path, checkpoint)? {
                continue;
            }
            for declaration in rule.declarations.iter() {
                select_specified(
                    &mut specified,
                    declaration,
                    Priority {
                        important: declaration.important,
                        inline: false,
                        specificity: rule.branch.specificity,
                        source_order: (rule.source_order.saturating_add(1), declaration.order),
                    },
                    false,
                );
            }
        }
        for declaration in &element.presentation {
            select_specified(
                &mut specified,
                declaration,
                Priority {
                    important: false,
                    inline: false,
                    specificity: Specificity::default(),
                    source_order: (0, declaration.order),
                },
                true,
            );
        }
        for declaration in &element.inline {
            select_specified(
                &mut specified,
                declaration,
                Priority {
                    important: declaration.important,
                    inline: true,
                    specificity: Specificity(1, 0, 0),
                    source_order: (usize::MAX, declaration.order),
                },
                false,
            );
        }

        let font_size = resolve_font_size(
            specified.get("font-size").map(|value| value.value.as_str()),
            parent.font_size,
            root_font_size,
            specified
                .get("font-size")
                .is_some_and(|value| value.presentation),
        )
        .unwrap_or(parent.font_size);
        let font_family = resolve_inherited_text_value(
            specified
                .get("font-family")
                .map(|value| value.value.as_str()),
            &parent.font_family,
            DEFAULT_FONT_FAMILY,
        );
        let font_weight = resolve_optional_text_value(
            specified
                .get("font-weight")
                .map(|value| value.value.as_str()),
            parent.font_weight.as_deref(),
        );
        let font_style = resolve_optional_text_value(
            specified
                .get("font-style")
                .map(|value| value.value.as_str()),
            parent.font_style.as_deref(),
        );
        let line_height = resolve_line_height(
            specified
                .get("line-height")
                .map(|value| value.value.as_str()),
            font_size,
            &parent.line_height,
            root_font_size,
        );
        let fill = resolve_inherited_text_value(
            specified.get("fill").map(|value| value.value.as_str()),
            &parent.fill,
            DEFAULT_FILL,
        );
        let color = resolve_optional_color(
            specified.get("color").map(|value| value.value.as_str()),
            parent.color.as_deref(),
        );
        let mut background_color = resolve_background(
            specified
                .get("background-color")
                .map(|value| value.value.as_str()),
            parent.background_color.as_deref(),
        );
        if background_color
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("currentcolor"))
        {
            background_color = Some(color.as_deref().unwrap_or(DEFAULT_COLOR).to_string());
        }
        let background_color_specified = specified.contains_key("background-color");
        checkpoint()?;
        Ok(ResolvedStyle {
            font_size,
            font_family,
            font_weight,
            font_style,
            line_height,
            fill,
            color,
            background_color,
            background_color_specified,
        })
    }

    pub(super) fn resolve_foreign_object<E>(
        &mut self,
        svg_ancestors: &[SourceElement],
        foreign_object_tag: &str,
        html: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<ResolvedFallbackTypography, E> {
        let base_depth = svg_ancestors.len().checked_add(1).unwrap_or(usize::MAX);
        if !self
            .budget
            .check_ancestry_depth(base_depth, selector_limit)?
        {
            return Ok(default_fallback_typography());
        }
        let foreign_object = Self::source_element(foreign_object_tag, Namespace::Svg, checkpoint)?;
        let mut base_path = svg_ancestors.to_vec();
        base_path.push(foreign_object);
        let mut html_stack = Vec::new();
        let mut first_style = None;
        let mut all_same_style = true;
        let mut common_path = None;
        let mut cursor = 0usize;
        let mut iteration = 0usize;
        while let Some(relative) = find_with_checkpoints(&html[cursor..], "<", checkpoint)? {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            let start = cursor + relative;
            if html_text_is_visible(&html[cursor..start]) {
                self.observe_text_path(
                    join_path(&base_path, &html_stack),
                    &mut first_style,
                    &mut all_same_style,
                    &mut common_path,
                    checkpoint,
                    selector_limit,
                )?;
                if self.budget.matching_exhausted {
                    return Ok(default_fallback_typography());
                }
            }
            let Some(end) = find_tag_end_with_checkpoints(html, start, checkpoint)? else {
                break;
            };
            let end = end + 1;
            let tag = &html[start..end];
            if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") {
                cursor = end;
                continue;
            }
            if let Some(name) = end_tag_name(tag) {
                if html_stack
                    .last()
                    .is_some_and(|element| element.local_name.eq_ignore_ascii_case(name))
                {
                    html_stack.pop();
                }
                cursor = end;
                continue;
            }
            if start_tag_name(tag).is_some() {
                let next_depth = base_path
                    .len()
                    .checked_add(html_stack.len())
                    .and_then(|depth| depth.checked_add(1))
                    .unwrap_or(usize::MAX);
                if !self
                    .budget
                    .check_ancestry_depth(next_depth, selector_limit)?
                {
                    return Ok(default_fallback_typography());
                }
                let element = Self::source_element(tag, Namespace::Xhtml, checkpoint)?;
                html_stack.push(element);
                if is_void_html_element(
                    html_stack
                        .last()
                        .expect("element was pushed")
                        .local_name
                        .as_str(),
                ) || tag.trim_end().ends_with("/>")
                {
                    html_stack.pop();
                }
            }
            cursor = end;
        }
        if html_text_is_visible(&html[cursor..]) {
            self.observe_text_path(
                join_path(&base_path, &html_stack),
                &mut first_style,
                &mut all_same_style,
                &mut common_path,
                checkpoint,
                selector_limit,
            )?;
            if self.budget.matching_exhausted {
                return Ok(default_fallback_typography());
            }
        }
        if first_style.is_none() {
            self.observe_text_path(
                base_path.clone(),
                &mut first_style,
                &mut all_same_style,
                &mut common_path,
                checkpoint,
                selector_limit,
            )?;
            if self.budget.matching_exhausted {
                return Ok(default_fallback_typography());
            }
        }

        let common_path = common_path.unwrap_or_else(|| base_path.clone());
        let style = if all_same_style {
            first_style.expect("at least one text style")
        } else {
            self.resolve_full_path(&common_path, checkpoint, selector_limit)?
        };
        let mut label_background = None;
        let mut background_color_specified = false;
        let mut label_background_marker = false;
        for depth in base_path.len()..common_path.len() {
            let element = &common_path[depth];
            label_background_marker |= element.classes.contains("labelBkg");
            let background_style =
                self.resolve_full_path(&common_path[..=depth], checkpoint, selector_limit)?;
            if background_style.background_color_specified {
                background_color_specified = true;
                match background_style.background_color {
                    Some(color) if !color.eq_ignore_ascii_case("transparent") => {
                        label_background = Some(color);
                    }
                    // `transparent`, `initial`, and `unset` are specified values that
                    // clear an earlier owner background; they must not leave a stale
                    // parent rectangle visible in the generated fallback.
                    Some(_) | None => label_background = None,
                }
            }
        }
        if label_background.is_none() && label_background_marker && !background_color_specified {
            label_background = Some("rgba(232, 232, 232, 0.5)".to_string());
        }
        if self.budget.matching_exhausted {
            return Ok(default_fallback_typography());
        }
        checkpoint()?;
        Ok(fallback_typography(style, label_background))
    }

    fn observe_text_path<E>(
        &mut self,
        path: Vec<SourceElement>,
        first_style: &mut Option<ResolvedStyle>,
        all_same_style: &mut bool,
        common_path: &mut Option<Vec<SourceElement>>,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let style = self.resolve_full_path(&path, checkpoint, selector_limit)?;
        if let Some(first_style) = first_style.as_ref() {
            if !same_effective_typography(first_style, &style) {
                *all_same_style = false;
            }
        } else {
            *first_style = Some(style);
        }
        if let Some(common) = common_path.as_mut() {
            let common_len = common
                .iter()
                .zip(path.iter())
                .take_while(|(left, right)| *left == *right)
                .count();
            common.truncate(common_len);
        } else {
            *common_path = Some(path);
        }
        Ok(())
    }

    fn resolve_full_path<E>(
        &mut self,
        path: &[SourceElement],
        checkpoint: &mut impl FnMut() -> Result<(), E>,
        selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<ResolvedStyle, E> {
        let mut computed = None;
        let mut root_font_size = DEFAULT_FONT_SIZE;
        for (index, _) in path.iter().enumerate() {
            checkpoint_loop(index, checkpoint)?;
            let style = self.resolve_path(
                &path[..=index],
                computed.as_ref(),
                root_font_size,
                checkpoint,
                selector_limit,
            )?;
            if index == 0 {
                root_font_size = style.font_size;
            }
            computed = Some(style);
        }
        checkpoint()?;
        Ok(computed.unwrap_or_else(default_style))
    }
}

fn parse_tag_attributes<E>(
    tag: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<SourceAttribute>, E> {
    let mut attributes = Vec::new();
    let mut cursor = 1usize;
    let mut iteration = 0usize;

    // Skip the opening element name. `source_element` is called only for start
    // tags, so a compact XML/HTML attribute scan is enough here and avoids
    // conflating the current source context with generated fallback markup.
    while cursor < tag.len() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let Some(character) = tag[cursor..].chars().next() else {
            break;
        };
        if character.is_whitespace() || character == '>' || character == '/' {
            break;
        }
        cursor += character.len_utf8();
    }

    while cursor < tag.len() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let Some(character) = tag[cursor..].chars().next() else {
            break;
        };
        if character.is_whitespace() {
            cursor += character.len_utf8();
            continue;
        }
        if character == '>' || character == '/' {
            break;
        }

        let name_start = cursor;
        while cursor < tag.len() {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            let Some(candidate) = tag[cursor..].chars().next() else {
                break;
            };
            if candidate.is_whitespace() || matches!(candidate, '=' | '>' | '/') {
                break;
            }
            cursor += candidate.len_utf8();
        }
        if cursor == name_start {
            cursor += character.len_utf8();
            continue;
        }
        let name = tag[name_start..cursor].to_ascii_lowercase();

        while cursor < tag.len() {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            let Some(candidate) = tag[cursor..].chars().next() else {
                break;
            };
            if !candidate.is_whitespace() {
                break;
            }
            cursor += candidate.len_utf8();
        }

        let mut value = None;
        if tag[cursor..].starts_with('=') {
            cursor += 1;
            while cursor < tag.len() {
                checkpoint_loop(iteration, checkpoint)?;
                iteration = iteration.saturating_add(1);
                let Some(candidate) = tag[cursor..].chars().next() else {
                    break;
                };
                if !candidate.is_whitespace() {
                    break;
                }
                cursor += candidate.len_utf8();
            }
            if let Some(quote) = tag[cursor..]
                .chars()
                .next()
                .filter(|character| matches!(character, '\'' | '"'))
            {
                cursor += quote.len_utf8();
                let value_start = cursor;
                while cursor < tag.len() {
                    checkpoint_loop(iteration, checkpoint)?;
                    iteration = iteration.saturating_add(1);
                    let Some(candidate) = tag[cursor..].chars().next() else {
                        break;
                    };
                    if candidate == quote {
                        value = Some(tag[value_start..cursor].to_string());
                        cursor += candidate.len_utf8();
                        break;
                    }
                    cursor += candidate.len_utf8();
                }
            } else {
                let value_start = cursor;
                while cursor < tag.len() {
                    checkpoint_loop(iteration, checkpoint)?;
                    iteration = iteration.saturating_add(1);
                    let Some(candidate) = tag[cursor..].chars().next() else {
                        break;
                    };
                    if candidate.is_whitespace() || matches!(candidate, '>' | '/') {
                        break;
                    }
                    cursor += candidate.len_utf8();
                }
                if value_start < cursor {
                    value = Some(tag[value_start..cursor].to_string());
                }
            }
        }
        attributes.push(SourceAttribute { name, value });
    }

    checkpoint()?;
    Ok(attributes)
}

fn attribute_value<'a>(attributes: &'a [SourceAttribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .rev()
        .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
        .and_then(|attribute| attribute.value.as_deref())
}

fn source_selector_weight(
    local_name: &str,
    id: Option<&str>,
    classes: &HashSet<String>,
    attributes: &HashMap<String, Option<String>>,
) -> usize {
    let mut weight = 1usize.saturating_add(local_name.len());
    if let Some(id) = id {
        weight = weight.saturating_add(1).saturating_add(id.len());
    }
    for class in classes {
        weight = weight.saturating_add(1).saturating_add(class.len());
    }
    for (name, value) in attributes {
        weight = weight.saturating_add(1).saturating_add(name.len());
        if let Some(value) = value {
            weight = weight.saturating_add(1).saturating_add(value.len());
        }
    }
    weight
}

fn join_path(base_path: &[SourceElement], html_stack: &[SourceElement]) -> Vec<SourceElement> {
    let mut path = Vec::with_capacity(base_path.len().saturating_add(html_stack.len()));
    path.extend_from_slice(base_path);
    path.extend_from_slice(html_stack);
    path
}

fn html_text_is_visible(text: &str) -> bool {
    text.chars().any(|character| !character.is_whitespace())
}

fn is_void_html_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn same_effective_typography(left: &ResolvedStyle, right: &ResolvedStyle) -> bool {
    left.font_size.to_bits() == right.font_size.to_bits()
        && left.font_family == right.font_family
        && left.font_weight == right.font_weight
        && left.font_style == right.font_style
        && left.line_height.pixels(left.font_size).to_bits()
            == right.line_height.pixels(right.font_size).to_bits()
        && effective_html_text_paint(left) == effective_html_text_paint(right)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Priority {
    important: bool,
    inline: bool,
    specificity: Specificity,
    source_order: (usize, usize),
}

#[derive(Clone, Debug)]
struct Specified {
    priority: Priority,
    value: String,
    presentation: bool,
}

fn select_specified(
    specified: &mut HashMap<String, Specified>,
    declaration: &Declaration,
    priority: Priority,
    presentation: bool,
) {
    let property = if declaration.property == "background" {
        "background-color"
    } else {
        declaration.property.as_str()
    };
    if !is_supported_property(property)
        || !is_admitted_value(property, &declaration.value, presentation)
    {
        return;
    }
    if specified
        .get(property)
        .is_none_or(|current| priority > current.priority)
    {
        specified.insert(
            property.to_string(),
            Specified {
                priority,
                value: declaration.value.clone(),
                presentation,
            },
        );
    }
}

fn default_style() -> ResolvedStyle {
    ResolvedStyle {
        font_size: DEFAULT_FONT_SIZE,
        font_family: DEFAULT_FONT_FAMILY.to_string(),
        font_weight: None,
        font_style: None,
        line_height: LineHeight::Normal,
        fill: DEFAULT_FILL.to_string(),
        color: None,
        background_color: None,
        background_color_specified: false,
    }
}

fn default_fallback_typography() -> ResolvedFallbackTypography {
    fallback_typography(default_style(), None)
}

fn fallback_typography(
    style: ResolvedStyle,
    label_background: Option<String>,
) -> ResolvedFallbackTypography {
    let fill = effective_html_text_paint(&style).to_string();
    let line_height = style.line_height.pixels(style.font_size);
    ResolvedFallbackTypography {
        font_size: style.font_size,
        font_family: style.font_family,
        font_weight: style.font_weight,
        font_style: style.font_style,
        line_height,
        fill,
        label_background,
    }
}

fn is_supported_property(property: &str) -> bool {
    matches!(
        property,
        "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "line-height"
            | "fill"
            | "color"
            | "background-color"
    )
}

fn is_admitted_value(property: &str, value: &str, presentation: bool) -> bool {
    let value = value.trim();
    if value.is_empty()
        || lower_ascii_contains(value, "var(")
        || lower_ascii_contains(value, "calc(")
        || lower_ascii_contains(value, "env(")
    {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "inherit" | "initial" | "unset") {
        return true;
    }
    match property {
        "font-size" => {
            matches!(
                lower.as_str(),
                "xx-small"
                    | "x-small"
                    | "small"
                    | "medium"
                    | "large"
                    | "x-large"
                    | "xx-large"
                    | "smaller"
                    | "larger"
            ) || ["px", "rem", "em", "%"]
                .iter()
                .any(|unit| parse_css_number_with_unit(&lower, unit).is_some())
                || (presentation
                    && lower
                        .parse::<f64>()
                        .ok()
                        .and_then(bounded_positive)
                        .is_some())
        }
        "line-height" => {
            lower == "normal"
                || lower
                    .parse::<f64>()
                    .ok()
                    .and_then(bounded_positive)
                    .is_some()
                || ["px", "rem", "em", "%"]
                    .iter()
                    .any(|unit| parse_css_number_with_unit(&lower, unit).is_some())
        }
        "font-weight" => is_admitted_font_weight(&lower),
        "font-style" => matches!(lower.as_str(), "normal" | "italic" | "oblique"),
        "fill" => is_admitted_paint(&lower),
        "color" | "background-color" => lower != "none" && is_admitted_paint(&lower),
        "font-family" => is_admitted_font_family(value),
        _ => false,
    }
}

fn lower_ascii_contains(value: &str, needle: &str) -> bool {
    value.to_ascii_lowercase().contains(needle)
}

fn parse_css_number_with_unit(value: &str, unit: &str) -> Option<f64> {
    let number = value.strip_suffix(unit)?;
    if number.is_empty() || number.chars().any(char::is_whitespace) {
        return None;
    }
    number.parse::<f64>().ok().and_then(bounded_positive)
}

fn is_admitted_font_weight(value: &str) -> bool {
    matches!(value, "normal" | "bold" | "bolder" | "lighter")
        || value
            .parse::<u16>()
            .ok()
            .is_some_and(|weight| (1..=1000).contains(&weight))
}

fn is_admitted_font_family(value: &str) -> bool {
    !value
        .chars()
        .any(|character| character.is_control() || matches!(character, '{' | '}' | ';'))
}

fn is_admitted_paint(value: &str) -> bool {
    if matches!(value, "none" | "transparent" | "currentcolor") {
        return true;
    }
    if value.starts_with('#') {
        return value
            .strip_prefix('#')
            .is_some_and(|hex| cssparser::color::parse_hash_color(hex.as_bytes()).is_ok());
    }
    if cssparser::color::parse_named_color(value).is_ok() {
        return true;
    }
    let Some(open) = value.find('(') else {
        return false;
    };
    let name = &value[..open];
    matches!(name, "rgb" | "rgba" | "hsl" | "hsla")
        && has_consistent_color_function_separators(value, name, open)
        && ThemeColor::parse(value).is_ok()
}

fn has_consistent_color_function_separators(value: &str, name: &str, open: usize) -> bool {
    let Some(without_close) = value.strip_suffix(')') else {
        return false;
    };
    let Some(body) = without_close.get(open + 1..) else {
        return false;
    };
    if !body.contains(',') {
        return true;
    }
    if body.contains('/') {
        return false;
    }

    let rgb_like = matches!(name, "rgb" | "rgba");
    let mut component_count = 0usize;
    let mut rgb_channels_are_percent = None;
    for component in body.split(',') {
        let component = component.trim();
        if component.is_empty() || component.chars().any(char::is_whitespace) {
            return false;
        }
        if rgb_like && component_count < 3 {
            let is_percent = component.ends_with('%');
            if rgb_channels_are_percent.is_some_and(|expected| expected != is_percent) {
                return false;
            }
            rgb_channels_are_percent.get_or_insert(is_percent);
        }
        component_count = component_count.saturating_add(1);
    }
    matches!(component_count, 3 | 4)
}

struct ParsedQualifiedRule {
    selector: String,
    body: String,
}

struct FallbackStylesheetParser;

fn consume_css_parser_tokens<'i, 't>(input: &mut Parser<'i, 't>) -> Result<(), ParseError<'i, ()>> {
    loop {
        match input.next_including_whitespace_and_comments() {
            Ok(_) => {}
            Err(error) if matches!(&error.kind, BasicParseErrorKind::EndOfInput) => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

impl<'i> QualifiedRuleParser<'i> for FallbackStylesheetParser {
    type Prelude = String;
    type QualifiedRule = Option<ParsedQualifiedRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let start = input.position();
        consume_css_parser_tokens(input)?;
        Ok(input.slice_from(start).trim().to_string())
    }

    fn parse_block<'t>(
        &mut self,
        selector: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let start = input.position();
        consume_css_parser_tokens(input)?;
        Ok(Some(ParsedQualifiedRule {
            selector,
            body: input.slice_from(start).trim().to_string(),
        }))
    }
}

impl<'i> AtRuleParser<'i> for FallbackStylesheetParser {
    type Prelude = String;
    type AtRule = Option<ParsedQualifiedRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        _name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        consume_css_parser_tokens(input)?;
        Ok(String::new())
    }

    fn rule_without_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        Ok(None)
    }

    fn parse_block<'t>(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        consume_css_parser_tokens(input)?;
        Ok(None)
    }
}

fn parse_stylesheet<E>(
    css: &str,
    rules: &mut Vec<Rule>,
    source_order: &mut usize,
    budget: &mut SelectorBudget,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<bool, E> {
    let css = css.trim();
    let css = css.strip_prefix("<![CDATA[").unwrap_or(css);
    let css = css.strip_suffix("]]>").unwrap_or(css);
    let css = strip_css_comments(css, checkpoint)?;
    let mut input = ParserInput::new(&css);
    let mut parser = Parser::new(&mut input);
    let mut rule_parser = FallbackStylesheetParser;
    let mut rule_index = 0usize;
    for parsed in StyleSheetParser::new(&mut parser, &mut rule_parser) {
        checkpoint_loop(rule_index, checkpoint)?;
        rule_index = rule_index.saturating_add(1);
        let Ok(Some(parsed)) = parsed else {
            continue;
        };
        if !budget.charge_qualified_rule(selector_limit)? {
            return Ok(false);
        }
        let selector = trim_with_checkpoints(&parsed.selector, checkpoint)?;
        if !budget.charge_selector_bytes(selector.len(), selector_limit)? {
            return Ok(false);
        }
        let parsed_declarations = parse_declarations_with_limit(
            &parsed.body,
            MAX_SELECTOR_DECLARATIONS_PER_RULE,
            checkpoint,
        )?;
        if let Some((actual, maximum)) = parsed_declarations.limit_exceeded {
            return budget.reject_admission(actual, maximum, selector_limit);
        }
        let declarations = parsed_declarations.declarations;
        if declarations.is_empty() {
            continue;
        }
        if !budget.charge_declarations(declarations.len(), selector_limit)? {
            return Ok(false);
        }
        let Some(branches) = split_selector_branches(selector, budget, checkpoint, selector_limit)?
        else {
            return Ok(false);
        };
        let mut parsed_branches = Vec::new();
        let mut invalid = false;
        for branch in branches {
            match parse_branch(branch, checkpoint)? {
                BranchParse::Admitted(branch) => {
                    if branch.component_count > MAX_SELECTOR_COMPONENTS_PER_BRANCH {
                        return budget.reject_admission(
                            branch.component_count,
                            MAX_SELECTOR_COMPONENTS_PER_BRANCH,
                            selector_limit,
                        );
                    }
                    if !budget.charge_components(branch.component_count, selector_limit)? {
                        return Ok(false);
                    }
                    parsed_branches.push(branch);
                }
                // A branch can be ordinary CSS syntax but outside the
                // deliberately small fallback matcher subset. Keeping
                // admitted siblings is safe because we never widen the
                // unadmitted branch into a class-only match.
                BranchParse::ValidButUnadmitted => {}
                BranchParse::Invalid => invalid = true,
                BranchParse::LimitExceeded(actual) => {
                    return budget.reject_admission(
                        actual,
                        MAX_SELECTOR_COMPONENTS_PER_BRANCH,
                        selector_limit,
                    );
                }
            }
        }
        if !invalid {
            if !budget.charge_rules(parsed_branches.len(), selector_limit)? {
                return Ok(false);
            }
            let rule_order = *source_order;
            let declaration_match_weight =
                declarations.iter().fold(0usize, |total, declaration| {
                    total
                        .checked_add(
                            1usize
                                .saturating_add(declaration.property.len())
                                .saturating_add(declaration.value.len()),
                        )
                        .unwrap_or(usize::MAX)
                });
            let declarations: Arc<[Declaration]> = declarations.into();
            for branch in parsed_branches {
                rules.push(Rule {
                    branch,
                    declarations: declarations.clone(),
                    declaration_match_weight,
                    source_order: rule_order,
                });
            }
            *source_order = source_order.saturating_add(1);
        }
    }
    checkpoint()?;
    Ok(true)
}

/// Removes CSS comments without treating comment-looking text inside a quoted
/// value as syntax. Keeping this pass local lets the fallback retain its
/// operation checkpoints while accepting Mermaid's CDATA-wrapped styles.
fn strip_css_comments<E>(
    css: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut output = String::with_capacity(css.len());
    let mut cursor = 0usize;
    let mut quote = None;
    let mut iteration = 0usize;
    while cursor < css.len() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        let tail = &css[cursor..];
        let Some(character) = tail.chars().next() else {
            break;
        };
        if let Some(current_quote) = quote {
            output.push(character);
            cursor += character.len_utf8();
            if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => {
                quote = Some(character);
                output.push(character);
                cursor += character.len_utf8();
            }
            '/' if tail.as_bytes().get(1) == Some(&b'*') => {
                let content = &tail[2..];
                let Some(end) = find_with_checkpoints(content, "*/", checkpoint)? else {
                    break;
                };
                cursor += 2 + end + 2;
                // Preserve a token boundary so `a/*comment*/b` cannot become
                // a different selector or declaration name.
                output.push(' ');
            }
            _ => {
                output.push(character);
                cursor += character.len_utf8();
            }
        }
    }
    checkpoint()?;
    Ok(output)
}

fn split_selector_branches<'a, E>(
    selector: &'a str,
    budget: &mut SelectorBudget,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
    selector_limit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<Option<Vec<&'a str>>, E> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    for (iteration, (index, ch)) in selector.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '[' if quote.is_none() => bracket_depth = bracket_depth.saturating_add(1),
            ']' if quote.is_none() => bracket_depth = bracket_depth.saturating_sub(1),
            '(' if quote.is_none() => paren_depth = paren_depth.saturating_add(1),
            ')' if quote.is_none() => paren_depth = paren_depth.saturating_sub(1),
            ',' if quote.is_none() && bracket_depth == 0 && paren_depth == 0 => {
                if !budget.charge_branch(selector_limit)? {
                    return Ok(None);
                }
                result.push(selector[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if !budget.charge_branch(selector_limit)? {
        return Ok(None);
    }
    result.push(selector[start..].trim());
    checkpoint()?;
    Ok(Some(result))
}

enum BranchParse {
    Admitted(Branch),
    ValidButUnadmitted,
    Invalid,
    LimitExceeded(usize),
}

enum CompoundParse {
    Admitted(Compound),
    ValidButUnadmitted,
    Invalid,
    LimitExceeded(usize),
}

enum AttributeParse {
    Admitted(AttributeSelector),
    ValidButUnadmitted,
    Invalid,
}

enum SelectorTokenError {
    Invalid,
    LimitExceeded(usize),
}

fn validate_unadmitted_selector(selector: &str) -> bool {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_start = None;
    let mut quote = None;
    let mut escaped = false;
    let mut compound_active = false;
    let mut descendant_pending = false;
    let mut operator_pending = false;
    let mut saw_compound = false;
    let mut skipped_column_pipe = None;

    for (index, ch) in selector.char_indices() {
        if skipped_column_pipe == Some(index) {
            skipped_column_pipe = None;
            continue;
        }
        if escaped {
            escaped = false;
            if bracket_depth == 0 && paren_depth == 0 {
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
            }
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        if bracket_depth > 0 {
            match ch {
                '\'' | '"' => quote = Some(ch),
                '[' => bracket_depth = bracket_depth.saturating_add(1),
                ']' => {
                    if bracket_depth == 1 {
                        let start = bracket_start.take().unwrap_or(index);
                        if !validate_attribute_fragment(&selector[start + 1..index]) {
                            return false;
                        }
                    }
                    bracket_depth -= 1;
                }
                _ => {}
            }
            continue;
        }
        if paren_depth > 0 {
            match ch {
                '\'' | '"' => quote = Some(ch),
                '(' => paren_depth = paren_depth.saturating_add(1),
                ')' => paren_depth -= 1,
                _ => {}
            }
            continue;
        }

        match ch {
            '\'' | '"' | ')' => return false,
            '[' => {
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
                bracket_start = Some(index);
                bracket_depth = 1;
            }
            '(' => {
                if !compound_active || operator_pending {
                    return false;
                }
                paren_depth = 1;
            }
            character if character.is_whitespace() => {
                if compound_active {
                    descendant_pending = true;
                }
            }
            '+' | '~' | '>' => {
                if !compound_active || operator_pending {
                    return false;
                }
                operator_pending = true;
                compound_active = false;
                descendant_pending = false;
            }
            '|' => {
                let next_index = index + ch.len_utf8();
                let next = selector[next_index..].chars().next();
                if next == Some('|') {
                    if !compound_active || operator_pending {
                        return false;
                    }
                    operator_pending = true;
                    compound_active = false;
                    descendant_pending = false;
                    skipped_column_pipe = Some(next_index);
                } else {
                    let previous = selector[..index].chars().next_back();
                    if operator_pending
                        || matches!(next, Some('+' | '~' | '>' | '|' | ','))
                        || previous.is_some_and(char::is_whitespace)
                        || next.is_none_or(char::is_whitespace)
                    {
                        return false;
                    }
                    // A single pipe is a namespace separator. CSS does not allow
                    // whitespace around it; only the `||` column combinator may be
                    // separated from its neighboring compounds.
                    descendant_pending = false;
                }
            }
            '#' | '.' => {
                let next = selector[index + ch.len_utf8()..].chars().next();
                if next.is_none_or(|character| !is_selector_ident_start(character)) {
                    return false;
                }
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
            }
            '*' => {
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
            }
            ':' => {
                let mut next_index = index + ch.len_utf8();
                let Some((_, next)) = selector[next_index..].char_indices().next() else {
                    return false;
                };
                if next == ':' {
                    next_index += next.len_utf8();
                }
                let Some((_, name_start)) = selector[next_index..].char_indices().next() else {
                    return false;
                };
                if !is_selector_ident_start(name_start) {
                    return false;
                }
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
            }
            _ if !is_selector_ident_start(ch) => return false,
            _ => {
                if descendant_pending {
                    descendant_pending = false;
                }
                operator_pending = false;
                compound_active = true;
                saw_compound = true;
            }
        }
    }

    !escaped
        && quote.is_none()
        && bracket_depth == 0
        && paren_depth == 0
        && saw_compound
        && !operator_pending
}

fn is_selector_ident_start(character: char) -> bool {
    character == '_'
        || character == '-'
        || character == '\\'
        || character.is_ascii_alphanumeric()
        || !character.is_ascii()
}

fn validate_attribute_fragment(fragment: &str) -> bool {
    let fragment = fragment.trim();
    let mut name_end = 0usize;
    for (index, character) in fragment.char_indices() {
        if is_selector_ident_start(character) {
            name_end = index + character.len_utf8();
        } else {
            break;
        }
    }
    if name_end == 0 {
        return false;
    }
    let rest = fragment[name_end..].trim_start();
    if rest.is_empty() {
        return true;
    }
    let operator = ["~=", "|=", "^=", "$=", "*=", "!=", "="]
        .into_iter()
        .find(|operator| rest.starts_with(operator));
    let Some(operator) = operator else {
        return false;
    };
    let value = rest[operator.len()..].trim();
    if value.is_empty() {
        return false;
    }
    if value.starts_with('\'') || value.starts_with('"') {
        let quote = value.chars().next().unwrap_or_default();
        value.len() >= 2 && value.ends_with(quote)
    } else {
        value.chars().all(|character| {
            !character.is_whitespace()
                && !matches!(
                    character,
                    '[' | ']' | '(' | ')' | ',' | '>' | '+' | '~' | '|'
                )
        })
    }
}

fn parse_branch<E>(
    selector: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<BranchParse, E> {
    if selector.is_empty() {
        return Ok(BranchParse::Invalid);
    }
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    let mut token = String::new();
    let mut pending = None;
    let mut unsupported = false;
    let mut component_floor = 0usize;
    let mut quote = None;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    for (iteration, ch) in selector.chars().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if let Some(current_quote) = quote {
            token.push(ch);
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                token.push(ch);
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                token.push(ch);
            }
            ']' => {
                if bracket_depth == 0 {
                    return Ok(BranchParse::Invalid);
                }
                bracket_depth -= 1;
                token.push(ch);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                token.push(ch);
            }
            ')' => {
                if paren_depth == 0 {
                    return Ok(BranchParse::Invalid);
                }
                paren_depth -= 1;
                token.push(ch);
            }
            ch if bracket_depth > 0 || paren_depth > 0 => token.push(ch),
            ch if ch.is_whitespace() => {
                match flush_selector_token(
                    &mut token,
                    &mut compounds,
                    &mut combinators,
                    &mut pending,
                    &mut component_floor,
                ) {
                    Ok(()) => {}
                    Err(SelectorTokenError::Invalid) => return Ok(BranchParse::Invalid),
                    Err(SelectorTokenError::LimitExceeded(actual)) => {
                        return Ok(BranchParse::LimitExceeded(actual));
                    }
                }
                if !compounds.is_empty() {
                    pending.get_or_insert(Combinator::Descendant);
                }
            }
            '>' => {
                match flush_selector_token(
                    &mut token,
                    &mut compounds,
                    &mut combinators,
                    &mut pending,
                    &mut component_floor,
                ) {
                    Ok(()) => {}
                    Err(SelectorTokenError::Invalid) => return Ok(BranchParse::Invalid),
                    Err(SelectorTokenError::LimitExceeded(actual)) => {
                        return Ok(BranchParse::LimitExceeded(actual));
                    }
                }
                if compounds.is_empty() {
                    return Ok(BranchParse::Invalid);
                }
                if pending == Some(Combinator::Child) {
                    return Ok(BranchParse::Invalid);
                }
                // Whitespace immediately before `>` is part of the same explicit
                // child combinator, so replace the provisional descendant marker.
                pending = Some(Combinator::Child);
            }
            '+' | '~' | '|' | ':' => {
                unsupported = true;
                token.push(ch);
            }
            _ => token.push(ch),
        }
    }
    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Ok(BranchParse::Invalid);
    }
    match flush_selector_token(
        &mut token,
        &mut compounds,
        &mut combinators,
        &mut pending,
        &mut component_floor,
    ) {
        Ok(()) => {}
        Err(SelectorTokenError::Invalid) => return Ok(BranchParse::Invalid),
        Err(SelectorTokenError::LimitExceeded(actual)) => {
            return Ok(BranchParse::LimitExceeded(actual));
        }
    }
    if compounds.is_empty() {
        return Ok(BranchParse::Invalid);
    }
    if unsupported {
        return Ok(if validate_unadmitted_selector(selector) {
            BranchParse::ValidButUnadmitted
        } else {
            BranchParse::Invalid
        });
    }
    if pending.is_some() || combinators.len() + 1 != compounds.len() {
        return Ok(BranchParse::Invalid);
    }
    let mut component_count = combinators.len();
    if component_count > MAX_SELECTOR_COMPONENTS_PER_BRANCH {
        return Ok(BranchParse::LimitExceeded(component_count));
    }
    let mut specificity = Specificity::default();
    let mut parsed = Vec::new();
    for compound in compounds {
        match parse_compound(&compound, &mut component_count, checkpoint)? {
            CompoundParse::Admitted(compound) => {
                specificity.0 = specificity
                    .0
                    .saturating_add(u32::from(compound.id.is_some()));
                specificity.1 = specificity.1.saturating_add(
                    u32::try_from(
                        compound
                            .classes
                            .len()
                            .saturating_add(compound.attributes.len()),
                    )
                    .unwrap_or(u32::MAX),
                );
                specificity.2 = specificity
                    .2
                    .saturating_add(u32::from(compound.local_name.is_some()));
                parsed.push(compound);
            }
            CompoundParse::ValidButUnadmitted => {
                return Ok(BranchParse::ValidButUnadmitted);
            }
            CompoundParse::Invalid => return Ok(BranchParse::Invalid),
            CompoundParse::LimitExceeded(actual) => {
                return Ok(BranchParse::LimitExceeded(actual));
            }
        }
    }
    Ok(BranchParse::Admitted(Branch {
        compounds: parsed,
        combinators,
        specificity,
        component_count,
        selector_weight: selector.len().max(1),
    }))
}

fn flush_selector_token(
    token: &mut String,
    compounds: &mut Vec<String>,
    combinators: &mut Vec<Combinator>,
    pending: &mut Option<Combinator>,
    component_floor: &mut usize,
) -> Result<(), SelectorTokenError> {
    let value = token.trim();
    if value.is_empty() {
        return Ok(());
    }
    if pending.is_some() {
        if compounds.is_empty() || combinators.len() + 1 != compounds.len() {
            return Err(SelectorTokenError::Invalid);
        }
    } else if !compounds.is_empty() {
        return Err(SelectorTokenError::Invalid);
    }
    let additional = 1usize.saturating_add(usize::from(pending.is_some()));
    let actual = component_floor
        .checked_add(additional)
        .unwrap_or(usize::MAX);
    if actual > MAX_SELECTOR_COMPONENTS_PER_BRANCH {
        return Err(SelectorTokenError::LimitExceeded(actual));
    }
    if let Some(combinator) = pending.take() {
        combinators.push(combinator);
    }
    *component_floor = actual;
    compounds.push(value.to_string());
    token.clear();
    Ok(())
}

fn parse_compound<E>(
    token: &str,
    component_count: &mut usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<CompoundParse, E> {
    let mut rest = token.trim();
    let mut local_name = None;
    let mut id = None;
    let mut classes = Vec::new();
    let mut attributes = Vec::new();
    if rest.starts_with('*') {
        if let Err(actual) = charge_branch_component(component_count) {
            return Ok(CompoundParse::LimitExceeded(actual));
        }
        rest = &rest[1..];
    } else if !rest.starts_with(['#', '.', '[']) {
        let end = identifier_prefix_len(rest);
        if end == 0 {
            return Ok(CompoundParse::Invalid);
        }
        if let Err(actual) = charge_branch_component(component_count) {
            return Ok(CompoundParse::LimitExceeded(actual));
        }
        local_name = Some(rest[..end].to_ascii_lowercase());
        rest = &rest[end..];
    }
    while !rest.is_empty() {
        let marker = rest.chars().next().expect("rest is non-empty");
        match marker {
            '#' | '.' => {
                let value = &rest[marker.len_utf8()..];
                let end = identifier_prefix_len(value);
                if end == 0 {
                    return Ok(CompoundParse::Invalid);
                }
                let value = &value[..end];
                if marker == '#' {
                    if id.is_some() {
                        return Ok(CompoundParse::Invalid);
                    }
                    if let Err(actual) = charge_branch_component(component_count) {
                        return Ok(CompoundParse::LimitExceeded(actual));
                    }
                    id = Some(value.to_string());
                } else {
                    if let Err(actual) = charge_branch_component(component_count) {
                        return Ok(CompoundParse::LimitExceeded(actual));
                    }
                    classes.push(value.to_string());
                }
                rest = &rest[marker.len_utf8() + end..];
            }
            '[' => {
                let Some(end) = find_attribute_selector_end(rest) else {
                    return Ok(CompoundParse::Invalid);
                };
                match parse_attribute_selector(&rest[1..end], checkpoint)? {
                    AttributeParse::Admitted(attribute) => {
                        if let Err(actual) = charge_branch_component(component_count) {
                            return Ok(CompoundParse::LimitExceeded(actual));
                        }
                        attributes.push(attribute);
                    }
                    AttributeParse::ValidButUnadmitted => {
                        return Ok(CompoundParse::ValidButUnadmitted);
                    }
                    AttributeParse::Invalid => return Ok(CompoundParse::Invalid),
                }
                rest = &rest[end + 1..];
            }
            _ => return Ok(CompoundParse::Invalid),
        }
    }
    checkpoint()?;
    Ok(CompoundParse::Admitted(Compound {
        // No `@namespace` declarations are admitted by this fallback subset,
        // so an unprefixed type selector remains namespace-neutral.
        namespace: None,
        local_name,
        id,
        classes,
        attributes,
    }))
}

fn charge_branch_component(component_count: &mut usize) -> Result<(), usize> {
    let actual = component_count.checked_add(1).unwrap_or(usize::MAX);
    *component_count = actual;
    if actual > MAX_SELECTOR_COMPONENTS_PER_BRANCH {
        Err(actual)
    } else {
        Ok(())
    }
}

fn identifier_prefix_len(value: &str) -> usize {
    let mut end = 0usize;
    for (offset, character) in value.char_indices() {
        if !is_css_identifier_character(character) {
            break;
        }
        end = offset + character.len_utf8();
    }
    end
}

fn find_attribute_selector_end(value: &str) -> Option<usize> {
    let mut quote = None;
    for (offset, character) in value.char_indices().skip(1) {
        if let Some(current_quote) = quote {
            if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            ']' => return Some(offset),
            '[' => return None,
            _ => {}
        }
    }
    None
}

fn parse_attribute_selector<E>(
    source: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<AttributeParse, E> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let parsed = parser.parse_entirely(|input| {
        let name = input.expect_ident_cloned()?.to_ascii_lowercase();
        if input.is_exhausted() {
            return Ok(AttributeParse::Admitted(AttributeSelector {
                name,
                matcher: AttributeMatcher::Exists,
            }));
        }

        let operator = input.next()?.clone();
        let value = input.expect_ident_or_string()?.to_string();
        let modifier = if input.is_exhausted() {
            None
        } else {
            let modifier = input.expect_ident_cloned()?;
            if !matches!(modifier.as_ref(), "i" | "I" | "s" | "S") {
                return Err(input.new_custom_error::<(), ()>(()));
            }
            Some(modifier)
        };

        let parsed = match operator {
            Token::Delim('=') if modifier.is_none() => {
                AttributeParse::Admitted(AttributeSelector {
                    name,
                    matcher: AttributeMatcher::Exact(value),
                })
            }
            Token::IncludeMatch if modifier.is_none() => {
                AttributeParse::Admitted(AttributeSelector {
                    name,
                    matcher: AttributeMatcher::ContainsToken(value),
                })
            }
            Token::Delim('=')
            | Token::IncludeMatch
            | Token::DashMatch
            | Token::PrefixMatch
            | Token::SuffixMatch
            | Token::SubstringMatch => AttributeParse::ValidButUnadmitted,
            _ => return Err(input.new_custom_error::<(), ()>(())),
        };
        Ok(parsed)
    });
    checkpoint()?;
    Ok(parsed.unwrap_or(AttributeParse::Invalid))
}

fn is_css_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '-'
        || character == '_'
        || !character.is_ascii()
}

fn matches_branch<E>(
    branch: &Branch,
    path: &[SourceElement],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    let rightmost = branch.compounds.len() - 1;
    if !matches_compound(
        &branch.compounds[rightmost],
        path.last().expect("path is non-empty"),
    ) {
        return Ok(false);
    }
    let mut states = vec![false; path.len()];
    states[path.len() - 1] = true;
    for selector_index in (0..rightmost).rev() {
        let mut next = vec![false; path.len()];
        match branch.combinators[selector_index] {
            Combinator::Child => {
                for path_index in 1..path.len() {
                    checkpoint_loop(path_index, checkpoint)?;
                    next[path_index - 1] = states[path_index]
                        && matches_compound(
                            &branch.compounds[selector_index],
                            &path[path_index - 1],
                        );
                }
            }
            Combinator::Descendant => {
                let mut has_descendant_match = false;
                for path_index in (0..path.len()).rev() {
                    checkpoint_loop(path_index, checkpoint)?;
                    next[path_index] = has_descendant_match
                        && matches_compound(&branch.compounds[selector_index], &path[path_index]);
                    has_descendant_match |= states[path_index];
                }
            }
        }
        states = next;
    }
    checkpoint()?;
    Ok(states.into_iter().any(|matched| matched))
}

fn matches_compound(compound: &Compound, element: &SourceElement) -> bool {
    compound
        .namespace
        .is_none_or(|namespace| namespace == element.namespace)
        && compound
            .local_name
            .as_deref()
            .is_none_or(|name| name.eq_ignore_ascii_case(&element.local_name))
        && compound
            .id
            .as_deref()
            .is_none_or(|id| element.id.as_deref() == Some(id))
        && compound
            .classes
            .iter()
            .all(|class| element.classes.contains(class))
        && compound
            .attributes
            .iter()
            .all(|selector| attribute_selector_matches(selector, element))
}

fn attribute_selector_matches(selector: &AttributeSelector, element: &SourceElement) -> bool {
    let Some(value) = element.attributes.get(&selector.name) else {
        return false;
    };
    match &selector.matcher {
        AttributeMatcher::Exists => true,
        AttributeMatcher::Exact(expected) => value.as_deref() == Some(expected),
        AttributeMatcher::ContainsToken(expected) => value
            .as_deref()
            .is_some_and(|value| value.split_whitespace().any(|token| token == expected)),
    }
}

struct ParsedDeclarations {
    declarations: Vec<Declaration>,
    limit_exceeded: Option<(usize, usize)>,
}

fn parse_declarations<E>(
    style: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Declaration>, E> {
    Ok(parse_declarations_with_limit(style, usize::MAX, checkpoint)?.declarations)
}

fn parse_declarations_with_limit<E>(
    style: &str,
    maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<ParsedDeclarations, E> {
    let mut declarations = Vec::new();
    let mut scanner = StyleDeclarationScanner::new(style);
    let mut order = 0usize;
    while let Some(raw) = scanner.next_with_checkpoints(checkpoint)? {
        checkpoint_loop(order, checkpoint)?;
        let Some(colon) = find_css_declaration_colon(raw, checkpoint)? else {
            order = order.saturating_add(1);
            continue;
        };
        let property = raw[..colon].trim();
        let (value, important) = split_trailing_important(&raw[colon + 1..]);
        if property.is_empty() || value.is_empty() {
            order = order.saturating_add(1);
            continue;
        }
        if declarations.len() >= maximum {
            return Ok(ParsedDeclarations {
                declarations,
                limit_exceeded: Some((maximum.saturating_add(1), maximum)),
            });
        }
        declarations.push(Declaration {
            property: property.to_ascii_lowercase(),
            value: value.to_string(),
            important,
            order,
        });
        order = order.saturating_add(1);
    }
    checkpoint()?;
    Ok(ParsedDeclarations {
        declarations,
        limit_exceeded: None,
    })
}

fn find_css_declaration_colon<E>(
    declaration: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    let mut quote = None;
    let mut paren_depth = 0usize;
    for (iteration, (offset, character)) in declaration.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if let Some(current_quote) = quote {
            if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ':' if paren_depth == 0 => return Ok(Some(offset)),
            _ => {}
        }
    }
    checkpoint()?;
    Ok(None)
}

fn split_trailing_important(value: &str) -> (&str, bool) {
    let value = value.trim();
    let Some(marker) = value.rfind('!') else {
        return (value, false);
    };
    if value[marker + '!'.len_utf8()..]
        .trim()
        .eq_ignore_ascii_case("important")
    {
        (value[..marker].trim(), true)
    } else {
        (value, false)
    }
}

fn resolve_font_size(
    value: Option<&str>,
    parent: f64,
    root: f64,
    svg_unitless: bool,
) -> Option<f64> {
    let value = value?.trim().to_ascii_lowercase();
    if matches!(value.as_str(), "inherit" | "unset") {
        return finite_positive(parent);
    }
    if value == "initial" {
        return finite_positive(DEFAULT_FONT_SIZE);
    }
    if let Some(number) = value.strip_suffix("px").and_then(parse_number) {
        return bounded_positive(number);
    }
    if let Some(number) = value.strip_suffix("rem").and_then(parse_number) {
        return bounded_positive(root * number);
    }
    if let Some(number) = value.strip_suffix("em").and_then(parse_number) {
        return bounded_positive(parent * number);
    }
    if let Some(number) = value.strip_suffix('%').and_then(parse_number) {
        return bounded_positive(parent * number / 100.0);
    }
    if svg_unitless {
        return value.parse::<f64>().ok().and_then(bounded_positive);
    }
    match value.as_str() {
        "xx-small" => finite_positive(9.0),
        "x-small" => finite_positive(10.0),
        "small" => finite_positive(13.0),
        "medium" => finite_positive(16.0),
        "large" => finite_positive(18.0),
        "x-large" => finite_positive(24.0),
        "xx-large" => finite_positive(32.0),
        "smaller" => finite_positive(parent * 0.833_333_333_3),
        "larger" => finite_positive(parent * 1.2),
        _ => None,
    }
}

fn resolve_inherited_text_value(value: Option<&str>, parent: &str, initial: &str) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return parent.to_string();
    };
    match value.to_ascii_lowercase().as_str() {
        "inherit" | "unset" => parent.to_string(),
        "initial" => initial.to_string(),
        _ => value.to_string(),
    }
}

fn resolve_optional_text_value(value: Option<&str>, parent: Option<&str>) -> Option<String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return parent.map(str::to_owned);
    };
    match value.to_ascii_lowercase().as_str() {
        "inherit" | "unset" => parent.map(str::to_owned),
        "initial" => None,
        _ => Some(value.to_string()),
    }
}

fn resolve_optional_color(value: Option<&str>, parent: Option<&str>) -> Option<String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return parent.map(str::to_owned);
    };
    match value.to_ascii_lowercase().as_str() {
        "inherit" | "unset" => parent.map(str::to_owned),
        "initial" => Some(DEFAULT_COLOR.to_string()),
        "currentcolor" => Some(parent.unwrap_or(DEFAULT_COLOR).to_string()),
        _ => Some(value.to_string()),
    }
}

fn effective_html_text_paint(style: &ResolvedStyle) -> &str {
    style.color.as_deref().unwrap_or(&style.fill)
}

fn resolve_line_height(
    value: Option<&str>,
    font_size: f64,
    inherited: &LineHeight,
    root: f64,
) -> LineHeight {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return inherited.clone();
    };
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "inherit" | "unset") {
        return inherited.clone();
    }
    if matches!(lower.as_str(), "normal" | "initial") {
        return LineHeight::Normal;
    }
    if let Ok(number) = lower.parse::<f64>() {
        return bounded_positive(number)
            .map(LineHeight::Multiplier)
            .unwrap_or_else(|| inherited.clone());
    }
    if let Some(number) = lower.strip_suffix("px").and_then(parse_number) {
        return bounded_positive(number)
            .map(LineHeight::AbsolutePx)
            .unwrap_or_else(|| inherited.clone());
    }
    if let Some(number) = lower.strip_suffix("rem").and_then(parse_number) {
        return bounded_positive(root * number)
            .map(LineHeight::AbsolutePx)
            .unwrap_or_else(|| inherited.clone());
    }
    if let Some(number) = lower.strip_suffix("em").and_then(parse_number) {
        return bounded_positive(font_size * number)
            .map(LineHeight::AbsolutePx)
            .unwrap_or_else(|| inherited.clone());
    }
    if let Some(number) = lower.strip_suffix('%').and_then(parse_number) {
        return bounded_positive(font_size * number / 100.0)
            .map(LineHeight::AbsolutePx)
            .unwrap_or_else(|| inherited.clone());
    }
    inherited.clone()
}

fn resolve_background(value: Option<&str>, parent: Option<&str>) -> Option<String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;
    match value.to_ascii_lowercase().as_str() {
        "initial" | "unset" => None,
        "inherit" => parent.map(str::to_owned),
        _ => Some(value.to_string()),
    }
}

fn parse_number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn finite_positive(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value > 0.0)
}

fn bounded_positive(value: f64) -> Option<f64> {
    finite_positive(value).filter(|value| *value <= MAX_CSS_NUMERIC_VALUE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    #[test]
    fn selector_match_work_budget_accepts_the_boundary_and_rejects_the_next_unit() {
        let mut budget = SelectorBudget::default();
        let mut ignore_limit = |_, _| Ok::<(), Infallible>(());
        assert!(
            budget
                .charge_match_work(MAX_SELECTOR_MATCH_WORK, &mut ignore_limit)
                .unwrap()
        );

        let mut observed = None;
        assert!(
            !budget
                .charge_match_work(1, &mut |actual, maximum| {
                    observed = Some((actual, maximum));
                    Ok::<(), Infallible>(())
                })
                .unwrap()
        );
        assert_eq!(
            observed,
            Some((MAX_SELECTOR_MATCH_WORK + 1, MAX_SELECTOR_MATCH_WORK))
        );
    }

    #[test]
    fn selector_ancestry_budget_accepts_the_boundary_and_rejects_the_next_level() {
        let mut budget = SelectorBudget::default();
        let mut ignore_limit = |_, _| Ok::<(), Infallible>(());
        assert!(
            budget
                .check_ancestry_depth(MAX_SELECTOR_ANCESTRY_DEPTH, &mut ignore_limit)
                .unwrap()
        );

        let mut observed = None;
        assert!(
            !budget
                .check_ancestry_depth(MAX_SELECTOR_ANCESTRY_DEPTH + 1, &mut |actual, maximum| {
                    observed = Some((actual, maximum));
                    Ok::<(), Infallible>(())
                },)
                .unwrap()
        );
        assert_eq!(
            observed,
            Some((MAX_SELECTOR_ANCESTRY_DEPTH + 1, MAX_SELECTOR_ANCESTRY_DEPTH,))
        );
    }

    #[test]
    fn selector_budget_reports_checked_add_overflow_without_wrapping() {
        let mut used = usize::MAX - 1;
        let mut exhausted = false;
        let mut observed = None;
        assert!(
            !checked_charge(
                &mut used,
                &mut exhausted,
                2,
                usize::MAX - 1,
                &mut |actual, maximum| {
                    observed = Some((actual, maximum));
                    Ok::<(), Infallible>(())
                },
            )
            .unwrap()
        );
        assert_eq!(observed, Some((usize::MAX, usize::MAX - 1)));
        assert_eq!(used, usize::MAX - 1);
        assert!(exhausted);
    }

    #[test]
    fn selector_component_floor_also_bounds_unadmitted_branches() {
        let selector = std::iter::repeat_n("a:", 33).collect::<Vec<_>>().join(" ");
        let mut checkpoint = || Ok::<(), Infallible>(());
        let parsed = parse_branch(&selector, &mut checkpoint).unwrap();
        assert!(matches!(parsed, BranchParse::LimitExceeded(65)));
    }

    #[test]
    fn unadmitted_selector_validation_rejects_trailing_operators() {
        for selector in [
            "span:",
            "span::",
            ".a +",
            ".a + > .b",
            ".a ~",
            ".a |",
            ".a ||",
            "a | b",
            "a| b",
            "a |b",
            "span:hover [data-tags^=]",
        ] {
            assert!(
                !validate_unadmitted_selector(selector),
                "malformed selector should fail closed: {selector}"
            );
        }
    }

    #[test]
    fn unadmitted_selector_validation_accepts_supported_css_shapes_without_admitting_them() {
        for selector in [
            ":root",
            "span:hover",
            "span::before",
            ":not(&)",
            ".a + .b",
            ".a ~ .b",
            "svg|a",
            "*|span",
            "a||b",
            "[data-tags^=choice]",
        ] {
            assert!(
                validate_unadmitted_selector(selector),
                "valid unsupported selector should remain ignorable: {selector}"
            );
        }
    }

    #[test]
    fn stylesheet_parser_strips_a_cdata_prefix_even_without_a_suffix() {
        let mut rules = Vec::new();
        let mut source_order = 0usize;
        let mut budget = SelectorBudget::default();
        let mut checkpoint = || Ok::<(), Infallible>(());
        let mut selector_limit = |_, _| Ok::<(), Infallible>(());

        assert!(
            parse_stylesheet(
                "<![CDATA[.nodeLabel{font-size:13px}",
                &mut rules,
                &mut source_order,
                &mut budget,
                &mut checkpoint,
                &mut selector_limit,
            )
            .unwrap()
        );
        assert_eq!(rules.len(), 1);
    }
}
