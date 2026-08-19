use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

use super::{FlowchartConfigView, FlowchartLayoutSettings};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::text::TextStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartBaseTypographyOutcome {
    Inactive,
    Typed,
    ConfigOwned,
    Unsupported,
}

#[derive(Debug, Default)]
struct FlowchartBaseTypographyReceipt {
    stylesheet_verified: bool,
    visible_text_occurrences: usize,
}

#[derive(Debug)]
struct FlowchartBaseTypographyPlanInner {
    font_family_css: Box<str>,
    font_size_px: f64,
    html_font_size_px: f64,
    outcome: FlowchartBaseTypographyOutcome,
    terminal_receipt: OnceLock<FlowchartBaseTypographyReceipt>,
}

/// One family-local winner shared by Flowchart/Swimlane layout, SVG emission, and evidence.
///
/// The module deliberately owns only family-wide `FontStack` and `FontSize`. Label-local source
/// styles and typed target rules retain their existing, more specific winner paths.
#[derive(Debug, Clone)]
pub(crate) struct FlowchartBaseTypographyPlan {
    inner: Arc<FlowchartBaseTypographyPlanInner>,
}

#[derive(Debug)]
pub(crate) struct FlowchartBaseTypographyStyles {
    pub(crate) font_family: String,
    pub(crate) font_size: f64,
    pub(crate) text_style: TextStyle,
    pub(crate) html_label_text_style: TextStyle,
}

impl FlowchartBaseTypographyPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let config = FlowchartConfigView::new(effective_config.as_value());
        let configured_font_family = config.font_family();
        let configured_font_size = config.render_font_size();
        let configured_text_style =
            config.render_text_style(&configured_font_family, configured_font_size);
        let configured_html_style =
            config.html_label_measurement_base_style(&configured_text_style);

        let Some(theme) = theme else {
            return Self::new(
                configured_font_family,
                configured_font_size,
                configured_html_style.font_size,
                FlowchartBaseTypographyOutcome::Inactive,
            );
        };

        let ownership = super::flowchart_typography_config_ownership(effective_config);
        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_typography = false;
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let typed_font_stack_applied = typed_font_stack && !ownership.font_stack;
        let typed_font_size_applied = typed_font_size && !ownership.font_size;
        let font_family = if typed_font_stack_applied {
            theme.typography().font_stack().as_css()
        } else {
            configured_font_family
        };
        let font_size = if typed_font_size_applied {
            f64::from(theme.typography().font_size_px())
        } else {
            configured_font_size
        };
        let html_font_size = if typed_font_size_applied {
            font_size
        } else {
            configured_html_style.font_size
        };
        let requested = typed_font_stack || typed_font_size;
        let applied = typed_font_stack_applied || typed_font_size_applied;
        let outcome = if unsupported_typography {
            FlowchartBaseTypographyOutcome::Unsupported
        } else if applied {
            FlowchartBaseTypographyOutcome::Typed
        } else if requested {
            FlowchartBaseTypographyOutcome::ConfigOwned
        } else {
            FlowchartBaseTypographyOutcome::Inactive
        };

        Self::new(font_family, font_size, html_font_size, outcome)
    }

    fn new(
        font_family_css: String,
        font_size_px: f64,
        html_font_size_px: f64,
        outcome: FlowchartBaseTypographyOutcome,
    ) -> Self {
        Self {
            inner: Arc::new(FlowchartBaseTypographyPlanInner {
                font_family_css: font_family_css.into_boxed_str(),
                font_size_px,
                html_font_size_px,
                outcome,
                terminal_receipt: OnceLock::new(),
            }),
        }
    }

    pub(crate) fn layout_settings(
        &self,
        effective_config: &serde_json::Value,
    ) -> FlowchartLayoutSettings {
        let config = FlowchartConfigView::new(effective_config);
        let mut settings = config.layout_settings();
        self.apply_to_styles(
            &mut settings.text_style,
            &mut settings.html_label_text_style,
        );
        settings
    }

    pub(crate) fn render_styles(
        &self,
        effective_config: &serde_json::Value,
    ) -> FlowchartBaseTypographyStyles {
        let config = FlowchartConfigView::new(effective_config);
        let mut text_style =
            config.render_text_style(self.inner.font_family_css.as_ref(), self.inner.font_size_px);
        let mut html_label_text_style = config.html_label_measurement_base_style(&text_style);
        self.apply_to_styles(&mut text_style, &mut html_label_text_style);
        FlowchartBaseTypographyStyles {
            font_family: self.inner.font_family_css.to_string(),
            font_size: self.inner.font_size_px,
            text_style,
            html_label_text_style,
        }
    }

    fn apply_to_styles(&self, text_style: &mut TextStyle, html_label_text_style: &mut TextStyle) {
        text_style.font_family = Some(self.inner.font_family_css.to_string());
        text_style.font_size = self.inner.font_size_px;
        html_label_text_style.font_family = Some(self.inner.font_family_css.to_string());
        html_label_text_style.font_size = self.inner.html_font_size_px;
    }

    pub(crate) fn record_terminal_svg(&self, svg: &str, diagram_id: &str) -> bool {
        self.inner
            .terminal_receipt
            .set(observe_terminal_svg(self, svg, diagram_id))
            .is_ok()
    }

    pub(crate) fn finish_evidence(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let Some(receipt) = self.inner.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        if receipt.visible_text_occurrences == 0 {
            evidence.mark_not_applicable(key);
            return evidence;
        }

        match self.inner.outcome {
            FlowchartBaseTypographyOutcome::Typed if receipt.stylesheet_verified => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            FlowchartBaseTypographyOutcome::Typed | FlowchartBaseTypographyOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            FlowchartBaseTypographyOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            FlowchartBaseTypographyOutcome::Inactive => {}
        }
        evidence
    }
}

fn observe_terminal_svg(
    plan: &FlowchartBaseTypographyPlan,
    svg: &str,
    diagram_id: &str,
) -> FlowchartBaseTypographyReceipt {
    scan_terminal_svg(svg, diagram_id)
        .map(|observation| FlowchartBaseTypographyReceipt {
            stylesheet_verified: typography_rule_matches(
                &observation.stylesheet,
                &format!("#{}", crate::svg::escape_css_identifier(diagram_id)),
                plan.inner.font_family_css.as_ref(),
                plan.inner.font_size_px,
                true,
            ) && typography_rule_matches(
                &observation.stylesheet,
                &format!("#{} svg", crate::svg::escape_css_identifier(diagram_id)),
                plan.inner.font_family_css.as_ref(),
                plan.inner.font_size_px,
                true,
            ) && typography_rule_matches(
                &observation.stylesheet,
                &format!("#{} .label", crate::svg::escape_css_identifier(diagram_id)),
                plan.inner.font_family_css.as_ref(),
                plan.inner.font_size_px,
                false,
            ),
            visible_text_occurrences: observation.visible_text_occurrences,
        })
        .unwrap_or_default()
}

const SVG_NAMESPACE: &[u8] = b"http://www.w3.org/2000/svg";

#[derive(Debug)]
struct FlowchartTerminalSvgObservation {
    stylesheet: String,
    visible_text_occurrences: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleTextKind {
    NativeText,
    ForeignObject,
}

#[derive(Debug)]
struct OpenVisibleText {
    depth: usize,
    kind: VisibleTextKind,
    has_content: bool,
}

#[derive(Debug)]
struct OpenStylesheet {
    depth: usize,
    css: String,
}

fn scan_terminal_svg(svg: &str, diagram_id: &str) -> Option<FlowchartTerminalSvgObservation> {
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut stylesheet_count = 0usize;
    let mut stylesheet = None;
    let mut open_stylesheet: Option<OpenStylesheet> = None;
    let mut open_visible_text = Vec::<OpenVisibleText>::new();
    let mut visible_text_occurrences = 0usize;

    loop {
        match reader.read_event().ok()? {
            Event::Start(element) => {
                if depth == 0 {
                    if root_seen || root_closed || !is_svg_element(&reader, &element, b"svg") {
                        return None;
                    }
                    if element_attribute(&element, b"id")?.as_deref() != Some(diagram_id) {
                        return None;
                    }
                    root_seen = true;
                }

                if open_stylesheet.is_some() {
                    return None;
                }
                if is_svg_element(&reader, &element, b"style") {
                    stylesheet_count = stylesheet_count.checked_add(1)?;
                    open_stylesheet = Some(OpenStylesheet {
                        depth,
                        css: String::new(),
                    });
                }

                let visible_kind = if is_svg_element(&reader, &element, b"text") {
                    Some(VisibleTextKind::NativeText)
                } else if is_svg_element(&reader, &element, b"foreignObject") {
                    Some(VisibleTextKind::ForeignObject)
                } else {
                    None
                };
                if let Some(kind) = visible_kind {
                    open_visible_text.push(OpenVisibleText {
                        depth,
                        kind,
                        has_content: false,
                    });
                }
                if element_has_class_token(&element, "merman-prepared-math")? {
                    mark_open_foreign_objects_visible(&mut open_visible_text);
                }
                depth = depth.checked_add(1)?;
            }
            Event::Empty(element) => {
                if depth == 0 {
                    if root_seen || root_closed || !is_svg_element(&reader, &element, b"svg") {
                        return None;
                    }
                    if element_attribute(&element, b"id")?.as_deref() != Some(diagram_id) {
                        return None;
                    }
                    root_seen = true;
                    root_closed = true;
                }
                if open_stylesheet.is_some() {
                    return None;
                }
                if is_svg_element(&reader, &element, b"style") {
                    stylesheet_count = stylesheet_count.checked_add(1)?;
                    if stylesheet.replace(String::new()).is_some() {
                        return None;
                    }
                }
                if element_has_class_token(&element, "merman-prepared-math")? {
                    mark_open_foreign_objects_visible(&mut open_visible_text);
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1)?;
                if open_stylesheet
                    .as_ref()
                    .is_some_and(|capture| capture.depth == depth)
                {
                    let capture = open_stylesheet.take()?;
                    if stylesheet.replace(capture.css).is_some() {
                        return None;
                    }
                }
                if open_visible_text
                    .last()
                    .is_some_and(|candidate| candidate.depth == depth)
                {
                    let candidate = open_visible_text.pop()?;
                    if candidate.has_content {
                        visible_text_occurrences = visible_text_occurrences.checked_add(1)?;
                    }
                }
                if depth == 0 {
                    root_closed = true;
                }
            }
            Event::Text(text) => {
                let text = text.xml10_content().ok()?;
                observe_terminal_character_content(
                    text.as_ref(),
                    depth,
                    &mut open_stylesheet,
                    &mut open_visible_text,
                )?;
            }
            Event::CData(text) => {
                let text = text.xml10_content().ok()?;
                observe_terminal_character_content(
                    text.as_ref(),
                    depth,
                    &mut open_stylesheet,
                    &mut open_visible_text,
                )?;
            }
            Event::GeneralRef(reference) => {
                let character = resolve_xml_reference(&reference)?;
                let mut encoded = [0u8; 4];
                observe_terminal_character_content(
                    character.encode_utf8(&mut encoded),
                    depth,
                    &mut open_stylesheet,
                    &mut open_visible_text,
                )?;
            }
            Event::PI(_) | Event::DocType(_) => return None,
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }

    if depth != 0
        || !root_seen
        || !root_closed
        || stylesheet_count != 1
        || open_stylesheet.is_some()
        || !open_visible_text.is_empty()
    {
        return None;
    }
    Some(FlowchartTerminalSvgObservation {
        stylesheet: stylesheet?,
        visible_text_occurrences,
    })
}

fn is_svg_element(reader: &NsReader<&[u8]>, element: &BytesStart<'_>, name: &[u8]) -> bool {
    let (namespace, local_name) = reader.resolver().resolve_element(element.name());
    matches!(namespace, ResolveResult::Bound(namespace) if namespace.as_ref() == SVG_NAMESPACE)
        && local_name.as_ref() == name
}

fn element_attribute(element: &BytesStart<'_>, name: &[u8]) -> Option<Option<String>> {
    let mut value = None;
    for attribute in element.attributes() {
        let attribute = attribute.ok()?;
        if attribute.key.as_ref() != name {
            continue;
        }
        if value.is_some() {
            return None;
        }
        value = Some(
            attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, element.decoder())
                .ok()?
                .into_owned(),
        );
    }
    Some(value)
}

fn element_has_class_token(element: &BytesStart<'_>, expected: &str) -> Option<bool> {
    Some(
        element_attribute(element, b"class")?.is_some_and(|classes| {
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected)
        }),
    )
}

fn mark_open_foreign_objects_visible(open_visible_text: &mut [OpenVisibleText]) {
    for candidate in open_visible_text {
        if candidate.kind == VisibleTextKind::ForeignObject {
            candidate.has_content = true;
        }
    }
}

fn observe_terminal_character_content(
    content: &str,
    depth: usize,
    open_stylesheet: &mut Option<OpenStylesheet>,
    open_visible_text: &mut [OpenVisibleText],
) -> Option<()> {
    if let Some(stylesheet) = open_stylesheet {
        stylesheet.css.push_str(content);
        return Some(());
    }
    if depth == 0 {
        return content.trim().is_empty().then_some(());
    }
    if !content.trim().is_empty() {
        for candidate in open_visible_text {
            candidate.has_content = true;
        }
    }
    Some(())
}

fn resolve_xml_reference(reference: &BytesRef<'_>) -> Option<char> {
    if let Some(value) = reference.resolve_char_ref().ok()? {
        return crate::xml::is_xml_1_0_char(value).then_some(value);
    }
    match reference.decode().ok()?.as_ref() {
        "amp" => Some('&'),
        "apos" => Some('\''),
        "gt" => Some('>'),
        "lt" => Some('<'),
        "quot" => Some('"'),
        _ => None,
    }
}

fn typography_rule_matches(
    stylesheet: &str,
    selector: &str,
    expected_font_family: &str,
    expected_font_size: f64,
    require_font_size: bool,
) -> bool {
    let Some(body) = unique_rule_body(stylesheet, selector) else {
        return false;
    };
    let mut font_family_count = 0usize;
    let mut font_size_count = 0usize;
    for declaration in body.split_terminator(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            return false;
        };
        match property {
            "font-family" => {
                font_family_count = font_family_count.saturating_add(1);
                if value != expected_font_family {
                    return false;
                }
            }
            "font-size" => {
                font_size_count = font_size_count.saturating_add(1);
                let Some(value) = value.strip_suffix("px") else {
                    return false;
                };
                let Ok(value) = value.parse::<f64>() else {
                    return false;
                };
                if value.to_bits() != expected_font_size.to_bits() {
                    return false;
                }
            }
            _ => {}
        }
    }
    font_family_count == 1 && (!require_font_size || font_size_count == 1)
}

fn unique_rule_body<'a>(stylesheet: &'a str, selector: &str) -> Option<&'a str> {
    let prefix = format!("{selector}{{");
    let mut matches = stylesheet.match_indices(&prefix);
    let (_, suffix) = matches.next().map(|(offset, _)| {
        let body_start = offset.saturating_add(prefix.len());
        (offset, &stylesheet[body_start..])
    })?;
    if matches.next().is_some() {
        return None;
    }
    let end = suffix.find('}')?;
    Some(&suffix[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    fn typed_plan(config: MermaidConfig) -> FlowchartBaseTypographyPlan {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("valid test font"))
            .with_font_size_px(23.0)
            .expect("valid test font size");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::FLOWCHART, typography),
            ))
            .expect("compile Flowchart base typography test theme");
        FlowchartBaseTypographyPlan::resolve(
            Some(&theme.resolve(DiagramFamilyId::FLOWCHART)),
            &config,
        )
    }

    #[test]
    fn typed_base_typography_overrides_layout_and_html_measurement_styles() {
        let plan = typed_plan(MermaidConfig::from_value(serde_json::json!({})));
        let settings = plan.layout_settings(&serde_json::json!({}));

        assert_eq!(
            settings.text_style.font_family.as_deref(),
            Some("Excalifont")
        );
        assert_eq!(settings.text_style.font_size, 23.0);
        assert_eq!(
            settings.html_label_text_style.font_family.as_deref(),
            Some("Excalifont")
        );
        assert_eq!(settings.html_label_text_style.font_size, 23.0);
    }

    #[test]
    fn terminal_receipt_streams_deep_svg_without_recursive_tree_materialization() {
        let plan = typed_plan(MermaidConfig::from_value(serde_json::json!({})));
        let diagram_id = "deep-flowchart";
        let mut svg = format!(
            concat!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"{}\">",
                "<style>",
                "#{}{{font-family:Excalifont;font-size:23px;}}",
                "#{} svg{{font-family:Excalifont;font-size:23px;}}",
                "#{} .label{{font-family:Excalifont;}}",
                "</style>"
            ),
            diagram_id, diagram_id, diagram_id, diagram_id
        );
        for _ in 0..8_192 {
            svg.push_str("<g>");
        }
        svg.push_str("<text>deep label</text>");
        for _ in 0..8_192 {
            svg.push_str("</g>");
        }
        svg.push_str("</svg>");

        let receipt = observe_terminal_svg(&plan, &svg, diagram_id);

        assert!(receipt.stylesheet_verified);
        assert_eq!(receipt.visible_text_occurrences, 1);
    }
}
