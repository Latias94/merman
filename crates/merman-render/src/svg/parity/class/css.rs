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

fn write_class_palette_css(
    out: &mut impl SvgOutput,
    id: &str,
    theme: &crate::class::ClassCssThemeBinding,
) -> Result<()> {
    if theme.palette.is_empty() {
        return out.checkpoint();
    }
    let look = &theme.palette_look;
    for (index, (border, background)) in theme.palette.iter().enumerate() {
        let _ = write!(
            out,
            r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].node .outer-path path{{stroke:{border};"#
        );
        if let Some(background) = background {
            let _ = write!(out, "fill:{background};");
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
    typography_theme: &crate::class::ClassTextThemePlan,
    relation_theme: &crate::class::ClassRelationThemePlan,
) -> Result<()> {
    let theme = typography_theme.css_binding();
    let id = crate::svg::escape_css_identifier(diagram_id);
    let info_css = typography_theme.common_css();
    let font_family = info_css.font_family();
    let class_text = theme.class_text.as_str();
    let note_text = theme.note_text.as_str();
    let line_color = relation_theme.relation_css_default();
    let main_bkg = theme.main_bkg.as_str();
    let node_border = theme.node_border.as_str();
    let class_group_text = theme.class_group_text.as_str();
    let (cluster_bkg, cluster_border) = relation_theme.cluster_css_defaults();
    let title_color = relation_theme.namespace_title_css_default();
    let text_color = theme.text_color.as_str();
    let stroke_width = relation_theme.relation_width().compatibility_css();
    let edge_label_background = theme.edge_label_background.as_str();

    let _base_font_emission = info_css.write_prefix_with_font_emission(out, diagram_id)?;
    write_class_palette_css(out, &id, theme)?;

    let _ = write!(
        out,
        r#"#{} g.classGroup text{{fill:{};stroke:none;font-family:{};font-size:10px;}}#{} g.classGroup text .title{{font-weight:bolder;}}#{} .cluster-label text{{fill:{};}}#{} .cluster-label span{{color:{};}}#{} .cluster-label span p{{background-color:transparent;}}#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span{{color:{};}}#{} .nodeLabel,#{} .edgeLabel{{color:{};}}#{} .noteLabel .nodeLabel,#{} .noteLabel .edgeLabel{{color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .label text{{fill:{};}}#{} .labelBkg{{background:{};}}#{} .edgeLabel .label span{{background:{};}}#{} .classTitle{{font-weight:bolder;}}"#,
        id.as_ref(),
        class_group_text,
        font_family,
        id.as_ref(),
        id.as_ref(),
        title_color,
        id.as_ref(),
        title_color,
        id.as_ref(),
        id.as_ref(),
        cluster_bkg,
        cluster_border,
        id.as_ref(),
        title_color,
        id.as_ref(),
        title_color,
        id.as_ref(),
        id.as_ref(),
        class_text,
        id.as_ref(),
        id.as_ref(),
        note_text,
        id.as_ref(),
        main_bkg,
        id.as_ref(),
        class_text,
        id.as_ref(),
        main_bkg,
        id.as_ref(),
        main_bkg,
        id.as_ref()
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:{};}}#{} .divider{{stroke:{};stroke-width:1;}}#{} g.clickable{{cursor:pointer;}}#{} g.classGroup rect{{fill:{};stroke:{};}}#{} g.classGroup line{{stroke:{};stroke-width:1;}}#{} .classLabel .box{{stroke:none;stroke-width:0;fill:{};opacity:0.5;}}#{} .classLabel .label{{fill:{};font-size:10px;}}#{} .relation{{stroke:{};stroke-width:{};fill:none;}}#{} .dashed-line{{stroke-dasharray:3;}}#{} .dotted-line{{stroke-dasharray:1 2;}}"#,
        id.as_ref(),
        id.as_ref(),
        id.as_ref(),
        id.as_ref(),
        id.as_ref(),
        main_bkg,
        node_border,
        stroke_width,
        id.as_ref(),
        node_border,
        id.as_ref(),
        id.as_ref(),
        main_bkg,
        node_border,
        id.as_ref(),
        node_border,
        id.as_ref(),
        main_bkg,
        id.as_ref(),
        node_border,
        id.as_ref(),
        line_color,
        stroke_width,
        id.as_ref(),
        id.as_ref()
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
        id.as_ref(),
        fmt(crate::class::CLASS_CARDINALITY_FONT_SIZE_PX),
        id.as_ref(),
        text_color,
        id.as_ref(),
        edge_label_background,
        id.as_ref(),
        edge_label_background,
        id.as_ref(),
        edge_label_background,
        edge_label_background
    );
    out.checkpoint()?;

    write_class_icon_css(out, &id);
    out.checkpoint()?;
    let _root_font_emission =
        info_css.write_root_with_font_emission(out, diagram_id, diagram_id)?;

    Ok(())
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
    fn class_css_propagates_writer_checkpoint_failure() {
        let mermaid_config = merman_core::presentation::PresentationConfig::default();
        let typography_theme = crate::class::ClassTextThemePlan::resolve(None, &mermaid_config);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let relation_theme =
            crate::class::ClassRelationThemePlan::resolve(None, &mermaid_config, 0, 0, &meter)
                .unwrap()
                .with_cluster_domain(None, &mermaid_config, 0, &meter)
                .unwrap();
        let mut successful = CheckpointSink::default();
        write_class_css(
            &mut successful,
            "class-css-receipt",
            &typography_theme,
            &relation_theme,
        )
        .expect("write complete Class CSS");

        for checkpoint in 1..=successful.checkpoint_count {
            let mut rejecting = CheckpointSink::rejecting(checkpoint);
            let error = write_class_css(
                &mut rejecting,
                "class-css-receipt",
                &typography_theme,
                &relation_theme,
            )
            .expect_err("a failed Class CSS checkpoint must prevent successful CSS output");
            assert!(matches!(error, crate::Error::InvalidModel { .. }));
        }
    }
}
