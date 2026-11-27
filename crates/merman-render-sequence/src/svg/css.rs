use super::prelude::*;
use merman_render_core::{
    number_format::canonicalize_number, svg_css::write_mermaid_base_css_root_rule,
};

const BASELINE_LINE_STROKE_WIDTH_PX: f64 = 2.0;

pub(super) fn lifeline_effective_stroke_width(native_width: Option<f32>) -> f64 {
    // Mermaid's broad line rule overrides the 0.5px presentation attribute.
    native_width
        .map(|width| canonicalize_number(f64::from(width)))
        .unwrap_or(BASELINE_LINE_STROKE_WIDTH_PX)
}

pub(super) fn write_sequence_css(
    mut out: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    prepared: &crate::preparation::SequencePreparedArtifact,
) -> Result<()> {
    out.checkpoint()?;
    // Mirrors Mermaid 12 `diagrams/sequence/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = diagram_id;
    let typography = prepared.typography();
    let theme = typography.compat_binding();
    let native = prepared.native();
    let css = prepared.css();
    let font = &theme.font_family;
    let root_font = &theme.root_font_family;
    let font_size_css = &theme.font_size_css;
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
    let final_actor_fill = &theme.actor_fill;
    let final_actor_stroke = &theme.actor_border;
    let final_lifeline_stroke = &theme.actor_line;
    let final_message_stroke = &theme.signal_color;
    let native_width = native.map(|theme| theme.form.stroke_width as f32);
    let message_width =
        native.map(|theme| format!("{}px", fmt(f64::from(theme.form.connector_width as f32))));
    let message_width = message_width.as_deref().unwrap_or("1.5");
    let final_sequence_number_fill = &theme.sequence_number;
    let final_keyword_fill = &theme.label_box_fill;
    let final_keyword_stroke = &theme.label_box_border;
    let final_note_fill = &theme.note_fill;
    let final_note_stroke = &theme.note_border;
    let _ = write!(
        &mut out,
        r#"#{} .actor{{stroke:{};fill:{};stroke-width:{};}}"#,
        id, final_actor_stroke, final_actor_fill, stroke_width
    );
    let actor_text_fill = css.actor_text_fill.as_str();
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
    if let Some(width) = native_width {
        let _ = write!(
            &mut out,
            r#"#{} .actor-line{{stroke:{};stroke-width:{}px;}}"#,
            id,
            final_lifeline_stroke,
            fmt(f64::from(width))
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
    if prepared.native().is_some() {
        let _ = write!(
            &mut out,
            r#"#{id} [id="{id}-arrowhead"] path.merman-sequence-open-arrow{{fill:none;}}"#,
        );
    }
    let _ = write!(
        &mut out,
        r#"#{} .sequenceNumber,#{} .sequenceNumber>tspan{{fill:{};}}"#,
        id, id, final_sequence_number_fill
    );
    let message_text_fill = css.message_text_fill.as_str();
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
    let label_text_fill = css.label_text_fill.as_str();
    let _ = write!(
        &mut out,
        r#"#{} .labelText,#{} .labelText>tspan{{{}stroke:none;}}"#,
        id, id, label_text_fill
    );
    let loop_text_fill = css.loop_text_fill.as_str();
    let _ = write!(
        &mut out,
        r#"#{} .loopText,#{} .loopText>tspan{{{}stroke:none;}}"#,
        id, id, loop_text_fill
    );
    let section_title_fill = css.section_title_fill.as_str();
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle,#{} .sectionTitle>tspan{{{}stroke:none;}}"#,
        id, id, section_title_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopLine{{stroke-width:2px;stroke-dasharray:2,2;stroke:{};fill:{};}}"#,
        id, label_box_border, label_box_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .note{{stroke:{};fill:{};}}"#,
        id, final_note_stroke, final_note_fill
    );
    let note_text_fill = css.note_text_fill.as_str();
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
        r#"#{} .actor-man circle,#{} line{{fill:{};stroke-width:{}px;}}"#,
        id,
        id,
        actor_fill,
        fmt(BASELINE_LINE_STROKE_WIDTH_PX)
    );
    if native.is_some() {
        let _ = write!(
            &mut out,
            r#"#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{fill:{};}}"#,
            id, id, id, id, final_actor_fill
        );
        let _ = write!(
            &mut out,
            r#"#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{stroke:{};}}"#,
            id, id, id, id, final_actor_stroke
        );
    }
    for group in &css.text_groups {
        write_sequence_text_surface_css_group(&mut out, &id, &group.declarations, &group.surfaces);
    }
    let _ = write!(
        &mut out,
        r#"#{} g rect.rect{{filter:{};stroke:{};}}"#,
        id, drop_shadow, node_border
    );
    write_mermaid_base_css_root_rule(out, id, root_font)?;
    out.checkpoint()?;
    Ok(())
}

fn write_sequence_text_surface_css_group(
    out: &mut impl std::fmt::Write,
    diagram_id: &impl std::fmt::Display,
    declarations: &str,
    surfaces: &[crate::preparation::SequenceTextSurface],
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
    use merman_render_core::{
        resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId},
        svg_css::escape_css_identifier,
        text::DeterministicTextMeasurer,
    };
    use serde_json::json;
    use std::cell::Cell;

    fn prepared_css(config: &serde_json::Value) -> crate::preparation::SequencePreparedArtifact {
        let parsed = merman_core::MermaidEngine::new().parse_diagram_for_render_model_sync(
            "sequenceDiagram\nautonumber\nA->>+B: Hello\nnote over B: Note\nloop control\nB-->>A: Reply\nend\nB-->>-A: Done",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap().into_presentation();
        let merman_model::presentation::RenderSemanticModel::Sequence(model) = parsed.model()
        else {
            panic!("expected Sequence model");
        };
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        // These materialized CSS defaults deliberately declare no authored configuration owners.
        let config =
            merman_model::presentation::PresentationConfig::try_from_frozen_parts_controlled(
                merman_model::presentation::FrozenPresentationConfigParts::new(
                    merman_model::MermaidConfig::from_value(
                        merman_model::json::clone_value_nonrecursive(config),
                    ),
                ),
                &merman_model::resources::InputResourcePolicy::for_profile(
                    merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
                ),
                &merman_model::OperationControl::new(),
            )
            .expect("frozen CSS config construction should not cancel")
            .expect("unowned CSS defaults should be valid");
        crate::preparation::prepare_sequence_diagram_typed_with_title_and_work_meter(
            model,
            None,
            &config,
            None,
            &DeterministicTextMeasurer::default(),
            None,
            meter,
        )
        .unwrap()
    }

    fn sequence_css(id: &str, config: &serde_json::Value) -> String {
        let binding = prepared_css(config);
        let mut out = String::new();
        write_sequence_css(&mut out, escape_css_identifier(id).as_ref(), &binding).unwrap();
        out
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
        let binding = prepared_css(&config);
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
        assert!(matches!(result, Err(Error::ResourceLimitExceeded(_))));
        assert!(short.as_str().len() < expected.len());
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
            &prepared_css(&json!({})),
        );

        assert!(matches!(result, Err(Error::ResourceLimitExceeded(_))));
        assert_eq!(writes.get(), 0);
        assert!(out.as_str().is_empty());
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
            "fontFamily": "Courier, monospace",
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
        assert!(css.ends_with("#seq :root{--mermaid-font-family:Courier,monospace;}"));
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
}
