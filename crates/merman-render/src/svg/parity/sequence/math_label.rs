use super::super::*;
use crate::sequence::{SequenceMathHeightMode, prepared_sequence_math_terminal_geometry};

#[derive(Clone, Copy)]
pub(super) struct SequenceKatexLabel<'a> {
    pub(super) html: &'a str,
    pub(super) width: f64,
    pub(super) height: f64,
}

pub(super) fn sequence_katex_label<'a>(
    prepared: Option<&'a crate::math::PreparedMathLabel>,
    style: &TextStyle,
    height_mode: SequenceMathHeightMode,
) -> Option<SequenceKatexLabel<'a>> {
    let prepared = prepared?;
    let (width, height) =
        prepared_sequence_math_terminal_geometry(Some(prepared), style, height_mode)?
            .browser_box_size();
    Some(SequenceKatexLabel {
        html: prepared.browser_xhtml(),
        width,
        height,
    })
}

pub(super) fn write_sequence_katex_foreign_object(
    out: &mut impl SvgOutput,
    label: &SequenceKatexLabel<'_>,
    x: f64,
    y: f64,
) {
    let _ = write!(
        out,
        r#"<foreignObject height="{h}" width="{w}" x="{x}" y="{y}"><div style="width: fit-content;" xmlns="http://www.w3.org/1999/xhtml">{html}</div></foreignObject>"#,
        h = fmt(label.height),
        w = fmt(label.width),
        x = fmt(x),
        y = fmt(y),
        html = label.html,
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::math::{ConfiguredMathBackend, MathRenderer};
    use crate::text::{TextMetrics, TextStyle};

    use super::*;

    #[derive(Debug)]
    struct FixedSequenceMathRenderer;

    impl MathRenderer for FixedSequenceMathRenderer {
        fn render_html_label(
            &self,
            _text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            None
        }

        fn render_sequence_html_label(
            &self,
            _text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            Some("<span class=\"prepared\">x²</span>".to_owned())
        }

        fn measure_sequence_html_label_with_style(
            &self,
            _text: &str,
            _config: &merman_core::MermaidConfig,
            style: &TextStyle,
        ) -> Option<TextMetrics> {
            Some(TextMetrics {
                width: 33.0,
                height: style.font_size,
                line_count: 1,
            })
        }
    }

    #[test]
    fn sequence_writer_reads_browser_projection_without_reentering_a_backend() {
        let backend = ConfiguredMathBackend::external(Arc::new(FixedSequenceMathRenderer));
        let config = merman_core::MermaidConfig::default();
        let style = TextStyle {
            font_size: 29.0,
            ..TextStyle::default()
        };
        let meter = Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let outcome = backend
            .prepare(
                crate::math::PrepareMathLabelRequest::sequence(
                    "$$x^2$$", &config, &style, "#e5e7eb",
                ),
                &meter,
            )
            .unwrap();
        let prepared = outcome.prepared().expect("prepared Sequence math");

        let label = sequence_katex_label(Some(prepared), &style, SequenceMathHeightMode::Draw)
            .expect("writer projection");

        assert_eq!(label.html, prepared.browser_xhtml());
        assert_eq!(label.width, 33.0);
        assert!(label.height >= 29.0);
    }
}
