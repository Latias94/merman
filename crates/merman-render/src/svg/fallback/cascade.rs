use super::css::split_style_declarations;
use crate::svg::pipeline::{
    SvgTagScanner, checkpoint_loop, end_tag_name, find_tag_end_with_checkpoints,
    find_with_checkpoints, start_tag_name, trim_with_checkpoints,
};
use crate::text::TextStyle;
use cssparser::{
    AtRuleParser, CowRcStr, ParseError, Parser, ParserInput, ParserState, QualifiedRuleParser,
    StyleSheetParser, Token,
};
use std::collections::HashMap;
use std::sync::Arc;

const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_FONT_FAMILY: &str = "trebuchet ms,verdana,arial,sans-serif";
const DEFAULT_FILL: &str = "#333";
const DEFAULT_COLOR: &str = "#000";
// Fallback text is a bounded adapter, so pathological CSS magnitudes must not enter output or
// overflow relative-unit and line-height calculations.
const MAX_CSS_NUMERIC_VALUE: f64 = 1_000_000.0;
const MAX_UNIVERSAL_POSTINGS: usize = 4096;

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
    pub(super) classes: Vec<String>,
    attributes: Vec<SourceAttribute>,
    pub(super) inline: Vec<Declaration>,
    pub(super) presentation: Vec<Declaration>,
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
}

#[derive(Clone, Debug)]
struct Rule {
    branch: Branch,
    declarations: Arc<[Declaration]>,
    source_order: usize,
}

#[derive(Debug)]
pub(super) struct CascadeIndex {
    rules: Vec<Rule>,
    id_postings: HashMap<String, Vec<usize>>,
    class_postings: HashMap<String, Vec<usize>>,
    type_postings: HashMap<(Namespace, String), Vec<usize>>,
    attribute_postings: HashMap<String, Vec<usize>>,
    universal_postings: Vec<usize>,
}

impl CascadeIndex {
    fn with_postings(rules: Vec<Rule>) -> Self {
        let mut index = Self {
            rules,
            id_postings: HashMap::new(),
            class_postings: HashMap::new(),
            type_postings: HashMap::new(),
            attribute_postings: HashMap::new(),
            universal_postings: Vec::new(),
        };
        for (rule_index, rule) in index.rules.iter().enumerate() {
            let target = rule
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
            } else if let (Some(namespace), Some(local_name)) =
                (target.namespace, &target.local_name)
            {
                index
                    .type_postings
                    .entry((namespace, local_name.clone()))
                    .or_default()
                    .push(rule_index);
            } else {
                // A universal rightmost selector is the only branch that cannot be narrowed by
                // source-element postings. Keep this bucket explicitly capped so a compact
                // hostile stylesheet cannot turn every fallback label into an unbounded scan.
                if index.universal_postings.len() < MAX_UNIVERSAL_POSTINGS {
                    index.universal_postings.push(rule_index);
                }
            }
        }
        index
    }

    fn candidate_rule_indices(&self, element: &SourceElement) -> Vec<usize> {
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
        for class in &element.classes {
            if let Some(postings) = self.class_postings.get(class) {
                candidates.extend(postings);
            }
        }
        if let Some(postings) = self
            .type_postings
            .get(&(element.namespace, element.local_name.to_ascii_lowercase()))
        {
            candidates.extend(postings);
        }
        for attribute in &element.attributes {
            if let Some(postings) = self.attribute_postings.get(&attribute.name) {
                candidates.extend(postings);
            }
        }
        candidates
    }

    pub(super) fn new<E>(
        svg: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut rules = Vec::new();
        let mut source_order = 0usize;
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
            parse_stylesheet(css, &mut rules, &mut source_order, checkpoint)?;
        }
        checkpoint()?;
        Ok(Self::with_postings(rules))
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
        checkpoint()?;
        Ok(SourceElement {
            namespace,
            local_name,
            id,
            classes,
            attributes,
            inline,
            presentation,
        })
    }

    pub(super) fn resolve_path<E>(
        &self,
        path: &[SourceElement],
        inherited: Option<&ResolvedStyle>,
        root_font_size: f64,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ResolvedStyle, E> {
        let element = path.last().expect("source path is non-empty");
        let parent = inherited.cloned().unwrap_or_else(default_style);
        let mut specified: HashMap<String, Specified> = HashMap::new();
        for (candidate_index, rule_index) in
            self.candidate_rule_indices(element).into_iter().enumerate()
        {
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
        let background_color = resolve_background(
            specified
                .get("background-color")
                .map(|value| value.value.as_str()),
            parent.background_color.as_deref(),
        );
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
        &self,
        svg_ancestors: &[SourceElement],
        foreign_object_tag: &str,
        html: &str,
        checkpoint: &mut impl FnMut() -> Result<(), E>,
    ) -> Result<ResolvedFallbackTypography, E> {
        let foreign_object = Self::source_element(foreign_object_tag, Namespace::Svg, checkpoint)?;
        let mut base_path = svg_ancestors.to_vec();
        base_path.push(foreign_object);
        let mut html_stack = Vec::new();
        let mut text_paths = Vec::new();
        let mut label_background_path = None;
        let mut cursor = 0usize;
        let mut iteration = 0usize;
        while let Some(relative) = find_with_checkpoints(&html[cursor..], "<", checkpoint)? {
            checkpoint_loop(iteration, checkpoint)?;
            iteration = iteration.saturating_add(1);
            let start = cursor + relative;
            if html_text_is_visible(&html[cursor..start]) {
                text_paths.push(join_path(&base_path, &html_stack));
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
                let element = Self::source_element(tag, Namespace::Xhtml, checkpoint)?;
                html_stack.push(element);
                if html_stack
                    .last()
                    .is_some_and(|element| element.classes.iter().any(|class| class == "labelBkg"))
                    && label_background_path.is_none()
                {
                    label_background_path = Some(join_path(&base_path, &html_stack));
                }
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
            text_paths.push(join_path(&base_path, &html_stack));
        }
        if text_paths.is_empty() {
            text_paths.push(base_path.clone());
        }

        let styles = text_paths
            .iter()
            .map(|path| self.resolve_full_path(path, checkpoint))
            .collect::<Result<Vec<_>, _>>()?;
        let style = if styles
            .windows(2)
            .all(|pair| same_effective_typography(&pair[0], &pair[1]))
        {
            styles.into_iter().next().expect("at least one text style")
        } else {
            let common_path = deepest_common_path(&text_paths);
            self.resolve_full_path(&common_path, checkpoint)?
        };
        let label_background = label_background_path
            .map(|path| self.resolve_full_path(&path, checkpoint))
            .transpose()?
            .and_then(|style| {
                if style.background_color_specified {
                    style.background_color
                } else {
                    Some("rgba(232, 232, 232, 0.5)".to_string())
                }
            });
        let fill = effective_html_text_paint(&style).to_string();
        checkpoint()?;
        Ok(ResolvedFallbackTypography {
            font_size: style.font_size,
            font_family: style.font_family,
            font_weight: style.font_weight,
            font_style: style.font_style,
            line_height: style.line_height.pixels(style.font_size),
            fill,
            label_background,
        })
    }

    fn resolve_full_path<E>(
        &self,
        path: &[SourceElement],
        checkpoint: &mut impl FnMut() -> Result<(), E>,
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

fn deepest_common_path(paths: &[Vec<SourceElement>]) -> Vec<SourceElement> {
    let Some(first) = paths.first() else {
        return Vec::new();
    };
    let mut common = first.len();
    for path in &paths[1..] {
        common = common.min(path.len());
        while common > 0 && first[..common] != path[..common] {
            common -= 1;
        }
    }
    first[..common].to_vec()
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
    if !is_supported_property(&declaration.property)
        || !is_admitted_value(&declaration.property, &declaration.value, presentation)
    {
        return;
    }
    if specified
        .get(&declaration.property)
        .is_none_or(|current| priority > current.priority)
    {
        specified.insert(
            declaration.property.clone(),
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
    if !matches!(name, "rgb" | "rgba" | "hsl" | "hsla") || !value.ends_with(')') {
        return false;
    }
    let body = &value[open + 1..value.len() - 1];
    !body.is_empty()
        && !body.chars().any(|character| {
            character.is_control() || matches!(character, '{' | '}' | ';' | '"' | '\'')
        })
}

struct ParsedQualifiedRule {
    selector: String,
    body: String,
}

struct FallbackStylesheetParser;

fn consume_css_parser_tokens<'i, 't>(input: &mut Parser<'i, 't>) -> Result<(), ParseError<'i, ()>> {
    while !input.is_exhausted() {
        let token = input.next_including_whitespace_and_comments()?.clone();
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input.parse_nested_block(consume_css_parser_tokens)?;
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
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let css = css.trim();
    let css = css
        .strip_prefix("<![CDATA[")
        .unwrap_or(css)
        .strip_suffix("]]>")
        .unwrap_or(css);
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
        let selector = trim_with_checkpoints(&parsed.selector, checkpoint)?;
        let declarations = parse_declarations(&parsed.body, checkpoint)?;
        if declarations.is_empty() {
            continue;
        }
        let branches = split_selector_branches(selector, checkpoint)?;
        let mut parsed_branches = Vec::new();
        let mut invalid = false;
        for branch in branches {
            match parse_branch(branch, checkpoint)? {
                BranchParse::Admitted(branch) => parsed_branches.push(branch),
                // A branch can be ordinary CSS syntax but outside the
                // deliberately small fallback matcher subset. Keeping
                // admitted siblings is safe because we never widen the
                // unadmitted branch into a class-only match.
                BranchParse::ValidButUnadmitted => {}
                BranchParse::Invalid => invalid = true,
            }
        }
        if !invalid {
            let rule_order = *source_order;
            let declarations: Arc<[Declaration]> = declarations.into();
            for branch in parsed_branches {
                rules.push(Rule {
                    branch,
                    declarations: declarations.clone(),
                    source_order: rule_order,
                });
            }
            *source_order = source_order.saturating_add(1);
        }
    }
    checkpoint()?;
    Ok(())
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
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<&'a str>, E> {
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
                result.push(selector[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    result.push(selector[start..].trim());
    checkpoint()?;
    Ok(result)
}

enum BranchParse {
    Admitted(Branch),
    ValidButUnadmitted,
    Invalid,
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
                if flush_selector_token(&mut token, &mut compounds, &mut combinators, &mut pending)
                    .is_err()
                {
                    return Ok(BranchParse::Invalid);
                }
                if !compounds.is_empty() {
                    pending.get_or_insert(Combinator::Descendant);
                }
            }
            '>' => {
                if flush_selector_token(&mut token, &mut compounds, &mut combinators, &mut pending)
                    .is_err()
                {
                    return Ok(BranchParse::Invalid);
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
    if flush_selector_token(&mut token, &mut compounds, &mut combinators, &mut pending).is_err() {
        return Ok(BranchParse::Invalid);
    }
    if compounds.is_empty() || pending.is_some() || combinators.len() + 1 != compounds.len() {
        return Ok(BranchParse::Invalid);
    }
    if unsupported {
        return Ok(BranchParse::ValidButUnadmitted);
    }
    let mut specificity = Specificity::default();
    let mut parsed = Vec::new();
    for compound in compounds {
        match parse_compound(&compound, checkpoint)? {
            Some(compound) => {
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
            None => return Ok(BranchParse::Invalid),
        }
    }
    Ok(BranchParse::Admitted(Branch {
        compounds: parsed,
        combinators,
        specificity,
    }))
}

fn flush_selector_token(
    token: &mut String,
    compounds: &mut Vec<String>,
    combinators: &mut Vec<Combinator>,
    pending: &mut Option<Combinator>,
) -> Result<(), ()> {
    let value = token.trim();
    if value.is_empty() {
        return Ok(());
    }
    if let Some(combinator) = pending.take() {
        if compounds.is_empty() || combinators.len() + 1 != compounds.len() {
            return Err(());
        }
        combinators.push(combinator);
    } else if !compounds.is_empty() {
        return Err(());
    }
    compounds.push(value.to_string());
    token.clear();
    Ok(())
}

fn parse_compound<E>(
    token: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<Compound>, E> {
    let mut rest = token.trim();
    let mut local_name = None;
    let mut id = None;
    let mut classes = Vec::new();
    let mut attributes = Vec::new();
    if rest.starts_with('*') {
        rest = &rest[1..];
    } else if !rest.starts_with(['#', '.', '[']) {
        let end = identifier_prefix_len(rest);
        if end == 0 {
            return Ok(None);
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
                    return Ok(None);
                }
                let value = &value[..end];
                if marker == '#' {
                    if id.is_some() {
                        return Ok(None);
                    }
                    id = Some(value.to_string());
                } else {
                    classes.push(value.to_string());
                }
                rest = &rest[marker.len_utf8() + end..];
            }
            '[' => {
                let Some(end) = find_attribute_selector_end(rest) else {
                    return Ok(None);
                };
                let Some(attribute) = parse_attribute_selector(&rest[1..end], checkpoint)? else {
                    return Ok(None);
                };
                attributes.push(attribute);
                rest = &rest[end + 1..];
            }
            _ => return Ok(None),
        }
    }
    checkpoint()?;
    let namespace = match local_name.as_deref() {
        Some("span" | "p" | "div" | "b" | "strong" | "i" | "em") => Some(Namespace::Xhtml),
        Some(_) => Some(Namespace::Svg),
        None => None,
    };
    Ok(Some(Compound {
        namespace,
        local_name,
        id,
        classes,
        attributes,
    }))
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
) -> Result<Option<AttributeSelector>, E> {
    let source = source.trim();
    let name_end = identifier_prefix_len(source);
    if name_end == 0 {
        return Ok(None);
    }
    let name = source[..name_end].to_ascii_lowercase();
    let rest = source[name_end..].trim();
    let matcher = if rest.is_empty() {
        AttributeMatcher::Exists
    } else if let Some(value) = rest.strip_prefix("~=") {
        let Some(value) = parse_attribute_selector_value(value) else {
            return Ok(None);
        };
        AttributeMatcher::ContainsToken(value)
    } else if let Some(value) = rest.strip_prefix('=') {
        let Some(value) = parse_attribute_selector_value(value) else {
            return Ok(None);
        };
        AttributeMatcher::Exact(value)
    } else {
        return Ok(None);
    };
    checkpoint()?;
    Ok(Some(AttributeSelector { name, matcher }))
}

fn parse_attribute_selector_value(value: &str) -> Option<String> {
    let value = value.trim();
    let first = value.chars().next()?;
    if matches!(first, '\'' | '"') {
        let tail = &value[first.len_utf8()..];
        let end = tail.find(first)?;
        if !tail[end + first.len_utf8()..].trim().is_empty() {
            return None;
        }
        return Some(tail[..end].to_string());
    }
    is_identifier(value).then(|| value.to_string())
}

fn is_css_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '-'
        || character == '_'
        || !character.is_ascii()
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(is_css_identifier_character)
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
            .all(|class| element.classes.iter().any(|candidate| candidate == class))
        && compound
            .attributes
            .iter()
            .all(|selector| attribute_selector_matches(selector, element))
}

fn attribute_selector_matches(selector: &AttributeSelector, element: &SourceElement) -> bool {
    let Some(attribute) = element
        .attributes
        .iter()
        .rev()
        .find(|attribute| attribute.name == selector.name)
    else {
        return false;
    };
    match &selector.matcher {
        AttributeMatcher::Exists => true,
        AttributeMatcher::Exact(expected) => attribute.value.as_deref() == Some(expected),
        AttributeMatcher::ContainsToken(expected) => attribute
            .value
            .as_deref()
            .is_some_and(|value| value.split_whitespace().any(|token| token == expected)),
    }
}

fn parse_declarations<E>(
    style: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Declaration>, E> {
    let mut declarations = Vec::new();
    for (order, raw) in split_style_declarations(style, checkpoint)?
        .into_iter()
        .enumerate()
    {
        checkpoint_loop(order, checkpoint)?;
        let Some(colon) = find_css_declaration_colon(raw, checkpoint)? else {
            continue;
        };
        let property = raw[..colon].trim().to_ascii_lowercase();
        let (value, important) = split_trailing_important(&raw[colon + 1..]);
        let value = value.to_string();
        if property.is_empty() || value.is_empty() {
            continue;
        }
        declarations.push(Declaration {
            property,
            value,
            important,
            order,
        });
    }
    checkpoint()?;
    Ok(declarations)
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
