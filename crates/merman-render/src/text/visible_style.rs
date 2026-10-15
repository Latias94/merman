use super::trim_html_collapsible_ascii_whitespace;

#[cfg(test)]
thread_local! {
    static FACT_COLLECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Sufficient ownership facts for paint, typography, and applicability consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleTextStyleFacts {
    parse_valid: bool,
    visible_runs: usize,
    inherited_color_runs: usize,
    unverified_color_runs: usize,
    unverified_portable_color_runs: usize,
    inherited_font_family_runs: usize,
    inherited_font_size_runs: usize,
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
    UnmeasuredSource,
}

#[derive(Clone, Copy)]
struct VisibleTextOwners {
    color: VisibleTextColorOwner,
    font_family: VisibleTextTypographyOwner,
    font_size: VisibleTextTypographyOwner,
}

impl VisibleTextOwners {
    const INHERITED: Self = Self {
        color: VisibleTextColorOwner::Inherited,
        font_family: VisibleTextTypographyOwner::Inherited,
        font_size: VisibleTextTypographyOwner::Inherited,
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
            visible_runs: 0,
            inherited_color_runs: 0,
            unverified_color_runs: 0,
            unverified_portable_color_runs: 0,
            inherited_font_family_runs: 0,
            inherited_font_size_runs: 0,
        }
    }

    pub(crate) const fn not_requested() -> Self {
        Self::empty(false)
    }

    pub(crate) fn from_xhtml_fragment(fragment: &str) -> Self {
        #[cfg(test)]
        FACT_COLLECTIONS.set(FACT_COLLECTIONS.get() + 1);
        let wrapped = format!("<merman-visible-text>{fragment}</merman-visible-text>");
        let Ok(document) = crate::xml_dom::Document::parse(&wrapped) else {
            return Self::empty(false);
        };
        let mut facts = Self::empty(true);
        collect_visible_facts(document.root_element(), &mut facts);
        facts
    }

    pub(crate) fn plain_text(text: &str) -> Self {
        #[cfg(test)]
        FACT_COLLECTIONS.set(FACT_COLLECTIONS.get() + 1);
        let mut facts = Self::empty(true);
        if !trim_html_collapsible_ascii_whitespace(text).is_empty() {
            facts.observe(VisibleTextOwners::INHERITED);
        }
        facts
    }

    /// Returns the ownership facts for Mermaid's SVG Markdown projection.
    ///
    /// SVG label writers emit Markdown tokens as escaped text and only vary weight/style. Inline
    /// HTML attributes therefore remain visible text and cannot own `color` or `font-family`.
    pub(crate) fn from_svg_markdown_projection(markdown: &str) -> Self {
        let has_visible_text = super::mermaid_markdown_to_lines(markdown, true)
            .iter()
            .flatten()
            .any(|(word, _)| !trim_html_collapsible_ascii_whitespace(word).is_empty());
        Self::plain_text(if has_visible_text { markdown } else { "" })
    }

    pub(crate) fn with_unmeasured_typography(mut self, font_family: bool, font_size: bool) -> Self {
        if font_family {
            self.inherited_font_family_runs = 0;
        }
        if font_size {
            self.inherited_font_size_runs = 0;
        }
        self
    }

    pub(crate) const fn parse_valid(&self) -> bool {
        self.parse_valid
    }
    pub(crate) const fn has_visible_runs(&self) -> bool {
        self.visible_runs != 0
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
    pub(crate) const fn inherited_font_size_run_count(&self) -> usize {
        self.inherited_font_size_runs
    }
    pub(crate) const fn unverified_font_size_run_count(&self) -> usize {
        self.visible_runs - self.inherited_font_size_runs
    }
    #[cfg(test)]
    fn non_inherited_color_run_count(&self) -> usize {
        self.visible_runs - self.inherited_color_runs
    }

    fn observe(&mut self, owners: VisibleTextOwners) {
        self.visible_runs += 1;
        self.inherited_color_runs += usize::from(owners.color == VisibleTextColorOwner::Inherited);
        self.unverified_color_runs += usize::from(owners.color == VisibleTextColorOwner::CssClass);
        self.unverified_portable_color_runs += usize::from(matches!(
            owners.color,
            VisibleTextColorOwner::CssClass | VisibleTextColorOwner::Inline { portable: false }
        ));
        self.inherited_font_family_runs +=
            usize::from(owners.font_family == VisibleTextTypographyOwner::Inherited);
        self.inherited_font_size_runs +=
            usize::from(owners.font_size == VisibleTextTypographyOwner::Inherited);
    }
}

fn collect_visible_facts(root: crate::xml_dom::Node<'_, '_>, facts: &mut VisibleTextStyleFacts) {
    // roxmltree's descendants are a flat preorder range. Each ancestor state is pushed and
    // popped once, retaining inheritance without recursion or repeated ancestor scans.
    let mut parents = Vec::<(crate::xml_dom::NodeId, VisibleTextOwners)>::new();
    for node in root.descendants() {
        let parent = node.parent().map(|parent| parent.id());
        while parents.last().map(|(id, _)| *id) != parent && !parents.is_empty() {
            parents.pop();
        }
        let inherited = parents
            .last()
            .map_or(VisibleTextOwners::INHERITED, |(_, owners)| *owners);
        let owners = if node.is_element() {
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
                inherited.color
            };
            let local_family = if has_css_class || has_font_face {
                VisibleTextTypographyOwner::UnmeasuredSource
            } else {
                inherited.font_family
            };
            let local_size = if has_css_class || has_font_size {
                VisibleTextTypographyOwner::UnmeasuredSource
            } else {
                inherited.font_size
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
            }
        } else {
            inherited
        };
        if node.is_element() && is_fontawesome_icon(node) {
            facts.observe(owners);
        }
        if node.is_text()
            && node
                .text()
                .is_some_and(|text| !trim_html_collapsible_ascii_whitespace(text).is_empty())
        {
            facts.observe(owners);
        }
        if node.is_element() {
            parents.push((node.id(), owners));
        }
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
    let mut winner: Option<(bool, &str)> = None;
    crate::mermaid_style::visit_parsed_style_declarations(style, |declaration| {
        if declaration.property() != "color"
            || !crate::mermaid_style::is_safe_browser_css_color_value(declaration.value())
        {
            return;
        }
        if winner.is_none_or(|(important, _)| declaration.important() || !important) {
            winner = Some((declaration.important(), declaration.value()));
        }
    });
    let Some((important, value)) = winner else {
        return *local;
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
}
