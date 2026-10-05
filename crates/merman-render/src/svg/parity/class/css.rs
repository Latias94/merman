use super::super::*;

fn write_class_marker_css(
    out: &mut impl SvgOutput,
    id: &str,
    marker_id_suffix: &str,
    marker_class: &str,
    fill: &str,
    line_color: &str,
) {
    let _ = write!(
        out,
        r#"#{} [id$="-{}"],#{} .{}{{fill:{}!important;stroke:{}!important;stroke-width:1;}}"#,
        id, marker_id_suffix, id, marker_class, fill, line_color
    );
}
fn write_class_icon_css(out: &mut impl SvgOutput, id: &str) {
    let _ = write!(
        out,
        r#"#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        id, id
    );
}

// The same palette gate drives CSS slots and node stamping (colorThemeGate.ts).
pub(super) fn class_palette_size(config: &serde_json::Value) -> usize {
    if !matches!(
        config.get("theme").and_then(serde_json::Value::as_str),
        Some("redux-color" | "redux-dark-color")
    ) {
        return 0;
    }
    config
        .pointer("/themeVariables/borderColorArray")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

fn write_class_palette_css(
    out: &mut impl SvgOutput,
    id: &str,
    config: &serde_json::Value,
) -> Result<()> {
    if class_palette_size(config) == 0 {
        return out.checkpoint();
    }
    let Some(borders) = config
        .pointer("/themeVariables/borderColorArray")
        .and_then(serde_json::Value::as_array)
    else {
        return out.checkpoint();
    };
    let backgrounds = config
        .pointer("/themeVariables/bkgColorArray")
        .and_then(serde_json::Value::as_array)
        .filter(|colors| !colors.is_empty());
    let look = config
        .get("look")
        .and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|look| {
            !look.is_empty()
                && look
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .unwrap_or_else(|| "classic".into());
    let color = |value: &serde_json::Value| {
        // Stylis removes declaration-value whitespace from upstream generated CSS.
        value
            .as_str()
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|| value.to_string())
    };
    for (index, border) in borders.iter().enumerate() {
        let border = color(border);
        let _ = write!(
            out,
            r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].node .outer-path path{{stroke:{border};"#
        );
        if let Some(backgrounds) = backgrounds {
            let _ = write!(
                out,
                "fill:{};",
                color(&backgrounds[index % backgrounds.len()])
            );
        }
        out.push('}');
        let _ = write!(
            out,
            r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].node .divider path{{stroke:{border};}}"#
        );
        out.checkpoint()?;
    }
    Ok(())
}

pub(super) fn write_class_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    stylesheet_font_family: &str,
    stylesheet_font_size: &str,
    seal_typography_emission: bool,
) -> Result<Option<crate::class::ClassTypographyCssEmission>> {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let resolved_font_family = normalize_css_font_family(stylesheet_font_family);
    let info_css = super::super::css::InfoCssWriter::with_resolved_typography(
        effective_config,
        resolved_font_family.as_str(),
        stylesheet_font_size,
    );
    let theme = MermaidThemeAdapter::new(effective_config).class_diagram();
    let font_family = info_css.font_family();
    let class_text = theme.class_text.as_str();
    let note_text = theme.note_text.as_str();
    let line_color = theme.common.line_color.as_str();
    let main_bkg = theme.main_bkg.as_str();
    let node_border = theme.node_border.as_str();
    let class_group_text = theme.class_group_text.as_str();
    let cluster_bkg = theme.cluster_bkg.as_str();
    let cluster_border = theme.cluster_border.as_str();
    let title_color = theme.title_color.as_str();
    let text_color = theme.text_color.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let edge_label_background = theme_token(
        effective_config,
        "edgeLabelBackground",
        "rgba(232,232,232, 0.8)",
    );

    let base_font_emission = info_css.write_prefix(out, diagram_id)?;
    write_class_palette_css(out, &id, effective_config)?;

    let _ = write!(
        out,
        r#"#{} g.classGroup text{{fill:{};stroke:none;font-family:{};font-size:10px;}}#{} g.classGroup text .title{{font-weight:bolder;}}#{} .cluster-label text{{fill:{};}}#{} .cluster-label span{{color:{};}}#{} .cluster-label span p{{background-color:transparent;}}#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span{{color:{};}}#{} .nodeLabel,#{} .edgeLabel{{color:{};}}#{} .noteLabel .nodeLabel,#{} .noteLabel .edgeLabel{{color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .label text{{fill:{};}}#{} .labelBkg{{background:{};}}#{} .edgeLabel .label span{{background:{};}}#{} .classTitle{{font-weight:bolder;}}"#,
        id.as_str(),
        class_group_text,
        font_family,
        id.as_str(),
        id.as_str(),
        title_color,
        id.as_str(),
        title_color,
        id.as_str(),
        id.as_str(),
        cluster_bkg,
        cluster_border,
        id.as_str(),
        title_color,
        id.as_str(),
        title_color,
        id.as_str(),
        id.as_str(),
        class_text,
        id.as_str(),
        id.as_str(),
        note_text,
        id.as_str(),
        main_bkg,
        id.as_str(),
        class_text,
        id.as_str(),
        main_bkg,
        id.as_str(),
        main_bkg,
        id.as_str()
    );
    out.checkpoint()?;
    let class_group_font_family_css = font_family;
    let _ = write!(
        out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:{};}}#{} .divider{{stroke:{};stroke-width:1;}}#{} g.clickable{{cursor:pointer;}}#{} g.classGroup rect{{fill:{};stroke:{};}}#{} g.classGroup line{{stroke:{};stroke-width:1;}}#{} .classLabel .box{{stroke:none;stroke-width:0;fill:{};opacity:0.5;}}#{} .classLabel .label{{fill:{};font-size:10px;}}#{} .relation{{stroke:{};stroke-width:{};fill:none;}}#{} .dashed-line{{stroke-dasharray:3;}}#{} .dotted-line{{stroke-dasharray:1 2;}}"#,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        main_bkg,
        node_border,
        stroke_width,
        id.as_str(),
        node_border,
        id.as_str(),
        id.as_str(),
        main_bkg,
        node_border,
        id.as_str(),
        node_border,
        id.as_str(),
        main_bkg,
        id.as_str(),
        node_border,
        id.as_str(),
        line_color,
        stroke_width,
        id.as_str(),
        id.as_str()
    );
    out.checkpoint()?;

    for (marker_id_suffix, marker_class, fill) in [
        ("compositionStart", "composition", line_color),
        ("compositionEnd", "composition", line_color),
        ("dependencyStart", "dependency", line_color),
        ("dependencyEnd", "dependency", line_color),
        ("extensionStart", "extension", "transparent"),
        ("extensionEnd", "extension", "transparent"),
        ("aggregationStart", "aggregation", "transparent"),
        ("aggregationEnd", "aggregation", "transparent"),
        ("lollipopStart", "lollipop", main_bkg),
        ("lollipopEnd", "lollipop", main_bkg),
    ] {
        write_class_marker_css(out, &id, marker_id_suffix, marker_class, fill, line_color);
        out.checkpoint()?;
    }

    let _ = write!(
        out,
        r#"#{} .edgeTerminals{{font-size:{}px;line-height:initial;}}#{} .classTitleText{{text-anchor:middle;font-size:18px;fill:{};}}#{} .edgeLabel[data-look="neo"]{{background-color:{};text-align:center;}}#{} .edgeLabel[data-look="neo"] p{{background-color:{};}}#{} .edgeLabel[data-look="neo"] rect{{opacity:0.5;background-color:{};fill:{};}}"#,
        id.as_str(),
        fmt(crate::class::CLASS_CARDINALITY_FONT_SIZE_PX),
        id.as_str(),
        text_color,
        id.as_str(),
        edge_label_background,
        id.as_str(),
        edge_label_background,
        id.as_str(),
        edge_label_background,
        edge_label_background
    );
    out.checkpoint()?;

    write_class_icon_css(out, &id);
    out.checkpoint()?;
    let root_font_emission = info_css.write_root(out, diagram_id, diagram_id)?;

    Ok(seal_typography_emission.then(|| {
        crate::class::ClassTypographyCssEmission::from_successful_writes(
            base_font_emission.diagram_root_font_family_css(),
            base_font_emission.nested_svg_font_family_css(),
            base_font_emission.diagram_root_font_size_css(),
            base_font_emission.nested_svg_font_size_css(),
            class_group_font_family_css,
            root_font_emission.font_family_css(),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct CheckpointSink {
        reject_at: Option<usize>,
        checkpoint_count: usize,
        retained: String,
    }

    impl CheckpointSink {
        fn rejecting(checkpoint: usize) -> Self {
            Self {
                reject_at: Some(checkpoint),
                ..Self::default()
            }
        }
    }

    impl fmt::Write for CheckpointSink {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl SvgOutput for CheckpointSink {
        fn push_str(&mut self, value: &str) {
            self.retained.push_str(value);
        }

        fn push(&mut self, value: char) {
            self.retained.push(value);
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            self.checkpoint_count += 1;
            if self.reject_at == Some(self.checkpoint_count) {
                return Err(crate::Error::InvalidModel {
                    message: "test Class CSS sink rejected a checkpoint".to_string(),
                });
            }
            Ok(())
        }
    }

    #[test]
    fn class_css_emission_requires_every_writer_checkpoint() {
        let config = serde_json::json!({});
        let mut successful = CheckpointSink::default();
        let emission = write_class_css(
            &mut successful,
            "class-css-receipt",
            &config,
            "Inter,sans-serif",
            "18px",
            true,
        )
        .expect("write complete Class CSS");
        assert!(emission.is_some());

        for checkpoint in 1..=successful.checkpoint_count {
            let mut rejecting = CheckpointSink::rejecting(checkpoint);
            let error = write_class_css(
                &mut rejecting,
                "class-css-receipt",
                &config,
                "Inter,sans-serif",
                "18px",
                true,
            )
            .expect_err("a failed Class CSS checkpoint must prevent receipt emission");
            assert!(matches!(error, crate::Error::InvalidModel { .. }));
        }
    }
}
