use super::super::*;

#[derive(Debug)]
struct SequenceTextSurfaceCssEmission {
    final_fill: String,
    typed_fill: Option<String>,
}

#[derive(Debug)]
pub(super) struct SequenceThemeCssEmission {
    sequence_number_fill: String,
    typed_sequence_number_fill: Option<String>,
    text_surfaces: [SequenceTextSurfaceCssEmission; 7],
    frame_stroke: String,
    keyword_fill: String,
    typed_keyword_fill: Option<String>,
    keyword_stroke: String,
    typed_keyword_stroke: Option<String>,
}

impl SequenceThemeCssEmission {
    pub(super) fn sequence_number_fill(&self) -> &str {
        self.sequence_number_fill.as_str()
    }

    pub(super) fn typed_sequence_number_fill(&self) -> Option<&str> {
        self.typed_sequence_number_fill.as_deref()
    }

    pub(super) fn text_surface_fill(
        &self,
        surface: crate::sequence::SequenceTextSurface,
    ) -> (&str, Option<&str>) {
        let emission = &self.text_surfaces[surface.index()];
        (emission.final_fill.as_str(), emission.typed_fill.as_deref())
    }

    pub(super) fn frame_stroke(&self) -> &str {
        &self.frame_stroke
    }

    pub(super) fn keyword_fill(&self) -> &str {
        self.keyword_fill.as_str()
    }

    pub(super) fn typed_keyword_fill(&self) -> Option<&str> {
        self.typed_keyword_fill.as_deref()
    }

    pub(super) fn keyword_stroke(&self) -> &str {
        self.keyword_stroke.as_str()
    }

    pub(super) fn typed_keyword_stroke(&self) -> Option<&str> {
        self.typed_keyword_stroke.as_deref()
    }
}

pub(super) fn write_sequence_css(
    mut out: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    prepared: &crate::sequence::SequencePreparedCss,
) -> Result<SequenceThemeCssEmission> {
    out.checkpoint()?;
    // Mirrors Mermaid 12 `diagrams/sequence/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = diagram_id;
    let theme = prepared.raw.as_ref();
    let font = prepared.font.as_str();
    let font_size_css = prepared.font_size.as_str();
    let text_color = theme.text_color.as_str();
    let error_bkg = theme.error_bkg.as_str();
    let error_text = theme.error_text.as_str();
    let line_color = theme.line_color.as_str();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id, font, font_size_css, text_color
    );
    let _ = out.write_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_bkg, id, error_text, error_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, line_color, line_color, id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{};}}#{} p{{margin:0;}}"#,
        id, font, font_size_css, id
    );

    // Sequence styles.
    let actor_fill = theme.actor_fill.as_str();
    let stroke_width = theme.stroke_width.as_str();
    // The Sequence writer owns diagram-prefixed filter definitions and attribute references.
    let drop_shadow = scoped_drop_shadow(id, theme.drop_shadow.as_str());
    let label_box_border = theme.label_box_border.as_str();
    let activation_fill = theme.activation_fill.as_str();
    let activation_border = theme.activation_border.as_str();
    let node_border = theme.node_border.as_str();
    let label_box_filter = scoped_drop_shadow(id, theme.label_box_filter.as_str());
    let final_actor_fill = prepared.actor_fill.as_str();
    let final_actor_stroke = prepared.actor_stroke.as_str();
    let final_lifeline_stroke = prepared.lifeline_stroke.as_str();
    let final_message_stroke = prepared.message_stroke.as_str();
    let message_width = prepared.message_width.as_str();
    let final_sequence_number_fill = prepared.number_fill.as_str();
    let final_keyword_fill = prepared.keyword_fill.as_str();
    let final_keyword_stroke = prepared.keyword_stroke.as_str();
    let final_note_fill = prepared.note_fill.as_str();
    let final_note_stroke = prepared.note_stroke.as_str();
    let _ = write!(
        &mut out,
        r#"#{} .actor{{stroke:{};fill:{};stroke-width:{};}}"#,
        id, final_actor_stroke, final_actor_fill, stroke_width
    );
    let actor_text_fill = prepared.text
        [crate::sequence::SequenceTextSurface::ParticipantLabel.index()]
    .baseline_declaration
    .as_str();
    let _ = write!(
        &mut out,
        r#"#{} rect.actor.outer-path[data-look="neo"]{{filter:{};}}#{} rect.note[data-look="neo"]{{stroke:{};fill:{};filter:{};}}"#,
        id, drop_shadow, id, final_note_stroke, final_note_fill, drop_shadow
    );
    let _ = write!(
        &mut out,
        r#"#{} text.actor>tspan{{{}stroke:none;}}"#,
        id, actor_text_fill
    );
    if let Some(width) = &prepared.lifeline_width {
        let _ = write!(
            &mut out,
            r#"#{} .actor-line{{stroke:{};stroke-width:{}px;}}"#,
            id, final_lifeline_stroke, width
        );
    } else {
        let _ = write!(
            &mut out,
            r#"#{} .actor-line{{stroke:{};}}"#,
            id, final_lifeline_stroke
        );
    }
    let _ = write!(
        &mut out,
        r#"#{} .innerArc{{stroke-width:1.5;stroke-dasharray:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine0{{stroke-width:{message_width};stroke-dasharray:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine1{{stroke-width:{message_width};stroke-dasharray:2,2;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine0,#{} .messageLine1{{stroke:{};}}"#,
        id, id, final_message_stroke
    );
    // Native SVG parsing supports exact attribute matches, not CSS suffix selectors.
    // Keep browser specificity so source themeCSS retains its browser cascade priority.
    let _ = write!(
        &mut out,
        r#"#{id} [id="{id}-arrowhead"] path,#{id} [id="{id}-crosshead"] path,#{id} [id="{id}-filled-head"] path,#{id} [id="{id}-solidTopArrowHead"] path,#{id} [id="{id}-solidBottomArrowHead"] path{{fill:{final_message_stroke};stroke:{final_message_stroke};}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{id} [id="{id}-stickTopArrowHead"] path,#{id} [id="{id}-stickBottomArrowHead"] path{{stroke:{final_message_stroke};}}#{id} [id="{id}-sequencenumber"]{{fill:{final_message_stroke};}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .sequenceNumber,#{} .sequenceNumber>tspan{{fill:{};}}"#,
        id, id, final_sequence_number_fill
    );
    let message_text_fill = prepared.text
        [crate::sequence::SequenceTextSurface::MessageLabel.index()]
    .baseline_declaration
    .as_str();
    let _ = write!(
        &mut out,
        r#"#{} .messageText{{{}stroke:none;}}"#,
        id, message_text_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBox{{stroke:{};fill:{};filter:{};}}"#,
        id, final_keyword_stroke, final_keyword_fill, label_box_filter
    );
    let label_text_fill = prepared.text
        [crate::sequence::SequenceTextSurface::ControlKeyword.index()]
    .baseline_declaration
    .as_str();
    let _ = write!(
        &mut out,
        r#"#{} .labelText,#{} .labelText>tspan{{{}stroke:none;}}"#,
        id, id, label_text_fill
    );
    let loop_text_fill = prepared.text
        [crate::sequence::SequenceTextSurface::ControlPrimaryTitle.index()]
    .baseline_declaration
    .as_str();
    let _ = write!(
        &mut out,
        r#"#{} .loopText,#{} .loopText>tspan{{{}stroke:none;}}"#,
        id, id, loop_text_fill
    );
    let section_title_fill = prepared.text
        [crate::sequence::SequenceTextSurface::ControlSectionTitle.index()]
    .baseline_declaration
    .as_str();
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle,#{} .sectionTitle>tspan{{{}stroke:none;}}"#,
        id, id, section_title_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopLine{{stroke-width:2px;stroke-dasharray:2,2;stroke:{};fill:{};}}"#,
        id,
        prepared.frame_stroke.as_str(),
        label_box_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .note{{stroke:{};fill:{};}}"#,
        id, final_note_stroke, final_note_fill
    );
    let note_text_fill = prepared.text[crate::sequence::SequenceTextSurface::NoteLabel.index()]
        .baseline_declaration
        .as_str();
    let _ = write!(
        &mut out,
        r#"#{} .noteText,#{} .noteText>tspan{{{}stroke:none;}}"#,
        id, id, note_text_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .activation0{{fill:{};stroke:{};}}#{} .activation1{{fill:{};stroke:{};}}#{} .activation2{{fill:{};stroke:{};}}"#,
        id,
        activation_fill,
        activation_border,
        id,
        activation_fill,
        activation_border,
        id,
        activation_fill,
        activation_border
    );
    let _ = write!(&mut out, r#"#{} .actorPopupMenu{{position:absolute;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .actorPopupMenuPanel{{position:absolute;fill:{};box-shadow:0px 8px 16px 0px rgba(0,0,0,0.2);filter:drop-shadow(3px 5px 2px rgb(0 0 0 / 0.4));}}"#,
        id, actor_fill
    );
    // Glyph strokes inherit their actor group's palette color in Mermaid 12.
    let _ = write!(
        &mut out,
        r#"#{} .actor-man circle,#{} line{{fill:{};stroke-width:2px;}}"#,
        id, id, actor_fill
    );
    if prepared.typed_actor_fill {
        let typed_actor_fill = prepared.actor_fill.as_str();
        let _ = write!(
            &mut out,
            r#"#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{fill:{};}}"#,
            id, id, id, id, typed_actor_fill
        );
    }
    if prepared.typed_actor_stroke {
        let typed_actor_stroke = prepared.actor_stroke.as_str();
        let _ = write!(
            &mut out,
            r#"#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{stroke:{};}}"#,
            id, id, id, id, typed_actor_stroke
        );
    }
    if let Some(typed_activation_fill) = &prepared.activation_fill_override {
        let _ = write!(
            &mut out,
            r#"#{} .activation0,#{} .activation1,#{} .activation2{{fill:{};}}"#,
            id, id, id, typed_activation_fill
        );
    }
    if let Some(typed_activation_stroke) = &prepared.activation_stroke_override {
        let _ = write!(
            &mut out,
            r#"#{} .activation0,#{} .activation1,#{} .activation2{{stroke:{};}}"#,
            id, id, id, typed_activation_stroke
        );
    }
    let mut text_surfaces =
        crate::sequence::SequenceTextSurface::ALL.map(|surface| SequenceTextSurfaceCssEmission {
            final_fill: prepared.text[surface.index()].baseline_fill.clone(),
            typed_fill: None,
        });
    for group in &prepared.text_groups {
        if write_sequence_text_surface_css_group(
            &mut out,
            &id,
            &group.declarations,
            &group.surfaces,
        ) {
            for surface in &group.surfaces {
                let text = &prepared.text[surface.index()];
                if text.typed_fill {
                    text_surfaces[surface.index()].final_fill = text.fill.clone();
                    text_surfaces[surface.index()].typed_fill = Some(text.fill.clone());
                }
            }
        }
    }
    let _ = write!(
        &mut out,
        r#"#{} g rect.rect{{filter:{};stroke:{};}}"#,
        id, drop_shadow, node_border
    );
    crate::svg::parity::css::write_mermaid_base_css_root_rule_to(
        out,
        id,
        prepared.root_font.as_str(),
    )?;
    out.checkpoint()?;
    Ok(SequenceThemeCssEmission {
        sequence_number_fill: prepared.number_fill.clone(),
        typed_sequence_number_fill: prepared
            .typed_number_fill
            .then(|| prepared.number_fill.clone()),
        text_surfaces,
        frame_stroke: prepared.frame_stroke.clone(),
        keyword_fill: prepared.keyword_fill.clone(),
        typed_keyword_fill: prepared
            .typed_keyword_fill
            .then(|| prepared.keyword_fill.clone()),
        keyword_stroke: prepared.keyword_stroke.clone(),
        typed_keyword_stroke: prepared
            .typed_keyword_stroke
            .then(|| prepared.keyword_stroke.clone()),
    })
}

fn write_sequence_text_surface_css_group(
    out: &mut impl std::fmt::Write,
    diagram_id: &impl std::fmt::Display,
    declarations: &str,
    surfaces: &[crate::sequence::SequenceTextSurface],
) -> bool {
    let scoped = surfaces
        .iter()
        .flat_map(|surface| surface.terminal_selectors().split(','))
        .map(|selector| format!("#{diagram_id} {selector}"))
        .collect::<Vec<_>>()
        .join(",");
    write!(out, "{scoped}{{{declarations}}}").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use serde_json::json;
    use std::cell::Cell;

    fn prepared_css(
        config: &serde_json::Value,
        rules: &[(ThemeTarget, ThemeStylePatch)],
    ) -> crate::sequence::SequencePreparedCss {
        let theme = (!rules.is_empty()).then(|| {
            let styles = rules
                .iter()
                .fold(ThemeRuleSet::default(), |styles, (target, patch)| {
                    styles.with_rule(ThemeRule::new(*target, patch.clone()))
                });
            DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(styles))
                .unwrap()
        });
        let resolved = theme
            .as_ref()
            .map(|theme| theme.resolve(crate::DiagramFamilyId::SEQUENCE));
        let parsed = merman_core::Engine::new().parse_diagram_for_render_model_sync(
            "sequenceDiagram\nautonumber\nA->>+B: Hello\nnote over B: Note\nloop control\nB-->>A: Reply\nend\nB-->>-A: Done",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap();
        let merman_core::RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let prepared = crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
            model,
            None,
            &merman_core::MermaidConfig::from_value(config.clone()),
            resolved.as_ref(),
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            meter,
        )
        .unwrap();
        prepared.css().clone()
    }

    fn sequence_css_with_rules(
        id: &str,
        config: &serde_json::Value,
        rules: &[(ThemeTarget, ThemeStylePatch)],
    ) -> String {
        let binding = prepared_css(config, rules);
        let mut out = String::new();
        write_sequence_css(
            &mut out,
            crate::svg::escape_css_identifier(id).as_str(),
            &binding,
        )
        .unwrap();
        out
    }

    fn sequence_css(id: &str, config: &serde_json::Value) -> String {
        sequence_css_with_rules(id, config, &[])
    }

    #[derive(Clone, Copy)]
    struct TrackedDiagramId<'a> {
        writes: &'a Cell<usize>,
    }

    impl std::fmt::Display for TrackedDiagramId<'_> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.writes.set(self.writes.get() + 1);
            formatter.write_str("seq")
        }
    }

    #[test]
    fn sequence_css_streams_with_exact_svg_budget_and_rejects_one_byte_short() {
        let config = json!({});
        let binding = prepared_css(&config, &[]);
        let expected = sequence_css("sequence-budget", &config);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len())
            .expect("valid exact SVG limit");
        let exact_meter = OperationWorkMeter::new(exact_policy);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        write_sequence_css(&mut exact, "sequence-budget", &binding)
            .expect("write exact Sequence CSS budget");
        exact.checkpoint().expect("exact Sequence CSS budget");
        assert_eq!(exact.finish().expect("finish exact Sequence CSS"), expected);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len() - 1)
            .expect("valid short SVG limit");
        let short_meter = OperationWorkMeter::new(short_policy);
        let mut short = BoundedSvgOutput::new(&short_meter);
        let result = write_sequence_css(&mut short, "sequence-budget", &binding);
        assert!(matches!(
            result,
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert!(short.as_str().len() <= expected.len() - 1);
    }

    #[test]
    fn sequence_css_rejects_an_already_failed_sink_before_formatting() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        out.push_str("too long");
        let writes = Cell::new(0);
        let result = write_sequence_css(
            &mut out,
            TrackedDiagramId { writes: &writes },
            &prepared_css(&json!({}), &[]),
        );

        assert!(matches!(
            result,
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert_eq!(writes.get(), 0);
        assert!(out.as_str().is_empty());
    }

    #[test]
    fn prepared_text_groups_emit_all_seven_final_surfaces_with_fresh_receipts() {
        let rules = [
            (ThemeTarget::ActorLabel, "#123456"),
            (ThemeTarget::MessageLabel, "#234567"),
            (ThemeTarget::NoteLabel, "#345678"),
            (ThemeTarget::LoopLabel, "#456789"),
        ]
        .map(|(target, fill)| {
            (
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid(fill).unwrap()),
            )
        });
        let binding = prepared_css(&json!({}), &rules);
        assert_eq!(binding.text_groups.len(), 4);
        assert_eq!(
            binding.text_groups[0].surfaces,
            vec![
                crate::sequence::SequenceTextSurface::ParticipantLabel,
                crate::sequence::SequenceTextSurface::BoxTitle,
            ]
        );
        let mut first = String::new();
        let first_emission = write_sequence_css(&mut first, "seq", &binding).unwrap();
        let mut second = String::new();
        let second_emission = write_sequence_css(&mut second, "seq", &binding).unwrap();
        assert_eq!(first, second);
        for surface in crate::sequence::SequenceTextSurface::ALL {
            let expected = match surface.role() {
                crate::sequence::SequenceTypographyRole::Actor => "#123456",
                crate::sequence::SequenceTypographyRole::Message => "#234567",
                crate::sequence::SequenceTypographyRole::Note => "#345678",
                crate::sequence::SequenceTypographyRole::Loop => "#456789",
            };
            assert_eq!(
                first_emission.text_surface_fill(surface),
                (expected, Some(expected))
            );
            assert_eq!(
                second_emission.text_surface_fill(surface),
                (expected, Some(expected))
            );
            for selector in surface.terminal_selectors().split(',') {
                assert!(first.contains(&format!("#seq {selector}")));
            }
        }
    }

    #[test]
    fn sequence_css_uses_configured_font_size() {
        let css = sequence_css("seq", &json!({"themeVariables": {"fontSize": "24px"}}));

        assert!(css.contains(
            r#"#seq{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:#333;}"#
        ));
        assert!(css.contains(r#"#seq svg{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;}#seq p{margin:0;}"#));
    }

    #[test]
    fn sequence_css_escapes_the_diagram_id_as_a_css_identifier() {
        let css = sequence_css("seq:prod", &json!({}));

        assert!(css.contains(r"#seq\:prod .messageLine0"), "{css}");
        assert!(!css.contains("#seq:prod"), "{css}");
    }

    #[test]
    fn sequence_css_scopes_builtin_drop_shadow_to_the_owned_filter() {
        let css = sequence_css(
            "seq",
            &json!({
                "look": "neo",
                "themeVariables": {"dropShadow": "url(#drop-shadow)"}
            }),
        );

        // Actor, note, loop label and background rectangles share the emitted definition.
        assert_eq!(
            css.matches("filter:url(#seq-drop-shadow)").count(),
            4,
            "{css}"
        );
        assert!(!css.contains("url(#drop-shadow)"), "{css}");

        let custom = sequence_css(
            "seq",
            &json!({
                "look": "neo",
                "themeVariables": {"dropShadow": "url(#custom-shadow)"}
            }),
        );
        assert_eq!(
            custom.matches("filter:url(#custom-shadow)").count(),
            4,
            "{custom}"
        );
        assert!(!custom.contains("url(#seq-custom-shadow)"), "{custom}");
    }

    #[test]
    fn sequence_css_preserves_actor_glyph_inherited_palette_stroke() {
        let css = sequence_css(
            "seq",
            &json!({
                "theme": "redux-color",
                "themeVariables": {
                    "actorBorder": "#220000",
                    "actorBkg": "#330000",
                    "borderColorArray": ["#0055cc", "#00aa77"]
                }
            }),
        );

        // A child stroke declaration would override the parent's per-actor palette.
        assert!(
            css.contains(r#"#seq .actor-man circle,#seq line{fill:#330000;stroke-width:2px;}"#)
        );
        assert!(!css.contains(".actor-man line{"));
        assert!(!css.contains(".actor-man circle,#seq line{stroke:"));
    }

    #[test]
    fn sequence_css_honors_mermaid_12_theme_options() {
        let cfg = json!({
            "look": "neo",
            "sequence": {"noteFontWeight": 700},
            "themeVariables": {
                "fontFamily": "Inter, Arial",
                "textColor": "#abc001",
                "errorBkgColor": "#100000",
                "errorTextColor": "#ffeeee",
                "lineColor": "#123456",
                "actorBorder": "#220000",
                "actorBkg": "#330000",
                "strokeWidth": 2,
                "dropShadow": "drop-shadow(1px 2px 3px rgba(0,0,0,.4))",
                "actorTextColor": "#fafafa",
                "actorLineColor": "#444444",
                "signalColor": "#555555",
                "sequenceNumberColor": "#666666",
                "signalTextColor": "#777777",
                "labelBoxBorderColor": "#888888",
                "labelBoxBkgColor": "#999999",
                "labelTextColor": "#aaaaaa",
                "loopTextColor": "#bbbbbb",
                "noteBorderColor": "#cccccc",
                "noteBkgColor": "#dddddd",
                "noteTextColor": "#eeeeee",
                "noteFontWeight": 600,
                "activationBkgColor": "#010203",
                "activationBorderColor": "#040506",
                "nodeBorder": "#070809"
            }
        });

        let css = sequence_css("seq", &cfg);

        assert!(css.contains(r#"#seq{font-family:Inter,Arial;font-size:16px;fill:#abc001;}"#));
        assert!(css.contains(
            r#"#seq .error-icon{fill:#100000;}#seq .error-text{fill:#ffeeee;stroke:#ffeeee;}"#
        ));
        assert!(css.contains(
            r#"#seq .marker{fill:#123456;stroke:#123456;}#seq .marker.cross{stroke:#123456;}"#
        ));
        assert!(css.contains(r#"#seq .actor{stroke:#220000;fill:#330000;stroke-width:2;}"#));
        assert!(css.contains(r#"#seq rect.actor.outer-path[data-look="neo"]{filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#));
        assert!(css.contains(r#"#seq rect.note[data-look="neo"]{stroke:#cccccc;fill:#dddddd;filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#));
        assert!(css.contains(r#"#seq text.actor>tspan{fill:#fafafa;stroke:none;}"#));
        assert!(css.contains(r#"#seq .actor-line{stroke:#444444;}"#));
        assert!(
            css.contains(r#"#seq .actor-man circle,#seq line{fill:#330000;stroke-width:2px;}"#)
        );
        assert!(!css.contains(r#"#seq .actor-man circle,#seq line{stroke:"#));
        assert!(css.contains(r#"#seq .messageLine0{stroke-width:1.5;stroke-dasharray:none;}"#));
        assert!(css.contains(r#"#seq .messageLine0,#seq .messageLine1{stroke:#555555;}"#));
        assert!(css.contains(r#"#seq .sequenceNumber,#seq .sequenceNumber>tspan{fill:#666666;}"#));
        assert!(css.contains(r#"#seq .messageText{fill:#777777;stroke:none;}"#));
        assert!(css.contains(r#"#seq .labelBox{stroke:#888888;fill:#999999;filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#));
        assert!(
            css.contains(
                r#"#seq .sectionTitle,#seq .sectionTitle>tspan{fill:#bbbbbb;stroke:none;}"#
            )
        );
        assert!(css.contains(r#"#seq .note{stroke:#cccccc;fill:#dddddd;}"#));
        assert!(css.contains(r#"#seq .noteText,#seq .noteText>tspan{fill:#eeeeee;stroke:none;}"#));
        // styles.js deliberately omits weight on note tspans: theme 600 must not
        // override the weight inherited from the parent text's inline style.
        for rule in css.split('}').filter(|rule| rule.contains(".noteText")) {
            assert!(!rule.contains("font-weight"), "{rule}");
        }
        assert!(css.contains(r#"#seq .activation0{fill:#010203;stroke:#040506;}"#));
        assert!(css.contains(
            r#"#seq g rect.rect{filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));stroke:#070809;}"#
        ));
    }

    #[test]
    fn sequence_actor_stroke_css_is_scoped_to_actor_owned_dom() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({"themeVariables": {"actorBorder": "#220000"}}),
            &[(
                ThemeTarget::Actor,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .actor{stroke:#2563eb;"#));
        assert!(css.contains(
            r#"#seq .actor-man line,#seq .actor-man circle,#seq .actor line,#seq .actor circle{stroke:#2563eb;}"#
        ));
        assert_eq!(css.matches("#seq .actor{").count(), 1);
        assert!(!css.contains(r#"#seq .actor-man circle,#seq line{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq line{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq [id="seq-sequencenumber"]{stroke:#2563eb;"#));
    }

    #[test]
    fn sequence_actor_fill_css_is_scoped_to_actor_owned_dom() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({"themeVariables": {"actorBkg": "#330000"}}),
            &[(
                ThemeTarget::Actor,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#dc2626").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .actor{stroke:#9370DB;fill:#dc2626;"#));
        assert!(css.contains(
            r#"#seq .actor-man line,#seq .actor-man circle,#seq .actor line,#seq .actor circle{fill:#dc2626;}"#
        ));
        assert_eq!(css.matches("#seq .actor{").count(), 1);
        assert!(!css.contains(r#"#seq .actor-man circle,#seq line{stroke:#9370DB;fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq line{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageLine0{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq [id="seq-sequencenumber"]{fill:#dc2626;"#));
    }

    #[test]
    fn sequence_message_stroke_css_covers_lines_and_their_marker_table() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({"themeVariables": {"signalColor": "#555555"}}),
            &[(
                ThemeTarget::Message,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .messageLine0,#seq .messageLine1{stroke:#2563eb;}"#));
        assert!(css.contains(
            r#"#seq [id="seq-arrowhead"] path,#seq [id="seq-crosshead"] path,#seq [id="seq-filled-head"] path,#seq [id="seq-solidTopArrowHead"] path,#seq [id="seq-solidBottomArrowHead"] path{fill:#2563eb;stroke:#2563eb;}"#
        ));
        assert!(css.contains(
            r#"#seq [id="seq-stickTopArrowHead"] path,#seq [id="seq-stickBottomArrowHead"] path{stroke:#2563eb;}#seq [id="seq-sequencenumber"]{fill:#2563eb;}"#
        ));
        assert_eq!(css.matches(r#"#seq [id="seq-sequencenumber"]{"#).count(), 1);
        assert!(!css.contains(r#"#seq .note{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .activation0{stroke:#2563eb;}"#));
    }

    #[test]
    fn sequence_number_label_css_has_one_final_writer_owner() {
        let mut css = String::new();
        let binding = prepared_css(
            &json!({"themeVariables": {"sequenceNumberColor": "#fedcba"}}),
            &[(
                ThemeTarget::SequenceNumberLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )],
        );
        let emission = write_sequence_css(
            &mut css,
            crate::svg::escape_css_identifier("seq:prod").as_str(),
            &binding,
        )
        .expect("write Sequence number CSS");

        let owner = r#"#seq\:prod .sequenceNumber,#seq\:prod .sequenceNumber>tspan{fill:#123456;}"#;
        assert!(css.contains(owner));
        assert_eq!(css.matches(owner).count(), 1);
        assert!(!css.contains(r#"#seq\:prod .sequenceNumber{fill:#fedcba;}"#));
        assert_eq!(emission.sequence_number_fill(), "#123456");
        assert_eq!(emission.typed_sequence_number_fill(), Some("#123456"));
    }

    #[test]
    fn sequence_lifeline_paint_css_is_scoped_to_actor_lines() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({"themeVariables": {"actorLineColor": "#444444"}}),
            &[(
                ThemeTarget::Lifeline,
                ThemeStylePatch::default()
                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap())
                    .with_stroke_width(2.0)
                    .unwrap(),
            )],
        );

        assert!(css.contains(r#"#seq .actor-line{stroke:#2563eb;stroke-width:2px;}"#));
        assert_eq!(css.matches("#seq .actor-line{").count(), 1);
        assert!(!css.contains(r#"#seq .actor-line{stroke:#444444;"#));
        assert!(!css.contains(r#"#seq .actor{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .note{stroke:#2563eb;}"#));
    }

    #[test]
    fn sequence_note_paint_css_is_scoped_to_note_rects() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({
                "themeVariables": {
                    "noteBkgColor": "#dddddd",
                    "noteBorderColor": "#cccccc"
                }
            }),
            &[(
                ThemeTarget::Note,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#dc2626").unwrap())
                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .note{stroke:#2563eb;fill:#dc2626;}"#));
        assert_eq!(css.matches("#seq .note{").count(), 1);
        assert!(!css.contains(r#"#seq .note{stroke:#cccccc;fill:#dddddd;}"#));
        assert!(!css.contains(r#"#seq .noteText{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageText{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq .activation0{fill:#dc2626;"#));
    }

    #[test]
    fn sequence_activation_paint_css_is_scoped_to_activation_rects() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({
                "themeVariables": {
                    "activationBkgColor": "#dddddd",
                    "activationBorderColor": "#cccccc"
                }
            }),
            &[(
                ThemeTarget::Activation,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#dc2626").unwrap())
                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .activation0{fill:#dddddd;stroke:#cccccc;}"#));
        assert!(css.contains(
            r#"#seq .activation0,#seq .activation1,#seq .activation2{fill:#dc2626;}#seq .activation0,#seq .activation1,#seq .activation2{stroke:#2563eb;}"#
        ));
        assert!(!css.contains(r#"#seq .note{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;"#));
    }

    #[test]
    fn sequence_loop_box_paint_has_one_terminal_writer_owner() {
        let css = sequence_css_with_rules(
            "seq",
            &json!({
                "themeVariables": {
                    "labelBoxBkgColor": "#dddddd",
                    "labelBoxBorderColor": "#cccccc"
                }
            }),
            &[(
                ThemeTarget::LoopLabelBackground,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#dc2626").unwrap())
                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            )],
        );

        assert!(css.contains(r#"#seq .labelBox{stroke:#2563eb;fill:#dc2626;"#));
        assert_eq!(css.matches("#seq .labelBox{").count(), 1);
        assert!(!css.contains(r#"#seq .labelBox{stroke:#cccccc;fill:#dddddd;"#));
    }
}
