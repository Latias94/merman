use super::super::*;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SequenceThemeCssAdapter<'a> {
    pub(super) actor_fill: Option<&'a str>,
    pub(super) actor_stroke: Option<&'a str>,
    pub(super) lifeline_stroke: Option<&'a str>,
    pub(super) lifeline_stroke_width: Option<f32>,
    pub(super) message_stroke: Option<&'a str>,
    pub(super) sequence_number_fill: Option<&'a str>,
    pub(super) loop_fill: Option<&'a str>,
    pub(super) loop_stroke: Option<&'a str>,
    pub(super) note_fill: Option<&'a str>,
    pub(super) note_stroke: Option<&'a str>,
    pub(super) activation_fill: Option<&'a str>,
    pub(super) activation_stroke: Option<&'a str>,
    pub(super) actor_typography: Option<&'a crate::sequence::SequenceResolvedTypography>,
    pub(super) message_typography: Option<&'a crate::sequence::SequenceResolvedTypography>,
    pub(super) note_typography: Option<&'a crate::sequence::SequenceResolvedTypography>,
    pub(super) loop_typography: Option<&'a crate::sequence::SequenceResolvedTypography>,
}

fn typography_for_surface<'a>(
    adapter: &SequenceThemeCssAdapter<'a>,
    surface: crate::sequence::SequenceTextSurface,
) -> Option<&'a crate::sequence::SequenceResolvedTypography> {
    match surface.role() {
        crate::sequence::SequenceTypographyRole::Actor => adapter.actor_typography,
        crate::sequence::SequenceTypographyRole::Message => adapter.message_typography,
        crate::sequence::SequenceTypographyRole::Note => adapter.note_typography,
        crate::sequence::SequenceTypographyRole::Loop => adapter.loop_typography,
    }
}

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
    loop_fill: String,
    typed_loop_fill: Option<String>,
    loop_stroke: String,
    typed_loop_stroke: Option<String>,
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

    pub(super) fn loop_fill(&self) -> &str {
        self.loop_fill.as_str()
    }

    pub(super) fn typed_loop_fill(&self) -> Option<&str> {
        self.typed_loop_fill.as_deref()
    }

    pub(super) fn loop_stroke(&self) -> &str {
        self.loop_stroke.as_str()
    }

    pub(super) fn typed_loop_stroke(&self) -> Option<&str> {
        self.typed_loop_stroke.as_deref()
    }
}

#[cfg(test)]
pub(super) fn sequence_css(
    diagram_id: &str,
    font_size_px: f64,
    effective_config: &serde_json::Value,
) -> String {
    sequence_css_with_theme_adapter(
        diagram_id,
        font_size_px,
        effective_config,
        SequenceThemeCssAdapter::default(),
    )
}

#[cfg(test)]
pub(super) fn sequence_css_with_theme_adapter(
    diagram_id: &str,
    font_size_px: f64,
    effective_config: &serde_json::Value,
    typed: SequenceThemeCssAdapter<'_>,
) -> String {
    let mut out = String::new();
    let _ = write_sequence_css_with_theme_adapter(
        &mut out,
        diagram_id,
        font_size_px,
        effective_config,
        typed,
    );
    out
}

pub(super) fn write_sequence_css_with_theme_adapter(
    mut out: &mut impl std::fmt::Write,
    diagram_id: &str,
    font_size_px: f64,
    effective_config: &serde_json::Value,
    typed: SequenceThemeCssAdapter<'_>,
) -> SequenceThemeCssEmission {
    // Mirrors Mermaid 11.15 `diagrams/sequence/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = crate::svg::escape_css_identifier(diagram_id);
    let theme = MermaidThemeAdapter::new(effective_config).sequence_diagram();
    let font = theme.common.font_family_css.as_str();
    let text_color = theme.common.text_color.as_str();
    let error_bkg = theme.common.error_bkg.as_str();
    let error_text = theme.common.error_text.as_str();
    let line_color = theme.common.line_color.as_str();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
        id,
        font,
        fmt(font_size_px),
        text_color
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
        r#"#{} svg{{font-family:{};font-size:{}px;}}#{} p{{margin:0;}}"#,
        id,
        font,
        fmt(font_size_px),
        id
    );

    // Sequence styles.
    let actor_border = theme.actor_border.as_str();
    let actor_fill = theme.actor_fill.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let drop_shadow = theme.drop_shadow.as_str();
    let note_border = theme.note_border.as_str();
    let note_fill = theme.note_fill.as_str();
    let actor_text = theme.actor_text.as_str();
    let actor_line = theme.actor_line.as_str();
    let signal_color = theme.signal_color.as_str();
    let sequence_number = theme.sequence_number.as_str();
    let signal_text = theme.signal_text.as_str();
    let label_box_border = theme.label_box_border.as_str();
    let label_box_fill = theme.label_box_fill.as_str();
    let label_text = theme.label_text.as_str();
    let loop_text = theme.loop_text.as_str();
    let note_text = theme.note_text.as_str();
    let activation_fill = theme.activation_fill.as_str();
    let activation_border = theme.activation_border.as_str();
    let node_border = theme.node_border.as_str();
    let label_box_filter = theme.label_box_filter.as_str();
    let note_font_weight = theme.note_font_weight.as_str();
    let mut emission = SequenceThemeCssEmission {
        sequence_number_fill: sequence_number.to_owned(),
        typed_sequence_number_fill: None,
        text_surfaces: crate::sequence::SequenceTextSurface::ALL.map(|surface| {
            SequenceTextSurfaceCssEmission {
                final_fill: match surface {
                    crate::sequence::SequenceTextSurface::ParticipantLabel => actor_text,
                    crate::sequence::SequenceTextSurface::BoxTitle => text_color,
                    crate::sequence::SequenceTextSurface::MessageLabel => signal_text,
                    crate::sequence::SequenceTextSurface::NoteLabel => note_text,
                    crate::sequence::SequenceTextSurface::ControlKeyword => label_text,
                    crate::sequence::SequenceTextSurface::ControlPrimaryTitle
                    | crate::sequence::SequenceTextSurface::ControlSectionTitle => loop_text,
                }
                .to_owned(),
                typed_fill: None,
            }
        }),
        loop_fill: label_box_fill.to_owned(),
        typed_loop_fill: None,
        loop_stroke: label_box_border.to_owned(),
        typed_loop_stroke: None,
    };

    let _ = write!(
        &mut out,
        r#"#{} .actor{{stroke:{};fill:{};stroke-width:{};}}"#,
        id, actor_border, actor_fill, stroke_width
    );
    let _ = write!(
        &mut out,
        r#"#{} text.actor>tspan{{fill:{};stroke:none;}}"#,
        id, actor_text
    );
    let _ = write!(&mut out, r#"#{} .actor-line{{stroke:{};}}"#, id, actor_line);
    let _ = write!(
        &mut out,
        r#"#{} .innerArc{{stroke-width:1.5;stroke-dasharray:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine0{{stroke-width:1.5;stroke-dasharray:none;stroke:{};}}"#,
        id, signal_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine1{{stroke-width:1.5;stroke-dasharray:2,2;stroke:{};}}"#,
        id, signal_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-arrowhead"] path{{fill:{};stroke:{};}}"#,
        id, signal_color, signal_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .sequenceNumber{{fill:{};}}"#,
        id, sequence_number
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-sequencenumber"]{{fill:{};}}"#,
        id, signal_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-crosshead"] path{{fill:{};stroke:{};}}"#,
        id, signal_color, signal_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageText{{fill:{};stroke:none;}}"#,
        id, signal_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBox{{stroke:{};fill:{};filter:{};}}"#,
        id, label_box_border, label_box_fill, label_box_filter
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelText,#{} .labelText>tspan{{fill:{};stroke:none;}}"#,
        id, id, label_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopText,#{} .loopText>tspan{{fill:{};stroke:none;}}"#,
        id, id, loop_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .sectionTitle,#{} .sectionTitle>tspan{{fill:{};stroke:none;}}"#,
        id, id, loop_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopLine{{stroke-width:2px;stroke-dasharray:2,2;stroke:{};fill:{};}}"#,
        id, label_box_border, label_box_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .note{{stroke:{};fill:{};}}"#,
        id, note_border, note_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .noteText,#{} .noteText>tspan{{fill:{};stroke:none;{}}}"#,
        id, id, note_text, note_font_weight
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
    let _ = write!(
        &mut out,
        r#"#{} .actor-man line{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-man circle,#{} line{{stroke:{};fill:{};stroke-width:2px;}}"#,
        id, id, actor_border, actor_fill
    );
    if let Some(typed_actor_fill) = typed.actor_fill {
        let _ = write!(
            &mut out,
            r#"#{} .actor{{fill:{};}}#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{fill:{};}}"#,
            id, typed_actor_fill, id, id, id, id, typed_actor_fill
        );
    }
    if let Some(typed_actor_stroke) = typed.actor_stroke {
        let _ = write!(
            &mut out,
            r#"#{} .actor{{stroke:{};}}#{} .actor-man line,#{} .actor-man circle,#{} .actor line,#{} .actor circle{{stroke:{};}}"#,
            id, typed_actor_stroke, id, id, id, id, typed_actor_stroke
        );
    }
    if let Some(typed_lifeline_stroke) = typed.lifeline_stroke {
        let _ = write!(
            &mut out,
            r#"#{} .actor-line{{stroke:{};}}"#,
            id, typed_lifeline_stroke
        );
    }
    if let Some(typed_lifeline_stroke_width) = typed.lifeline_stroke_width {
        let _ = write!(
            &mut out,
            r#"#{} .actor-line{{stroke-width:{}px;}}"#,
            id,
            fmt(f64::from(typed_lifeline_stroke_width))
        );
    }
    if let Some(typed_message_stroke) = typed.message_stroke {
        let _ = write!(
            &mut out,
            r#"#{} .messageLine0,#{} .messageLine1{{stroke:{};}}"#,
            id, id, typed_message_stroke
        );
        let _ = write!(
            &mut out,
            r#"#{} [id$="-arrowhead"] path,#{} [id$="-crosshead"] path,#{} [id$="-filled-head"] path,#{} [id$="-solidTopArrowHead"] path,#{} [id$="-solidBottomArrowHead"] path{{fill:{};stroke:{};}}"#,
            id, id, id, id, id, typed_message_stroke, typed_message_stroke
        );
        let _ = write!(
            &mut out,
            r#"#{} [id$="-stickTopArrowHead"] path,#{} [id$="-stickBottomArrowHead"] path{{stroke:{};}}#{} [id$="-sequencenumber"]{{fill:{};}}"#,
            id, id, typed_message_stroke, id, typed_message_stroke
        );
    }
    if let Some(typed_sequence_number_fill) = typed.sequence_number_fill {
        let _ = write!(
            &mut out,
            r#"#{} .sequenceNumber,#{} .sequenceNumber>tspan{{fill:{};}}"#,
            id, id, typed_sequence_number_fill
        );
        emission.sequence_number_fill = typed_sequence_number_fill.to_owned();
        emission.typed_sequence_number_fill = Some(typed_sequence_number_fill.to_owned());
    }
    if let Some(typed_loop_fill) = typed.loop_fill {
        if write!(
            &mut out,
            r#"#{} .labelBox{{fill:{};}}"#,
            id, typed_loop_fill
        )
        .is_ok()
        {
            emission.loop_fill = typed_loop_fill.to_owned();
            emission.typed_loop_fill = Some(typed_loop_fill.to_owned());
        }
    }
    if let Some(typed_loop_stroke) = typed.loop_stroke {
        if write!(
            &mut out,
            r#"#{} .labelBox{{stroke:{};}}"#,
            id, typed_loop_stroke
        )
        .is_ok()
        {
            emission.loop_stroke = typed_loop_stroke.to_owned();
            emission.typed_loop_stroke = Some(typed_loop_stroke.to_owned());
        }
    }
    if let Some(typed_note_fill) = typed.note_fill {
        let _ = write!(&mut out, r#"#{} .note{{fill:{};}}"#, id, typed_note_fill);
    }
    if let Some(typed_note_stroke) = typed.note_stroke {
        let _ = write!(
            &mut out,
            r#"#{} .note{{stroke:{};}}"#,
            id, typed_note_stroke
        );
    }
    if let Some(typed_activation_fill) = typed.activation_fill {
        let _ = write!(
            &mut out,
            r#"#{} .activation0,#{} .activation1,#{} .activation2{{fill:{};}}"#,
            id, id, id, typed_activation_fill
        );
    }
    if let Some(typed_activation_stroke) = typed.activation_stroke {
        let _ = write!(
            &mut out,
            r#"#{} .activation0,#{} .activation1,#{} .activation2{{stroke:{};}}"#,
            id, id, id, typed_activation_stroke
        );
    }
    for surfaces in [
        &[
            crate::sequence::SequenceTextSurface::ParticipantLabel,
            crate::sequence::SequenceTextSurface::BoxTitle,
        ][..],
        &[crate::sequence::SequenceTextSurface::MessageLabel][..],
        &[crate::sequence::SequenceTextSurface::NoteLabel][..],
        &[
            crate::sequence::SequenceTextSurface::ControlPrimaryTitle,
            crate::sequence::SequenceTextSurface::ControlSectionTitle,
            crate::sequence::SequenceTextSurface::ControlKeyword,
        ][..],
    ] {
        for (declarations, declaration_surfaces) in
            grouped_sequence_text_surface_declarations(&typed, surfaces)
        {
            if declarations.is_empty()
                || !write_sequence_text_surface_css_group(
                    &mut out,
                    &id,
                    &declarations,
                    &declaration_surfaces,
                )
            {
                continue;
            }
            for surface in declaration_surfaces {
                let typography = typography_for_surface(&typed, surface);
                let Some(fill) =
                    typography.and_then(|typography| typography.typed_fill_for(surface))
                else {
                    continue;
                };
                let surface_emission = &mut emission.text_surfaces[surface.index()];
                surface_emission.final_fill = fill.to_owned();
                surface_emission.typed_fill = Some(fill.to_owned());
            }
        }
    }
    let _ = write!(
        &mut out,
        r#"#{} g rect.rect{{filter:{};stroke:{};}}"#,
        id, drop_shadow, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    emission
}

fn grouped_sequence_text_surface_declarations(
    adapter: &SequenceThemeCssAdapter<'_>,
    surfaces: &[crate::sequence::SequenceTextSurface],
) -> Vec<(String, Vec<crate::sequence::SequenceTextSurface>)> {
    let mut groups: Vec<(String, Vec<crate::sequence::SequenceTextSurface>)> = Vec::new();
    for &surface in surfaces {
        let declarations = typography_for_surface(adapter, surface)
            .and_then(|typography| typography.css_declarations_for(surface))
            .unwrap_or_default();
        if let Some((_, grouped_surfaces)) = groups
            .iter_mut()
            .find(|(grouped_declarations, _)| *grouped_declarations == declarations)
        {
            grouped_surfaces.push(surface);
        } else {
            groups.push((declarations, vec![surface]));
        }
    }
    groups
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
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use serde_json::json;

    #[test]
    fn sequence_css_streams_with_exact_svg_budget_and_rejects_one_byte_short() {
        let config = json!({});
        let expected = sequence_css("sequence-budget", 16.0, &config);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len())
            .expect("valid exact SVG limit");
        let exact_meter = OperationWorkMeter::new(exact_policy);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        write_sequence_css_with_theme_adapter(
            &mut exact,
            "sequence-budget",
            16.0,
            &config,
            SequenceThemeCssAdapter::default(),
        );
        exact.checkpoint().expect("exact Sequence CSS budget");
        assert_eq!(exact.finish().expect("finish exact Sequence CSS"), expected);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len() - 1)
            .expect("valid short SVG limit");
        let short_meter = OperationWorkMeter::new(short_policy);
        let mut short = BoundedSvgOutput::new(&short_meter);
        write_sequence_css_with_theme_adapter(
            &mut short,
            "sequence-budget",
            16.0,
            &config,
            SequenceThemeCssAdapter::default(),
        );
        assert!(matches!(
            short.checkpoint(),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert!(short.as_str().len() <= expected.len() - 1);
    }

    #[test]
    fn sequence_css_uses_configured_font_size() {
        let css = sequence_css("seq", 24.0, &json!({}));

        assert!(css.contains(
            r#"#seq{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:#333;}"#
        ));
        assert!(css.contains(r#"#seq svg{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;}#seq p{margin:0;}"#));
    }

    #[test]
    fn sequence_css_escapes_the_diagram_id_as_a_css_identifier() {
        let css = sequence_css("seq:prod", 16.0, &json!({}));

        assert!(css.contains(r"#seq\:prod .messageLine0"), "{css}");
        assert!(!css.contains("#seq:prod"), "{css}");
    }

    #[test]
    fn sequence_css_honors_mermaid_11_15_theme_options() {
        let cfg = json!({
            "look": "neo",
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

        let css = sequence_css("seq", 16.0, &cfg);

        assert!(css.contains(r#"#seq{font-family:Inter,Arial;font-size:16px;fill:#abc001;}"#));
        assert!(css.contains(
            r#"#seq .error-icon{fill:#100000;}#seq .error-text{fill:#ffeeee;stroke:#ffeeee;}"#
        ));
        assert!(css.contains(
            r#"#seq .marker{fill:#123456;stroke:#123456;}#seq .marker.cross{stroke:#123456;}"#
        ));
        assert!(css.contains(r#"#seq .actor{stroke:#220000;fill:#330000;stroke-width:2;}"#));
        assert!(css.contains(r#"#seq text.actor>tspan{fill:#fafafa;stroke:none;}"#));
        assert!(css.contains(r#"#seq .actor-line{stroke:#444444;}"#));
        assert!(css.contains(
            r#"#seq .messageLine0{stroke-width:1.5;stroke-dasharray:none;stroke:#555555;}"#
        ));
        assert!(css.contains(r#"#seq .sequenceNumber{fill:#666666;}"#));
        assert!(css.contains(r#"#seq .messageText{fill:#777777;stroke:none;}"#));
        assert!(css.contains(r#"#seq .labelBox{stroke:#888888;fill:#999999;filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));}"#));
        assert!(
            css.contains(
                r#"#seq .sectionTitle,#seq .sectionTitle>tspan{fill:#bbbbbb;stroke:none;}"#
            )
        );
        assert!(css.contains(r#"#seq .note{stroke:#cccccc;fill:#dddddd;}"#));
        assert!(css.contains(
            r#"#seq .noteText,#seq .noteText>tspan{fill:#eeeeee;stroke:none;font-weight:600;}"#
        ));
        assert!(css.contains(r#"#seq .activation0{fill:#010203;stroke:#040506;}"#));
        assert!(css.contains(
            r#"#seq g rect.rect{filter:drop-shadow(1px 2px 3px rgba(0,0,0,.4));stroke:#070809;}"#
        ));
    }

    #[test]
    fn sequence_actor_stroke_css_is_scoped_to_actor_owned_dom() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({"themeVariables": {"actorBorder": "#220000"}}),
            SequenceThemeCssAdapter {
                actor_stroke: Some("#2563eb"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .actor-man circle,#seq line{stroke:#220000;"#));
        assert!(css.contains(
            r#"#seq .actor{stroke:#2563eb;}#seq .actor-man line,#seq .actor-man circle,#seq .actor line,#seq .actor circle{stroke:#2563eb;}"#
        ));
        assert!(!css.contains(r#"#seq .actor-man circle,#seq line{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq line{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq [id$="-sequencenumber"]{stroke:#2563eb;"#));
    }

    #[test]
    fn sequence_actor_fill_css_is_scoped_to_actor_owned_dom() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({"themeVariables": {"actorBkg": "#330000"}}),
            SequenceThemeCssAdapter {
                actor_fill: Some("#dc2626"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .actor-man circle,#seq line{stroke:#9370DB;fill:#330000;"#));
        assert!(css.contains(
            r#"#seq .actor{fill:#dc2626;}#seq .actor-man line,#seq .actor-man circle,#seq .actor line,#seq .actor circle{fill:#dc2626;}"#
        ));
        assert!(!css.contains(r#"#seq .actor-man circle,#seq line{stroke:#9370DB;fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq line{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageLine0{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq [id$="-sequencenumber"]{fill:#dc2626;"#));
    }

    #[test]
    fn sequence_message_stroke_css_covers_lines_and_their_marker_table() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({"themeVariables": {"signalColor": "#555555"}}),
            SequenceThemeCssAdapter {
                message_stroke: Some("#2563eb"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .messageLine0,#seq .messageLine1{stroke:#2563eb;}"#));
        assert!(css.contains(
            r#"#seq [id$="-arrowhead"] path,#seq [id$="-crosshead"] path,#seq [id$="-filled-head"] path,#seq [id$="-solidTopArrowHead"] path,#seq [id$="-solidBottomArrowHead"] path{fill:#2563eb;stroke:#2563eb;}"#
        ));
        assert!(css.contains(
            r#"#seq [id$="-stickTopArrowHead"] path,#seq [id$="-stickBottomArrowHead"] path{stroke:#2563eb;}#seq [id$="-sequencenumber"]{fill:#2563eb;}"#
        ));
        assert!(!css.contains(r#"#seq .note{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .activation0{stroke:#2563eb;}"#));
    }

    #[test]
    fn sequence_number_label_css_records_the_final_cascade_winner() {
        let mut css = String::new();
        let emission = write_sequence_css_with_theme_adapter(
            &mut css,
            "seq:prod",
            16.0,
            &json!({"themeVariables": {"sequenceNumberColor": "#fedcba"}}),
            SequenceThemeCssAdapter {
                sequence_number_fill: Some("#123456"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        let baseline = css
            .find(r#"#seq\:prod .sequenceNumber{fill:#fedcba;}"#)
            .expect("configured Sequence number baseline");
        let typed = css
            .find(r#"#seq\:prod .sequenceNumber,#seq\:prod .sequenceNumber>tspan{fill:#123456;}"#)
            .expect("typed Sequence number terminal owner");
        assert!(
            baseline < typed,
            "typed Sequence number CSS must win by order"
        );
        assert_eq!(emission.sequence_number_fill(), "#123456");
        assert_eq!(emission.typed_sequence_number_fill(), Some("#123456"));
    }

    #[test]
    fn sequence_lifeline_paint_css_is_scoped_to_actor_lines() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({"themeVariables": {"actorLineColor": "#444444"}}),
            SequenceThemeCssAdapter {
                lifeline_stroke: Some("#2563eb"),
                lifeline_stroke_width: Some(2.0),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .actor-line{stroke:#444444;}"#));
        assert!(css.contains(r#"#seq .actor-line{stroke:#2563eb;}"#));
        assert!(css.contains(r#"#seq .actor-line{stroke-width:2px;}"#));
        assert!(!css.contains(r#"#seq .actor{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .note{stroke:#2563eb;}"#));
    }

    #[test]
    fn sequence_note_paint_css_is_scoped_to_note_rects() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({
                "themeVariables": {
                    "noteBkgColor": "#dddddd",
                    "noteBorderColor": "#cccccc"
                }
            }),
            SequenceThemeCssAdapter {
                note_fill: Some("#dc2626"),
                note_stroke: Some("#2563eb"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .note{stroke:#cccccc;fill:#dddddd;}"#));
        assert!(css.contains(r#"#seq .note{fill:#dc2626;}#seq .note{stroke:#2563eb;}"#));
        assert!(!css.contains(r#"#seq .noteText{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageText{stroke:#2563eb;"#));
        assert!(!css.contains(r#"#seq .activation0{fill:#dc2626;"#));
    }

    #[test]
    fn sequence_activation_paint_css_is_scoped_to_activation_rects() {
        let css = sequence_css_with_theme_adapter(
            "seq",
            16.0,
            &json!({
                "themeVariables": {
                    "activationBkgColor": "#dddddd",
                    "activationBorderColor": "#cccccc"
                }
            }),
            SequenceThemeCssAdapter {
                activation_fill: Some("#dc2626"),
                activation_stroke: Some("#2563eb"),
                ..SequenceThemeCssAdapter::default()
            },
        );

        assert!(css.contains(r#"#seq .activation0{fill:#dddddd;stroke:#cccccc;}"#));
        assert!(css.contains(
            r#"#seq .activation0,#seq .activation1,#seq .activation2{fill:#dc2626;}#seq .activation0,#seq .activation1,#seq .activation2{stroke:#2563eb;}"#
        ));
        assert!(!css.contains(r#"#seq .note{fill:#dc2626;"#));
        assert!(!css.contains(r#"#seq .messageLine0{stroke:#2563eb;"#));
    }
}
