use super::*;
use crate::Result;

#[derive(Debug, Clone, Default)]
pub(crate) struct TreemapNodeVisual {
    pub(crate) fill: String,
    pub(crate) stroke: String,
    pub(crate) rect_style: String,
    pub(crate) label_fill: String,
    pub(crate) value_fill: String,
    pub(crate) fitted_label_fill: String,
    pub(crate) fitted_value_fill: String,
    pub(crate) label_styles_suffix: String,
    pub(crate) label_styles_without_font_size_suffix: String,
    pub(crate) text_fill_ownership: TreemapTextFillOwnership,
    pub(crate) text_fill_without_font_size_ownership: TreemapTextFillOwnership,
    pub(super) label_paint: Option<DirectPaintExpectation>,
    pub(super) value_paint: Option<DirectPaintExpectation>,
    pub(super) node_label_winner: bool,
    pub(super) label_palette_owned: bool,
    pub(super) ordinal: usize,
}

impl TreemapNodeVisual {
    fn retained_bytes(&self) -> Option<usize> {
        [
            &self.fill,
            &self.stroke,
            &self.rect_style,
            &self.label_fill,
            &self.value_fill,
            &self.fitted_label_fill,
            &self.fitted_value_fill,
            &self.label_styles_suffix,
            &self.label_styles_without_font_size_suffix,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, value| bytes.checked_add(value.len()))
        .and_then(|bytes| {
            bytes.checked_add(
                self.label_paint
                    .as_ref()
                    .map_or(0, |paint| paint.css().len()),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                self.value_paint
                    .as_ref()
                    .map_or(0, |paint| paint.css().len()),
            )
        })
    }
}

#[derive(Default)]
struct OrdinalScale {
    range: Vec<String>,
    domain: std::collections::HashMap<String, usize>,
}

impl OrdinalScale {
    fn get(&mut self, key: &str) -> String {
        let idx = if let Some(idx) = self.domain.get(key).copied() {
            idx
        } else {
            let idx = self.domain.len();
            self.domain.insert(key.to_string(), idx);
            idx
        };
        if self.range.is_empty() {
            return String::new();
        }
        self.range[idx % self.range.len()].clone()
    }
}

fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let Some(idx) = haystack.find(needle) else {
        return haystack.to_string();
    };
    let mut out = String::with_capacity(haystack.len() - needle.len() + replacement.len());
    out.push_str(&haystack[..idx]);
    out.push_str(replacement);
    out.push_str(&haystack[idx + needle.len()..]);
    out
}

#[derive(Default)]
struct OrderedMap {
    order: Vec<(String, String)>,
    idx: std::collections::HashMap<String, usize>,
}

impl OrderedMap {
    fn set(&mut self, k: &str, v: &str) {
        if k.is_empty() {
            return;
        }
        if let Some(&i) = self.idx.get(k) {
            self.order[i].1 = v.to_string();
            return;
        }
        self.idx.insert(k.to_string(), self.order.len());
        self.order.push((k.to_string(), v.to_string()));
    }
}

fn treemap_is_label_style(key: &str) -> bool {
    matches!(
        key.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

#[derive(Debug, Clone, Default)]
struct TreemapCompiledStyles {
    label_styles: String,
    label_styles_without_font_size: String,
    text_fill_ownership: crate::treemap::TreemapTextFillOwnership,
    text_fill_without_font_size_ownership: crate::treemap::TreemapTextFillOwnership,
    node_styles: String,
    border_styles: Vec<String>,
    fill_ownership: TreemapTextFillOwnership,
    stroke_ownership: TreemapTextFillOwnership,
}

fn treemap_styles2_string(
    css_compiled_styles: &[String],
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<TreemapCompiledStyles> {
    // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
    // - preserve insertion order of the first occurrence of a key
    // - later occurrences override values, without changing order
    // - tolerate tokens without `:` (JS `split(':')` yields `value = undefined`)
    let mut m = OrderedMap::default();

    for entry in css_compiled_styles {
        work_meter.charge(1usize.saturating_add(entry.len().div_ceil(64)))?;
        let mut checkpoint = || work_meter.checkpoint(OperationPhase::Layout);
        crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
            entry,
            &mut checkpoint,
            |boundary| {
                work_meter.charge(1)?;
                let raw = boundary.raw();
                let s = raw
                    .trim()
                    .strip_suffix(';')
                    .unwrap_or_else(|| raw.trim())
                    .trim();
                if s.is_empty() {
                    return Ok(true);
                }
                let (k, v) =
                    if let Some(declaration) = crate::mermaid_style::parse_style_declaration(raw) {
                        (declaration.property_source(), declaration.source_value())
                    } else if let Some((k, v)) = s.split_once(':') {
                        (k.trim(), v.trim())
                    } else {
                        (s.trim(), "")
                    };
                m.set(k, v);
                Ok(true)
            },
        )?;
    }

    let mut label_styles: Vec<String> = Vec::new();
    let mut label_styles_without_font_size: Vec<String> = Vec::new();
    let mut node_styles: Vec<String> = Vec::new();
    let mut border_styles: Vec<String> = Vec::new();
    let mut text_fill_ownership = crate::treemap::TreemapTextFillOwnership::Generated;
    let mut text_fill_without_font_size_ownership = text_fill_ownership;
    let mut label_styles_contain_color = false;
    let mut label_styles_without_font_size_contain_color = false;
    let mut fill_ownership = TreemapTextFillOwnership::Generated;
    let mut stroke_ownership = TreemapTextFillOwnership::Generated;

    for (k, v) in &m.order {
        if v.is_empty() {
            continue;
        }
        work_meter.charge(
            1usize
                .saturating_add(k.len().div_ceil(64))
                .saturating_add(v.len().div_ceil(64)),
        )?;
        let decl = format!("{k}:{v}");
        let decl_imp = format!("{decl} !important");
        if treemap_is_label_style(k) {
            if k == "color" {
                // The emitted suffix converts this declaration to fill and appends !important.
                // Dynamic or invalid values cannot prove that the generated fill is shadowed.
                text_fill_ownership = if crate::mermaid_style::is_supported_css_color_value(v)
                    || v.eq_ignore_ascii_case("none")
                {
                    crate::treemap::TreemapTextFillOwnership::SourceOwned
                } else {
                    crate::treemap::TreemapTextFillOwnership::Unverified
                };
                text_fill_without_font_size_ownership = text_fill_ownership;
                // Preserve the writer's first-substring replacement, including values that
                // contain `color:` before the real declaration. Such output is not proof of
                // a source-owned fill; the two suffixes can have different first matches.
                if label_styles_contain_color {
                    text_fill_ownership = crate::treemap::TreemapTextFillOwnership::Unverified;
                }
                if label_styles_without_font_size_contain_color {
                    text_fill_without_font_size_ownership =
                        crate::treemap::TreemapTextFillOwnership::Unverified;
                }
            }
            let contains_color = decl_imp.contains("color:");
            label_styles_contain_color |= contains_color;
            label_styles.push(decl_imp.clone());
            if k.trim() != "font-size" {
                label_styles_without_font_size_contain_color |= contains_color;
                label_styles_without_font_size.push(decl_imp);
            }
        } else {
            let ownership = if crate::mermaid_style::is_supported_css_color_value(v)
                || v.eq_ignore_ascii_case("none")
            {
                TreemapTextFillOwnership::SourceOwned
            } else {
                TreemapTextFillOwnership::Unverified
            };
            let declaration = crate::mermaid_style::parse_style_declaration(&decl);
            let property = declaration
                .as_ref()
                .map_or(k.as_str(), |declaration| declaration.property());
            if property.eq_ignore_ascii_case("fill") {
                fill_ownership = ownership;
            }
            if property.eq_ignore_ascii_case("stroke") {
                stroke_ownership = ownership;
            }
            node_styles.push(decl_imp.clone());
            if k.contains("stroke") {
                border_styles.push(decl_imp);
            }
        }
    }

    Ok(TreemapCompiledStyles {
        label_styles: label_styles.join(";"),
        label_styles_without_font_size: label_styles_without_font_size.join(";"),
        text_fill_ownership,
        text_fill_without_font_size_ownership,
        node_styles: node_styles.join(";"),
        border_styles,
        fill_ownership,
        stroke_ownership,
    })
}

impl TreemapTypographyThemePlan {
    pub(super) fn prepare_node_visuals(
        mut self,
        layout: &TreemapDiagramLayout,
        work: &std::sync::Arc<OperationWorkMeter>,
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
    ) -> crate::Result<Self> {
        let mut retained = 0usize;
        let mut section_visuals = Vec::with_capacity(layout.sections.len());
        let mut leaf_visuals = Vec::with_capacity(layout.leaves.len());
        let mut colors = OrdinalScale::default();
        colors.range.push("transparent".to_owned());
        colors.range.extend(self.css.color_scale.iter().cloned());
        let mut peers = OrdinalScale::default();
        peers.range.push("transparent".to_owned());
        peers
            .range
            .extend(self.css.color_scale_peer.iter().cloned());
        let mut labels = OrdinalScale::default();
        labels
            .range
            .extend(self.css.color_scale_label.iter().cloned());
        let mut node_rules = BTreeMap::<usize, TreemapNodeRuleObservation>::new();
        let mut node_facets = BTreeMap::new();
        if let Some(theme) = theme {
            for route in theme.family_mechanism_routes() {
                match route.mechanism() {
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Node,
                        rule_index,
                        facet,
                        ..
                    } => {
                        node_facets.insert(
                            (rule_index, resolved_style_property_for_facet(facet)),
                            facet,
                        );
                        node_rules.entry(rule_index).or_default();
                        self.node_paint_requested = true;
                    }
                    FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Node,
                    }
                    | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Node,
                        ..
                    } => self.node_paint_requested = true,
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text | ThemeTarget::NodeLabel,
                        ..
                    }
                    | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text | ThemeTarget::NodeLabel,
                    }
                    | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text | ThemeTarget::NodeLabel,
                        ..
                    } => self.node_label_requested = true,
                    _ => {}
                }
            }
        }
        let mut series = std::collections::HashMap::<String, usize>::new();
        let mut node_ordinal = 0;
        let mut node_terminals = Vec::new();
        let mut palette_capabilities = BTreeSet::new();
        let mut palette_unverified = false;
        // Keep the original writer traversal and the three independent first-seen domains.
        for section in &layout.sections {
            work.charge(1)?;
            let fill = colors.get(&section.name);
            let stroke = peers.get(&section.name);
            let source = treemap_styles2_string(
                section.css_compiled_styles.as_deref().unwrap_or_default(),
                work,
            )?;
            let rect_style = if section.depth == 0 {
                "display: none;".to_owned()
            } else {
                format!("{};{}", source.node_styles, source.border_styles.join(";"))
            };
            let baseline = if section.depth == 0 {
                String::new()
            } else {
                labels.get(&section.name)
            };
            let label_palette_owned = theme.is_some()
                && section.depth != 0
                && label_palette_source_owned(
                    config,
                    labels.domain.get(&section.name).copied().unwrap_or(0),
                );
            let label_fill = self
                .label_text_fill_css()
                .filter(|_| !label_palette_owned)
                .map_or_else(|| baseline.clone(), str::to_owned);
            let value_fill = if section.depth == 0 {
                String::new()
            } else {
                self.value_text_fill_css()
                    .filter(|_| !label_palette_owned)
                    .map_or(baseline, str::to_owned)
            };
            let mut visual = TreemapNodeVisual {
                fill,
                stroke,
                rect_style,
                label_fill,
                value_fill,
                label_styles_suffix: replace_first(&source.label_styles, "color:", "fill:"),
                text_fill_ownership: source.text_fill_ownership,
                text_fill_without_font_size_ownership: source.text_fill_without_font_size_ownership,
                label_palette_owned,
                ..Default::default()
            };
            if section.depth != 0 {
                node_ordinal += 1;
                visual.ordinal = node_ordinal;
                if let Some(theme) = theme {
                    let slot = colors.domain.get(&section.name).copied().unwrap_or(0);
                    let peer_slot = peers.domain.get(&section.name).copied().unwrap_or(0);
                    let next = series.len() + 1;
                    let series_ordinal = *series.entry(section.name.clone()).or_insert(next);
                    let fill_owner =
                        palette_source_owner(config, slot, "cScale", source.fill_ownership);
                    let stroke_owner = palette_source_owner(
                        config,
                        peer_slot,
                        "cScalePeer",
                        source.stroke_ownership,
                    );
                    palette_unverified |= lower_node_paint(
                        theme,
                        &mut visual,
                        node_ordinal,
                        series_ordinal,
                        [fill_owner, stroke_owner],
                        &node_facets,
                        &mut node_rules,
                        &mut palette_capabilities,
                        work,
                    )?;
                    node_terminals.push((
                        node_ordinal,
                        fill_owner == TreemapTextFillOwnership::SourceOwned,
                    ));
                    self.lower_node_label(theme, &mut visual, work)?;
                }
            }
            retained = retained
                .checked_add(
                    visual
                        .retained_bytes()
                        .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?,
                )
                .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?;
            section_visuals.push(visual);
        }
        for leaf in &layout.leaves {
            work.charge(1)?;
            let fill = colors.get(leaf.parent_name.as_deref().unwrap_or(&leaf.name));
            let source = treemap_styles2_string(
                leaf.css_compiled_styles.as_deref().unwrap_or_default(),
                work,
            )?;
            let baseline = self.css.readable_leaf_label_fill(
                &fill,
                &source.node_styles,
                labels.get(&leaf.name),
            );
            let label_palette_owned = theme.is_some()
                && label_palette_source_owned(
                    config,
                    labels.domain.get(&leaf.name).copied().unwrap_or(0),
                );
            let label_fill = self
                .label_text_fill_css()
                .filter(|_| !label_palette_owned)
                .map_or_else(|| baseline.clone(), str::to_owned);
            let value_fill = self
                .value_text_fill_css()
                .filter(|_| !label_palette_owned)
                .map_or(baseline, str::to_owned);
            let mut visual = TreemapNodeVisual {
                stroke: fill.clone(),
                fill,
                rect_style: source.node_styles,
                fitted_label_fill: crate::svg::cssom_color_value(&label_fill),
                fitted_value_fill: crate::svg::cssom_color_value(&value_fill),
                label_fill,
                value_fill,
                label_styles_suffix: replace_first(&source.label_styles, "color:", "fill:"),
                label_styles_without_font_size_suffix: replace_first(
                    &source.label_styles_without_font_size,
                    "color:",
                    "fill:",
                ),
                text_fill_ownership: source.text_fill_ownership,
                text_fill_without_font_size_ownership: source.text_fill_without_font_size_ownership,
                label_palette_owned,
                ..Default::default()
            };
            node_ordinal += 1;
            visual.ordinal = node_ordinal;
            if let Some(theme) = theme {
                let category = leaf.parent_name.as_deref().unwrap_or(&leaf.name);
                let next = series.len() + 1;
                let series_ordinal = *series.entry(category.to_owned()).or_insert(next);
                let slot = colors.domain.get(category).copied().unwrap_or(0);
                let fill_owner =
                    palette_source_owner(config, slot, "cScale", source.fill_ownership);
                let stroke_owner =
                    palette_source_owner(config, slot, "cScale", source.stroke_ownership);
                palette_unverified |= lower_node_paint(
                    theme,
                    &mut visual,
                    node_ordinal,
                    series_ordinal,
                    [fill_owner, stroke_owner],
                    &node_facets,
                    &mut node_rules,
                    &mut palette_capabilities,
                    work,
                )?;
                node_terminals.push((
                    node_ordinal,
                    fill_owner == TreemapTextFillOwnership::SourceOwned,
                ));
                self.lower_node_label(theme, &mut visual, work)?;
            }
            retained = retained
                .checked_add(
                    visual
                        .retained_bytes()
                        .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?,
                )
                .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?;
            leaf_visuals.push(visual);
        }
        work.charge(retained)?;
        // Charge only selected, folded fields, then transfer the temporary visuals into
        // their existing source owners after resource admission succeeds.
        let label_receipt_bytes = if self.node_label_requested {
            layout
                .sections
                .len()
                .checked_add(layout.leaves.len())
                .and_then(|count| count.checked_mul(1 + usize::from(layout.show_values)))
                .and_then(|count| count.checked_mul(std::mem::size_of::<TreemapLabelTerminal>()))
                .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?
        } else {
            0
        };
        let reservation = work.reserve_prepared_text_retained_bytes(
            retained
                .checked_add(label_receipt_bytes)
                .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?,
        )?;
        for (source, visual) in self.section_source_styles.iter_mut().zip(section_visuals) {
            source.visual = visual;
        }
        for (source, visual) in self.leaf_source_styles.iter_mut().zip(leaf_visuals) {
            source.visual = visual;
        }
        self._visual_retained_reservation = Some(reservation);
        for (index, observation) in node_rules {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Node,
            };
            if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.capabilities.is_empty() {
                self.evidence.mark_not_applicable(key);
            } else {
                self.node_pending.insert(key, observation.capabilities);
            }
        }
        if let Some(theme) = theme {
            let owned = node_terminals
                .iter()
                .map(|(_, owned)| *owned)
                .collect::<Vec<_>>();
            reconcile_unsupported_terminal_domains(
                theme,
                &mut self.evidence,
                &[UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(node_ordinal, ThemeVariant::Default),
                )
                .with_source_owned_fill(&owned)],
                work,
            )?;
            if theme.ordinal_palette_disposition(ThemeTarget::Node)
                == Some(FamilyThemeDisposition::TypedAdapter)
            {
                let key = FamilyThemeMechanismKey::OrdinalPalette {
                    target: ThemeTarget::Node,
                };
                if palette_unverified {
                    self.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                } else if palette_capabilities.is_empty() {
                    self.evidence.mark_not_applicable(key);
                } else {
                    self.node_pending.insert(key, palette_capabilities);
                }
            }
        }
        Ok(self)
    }

    fn lower_node_label(
        &self,
        theme: &ResolvedDiagramTheme,
        visual: &mut TreemapNodeVisual,
        work: &OperationWorkMeter,
    ) -> crate::Result<()> {
        if !self.node_label_requested {
            return Ok(());
        }
        let style = theme.text_style_with_role_override(
            ThemeTarget::Text,
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            Some(visual.ordinal),
            work,
        )?;
        visual.node_label_winner = style
            .fill_resolution()
            .winner()
            .is_some_and(|origin| origin.target() == ThemeTarget::NodeLabel);
        if !visual.node_label_winner {
            return Ok(());
        }
        let paint = resolve_direct_static_fill(
            theme,
            &style,
            &[ThemeTarget::NodeLabel],
            DirectStaticSelectorDomain::Default,
        );
        for (value, fill, fitted, pending) in [
            (
                false,
                &mut visual.label_fill,
                &mut visual.fitted_label_fill,
                &mut visual.label_paint,
            ),
            (
                true,
                &mut visual.value_fill,
                &mut visual.fitted_value_fill,
                &mut visual.value_paint,
            ),
        ] {
            let config_owned = if value {
                self.value_config_owns_text_fill
            } else {
                self.label_config_owns_text_fill
            };
            if config_owned || visual.label_palette_owned {
                continue;
            }
            if let Some(paint) = &paint {
                *fill = paint.css().to_owned();
                *fitted = crate::svg::cssom_color_value(fill);
                *pending = Some(DirectPaintExpectation::new(
                    paint.rule_index(),
                    paint.css(),
                    paint.capability(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct TreemapNodeRuleObservation {
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

fn label_palette_source_owned(config: &PresentationConfig, slot: usize) -> bool {
    config.config_paint_overrides_typed_default(&format!("themeVariables.cScaleLabel{}", slot % 12))
}

fn palette_source_owner(
    config: &PresentationConfig,
    slot: usize,
    prefix: &str,
    source: TreemapTextFillOwnership,
) -> TreemapTextFillOwnership {
    if source != TreemapTextFillOwnership::Generated {
        return source;
    }
    // The body scales prepend the hidden root's transparent slot. Default Mermaid
    // deliberately ignores authored cScale/cScalePeer values for these body scales.
    let custom = crate::config::config_string(config.as_value(), &["theme"])
        .is_some_and(|name| name != "default");
    let slot = slot % 13;
    if custom
        && slot != 0
        && config
            .config_paint_overrides_typed_default(&format!("themeVariables.{prefix}{}", slot - 1))
    {
        TreemapTextFillOwnership::SourceOwned
    } else {
        TreemapTextFillOwnership::Generated
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Node winners retain their existing visual, source, rule and palette owners."
)]
fn lower_node_paint(
    theme: &ResolvedDiagramTheme,
    visual: &mut TreemapNodeVisual,
    ordinal: usize,
    series_ordinal: usize,
    owners: [TreemapTextFillOwnership; 2],
    facets: &BTreeMap<(usize, ResolvedStyleProperty), FamilyThemeRuleFacet>,
    observations: &mut BTreeMap<usize, TreemapNodeRuleObservation>,
    palette_capabilities: &mut BTreeSet<ThemeCapability>,
    work: &OperationWorkMeter,
) -> crate::Result<bool> {
    let style = theme.style_with_work_meter(
        ThemeTarget::Node,
        ThemeVariant::Default,
        Some(ordinal),
        work,
    )?;
    let fill = resolve_direct_static_fill(
        theme,
        &style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    );
    let stroke = crate::family::resolve_direct_static_stroke(
        theme,
        &style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    );
    for (index, css, paint) in [
        (0, &mut visual.fill, fill.as_ref()),
        (1, &mut visual.stroke, stroke.as_ref()),
    ] {
        if owners[index] == TreemapTextFillOwnership::Generated
            && let Some(paint) = paint
        {
            *css = paint.css().to_owned();
        }
    }
    work.charge(facets.len())?;
    for (property, origin) in style.winner_rule_properties() {
        let Some(facet) = facets.get(&(origin.rule_index(), property)) else {
            continue;
        };
        let owner_index = match property {
            ResolvedStyleProperty::Fill => Some(0),
            ResolvedStyleProperty::Stroke => Some(1),
            _ => None,
        };
        if owner_index.is_some_and(|index| owners[index] == TreemapTextFillOwnership::SourceOwned) {
            continue;
        }
        let observation = observations
            .get_mut(&origin.rule_index())
            .expect("registered Treemap Node rule");
        let paint = match property {
            ResolvedStyleProperty::Fill => fill.as_ref(),
            ResolvedStyleProperty::Stroke => stroke.as_ref(),
            _ => None,
        };
        if owner_index.is_some_and(|index| owners[index] == TreemapTextFillOwnership::Generated)
            && let Some(paint) = paint
            && paint.rule_index() == origin.rule_index()
        {
            observation.capabilities.insert(paint.capability());
        } else {
            observation
                .residual
                .get_or_insert(unsupported_residual_for_facet(*facet));
        }
    }
    if style.fill_resolution().specified().is_unspecified()
        && owners[0] == TreemapTextFillOwnership::Generated
        && theme.ordinal_palette_disposition(ThemeTarget::Node)
            == Some(FamilyThemeDisposition::TypedAdapter)
        && let Some(color) = theme.series_color(ThemeTarget::Node, series_ordinal)
    {
        visual.fill = color.as_css();
        palette_capabilities.insert(if color.is_transparent() {
            ThemeCapability::TransparentPaint
        } else {
            ThemeCapability::SolidPaint
        });
    }
    Ok(style.fill_resolution().specified().is_unspecified()
        && owners[0] == TreemapTextFillOwnership::Unverified)
}

#[derive(Debug)]
pub(super) struct TreemapLabelTerminal {
    pub(super) ordinal: usize,
    pub(super) source_owned: bool,
    pub(super) unverified: bool,
    pub(super) paint: Option<(usize, ThemeCapability)>,
}

pub(super) fn record_text_paint_evidence(
    theme: &ResolvedDiagramTheme,
    terminals: &[TreemapLabelTerminal],
    evidence: &mut FamilyThemeOutcomes,
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    let mut observations = BTreeMap::<(usize, ThemeTarget), TreemapNodeRuleObservation>::new();
    let mut facets = BTreeMap::new();
    for route in theme.family_mechanism_routes() {
        if let FamilyThemeMechanism::RuleFacet {
            target: target @ (ThemeTarget::Text | ThemeTarget::NodeLabel),
            rule_index,
            facet,
            ..
        } = route.mechanism()
        {
            facets.insert(
                (rule_index, resolved_style_property_for_facet(facet)),
                facet,
            );
            observations.entry((rule_index, target)).or_default();
        }
    }
    let mut palette_targets = BTreeSet::new();
    let mut effect_keys = BTreeSet::new();
    for terminal in terminals {
        let style = theme.text_style_with_role_override(
            ThemeTarget::Text,
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            Some(terminal.ordinal),
            work,
        )?;
        work.charge(facets.len())?;
        for (property, origin) in style.winner_rule_properties() {
            let Some(facet) = facets.get(&(origin.rule_index(), property)) else {
                continue;
            };
            if property == ResolvedStyleProperty::Fill && terminal.source_owned {
                continue;
            }
            let observation = observations
                .get_mut(&(origin.rule_index(), origin.target()))
                .expect("registered Treemap text rule");
            if property == ResolvedStyleProperty::Fill
                && !terminal.unverified
                && let Some((index, capability)) = terminal.paint
                && index == origin.rule_index()
            {
                observation.capabilities.insert(capability);
            } else {
                observation
                    .residual
                    .get_or_insert(unsupported_residual_for_facet(*facet));
            }
        }
        for target in [ThemeTarget::Text, ThemeTarget::NodeLabel] {
            if !terminal.source_owned
                && style.fill_resolution().specified().is_unspecified()
                && theme.series_color(target, terminal.ordinal).is_some()
            {
                palette_targets.insert(target);
            }
            if let Some(crate::diagram_theme::ResolvedThemeEffect::Binding { binding, .. }) =
                theme.resolve_effect(target, style.effect_resolution())
            {
                effect_keys.insert(FamilyThemeMechanismKey::EffectBinding {
                    target: binding.target(),
                    effect_id: binding.effect_id().to_owned(),
                });
            }
        }
    }
    for ((index, target), observation) in observations {
        let key = FamilyThemeMechanismKey::Rule { index, target };
        if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
    // Both label roles retain their node ordinal. Fitting can hide one role without
    // renumbering later nodes, and source ownership can differ between label and value.
    for route in theme.family_mechanism_routes() {
        let applicable = match route.mechanism() {
            FamilyThemeMechanism::OrdinalPalette {
                target: target @ (ThemeTarget::Text | ThemeTarget::NodeLabel),
            } => palette_targets.contains(&target),
            FamilyThemeMechanism::EffectBinding {
                target: ThemeTarget::Text | ThemeTarget::NodeLabel,
                ..
            } => effect_keys.contains(&theme.family_mechanism_key(*route)),
            _ => continue,
        };
        let key = theme.family_mechanism_key(*route);
        if applicable {
            let reason = if matches!(
                route.mechanism(),
                FamilyThemeMechanism::OrdinalPalette { .. }
            ) {
                FamilyThemeResidualReason::UnsupportedOrdinalPalette
            } else {
                FamilyThemeResidualReason::UnsupportedEffect
            };
            evidence.mark_residual(key, reason);
        } else {
            evidence.mark_not_applicable(key);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> TreemapDiagramLayout {
        serde_json::from_value(serde_json::json!({
            "title_height": 0, "width": 100, "height": 100, "use_max_width": true,
            "diagram_padding": 8, "show_values": true, "value_format": ",",
            "sections": [
                {"name":"root","depth":0,"value":10,"x0":0,"y0":0,"x1":100,"y1":100},
                {"name":"branch","depth":1,"value":10,"x0":0,"y0":0,"x1":100,"y1":100}
            ],
            "leaves": [
                {"name":"first","parent_name":"branch","value":5,"x0":0,"y0":0,"x1":50,"y1":100,
                 "css_compiled_styles":["color:#123456;font-size:20px;fill:#aabbcc", "color:#654321"]},
                {"name":"second","parent_name":"branch","value":5,"x0":50,"y0":0,"x1":100,"y1":100}
            ]
        })).unwrap()
    }

    #[test]
    fn visual_reservation_accepts_exact_retained_bytes_and_rejects_one_less() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].css_compiled_styles = None;
        let config = PresentationConfig::empty_object();
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let baseline = TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::clone(&meter),
        )
        .unwrap();
        let exact_bytes = meter.prepared_text_retained_bytes();
        drop(baseline);
        for (budget, succeeds) in [(exact_bytes, true), (exact_bytes - 1, false)] {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, budget)
                    .unwrap(),
            ));
            let result = TreemapTypographyThemePlan::resolve(
                None,
                &config,
                &layout,
                std::sync::Arc::clone(&meter),
            );
            if succeeds {
                let plan = result.expect("exact final retained-byte budget fits");
                assert_eq!(meter.prepared_text_retained_bytes(), exact_bytes);
                drop(plan);
            } else {
                assert!(matches!(
                    result,
                    Err(crate::Error::ResourceLimitExceeded(_))
                ));
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn visual_reservation_counts_folded_fields_and_ignores_unused_palette_slots() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].parent_name = None;
        layout.leaves[0].css_compiled_styles = Some(vec!["fill:red;".repeat(1000)]);
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "custom", "themeVariables": {"cScale11": "x".repeat(70_000)}
        }));
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 65_536)
                .unwrap(),
        ));
        let plan = TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::clone(&meter),
        )
        .expect("actual visual fits retained budget");
        let visual = plan.leaf_visual(0).unwrap();
        assert_eq!(visual.rect_style, "fill:red !important");
        assert_eq!(
            plan._visual_retained_reservation
                .as_ref()
                .unwrap()
                .retained_bytes(),
            visual.retained_bytes().unwrap()
        );
        assert!(meter.prepared_text_retained_bytes_peak() < 65_536);
        drop(plan);
        assert_eq!(
            meter.prepared_text_retained_bytes(),
            0,
            "artifact drop releases both reservations"
        );
    }

    #[test]
    fn prepared_visuals_keep_independent_ordinal_domains_and_fitted_source_variants() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "custom",
            "themeVariables": {"cScale0":"#111111", "cScale1":"#222222", "cScaleLabel0":"#333333", "cScaleLabel1":"#444444", "cScaleLabel2":"#555555"}
        }));
        let layout = layout();
        let plan = TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let root = plan.section_visual(0).unwrap();
        let branch = plan.section_visual(1).unwrap();
        let first = plan.leaf_visual(0).unwrap();
        let second = plan.leaf_visual(1).unwrap();
        assert_eq!(root.fill, "transparent");
        assert!(root.label_fill.is_empty());
        assert_eq!(branch.fill, "#111111");
        assert_eq!(branch.label_fill, "#333333");
        assert_eq!(first.fill, branch.fill);
        assert_eq!(second.fill, branch.fill);
        assert_eq!(first.label_fill, "#444444");
        assert_eq!(second.label_fill, "#555555");
        assert_eq!(
            first.label_styles_suffix,
            "fill:#654321 !important;font-size:20px !important"
        );
        assert_eq!(
            first.label_styles_without_font_size_suffix,
            "fill:#654321 !important"
        );
        assert_eq!(first.rect_style, "fill:#aabbcc !important");
        assert_eq!(
            first.text_fill_ownership,
            TreemapTextFillOwnership::SourceOwned
        );
        assert_eq!(
            first.text_fill_without_font_size_ownership,
            TreemapTextFillOwnership::SourceOwned
        );
        assert!(
            plan.terminal_receipt.get().is_none(),
            "preparation cannot seal actual emission"
        );
    }

    #[test]
    fn prepared_default_theme_keeps_fixed_fill_scale_and_explicit_label_scale() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": {"cScale0":"#111111", "cScaleLabel0":"#333333"}
        }));
        let layout = layout();
        let plan = TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let branch = plan.section_visual(1).unwrap();
        assert_eq!(branch.fill, "hsl(240, 100%, 76.2745098039%)");
        assert_eq!(branch.label_fill, "#333333");
        assert_eq!(plan.leaf_visual(0).unwrap().fill, branch.fill);
    }

    #[test]
    fn prepared_visuals_preserve_transparent_leaf_readability_fallback() {
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].parent_name = None;
        layout.leaves[0].css_compiled_styles = None;
        let config = crate::test_support::presentation_config(
            serde_json::json!({"themeVariables":{"cScaleLabel0":"#ffffff", "textColor":"#123456"}}),
        );
        let plan = TreemapTypographyThemePlan::resolve(
            None,
            &config,
            &layout,
            std::sync::Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let leaf = plan.leaf_visual(0).unwrap();
        assert_eq!(leaf.fill, "transparent");
        assert_eq!(leaf.label_fill, "#123456");
        assert_eq!(leaf.value_fill, "#123456");
    }

    #[test]
    fn body_palette_source_ownership_follows_the_wrapped_thirteen_slot_scale() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "custom", "themeVariables": {"cScale0": "#112233"}
        }));
        for (slot, expected) in [(0, false), (1, true), (12, false), (13, false), (14, true)] {
            assert_eq!(
                palette_source_owner(&config, slot, "cScale", TreemapTextFillOwnership::Generated)
                    == TreemapTextFillOwnership::SourceOwned,
                expected,
                "slot {slot}"
            );
        }
    }

    #[test]
    fn terminal_receipt_rejects_missing_or_wrong_node_and_label_paints() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch,
        };
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].css_compiled_styles = None;
        layout.show_values = false;
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap())
                                .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                        )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::TREEMAP);
        for (emit_node, wrong_node, wrong_label, succeeds) in [
            (true, false, false, true),
            (false, false, false, false),
            (true, true, false, false),
            (true, false, true, false),
        ] {
            let work = std::sync::Arc::new(OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let plan = TreemapTypographyThemePlan::resolve(
                Some(&theme),
                &PresentationConfig::empty_object(),
                &layout,
                std::sync::Arc::clone(&work),
            )
            .unwrap();
            let visual = plan.leaf_visual(0).unwrap();
            let mut receipt = plan.begin_terminal_receipt(&layout).unwrap();
            receipt.record_css_emission(TreemapTypographyCssEmission::new(
                plan.font_family_css(),
                true,
                true,
                &plan.css.label_color,
                &plan.css.value_color,
            ));
            if emit_node {
                receipt.record_node(
                    false,
                    0,
                    &visual.fill,
                    if wrong_node {
                        "#000000"
                    } else {
                        &visual.stroke
                    },
                    &visual.rect_style,
                );
            }
            receipt.record_text(
                TreemapTextRole::LeafLabel,
                true,
                visual.text_fill_ownership,
                &plan.leaf_text_style(0, 38.0),
                true,
                if wrong_label {
                    "#000000"
                } else {
                    &visual.fitted_label_fill
                },
            );
            assert_eq!(
                plan.record_terminal(receipt, Some(&theme), &work).unwrap(),
                succeeds
            );
        }
    }
}
