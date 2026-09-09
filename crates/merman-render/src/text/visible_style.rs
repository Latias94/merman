use super::trim_html_collapsible_ascii_whitespace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleTextStyleFacts {
    parse_valid: bool,
    runs: Box<[VisibleTextRunStyleFact]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleTextRunStyleFact {
    content: Box<str>,
    kind: VisibleTextRunKind,
    color_owner: VisibleTextColorOwner,
    font_family_owner: VisibleTextTypographyOwner,
    font_size_owner: VisibleTextTypographyOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleTextRunKind {
    Text,
    FontAwesomeIcon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VisibleTextColorOwner {
    Inherited,
    CssClass,
    Inline(Box<str>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VisibleTextTypographyOwner {
    Inherited,
    UnmeasuredSource,
}

impl VisibleTextStyleFacts {
    pub(crate) fn from_xhtml_fragment(fragment: &str) -> Self {
        let wrapped = format!("<merman-visible-text>{fragment}</merman-visible-text>");
        let Ok(document) = roxmltree::Document::parse(&wrapped) else {
            return Self {
                parse_valid: false,
                runs: Box::default(),
            };
        };
        let mut runs = Vec::new();
        collect_visible_runs(
            document.root_element(),
            &VisibleTextColorOwner::Inherited,
            &VisibleTextTypographyOwner::Inherited,
            &VisibleTextTypographyOwner::Inherited,
            &mut runs,
        );
        Self {
            parse_valid: true,
            runs: runs.into_boxed_slice(),
        }
    }

    pub(crate) fn plain_text(text: &str) -> Self {
        let runs = (!trim_html_collapsible_ascii_whitespace(text).is_empty())
            .then(|| VisibleTextRunStyleFact {
                content: text.into(),
                kind: VisibleTextRunKind::Text,
                color_owner: VisibleTextColorOwner::Inherited,
                font_family_owner: VisibleTextTypographyOwner::Inherited,
                font_size_owner: VisibleTextTypographyOwner::Inherited,
            })
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            parse_valid: true,
            runs,
        }
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

    pub(crate) const fn parse_valid(&self) -> bool {
        self.parse_valid
    }

    pub(crate) fn has_visible_runs(&self) -> bool {
        !self.runs.is_empty()
    }

    pub(crate) fn visible_run_count(&self) -> usize {
        self.runs.len()
    }

    pub(crate) fn inherited_color_run_count(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.color_owner, VisibleTextColorOwner::Inherited))
            .count()
    }

    /// CSS classes may override inherited color without a locally known declaration.
    pub(crate) fn unverified_color_run_count(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.color_owner, VisibleTextColorOwner::CssClass))
            .count()
    }

    pub(crate) fn inherited_font_family_run_count(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.font_family_owner, VisibleTextTypographyOwner::Inherited))
            .count()
    }

    pub(crate) fn unverified_font_family_run_count(&self) -> usize {
        self.runs.len() - self.inherited_font_family_run_count()
    }

    pub(crate) fn inherited_font_size_run_count(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.font_size_owner, VisibleTextTypographyOwner::Inherited))
            .count()
    }

    pub(crate) fn unverified_font_size_run_count(&self) -> usize {
        self.runs.len() - self.inherited_font_size_run_count()
    }

    #[cfg(test)]
    fn non_inherited_color_run_count(&self) -> usize {
        self.runs.len() - self.inherited_color_run_count()
    }
}

fn collect_visible_runs(
    node: roxmltree::Node<'_, '_>,
    inherited_color_owner: &VisibleTextColorOwner,
    inherited_font_family_owner: &VisibleTextTypographyOwner,
    inherited_font_size_owner: &VisibleTextTypographyOwner,
    runs: &mut Vec<VisibleTextRunStyleFact>,
) {
    let (color_owner, font_family_owner, font_size_owner) = if node.is_element() {
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
        let local_color_owner = if has_css_class {
            &VisibleTextColorOwner::CssClass
        } else {
            inherited_color_owner
        };
        let local_font_family_owner = if has_css_class || has_font_face {
            &VisibleTextTypographyOwner::UnmeasuredSource
        } else {
            inherited_font_family_owner
        };
        let local_font_size_owner = if has_css_class || has_font_size {
            &VisibleTextTypographyOwner::UnmeasuredSource
        } else {
            inherited_font_size_owner
        };
        (
            inline_color_owner(node.attribute("style"), local_color_owner),
            inline_font_family_owner(
                node.attribute("style"),
                inherited_font_family_owner,
                local_font_family_owner,
                has_css_class,
            ),
            inline_font_size_owner(
                node.attribute("style"),
                inherited_font_size_owner,
                local_font_size_owner,
                has_css_class,
            ),
        )
    } else {
        (
            inherited_color_owner.clone(),
            inherited_font_family_owner.clone(),
            inherited_font_size_owner.clone(),
        )
    };

    if node.is_element() && is_fontawesome_icon(node) {
        runs.push(VisibleTextRunStyleFact {
            content: node.attribute("class").unwrap_or_default().into(),
            kind: VisibleTextRunKind::FontAwesomeIcon,
            color_owner: color_owner.clone(),
            font_family_owner: font_family_owner.clone(),
            font_size_owner: font_size_owner.clone(),
        });
    }

    if let Some(text) = node.text().filter(|_| node.is_text())
        && !trim_html_collapsible_ascii_whitespace(text).is_empty()
    {
        runs.push(VisibleTextRunStyleFact {
            content: text.into(),
            kind: VisibleTextRunKind::Text,
            color_owner: color_owner.clone(),
            font_family_owner: font_family_owner.clone(),
            font_size_owner: font_size_owner.clone(),
        });
    }

    for child in node.children() {
        collect_visible_runs(
            child,
            &color_owner,
            &font_family_owner,
            &font_size_owner,
            runs,
        );
    }
}

fn inline_color_owner(
    style: Option<&str>,
    inherited: &VisibleTextColorOwner,
) -> VisibleTextColorOwner {
    let Some(style) = style else {
        return inherited.clone();
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
    let Some((_, value)) = winner else {
        return inherited.clone();
    };
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "currentcolor" | "inherit" | "unset"
    ) {
        inherited.clone()
    } else {
        VisibleTextColorOwner::Inline(value.trim().into())
    }
}

fn inline_font_family_owner(
    style: Option<&str>,
    parent: &VisibleTextTypographyOwner,
    local: &VisibleTextTypographyOwner,
    has_css_class: bool,
) -> VisibleTextTypographyOwner {
    match style.and_then(crate::mermaid_style::css_font_family_override) {
        None => local.clone(),
        Some(override_)
            if override_.ownership() == crate::mermaid_style::CssFontFamilyOwnership::Inherited
                && (!has_css_class || override_.important()) =>
        {
            parent.clone()
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
        None => local.clone(),
        Some(override_)
            if override_.ownership() == crate::mermaid_style::CssFontSizeOwnership::Inherited
                && (!has_css_class || override_.important()) =>
        {
            parent.clone()
        }
        Some(_) => VisibleTextTypographyOwner::UnmeasuredSource,
    }
}

fn is_fontawesome_icon(node: roxmltree::Node<'_, '_>) -> bool {
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
