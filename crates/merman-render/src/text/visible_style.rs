use super::trim_html_collapsible_ascii_whitespace;

#[cfg(test)]
thread_local! {
    static FACT_COLLECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Sufficient ownership facts for paint, typography, and applicability consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleTextStyleFacts {
    parse_valid: bool,
    content_runs: usize,
    visible_runs: usize,
    font_visibility_verified: bool,
    inherited_color_runs: usize,
    unverified_color_runs: usize,
    unverified_portable_color_runs: usize,
    inherited_font_family_runs: usize,
    source_owned_font_family_runs: usize,
    inherited_font_size_runs: usize,
    inherited_font_weight_runs: usize,
    inherited_font_style_runs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleTextColorOwner {
    Inherited,
    CssClass,
    Inline { portable: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleTextTypographyOwner {
    Inherited,
    SourceOwned,
    UnmeasuredSource,
}

#[derive(Clone, Copy)]
enum VisibleTextStylePolicy {
    Inline,
    #[cfg(feature = "diagram-mindmap")]
    Mindmap,
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling,
}

#[derive(Clone, Copy)]
struct VisibleTextOwners {
    color: VisibleTextColorOwner,
    font_family: VisibleTextTypographyOwner,
    font_size: VisibleTextTypographyOwner,
    font_weight: VisibleTextTypographyOwner,
    font_style: VisibleTextTypographyOwner,
    used_font_size: Option<f64>,
    host_font_size: bool,
}

impl VisibleTextOwners {
    const INHERITED: Self = Self {
        color: VisibleTextColorOwner::Inherited,
        font_family: VisibleTextTypographyOwner::Inherited,
        font_size: VisibleTextTypographyOwner::Inherited,
        font_weight: VisibleTextTypographyOwner::Inherited,
        font_style: VisibleTextTypographyOwner::Inherited,
        used_font_size: None,
        host_font_size: false,
    };
}

impl VisibleTextStyleFacts {
    #[cfg(test)]
    pub(crate) fn count_collections<T>(action: impl FnOnce() -> T) -> (T, usize) {
        let before = FACT_COLLECTIONS.get();
        let result = action();
        (result, FACT_COLLECTIONS.get() - before)
    }

    const fn empty(parse_valid: bool) -> Self {
        Self {
            parse_valid,
            content_runs: 0,
            visible_runs: 0,
            font_visibility_verified: parse_valid,
            inherited_color_runs: 0,
            unverified_color_runs: 0,
            unverified_portable_color_runs: 0,
            inherited_font_family_runs: 0,
            source_owned_font_family_runs: 0,
            inherited_font_size_runs: 0,
            inherited_font_weight_runs: 0,
            inherited_font_style_runs: 0,
        }
    }

    pub(crate) const fn not_requested() -> Self {
        Self::empty(false)
    }

    pub(crate) fn from_xhtml_fragment(fragment: &str) -> Self {
        Self::from_xhtml_fragment_with_policy(fragment, VisibleTextStylePolicy::Inline, None, None)
    }

    /// Excludes only text proven to have zero font size in the actual prepared XHTML.
    pub(crate) fn from_xhtml_fragment_at_font_size(fragment: &str, font_size: Option<f64>) -> Self {
        Self::from_xhtml_fragment_with_policy(
            fragment,
            VisibleTextStylePolicy::Inline,
            Some(font_size),
            None,
        )
    }

    /// Mindmap's known span-color rules are direct declarations, not inherited ancestor color.
    #[cfg(feature = "diagram-mindmap")]
    pub(crate) fn from_mindmap_xhtml_fragment(fragment: &str) -> Self {
        Self::from_xhtml_fragment_with_policy(fragment, VisibleTextStylePolicy::Mindmap, None, None)
    }

    /// Event Modeling declares its family font and optional raw size on every box span.
    #[cfg(feature = "diagram-event-modeling")]
    pub(crate) fn from_eventmodeling_xhtml_fragment_at_font_size(
        fragment: &str,
        font_size: Option<f64>,
        span_font_size_css: Option<&str>,
    ) -> Self {
        Self::from_xhtml_fragment_with_policy(
            fragment,
            VisibleTextStylePolicy::EventModeling,
            Some(font_size),
            span_font_size_css,
        )
    }

    fn from_xhtml_fragment_with_policy(
        fragment: &str,
        policy: VisibleTextStylePolicy,
        font_size: Option<Option<f64>>,
        span_font_size_css: Option<&str>,
    ) -> Self {
        #[cfg(test)]
        FACT_COLLECTIONS.set(FACT_COLLECTIONS.get() + 1);
        let wrapped = format!("<merman-visible-text>{fragment}</merman-visible-text>");
        let Ok(document) = crate::xml_dom::Document::parse(&wrapped) else {
            return Self::empty(false);
        };
        let mut facts = Self::empty(true);
        collect_visible_facts(
            document.root_element(),
            &mut facts,
            policy,
            font_size,
            span_font_size_css,
        );
        facts
    }

    pub(crate) fn plain_text(text: &str) -> Self {
        Self::plain_text_with_font_size(text, None)
    }

    pub(crate) fn plain_text_at_font_size(text: &str, font_size: Option<f64>) -> Self {
        Self::plain_text_with_font_size(text, Some(font_size))
    }

    fn plain_text_with_font_size(text: &str, font_size: Option<Option<f64>>) -> Self {
        #[cfg(test)]
        FACT_COLLECTIONS.set(FACT_COLLECTIONS.get() + 1);
        let mut facts = Self::empty(true);
        if !trim_html_collapsible_ascii_whitespace(text).is_empty() {
            facts.observe(
                VisibleTextOwners {
                    used_font_size: font_size.flatten(),
                    ..VisibleTextOwners::INHERITED
                },
                font_size.is_some(),
            );
        }
        facts
    }

    /// Returns the ownership facts for Mermaid's SVG Markdown projection.
    ///
    /// SVG label writers emit Markdown tokens as escaped text and only vary weight/style. Inline
    /// HTML attributes therefore remain visible text and cannot own `color` or `font-family`.
    pub(crate) fn from_svg_markdown_projection(markdown: &str) -> Self {
        Self::svg_markdown_projection_with_font_size(markdown, None)
    }

    pub(crate) fn from_svg_markdown_projection_at_font_size(
        markdown: &str,
        font_size: Option<f64>,
    ) -> Self {
        Self::svg_markdown_projection_with_font_size(markdown, Some(font_size))
    }

    fn svg_markdown_projection_with_font_size(
        markdown: &str,
        font_size: Option<Option<f64>>,
    ) -> Self {
        let has_visible_text = super::mermaid_markdown_to_lines(markdown, true)
            .iter()
            .flatten()
            .any(|(word, _)| !trim_html_collapsible_ascii_whitespace(word).is_empty());
        Self::plain_text_with_font_size(if has_visible_text { markdown } else { "" }, font_size)
    }

    pub(crate) const fn parse_valid(&self) -> bool {
        self.parse_valid
    }
    pub(crate) const fn has_visible_runs(&self) -> bool {
        self.visible_runs != 0
    }
    pub(crate) const fn has_content_runs(&self) -> bool {
        self.content_runs != 0
    }
    pub(crate) const fn font_visibility_verified(&self) -> bool {
        self.font_visibility_verified
    }
    pub(crate) const fn visible_run_count(&self) -> usize {
        self.visible_runs
    }
    pub(crate) const fn inherited_color_run_count(&self) -> usize {
        self.inherited_color_runs
    }
    /// CSS classes may override inherited color without a locally known declaration.
    pub(crate) const fn unverified_color_run_count(&self) -> usize {
        self.unverified_color_runs
    }
    /// Portable paint cannot resolve CSS variables or document-dependent inline colors.
    pub(crate) const fn unverified_portable_color_run_count(&self) -> usize {
        self.unverified_portable_color_runs
    }
    pub(crate) const fn inherited_font_family_run_count(&self) -> usize {
        self.inherited_font_family_runs
    }
    pub(crate) const fn unverified_font_family_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_family_runs
    }
    /// Known local families still count as unverified for existing inheritance-only consumers.
    #[cfg(any(feature = "diagram-mindmap", feature = "diagram-requirement"))]
    pub(crate) const fn source_owned_font_family_run_count(&self) -> usize {
        self.source_owned_font_family_runs
    }
    #[cfg(any(
        feature = "diagram-mindmap",
        feature = "diagram-requirement",
        feature = "diagram-event-modeling"
    ))]
    pub(crate) const fn unknown_font_family_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_family_runs - self.source_owned_font_family_runs
    }
    pub(crate) const fn inherited_font_size_run_count(&self) -> usize {
        self.inherited_font_size_runs
    }
    pub(crate) const fn unverified_font_size_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_size_runs
    }
    pub(crate) const fn unverified_font_weight_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_weight_runs
    }
    pub(crate) const fn unverified_font_style_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_style_runs
    }
    #[cfg(test)]
    fn non_inherited_color_run_count(&self) -> usize {
        self.visible_runs - self.inherited_color_runs
    }

    fn observe(&mut self, owners: VisibleTextOwners, filter_font_size: bool) {
        self.content_runs += 1;
        if filter_font_size {
            if owners.used_font_size == Some(0.0) {
                return;
            }
            self.font_visibility_verified &= owners.used_font_size.is_some();
        }
        self.visible_runs += 1;
        self.inherited_color_runs += usize::from(owners.color == VisibleTextColorOwner::Inherited);
        self.unverified_color_runs += usize::from(owners.color == VisibleTextColorOwner::CssClass);
        self.unverified_portable_color_runs += usize::from(matches!(
            owners.color,
            VisibleTextColorOwner::CssClass | VisibleTextColorOwner::Inline { portable: false }
        ));
        self.inherited_font_family_runs +=
            usize::from(owners.font_family == VisibleTextTypographyOwner::Inherited);
        self.source_owned_font_family_runs +=
            usize::from(owners.font_family == VisibleTextTypographyOwner::SourceOwned);
        self.inherited_font_size_runs +=
            usize::from(owners.font_size == VisibleTextTypographyOwner::Inherited);
        self.inherited_font_weight_runs +=
            usize::from(owners.font_weight == VisibleTextTypographyOwner::Inherited);
        self.inherited_font_style_runs +=
            usize::from(owners.font_style == VisibleTextTypographyOwner::Inherited);
    }
}

fn collect_visible_facts(
    root: crate::xml_dom::Node<'_, '_>,
    facts: &mut VisibleTextStyleFacts,
    policy: VisibleTextStylePolicy,
    font_size: Option<Option<f64>>,
    _span_font_size_css: Option<&str>,
) {
    // roxmltree's descendants are a flat preorder range. Each ancestor state is pushed and
    // popped once, retaining inheritance without recursion or repeated ancestor scans.
    let mut parents = Vec::<(crate::xml_dom::NodeId, VisibleTextOwners)>::new();
    // A stylesheet may target an earlier sibling; establish that uncertainty before filtering.
    let stylesheet_present = root
        .descendants()
        .any(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("style"));
    let root_owners = VisibleTextOwners {
        used_font_size: if stylesheet_present {
            None
        } else {
            font_size.flatten()
        },
        host_font_size: stylesheet_present,
        ..VisibleTextOwners::INHERITED
    };
    for node in root.descendants() {
        let parent = node.parent().map(|parent| parent.id());
        while parents.last().map(|(id, _)| *id) != parent && !parents.is_empty() {
            parents.pop();
        }
        let inherited = parents.last().map_or(root_owners, |(_, owners)| *owners);
        let owners = if node.is_element() {
            let (direct_root_family, direct_span_size_css) = match policy {
                #[cfg(feature = "diagram-event-modeling")]
                VisibleTextStylePolicy::EventModeling => {
                    let span = node.tag_name().name().eq_ignore_ascii_case("span");
                    (span, _span_font_size_css.filter(|_| span))
                }
                _ => (false, None::<&str>),
            };
            let has_css_class = node
                .attribute("class")
                .is_some_and(|class| !class.trim().is_empty());
            let has_font_face = node.tag_name().name().eq_ignore_ascii_case("font")
                && node
                    .attribute("face")
                    .is_some_and(|face| !face.trim().is_empty());
            let has_font_size = node.tag_name().name().eq_ignore_ascii_case("font")
                && node
                    .attribute("size")
                    .is_some_and(|size| !size.trim().is_empty());
            let local_color = if has_css_class {
                VisibleTextColorOwner::CssClass
            } else {
                match policy {
                    VisibleTextStylePolicy::Inline => inherited.color,
                    #[cfg(feature = "diagram-mindmap")]
                    VisibleTextStylePolicy::Mindmap
                        if node.tag_name().name().eq_ignore_ascii_case("span") =>
                    {
                        // Mindmap's section/root CSS sets every span's color directly. Its
                        // typed label overlay consumes that role; inline inheritance below
                        // still reads the actual parent and can restore a source-owned color.
                        match inherited.color {
                            // An unknown host class can also target descendant spans.
                            VisibleTextColorOwner::CssClass => VisibleTextColorOwner::CssClass,
                            _ => VisibleTextColorOwner::Inherited,
                        }
                    }
                    #[cfg(feature = "diagram-mindmap")]
                    VisibleTextStylePolicy::Mindmap => inherited.color,
                    #[cfg(feature = "diagram-event-modeling")]
                    VisibleTextStylePolicy::EventModeling => inherited.color,
                }
            };
            let local_family = if has_css_class || has_font_face {
                VisibleTextTypographyOwner::UnmeasuredSource
            } else if direct_root_family && !inherited.host_font_size {
                VisibleTextTypographyOwner::Inherited
            } else if super::xhtml_font_default(node.tag_name().name(), "font-family").is_some() {
                // Known HTML defaults are specified on the element, before its inline style.
                // An unresolved ancestor may still supply a host rule targeting descendants.
                if inherited.font_family == VisibleTextTypographyOwner::UnmeasuredSource {
                    VisibleTextTypographyOwner::UnmeasuredSource
                } else {
                    VisibleTextTypographyOwner::SourceOwned
                }
            } else {
                inherited.font_family
            };
            let local_size = if has_css_class || has_font_size {
                VisibleTextTypographyOwner::UnmeasuredSource
            } else if direct_span_size_css.is_some() && !inherited.host_font_size {
                VisibleTextTypographyOwner::Inherited
            } else {
                inherited.font_size
            };
            let host_font_size = inherited.host_font_size || has_css_class;
            let used_font_size = if font_size.is_none() || host_font_size {
                None
            } else {
                let default_size = if let Some(size_css) = direct_span_size_css {
                    // Relative declarations use this span's actual parent, not a root snapshot.
                    visible_text_font_size_after_style(
                        inherited.used_font_size,
                        &format!("font-size:{size_css}"),
                    )
                } else if has_font_size {
                    None
                } else {
                    super::xhtml_font_default(node.tag_name().name(), "font-size")
                        .map(|value| {
                            inherited.used_font_size.and_then(|size| {
                                crate::mermaid_style::resolve_mermaid_font_size_px(
                                    value,
                                    crate::mermaid_style::CssFontSizeContext::uniform(size),
                                )
                            })
                        })
                        .unwrap_or(inherited.used_font_size)
                };
                node.attribute("style").map_or(default_size, |style| {
                    font_size_after_style(inherited.used_font_size, default_size, style)
                })
            };
            VisibleTextOwners {
                color: inline_color_owner(
                    node.attribute("style"),
                    &inherited.color,
                    &local_color,
                    has_css_class,
                ),
                font_family: inline_font_family_owner(
                    node.attribute("style"),
                    &inherited.font_family,
                    &local_family,
                    has_css_class,
                ),
                font_size: inline_font_size_owner(
                    node.attribute("style"),
                    &inherited.font_size,
                    &local_size,
                    has_css_class,
                ),
                font_weight: inline_typography_owner(
                    node.attribute("style"),
                    "font-weight",
                    inherited.font_weight,
                    has_css_class
                        || super::xhtml_font_default(node.tag_name().name(), "font-weight")
                            .is_some(),
                    has_css_class,
                ),
                font_style: inline_typography_owner(
                    node.attribute("style"),
                    "font-style",
                    inherited.font_style,
                    has_css_class
                        || super::xhtml_font_default(node.tag_name().name(), "font-style")
                            .is_some(),
                    has_css_class,
                ),
                used_font_size,
                host_font_size,
            }
        } else {
            inherited
        };
        if node.is_element() && is_fontawesome_icon(node) {
            facts.observe(owners, font_size.is_some());
        }
        if node.is_text()
            && !node
                .parent()
                .is_some_and(|parent| parent.tag_name().name().eq_ignore_ascii_case("style"))
            && node
                .text()
                .is_some_and(|text| !trim_html_collapsible_ascii_whitespace(text).is_empty())
        {
            facts.observe(owners, font_size.is_some());
        }
        if node.is_element() {
            parents.push((node.id(), owners));
        }
    }
    if stylesheet_present {
        // An adopted stylesheet may target any visible run. Do not interpret arbitrary CSS.
        facts.inherited_color_runs = 0;
        facts.unverified_color_runs = facts.visible_runs;
        facts.unverified_portable_color_runs = facts.visible_runs;
        facts.inherited_font_family_runs = 0;
        facts.source_owned_font_family_runs = 0;
        facts.inherited_font_size_runs = 0;
        facts.inherited_font_weight_runs = 0;
        facts.inherited_font_style_runs = 0;
    }
}

/// Resolves the existing admitted font-size subset while leaving host-dependent CSS unknown.
pub(crate) fn visible_text_font_size_after_style(parent: Option<f64>, style: &str) -> Option<f64> {
    font_size_after_style(parent, parent, style)
}

fn font_size_after_style(parent: Option<f64>, fallback: Option<f64>, style: &str) -> Option<f64> {
    use crate::mermaid_style::{self, CssFontSizeContext, CssFontSizeOwnership};

    let mut winner = None;
    mermaid_style::visit_style_declaration_boundaries(style, |boundary| {
        let Some(owner) = mermaid_style::css_font_size_override(boundary.raw()) else {
            return;
        };
        if winner.is_some_and(|(important, _)| important && !owner.important()) {
            return;
        }
        let size = match owner.ownership() {
            CssFontSizeOwnership::Inherited => parent,
            CssFontSizeOwnership::SourceOwned | CssFontSizeOwnership::Unverified => {
                mermaid_style::parse_style_declaration(boundary.raw()).and_then(|declaration| {
                    if declaration.property() != "font-size" {
                        return None;
                    }
                    if declaration.font_size_is_literal_zero() {
                        return Some(0.0);
                    }
                    declaration
                        .analysis()
                        .context_free_font_size_px()
                        .or_else(|| {
                            if declaration.analysis().depends_on_root_font_size()
                                || owner.ownership() == CssFontSizeOwnership::Unverified
                            {
                                None
                            } else {
                                parent.and_then(|size| {
                                    declaration
                                        .resolve_font_size_px(CssFontSizeContext::uniform(size))
                                })
                            }
                        })
                })
            }
        };
        winner = Some((owner.important(), size));
    });
    winner.map_or(fallback, |(_, size)| size)
}

fn inline_typography_owner(
    style: Option<&str>,
    property: &str,
    inherited: VisibleTextTypographyOwner,
    source_default: bool,
    has_css_class: bool,
) -> VisibleTextTypographyOwner {
    let local = if source_default {
        VisibleTextTypographyOwner::UnmeasuredSource
    } else {
        inherited
    };
    let Some(style) = style else {
        return local;
    };
    let mut winner: Option<(bool, &str)> = None;
    crate::mermaid_style::visit_parsed_style_declarations(style, |declaration| {
        let reset = declaration.property() == "all"
            && ["inherit", "initial", "unset", "revert", "revert-layer"]
                .iter()
                .any(|keyword| declaration.value().trim().eq_ignore_ascii_case(keyword));
        if (declaration.property() == property || declaration.property() == "font" || reset)
            && winner.is_none_or(|(important, _)| declaration.important() || !important)
        {
            winner = Some((declaration.important(), declaration.value()));
        }
    });
    match winner {
        None => local,
        Some((important, value))
            if (value.trim().eq_ignore_ascii_case("inherit")
                || value.trim().eq_ignore_ascii_case("unset"))
                && (!has_css_class || important) =>
        {
            inherited
        }
        Some(_) => VisibleTextTypographyOwner::UnmeasuredSource,
    }
}

fn inline_color_owner(
    style: Option<&str>,
    parent: &VisibleTextColorOwner,
    local: &VisibleTextColorOwner,
    has_css_class: bool,
) -> VisibleTextColorOwner {
    let Some(style) = style else {
        return *local;
    };
    let mut winner: Option<(bool, Option<&str>)> = None;
    crate::mermaid_style::visit_style_declaration_boundaries(style, |boundary| {
        let candidate = if let Some(declaration) =
            crate::mermaid_style::parse_style_declaration(boundary.raw())
        {
            if declaration.property() == "color" {
                if crate::mermaid_style::is_safe_browser_css_color_value(declaration.value()) {
                    Some((declaration.important(), Some(declaration.value())))
                } else {
                    let mut value = cssparser::Parser::new(declaration.value());
                    // A valid browser color may exceed the named/portable subset (escaped names,
                    // system colors or modern functions). Retain its priority without evaluating it.
                    let unknown_color = declaration.is_single_component_value()
                        && matches!(
                            value.next(),
                            Ok(cssparser::Token::Ident(_)
                                | cssparser::Token::Hash(_)
                                | cssparser::Token::IDHash(_)
                                | cssparser::Token::Function(_))
                        );
                    unknown_color.then_some((declaration.important(), None))
                }
            } else if declaration.property() == "all" {
                if declaration.inherits_property_value() {
                    Some((declaration.important(), Some(declaration.value())))
                } else if matches!(
                    declaration.value().trim().to_ascii_lowercase().as_str(),
                    "initial" | "revert" | "revert-layer"
                ) || declaration.analysis().has_dynamic_reference_function()
                {
                    Some((declaration.important(), None))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Retained browser CSS can exceed the safe-emission subset. A comment must not
            // turn a local color into proof of inherited paint; retain uncertainty in cascade order.
            let raw = boundary.raw().trim().trim_end_matches(';');
            let mut parser = cssparser::Parser::new(raw);
            let affects_color = parser.expect_ident_cloned().is_ok_and(|property| {
                property.eq_ignore_ascii_case("color") || property.eq_ignore_ascii_case("all")
            }) && parser.expect_colon().is_ok();
            let color_component = affects_color
                && matches!(
                    parser.next(),
                    Ok(cssparser::Token::Ident(_)
                        | cssparser::Token::Hash(_)
                        | cssparser::Token::IDHash(_)
                        | cssparser::Token::Function(_))
                );
            if color_component && parser.is_exhausted() {
                Some((false, None))
            } else if color_component
                && parser.try_parse(cssparser::parse_important).is_ok()
                && parser.is_exhausted()
            {
                Some((true, None))
            } else {
                None
            }
        };
        if let Some((important, value)) = candidate
            && winner.is_none_or(|(previous_important, _)| important || !previous_important)
        {
            winner = Some((important, value));
        }
    });
    let (important, value) = match winner {
        None => return *local,
        Some((important, Some(value))) => (important, value),
        Some((_, None)) => return VisibleTextColorOwner::CssClass,
    };
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "currentcolor" | "inherit" | "unset"
    ) {
        if !has_css_class || important {
            *parent
        } else {
            *local
        }
    } else {
        VisibleTextColorOwner::Inline {
            portable: crate::mermaid_style::is_supported_css_color_value(value.trim())
                || value.trim().eq_ignore_ascii_case("initial"),
        }
    }
}

fn inline_font_family_owner(
    style: Option<&str>,
    parent: &VisibleTextTypographyOwner,
    local: &VisibleTextTypographyOwner,
    has_css_class: bool,
) -> VisibleTextTypographyOwner {
    match style.and_then(crate::mermaid_style::css_font_family_override) {
        None => *local,
        Some(override_)
            if override_.ownership() == crate::mermaid_style::CssFontFamilyOwnership::Inherited
                && (!has_css_class || override_.important()) =>
        {
            *parent
        }
        Some(override_)
            if override_.ownership()
                == crate::mermaid_style::CssFontFamilyOwnership::SourceOwned
                && (!has_css_class || override_.important()) =>
        {
            VisibleTextTypographyOwner::SourceOwned
        }
        Some(_) => VisibleTextTypographyOwner::UnmeasuredSource,
    }
}

fn inline_font_size_owner(
    style: Option<&str>,
    parent: &VisibleTextTypographyOwner,
    local: &VisibleTextTypographyOwner,
    has_css_class: bool,
) -> VisibleTextTypographyOwner {
    match style.and_then(crate::mermaid_style::css_font_size_override) {
        None => *local,
        Some(override_)
            if override_.ownership() == crate::mermaid_style::CssFontSizeOwnership::Inherited
                && (!has_css_class || override_.important()) =>
        {
            *parent
        }
        Some(_) => VisibleTextTypographyOwner::UnmeasuredSource,
    }
}

fn is_fontawesome_icon(node: crate::xml_dom::Node<'_, '_>) -> bool {
    node.tag_name().name().eq_ignore_ascii_case("i")
        && node.attribute("class").is_some_and(|class| {
            let mut has_prefix = false;
            let mut has_icon = false;
            for token in class.split_ascii_whitespace() {
                has_prefix |= matches!(token, "fa" | "fab" | "fak" | "fal" | "far" | "fas");
                has_icon |= token.starts_with("fa-");
            }
            has_prefix && has_icon
        })
}

#[cfg(test)]
mod tests {
    use super::VisibleTextStyleFacts;

    #[test]
    fn sized_facts_exclude_zero_runs_and_keep_positive_descendants() {
        for (fragment, size, visible) in [
            ("Hidden", Some(0.0), 0),
            ("<small><big>Hidden</big></small>", Some(0.0), 0),
            ("<span style='font-size:2em'>Hidden</span>", Some(0.0), 0),
            ("<span style='font-size:24pt'>Visible</span>", Some(0.0), 1),
            ("<span style='font-size:24px'>Visible</span>", None, 1),
            (
                "<span style='font-size:0px'>Hidden<span style='font-size:24px'>Visible</span></span>",
                Some(16.0),
                1,
            ),
        ] {
            let (facts, collections) = VisibleTextStyleFacts::count_collections(|| {
                VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(fragment, size)
            });
            assert_eq!(collections, 1, "{fragment}");
            assert!(facts.has_content_runs(), "{fragment}");
            assert!(facts.parse_valid(), "{fragment}");
            assert_eq!(facts.visible_run_count(), visible, "{fragment}");
            assert!(facts.font_visibility_verified(), "{fragment}");
        }
    }

    #[test]
    fn sized_facts_follow_declaration_priority_and_element_defaults() {
        for (fragment, visible) in [
            (
                "<span style='font-size:0px!important;font-size:24px'>Hidden</span>",
                0,
            ),
            (
                "<span style='font-size:0px;font-size:24px!important'>Visible</span>",
                1,
            ),
            ("<span style='font-size:-1px'>Inherited</span>", 1),
            ("<small style='font-size:-1px'>Inherited default</small>", 1),
            ("<span style='font-size:0rem'>Hidden</span>", 0),
            ("<span style='font-size:0cap'>Hidden</span>", 0),
        ] {
            let facts =
                VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(fragment, Some(16.0));
            assert_eq!(facts.visible_run_count(), visible, "{fragment}");
            assert!(facts.font_visibility_verified(), "{fragment}");
        }
        let facts = VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(
            "<span style='font-size:0;color:red'>Hidden</span><strong>Visible</strong>",
            Some(16.0),
        );
        assert_eq!(facts.visible_run_count(), 1);
        assert_eq!(facts.inherited_color_run_count(), 1);
        assert_eq!(facts.unverified_font_weight_run_count(), 1);
    }

    #[test]
    fn sized_facts_preserve_host_dependent_text_as_unverified() {
        for fragment in [
            "<span style='font-size:1rem'>Host root</span>",
            "<span style='font-size:calc(1em + 1px)'>Expression</span>",
            "<span class='host'>Host class</span>",
            "Earlier text<style>span{font-size:24px}</style>",
            "<span class='host'><span style='font-size:0px'>Host descendant selector</span></span>",
        ] {
            let facts =
                VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(fragment, Some(0.0));
            assert!(facts.has_visible_runs(), "{fragment}");
            assert!(facts.has_content_runs(), "{fragment}");
            assert!(!facts.font_visibility_verified(), "{fragment}");
        }
    }

    #[cfg(feature = "diagram-event-modeling")]
    #[test]
    fn eventmodeling_span_font_rules_follow_emitted_declarations_and_inline_overrides() {
        for (fragment, size, size_css, visible, family, inherited_size) in [
            ("<b>View</b>", Some(0.0), Some("0px"), 0, 0, 0),
            (
                "<b style='font-size:20px'><span>View</span></b>",
                Some(0.0),
                Some("0px"),
                0,
                0,
                0,
            ),
            (
                "<b style='font-size:20px'><span>View</span></b>",
                Some(0.0),
                None,
                1,
                1,
                0,
            ),
            (
                "<b style='font-size:20px'>View</b>",
                Some(0.0),
                Some("0px"),
                1,
                1,
                0,
            ),
            (
                "<span style='font-size:24pt'>View</span>",
                Some(0.0),
                Some("0px"),
                1,
                1,
                0,
            ),
            ("<code>Data</code>", Some(16.0), Some("16px"), 1, 0, 1),
            (
                "<code><span>Data</span></code>",
                Some(16.0),
                Some("16px"),
                1,
                1,
                1,
            ),
            (
                "<code style='font-size:20px'><span>Data</span></code>",
                Some(0.0),
                Some("0px"),
                0,
                0,
                0,
            ),
            (
                "<code><span style='font-size:20px'>Data</span></code>",
                Some(0.0),
                Some("0px"),
                1,
                1,
                0,
            ),
            (
                "<b style='font-size:20px'><span style='font-size:inherit'>View</span></b>",
                Some(0.0),
                Some("0px"),
                1,
                1,
                0,
            ),
            ("<span>View</span>", Some(16.0), Some("16px"), 1, 1, 1),
        ] {
            let facts = VisibleTextStyleFacts::from_eventmodeling_xhtml_fragment_at_font_size(
                fragment, size, size_css,
            );
            assert!(facts.parse_valid(), "{fragment}");
            assert!(facts.font_visibility_verified(), "{fragment}");
            assert_eq!(facts.visible_run_count(), visible, "{fragment}");
            assert_eq!(
                facts.inherited_font_family_run_count(),
                family,
                "{fragment}"
            );
            assert_eq!(facts.unknown_font_family_run_count(), 0, "{fragment}");
            assert_eq!(
                facts.inherited_font_size_run_count(),
                inherited_size,
                "{fragment}"
            );
        }
    }

    #[cfg(feature = "diagram-event-modeling")]
    #[test]
    fn eventmodeling_relative_span_sizes_follow_each_actual_parent() {
        for size_css in ["inherit", "100%", "2em"] {
            for (parent_size, visible) in [("0px", 0), ("20px", 1)] {
                let fragment = format!(
                    "<b style='font-size:{parent_size}'><span><span>View</span></span></b>"
                );
                let facts = VisibleTextStyleFacts::from_eventmodeling_xhtml_fragment_at_font_size(
                    &fragment,
                    Some(16.0),
                    Some(size_css),
                );
                assert!(facts.parse_valid(), "{size_css}: {fragment}");
                assert!(facts.font_visibility_verified(), "{size_css}: {fragment}");
                assert_eq!(facts.visible_run_count(), visible, "{size_css}: {fragment}");
            }
        }
    }

    #[cfg(feature = "diagram-event-modeling")]
    #[test]
    fn eventmodeling_span_rules_preserve_unknown_host_dependencies() {
        for fragment in [
            "<span class='host'>View</span>",
            "<b class='host'><span>View</span></b>",
            "<span style='font-size:var(--host-size)'>View</span>",
            "<span>View</span><style>span{font-size:20px}</style>",
        ] {
            let facts = VisibleTextStyleFacts::from_eventmodeling_xhtml_fragment_at_font_size(
                fragment,
                Some(0.0),
                Some("0px"),
            );
            assert!(facts.parse_valid(), "{fragment}");
            assert!(facts.has_visible_runs(), "{fragment}");
            assert!(!facts.font_visibility_verified(), "{fragment}");
        }
        let font = VisibleTextStyleFacts::from_eventmodeling_xhtml_fragment_at_font_size(
            "<span style='font-family:var(--host-font);font-size:20px'>View</span>",
            Some(0.0),
            Some("0px"),
        );
        assert!(font.font_visibility_verified());
        assert_eq!(font.inherited_font_family_run_count(), 0);
        assert_eq!(font.unknown_font_family_run_count(), 1);
    }

    #[test]
    fn sized_svg_facts_keep_html_spelling_as_literal_text() {
        let source = "<span style='font-size:0px;font-family:Other'>Literal</span>";
        let visible =
            VisibleTextStyleFacts::from_svg_markdown_projection_at_font_size(source, Some(16.0));
        assert!(visible.has_visible_runs());
        assert_eq!(visible.inherited_font_family_run_count(), 1);
        let hidden =
            VisibleTextStyleFacts::from_svg_markdown_projection_at_font_size(source, Some(0.0));
        assert!(hidden.has_content_runs());
        assert!(!hidden.has_visible_runs());
    }

    #[test]
    fn facts_only_retain_the_queried_ownership_summary() {
        let summary = |facts: VisibleTextStyleFacts| {
            (
                facts.parse_valid(),
                facts.visible_run_count(),
                facts.inherited_color_run_count(),
                facts.unverified_color_run_count(),
                facts.unverified_portable_color_run_count(),
                facts.inherited_font_family_run_count(),
                facts.unverified_font_family_run_count(),
                facts.inherited_font_size_run_count(),
                facts.unverified_font_size_run_count(),
            )
        };
        assert_eq!(
            summary(VisibleTextStyleFacts::plain_text("first")),
            summary(VisibleTextStyleFacts::plain_text("second"))
        );
        assert_eq!(
            summary(VisibleTextStyleFacts::from_xhtml_fragment(
                "<span style='color:red'>first</span>"
            )),
            summary(VisibleTextStyleFacts::from_xhtml_fragment(
                "<b style='color:#123456'>second</b>"
            ))
        );
    }

    #[test]
    fn portable_paint_distinguishes_dynamic_inline_colors_from_static_owners() {
        for (value, unknown) in [
            ("var(--ink)", 1),
            ("#123456", 0),
            ("initial", 0),
            ("transparent", 0),
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(&format!(
                "<span style=\"color:{value}\">Label</span>"
            ));
            assert!(facts.parse_valid());
            assert_eq!(
                facts.unverified_portable_color_run_count(),
                unknown,
                "{value}"
            );
        }
    }

    #[test]
    fn visible_text_facts_track_inherited_and_inline_color_runs() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<p>Before <span style=\"color: transparent\">Hidden</span> After</p>",
        );

        assert!(facts.parse_valid());
        assert!(facts.has_visible_runs());
        assert_eq!(facts.inherited_color_run_count(), 2);
        assert_eq!(facts.non_inherited_color_run_count(), 1);
    }

    #[test]
    fn nested_inherit_preserves_the_parent_color_owner() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span style=\"color:red\"><b style=\"color:inherit\">Owned</b></span>",
        );

        assert_eq!(facts.inherited_color_run_count(), 0);
        assert_eq!(facts.non_inherited_color_run_count(), 1);
    }

    #[test]
    fn invalid_or_unrelated_inline_declarations_do_not_steal_color_ownership() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span style=\"font-weight:bold;color:not a color\">Inherited</span>",
        );

        assert_eq!(facts.inherited_color_run_count(), 1);
        assert_eq!(facts.non_inherited_color_run_count(), 0);
    }

    #[test]
    fn css_class_ownership_is_not_mistaken_for_inherited_theme_color() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<p>Before <span class=\"label\">CSS-owned</span> After</p>",
        );

        assert_eq!(facts.visible_run_count(), 3);
        assert_eq!(facts.inherited_color_run_count(), 2);
        assert_eq!(facts.non_inherited_color_run_count(), 1);

        let inherited = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span class=\"label\" style=\"color:inherit\">CSS-owned</span>",
        );
        assert_eq!(inherited.inherited_color_run_count(), 0);
        assert_eq!(inherited.non_inherited_color_run_count(), 1);
    }

    #[test]
    fn important_color_inheritance_overrides_class_uncertainty_but_preserves_parent() {
        for value in ["inherit", "currentColor", "unset"] {
            let inherited = VisibleTextStyleFacts::from_xhtml_fragment(&format!(
                "<span class='host' style='color:{value} !important'>Inherited</span>",
            ));
            assert_eq!(inherited.inherited_color_run_count(), 1, "{value}");
            assert_eq!(inherited.unverified_color_run_count(), 0, "{value}");
            let source_owned = VisibleTextStyleFacts::from_xhtml_fragment(&format!(
                "<span style='color:red'><b class='host' style='color:{value} !important'>Owned</b></span>",
            ));
            assert_eq!(source_owned.inherited_color_run_count(), 0, "{value}");
            assert_eq!(source_owned.unverified_color_run_count(), 0, "{value}");
            assert_eq!(source_owned.non_inherited_color_run_count(), 1, "{value}");
            assert_eq!(
                source_owned.unverified_portable_color_run_count(),
                0,
                "{value}"
            );
            let uncertain = VisibleTextStyleFacts::from_xhtml_fragment(&format!(
                "<span class='host' style='color:{value}'>Unknown</span>",
            ));
            assert_eq!(uncertain.unverified_color_run_count(), 1, "{value}");
        }
    }

    #[test]
    fn visible_text_facts_track_font_family_ownership() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<p>Inherited <span style=\"font-family:Source Sans\">Owned</span> <span class=\"custom\">Unknown</span></p>",
        );

        assert_eq!(facts.visible_run_count(), 3);
        assert_eq!(facts.inherited_font_family_run_count(), 1);
        assert_eq!(facts.unverified_font_family_run_count(), 2);
    }

    #[test]
    fn inherited_font_family_declaration_preserves_parent_measurement_state() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span style=\"font-family:Source Sans\"><b style=\"font-family:inherit\">Owned</b></span>",
        );

        assert_eq!(facts.inherited_font_family_run_count(), 0);
        assert_eq!(facts.unverified_font_family_run_count(), 1);

        let inherited = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span style=\"font-family:inherit\">Inherited</span>",
        );
        assert_eq!(inherited.inherited_font_family_run_count(), 1);
        assert_eq!(inherited.unverified_font_family_run_count(), 0);
    }

    #[cfg(feature = "diagram-mindmap")]
    #[test]
    fn known_local_font_families_are_distinct_from_unknown_dependencies() {
        for (fragment, inherited, source_owned, unknown) in [
            ("<span>Root</span>", 1, 0, 0),
            ("<span></span>", 0, 0, 0),
            ("<span style='font-family:monospace'>Root</span>", 0, 1, 0),
            (
                "<span style='font-family:monospace'><b style='font-family:inherit'>Root</b></span>",
                0,
                1,
                0,
            ),
            ("<span class='host-font'>Root</span>", 0, 0, 1),
            (
                "<span style='font-family:var(--host-font)'>Root</span>",
                0,
                0,
                1,
            ),
            ("<span style='font-family:initial'>Root</span>", 0, 0, 1),
            (
                "<span class='host-font' style='font-family:monospace'>Root</span>",
                0,
                0,
                1,
            ),
            (
                "<span class='host-font' style='font-family:monospace !important'>Root</span>",
                0,
                1,
                0,
            ),
            (
                "<span class='host-font' style='font-family:inherit !important'>Root</span>",
                1,
                0,
                0,
            ),
            (
                "<span style='font-family:monospace !important;font-family:inherit'>Root</span>",
                0,
                1,
                0,
            ),
            (
                "<style>span{font-family:var(--host-font)}</style><span style='font-family:monospace'>Root</span>",
                0,
                0,
                1,
            ),
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert!(facts.parse_valid(), "{fragment}");
            assert_eq!(
                facts.inherited_font_family_run_count(),
                inherited,
                "{fragment}"
            );
            assert_eq!(
                facts.source_owned_font_family_run_count(),
                source_owned,
                "{fragment}"
            );
            assert_eq!(facts.unknown_font_family_run_count(), unknown, "{fragment}");
            assert_eq!(
                facts.unverified_font_family_run_count(),
                source_owned + unknown,
                "existing inheritance-only consumers keep their original meaning: {fragment}"
            );
        }
    }

    #[cfg(feature = "diagram-mindmap")]
    #[test]
    fn mindmap_span_color_reset_is_explicit_and_keeps_inline_inheritance_priority() {
        let fragment = "<em style='color:red'><span>Root</span></em>";
        let default_facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
        let mindmap_facts = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(fragment);
        assert_eq!(default_facts.inherited_color_run_count(), 0);
        assert_eq!(mindmap_facts.inherited_color_run_count(), 1);

        for style in [
            "color:inherit",
            "color:currentColor",
            "color:unset",
            "color:inherit !important;color:blue",
            "color:blue;color:inherit",
        ] {
            let fragment = format!("<em style='color:red'><span style='{style}'>Root</span></em>");
            let facts = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(&fragment);
            assert_eq!(facts.inherited_color_run_count(), 0, "{style}");
            assert_eq!(facts.unverified_portable_color_run_count(), 0, "{style}");
        }

        let local = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(
            "<em style='color:red'><span style='color:blue'>Root</span></em>",
        );
        assert_eq!(local.inherited_color_run_count(), 0);
        assert_eq!(local.unverified_portable_color_run_count(), 0);
        let nested = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(
            "<em style='color:red;font-family:monospace'><span><b>Root</b></span></em>",
        );
        assert_eq!(nested.inherited_color_run_count(), 1);
        assert_eq!(nested.source_owned_font_family_run_count(), 1);

        let unknown = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(
            "<em style='color:red'><span class='host-ink'>Root</span></em>",
        );
        assert_eq!(unknown.inherited_color_run_count(), 0);
        assert_eq!(unknown.unverified_portable_color_run_count(), 1);
        let host_ancestor = VisibleTextStyleFacts::from_mindmap_xhtml_fragment(
            "<em class='host-ink'><span>Root</span></em>",
        );
        assert_eq!(host_ancestor.inherited_color_run_count(), 0);
        assert_eq!(host_ancestor.unverified_portable_color_run_count(), 1);
    }

    #[test]
    fn quoted_semicolon_font_family_is_unmeasured_source_text() {
        let facts = VisibleTextStyleFacts::from_xhtml_fragment(
            r#"<span style='font-family:"a;b",sans-serif'>Owned</span>"#,
        );

        assert_eq!(facts.inherited_font_family_run_count(), 0);
        assert_eq!(facts.unverified_font_family_run_count(), 1);
    }

    #[test]
    fn font_shorthand_and_face_attribute_are_unmeasured_source_text() {
        for fragment in [
            "<span style='font:16px DescendantOwned'>Shorthand</span>",
            "<span style='all:initial'>Reset</span>",
            "<font face='DescendantOwned'>Face</font>",
            "<font face='DescendantOwned'><span style='font-family:inherit'>Nested</span></font>",
            "<span class='host' style='font-family:inherit'>Class</span>",
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(facts.inherited_font_family_run_count(), 0, "{fragment}");
            assert_eq!(facts.unverified_font_family_run_count(), 1, "{fragment}");
        }

        for fragment in [
            "<font face='DescendantOwned' style='font-family:inherit'>Inherited</font>",
            "<span class='host' style='font-family:inherit !important'>Important</span>",
            "<font face=''>Empty</font>",
            "<span style='all:red'>Invalid</span>",
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(facts.inherited_font_family_run_count(), 1, "{fragment}");
            assert_eq!(facts.unverified_font_family_run_count(), 0, "{fragment}");
        }
    }

    #[test]
    fn visible_text_facts_track_font_size_ownership() {
        for fragment in [
            "<span style='font-size:40px'>Sized</span>",
            "<span style='font:40px Source Sans'>Shorthand</span>",
            "<span style='all:initial'>Reset</span>",
            "<span style='font-size:/* host comment */40px'>Comment</span>",
            "<span class='host'>Class</span>",
            "<span class='host' style='font-size:inherit'>Class</span>",
            "<font size='7'>Legacy</font>",
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(facts.inherited_font_size_run_count(), 0, "{fragment}");
            assert_eq!(facts.unverified_font_size_run_count(), 1, "{fragment}");
        }

        for fragment in [
            "<span style='font-size:inherit'>Inherited</span>",
            "<span style='font-size:unset'>Unset</span>",
            "<span class='host' style='font-size:inherit !important'>Class</span>",
            "<font size='7' style='font-size:inherit'>Legacy</font>",
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(facts.inherited_font_size_run_count(), 1, "{fragment}");
            assert_eq!(facts.unverified_font_size_run_count(), 0, "{fragment}");
        }

        let nested = VisibleTextStyleFacts::from_xhtml_fragment(
            "<span style='font-size:40px'><b style='font-size:inherit'>Owned</b></span>",
        );
        assert_eq!(nested.inherited_font_size_run_count(), 0);
        assert_eq!(nested.unverified_font_size_run_count(), 1);

        for fragment in [
            "<span style='font-size:red'>Invalid</span>",
            "<span style='font:red'>Invalid</span>",
            "<span style='font:16px'>Invalid</span>",
            "<span style='all:red'>Invalid</span>",
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(facts.inherited_font_size_run_count(), 1, "{fragment}");
            assert_eq!(facts.unverified_font_size_run_count(), 0, "{fragment}");
        }
    }

    #[test]
    fn svg_markdown_projection_treats_inline_html_as_inherited_visible_text() {
        let facts = VisibleTextStyleFacts::from_svg_markdown_projection(
            r#"<span style="font-family:Owned;color:red">Visible</span>"#,
        );

        assert!(facts.has_visible_runs());
        assert_eq!(facts.inherited_color_run_count(), 1);
        assert_eq!(facts.inherited_font_family_run_count(), 1);
        assert_eq!(facts.unverified_font_family_run_count(), 0);
        assert_eq!(facts.inherited_font_size_run_count(), 1);
        assert_eq!(facts.unverified_font_size_run_count(), 0);
    }

    #[test]
    fn author_weight_style_and_stylesheets_keep_source_ownership() {
        for (fragment, weight, style) in [
            ("<span>Inherited</span>", 0, 0),
            ("<b>Weight</b>", 1, 0),
            ("<em>Style</em>", 0, 1),
            ("<cite>Style</cite>", 0, 1),
            ("<dfn>Style</dfn>", 0, 1),
            ("<var>Style</var>", 0, 1),
            ("<cite style='font-style:inherit'>Inherited</cite>", 0, 0),
            ("<span style='font-weight:700'>Weight</span>", 1, 0),
            ("<span style='font-style:italic'>Style</span>", 0, 1),
            ("<span style='all:initial'>Reset</span>", 1, 1),
            ("<span class='host'>Unknown</span>", 1, 1),
            ("<b style='font-weight:INHERIT'>Inherited</b>", 0, 0),
            (
                "<style>span{font-weight:700}</style><span>Unknown</span>",
                1,
                1,
            ),
        ] {
            let facts = VisibleTextStyleFacts::from_xhtml_fragment(fragment);
            assert_eq!(
                facts.unverified_font_weight_run_count(),
                weight,
                "{fragment}"
            );
            assert_eq!(facts.unverified_font_style_run_count(), style, "{fragment}");
        }
    }
}
