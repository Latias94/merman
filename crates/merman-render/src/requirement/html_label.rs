//! Requirement's sanitized XHTML shared by layout, measurement replay, and SVG emission.

use std::cell::Cell;
use std::sync::Arc;

use merman_markup::sanitize::SanitizeFailure;
use merman_model::{MermaidConfig, OperationPhase};

use crate::Result;
use crate::html::{HtmlOutputError, HtmlSanitizeSink};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};

#[derive(Debug)]
pub(crate) struct RequirementHtmlLabel {
    xhtml: String,
    _retained: PreparedTextRetainedReservation,
}

impl RequirementHtmlLabel {
    pub(super) fn prepare(
        raw: &str,
        config: &MermaidConfig,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Arc<Self>> {
        let owner_bytes = std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>();
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes)?;
        work_meter.charge(raw.len())?;
        let max = work_meter
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let parse_work = Cell::new(0);
        let sink = HtmlSanitizeSink::new(max, work_meter, &parse_work, OperationPhase::Layout);
        let decoded = raw
            .replace("ﬂ°°", "&#")
            .replace("ﬂ°", "&")
            .replace("¶ß", ";");
        let sanitized =
            match merman_markup::sanitize::sanitize_text_with_sink(&decoded, config, &sink) {
                Ok(sanitized) => sanitized,
                Err(SanitizeFailure::Output(HtmlOutputError::Operation(error))) => {
                    return Err(error.into());
                }
                Err(SanitizeFailure::Output(HtmlOutputError::Limit { attempted })) => {
                    work_meter.preflight_prepared_text_retained_bytes(
                        owner_bytes.saturating_add(attempted),
                    )?;
                    String::new()
                }
                // Keep the existing sanitizer's fail-closed input/allocation behavior.
                Err(_) => String::new(),
            };
        work_meter.charge(sanitized.len())?;
        let xhtml = crate::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, true);
        work_meter.checkpoint(OperationPhase::Layout)?;
        let retained = work_meter
            .reserve_prepared_text_retained_bytes(owner_bytes.saturating_add(xhtml.capacity()))?;
        Ok(Arc::new(Self {
            xhtml,
            _retained: retained,
        }))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.xhtml
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn prepared_xhtml_retains_breaks_code_and_inline_fonts_until_its_last_owner_drops() {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let config = MermaidConfig::from_value(serde_json::json!({"securityLevel": "loose"}));
        let label = RequirementHtmlLabel::prepare(
            "First<br/><br/><code>Account</code><span style='font-size:48px'>Last</span>",
            &config,
            &meter,
        )
        .expect("prepare Requirement's final HTML projection");
        let fragment = format!("<root>{}</root>", label.as_str());
        let document = crate::xml_dom::Document::parse(&fragment).expect("valid prepared XHTML");
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("br"))
                .count(),
            2,
        );
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("code") && node.text() == Some("Account") })
        );
        assert!(document.descendants().any(|node| {
            node.has_tag_name("span")
                && node
                    .attribute("style")
                    .is_some_and(|style| style.contains("48px"))
        }));
        let bytes = meter.prepared_text_retained_bytes();
        assert!(bytes >= label.xhtml.capacity());
        assert!(meter.used() > 0);
        assert_eq!(meter.projected_svg_bytes(), 0);
        let replay_owner = Arc::clone(&label);
        drop(label);
        assert_eq!(meter.prepared_text_retained_bytes(), bytes);
        drop(replay_owner);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
