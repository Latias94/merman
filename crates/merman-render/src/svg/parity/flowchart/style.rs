//! Flowchart style compilation helpers.

use super::*;
use crate::flowchart::{FlowchartSourceFacetStatus, FlowchartStyleDeclaration};
use cssparser::{BasicParseErrorKind, Parser, Token};
use indexmap::IndexMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(in crate::svg::parity) struct FlowchartCompiledStyles {
    edge_class_declarations: Vec<Arc<FlowchartStyleDeclaration>>,
    edge_animation_active: Option<bool>,
    pub(super) node_style: String,
    pub(super) rectangle_size: [Option<f64>; 2],
    pub(super) label_style: String,
    pub(super) label_div_decls: Vec<(String, String)>,
    pub(super) fill: Option<String>,
    pub(super) fill_opacity: f64,
    pub(super) stroke: Option<String>,
    stroke_source: Option<Arc<FlowchartStyleDeclaration>>,
    inline_stroke_source: Option<Arc<FlowchartStyleDeclaration>>,
    pub(super) stroke_width: Option<String>,
    pub(super) stroke_dasharray: Option<String>,
    radius_sources: [Option<Arc<FlowchartStyleDeclaration>>; 2],
    has_source_filter: bool,
    has_source_label_foreground: bool,
    edge_important_stroke_width: Option<Arc<FlowchartStyleDeclaration>>,
    inline_important_stroke_width: Option<Arc<FlowchartStyleDeclaration>>,
    label_sources: Vec<LabelDeclaration>,
}

#[derive(Debug, Clone)]
struct LabelDeclaration {
    prepared: Arc<FlowchartStyleDeclaration>,
    inline: bool,
}

#[derive(Debug, Clone, Default)]
pub(in crate::svg::parity::flowchart) struct FlowchartPreparedClassStyles {
    classes: FxHashMap<String, PreparedClassStyles>,
    #[cfg(test)]
    parsed_declaration_count: usize,
}

#[derive(Debug, Clone, Default)]
struct PreparedClassStyles {
    declarations: Vec<Arc<FlowchartStyleDeclaration>>,
    declaration_count: usize,
    has_rejected_color: bool,
}

impl PreparedClassStyles {
    fn parse(
        declarations: &[String],
        work_meter: Option<&crate::resources::OperationWorkMeter>,
    ) -> std::result::Result<Self, crate::resources::OperationWorkError> {
        let mut prepared = Self::default();
        for raw_group in declarations {
            charge_style_scan(work_meter, raw_group.len())?;
            for raw in crate::flowchart::flowchart_split_mermaid_style_decls(raw_group) {
                charge_style_work(work_meter, 1)?;
                prepared.declaration_count += 1;
                if let Some(declaration) = FlowchartStyleDeclaration::parse(raw) {
                    prepared.declarations.push(Arc::new(declaration));
                } else if is_source_color(raw) {
                    // A rejected color still belongs to the author for fallback selection.
                    prepared.has_rejected_color = true;
                }
            }
        }
        Ok(prepared)
    }
}

impl FlowchartPreparedClassStyles {
    pub(in crate::svg::parity::flowchart) fn prepare<'a>(
        class_defs: &IndexMap<String, Vec<String>>,
        class_ids: impl IntoIterator<Item = &'a str>,
        work_meter: Option<&crate::resources::OperationWorkMeter>,
    ) -> std::result::Result<Self, crate::resources::OperationWorkError> {
        let mut selected = FxHashSet::default();
        for class_id in class_ids {
            charge_style_work(work_meter, 1)?;
            selected.insert(class_id);
        }
        Self::prepare_selected(class_defs, &selected, work_meter)
    }

    #[inline(never)]
    fn prepare_selected(
        class_defs: &IndexMap<String, Vec<String>>,
        selected: &FxHashSet<&str>,
        work_meter: Option<&crate::resources::OperationWorkMeter>,
    ) -> std::result::Result<Self, crate::resources::OperationWorkError> {
        let mut selected_definitions = selected
            .iter()
            .filter_map(|class_id| class_defs.get_full(*class_id))
            .collect::<Vec<_>>();
        selected_definitions.sort_unstable_by_key(|(ordinal, _, _)| *ordinal);
        let mut classes = FxHashMap::default();
        #[cfg(test)]
        let mut parsed_declaration_count = 0usize;
        for (_, class_id, declarations) in selected_definitions {
            let prepared = PreparedClassStyles::parse(declarations, work_meter)?;
            #[cfg(test)]
            {
                parsed_declaration_count =
                    parsed_declaration_count.saturating_add(prepared.declaration_count);
            }
            classes.insert(class_id.clone(), prepared);
        }
        Ok(Self {
            classes,
            #[cfg(test)]
            parsed_declaration_count,
        })
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) const fn parsed_declaration_count(&self) -> usize {
        self.parsed_declaration_count
    }
}

/// Lazily prepares the assigned classes reached by the node paint pass.
pub(super) struct FlowchartNodeClassStyles<'a> {
    source: &'a IndexMap<String, Vec<String>>,
    prepared: FlowchartPreparedClassStyles,
}

impl<'a> FlowchartNodeClassStyles<'a> {
    pub(super) fn new(source: &'a IndexMap<String, Vec<String>>) -> Self {
        Self {
            source,
            prepared: FlowchartPreparedClassStyles::default(),
        }
    }

    pub(super) fn compile(
        &mut self,
        classes: &[String],
        inline_styles: &[String],
        include_default_class: bool,
    ) -> FlowchartCompiledStyles {
        if include_default_class {
            let classes =
                crate::flowchart::flowchart_effective_node_class_names(self.source, classes);
            self.compile_for_names(classes, inline_styles)
        } else {
            self.compile_for_names(classes.iter().map(String::as_str), inline_styles)
        }
    }

    fn compile_for_names<'b>(
        &mut self,
        classes: impl Iterator<Item = &'b str> + Clone,
        inline_styles: &[String],
    ) -> FlowchartCompiledStyles {
        // The node paint compiler has no meter of its own. The separate bulk preparation for
        // edges and clusters still charges class selection and parsing in definition order.
        for name in classes.clone() {
            if !self.prepared.classes.contains_key(name)
                && let Some((name, declarations)) = self.source.get_key_value(name)
            {
                let prepared = PreparedClassStyles::parse(declarations, None)
                    .expect("unmetered class style preparation cannot fail");
                self.prepared.classes.insert(name.clone(), prepared);
            }
        }
        flowchart_compile_prepared_styles_for_class_names(
            &self.prepared,
            classes,
            inline_styles,
            &[],
            None,
        )
        .expect("unmetered node style compilation cannot fail")
    }
}

fn charge_style_scan(
    work_meter: Option<&crate::resources::OperationWorkMeter>,
    bytes: usize,
) -> std::result::Result<(), crate::resources::OperationWorkError> {
    let chunks = bytes.saturating_add(63) / 64;
    charge_style_work(work_meter, chunks.saturating_add(1))
}

fn charge_style_work(
    work_meter: Option<&crate::resources::OperationWorkMeter>,
    units: usize,
) -> std::result::Result<(), crate::resources::OperationWorkError> {
    if let Some(work_meter) = work_meter {
        work_meter.charge(units)?;
    }
    Ok(())
}

impl FlowchartCompiledStyles {
    pub(super) fn has_source_filter(&self) -> bool {
        self.has_source_filter
    }

    pub(super) fn has_source_label_foreground(&self) -> bool {
        self.has_source_label_foreground
    }

    pub(super) fn emitted_edge_class_declarations(&self) -> &[Arc<FlowchartStyleDeclaration>] {
        &self.edge_class_declarations
    }

    pub(super) const fn edge_animation_active(&self) -> Option<bool> {
        self.edge_animation_active
    }

    pub(super) fn edge_marker_color(&self, hand_drawn: bool) -> Option<&str> {
        let source = if hand_drawn {
            self.inline_stroke_source.as_ref()?
        } else {
            self.stroke_source.as_ref()?
        };
        Some(if hand_drawn {
            source.raw()
        } else {
            marker_source_value(source)
        })
    }

    pub(super) fn projected_edge_source_style_bytes(
        &self,
        default_edge_style: &[String],
        edge_style: &[String],
        hand_drawn: bool,
    ) -> Option<usize> {
        let joined_inline = projected_joined_style_bytes(default_edge_style, edge_style)?;
        if hand_drawn {
            return Some(joined_inline);
        }

        let class_bytes = self.edge_class_declarations.iter().enumerate().try_fold(
            0usize,
            |bytes, (index, declaration)| {
                let declaration_bytes = usize::from(index != 0)
                    .checked_add(super::super::util::escaped_xml_len(
                        declaration.property_css(),
                    ))?
                    .checked_add(1)?
                    .checked_add(super::super::util::escaped_xml_len(
                        declaration.source_value(),
                    ))?;
                bytes.checked_add(declaration_bytes)
            },
        )?;
        let class_separator = usize::from(!self.edge_class_declarations.is_empty());
        let inline_bytes = if default_edge_style.is_empty() && edge_style.is_empty() {
            1
        } else {
            joined_inline.checked_mul(2)?.checked_add(3)?
        };
        class_bytes
            .checked_add(class_separator)?
            .checked_add(inline_bytes)
    }

    /// Drop node-only data after projecting the path's serialized size.
    pub(super) fn into_edge_artifact(mut self, hand_drawn: bool) -> Self {
        self.node_style = String::new();
        self.fill = None;
        self.stroke = None;
        self.stroke_width = None;
        self.stroke_dasharray = None;
        self.radius_sources = Default::default();
        if hand_drawn {
            self.edge_class_declarations = Vec::new();
            self.edge_important_stroke_width = self.inline_important_stroke_width.take();
        }
        self
    }

    pub(super) fn into_swimlane_edge_label_artifact(mut self) -> Self {
        self.edge_class_declarations = Vec::new();
        self.stroke_source = None;
        self.inline_stroke_source = None;
        self.edge_important_stroke_width = None;
        self.into_edge_artifact(false)
    }

    pub(super) fn effective_edge_label_text_style<'a>(
        &self,
        base: &'a crate::text::TextStyle,
    ) -> std::borrow::Cow<'a, crate::text::TextStyle> {
        self.effective_edge_label_text_style_with_provenance(base)
            .style
    }

    pub(super) fn effective_edge_label_text_style_with_provenance<'a>(
        &self,
        base: &'a crate::text::TextStyle,
    ) -> crate::flowchart::FlowchartTextStyleResolution<'a> {
        let mut cascade = crate::mermaid_style::TextStyleCascade::new(
            base,
            crate::mermaid_style::CssFontSizeContext::uniform(base.font_size),
        );
        let mut terminal_foreground = None;
        for declaration in &self.label_sources {
            cascade.observe(
                declaration.prepared.property(),
                declaration.prepared.value(),
                declaration.prepared.important(),
            );
            if declaration.prepared.property().trim() == "color" {
                terminal_foreground = Some(crate::flowchart::FlowchartTerminalForeground::new(
                    declaration.prepared.value(),
                    if declaration.inline {
                        crate::flowchart::FlowchartTerminalForegroundProvenance::InlineStyle
                    } else {
                        crate::flowchart::FlowchartTerminalForegroundProvenance::AssignedClass
                    },
                ));
            }
        }
        crate::flowchart::FlowchartTextStyleResolution {
            style: cascade.finish(),
            terminal_foreground,
            html_geometry: self.html_label_geometry(),
        }
    }

    pub(super) fn html_label_geometry(&self) -> crate::mermaid_style::HtmlTextGeometryDeclarations {
        use crate::mermaid_style::HtmlTextProperty;
        let mut geometry = crate::mermaid_style::HtmlTextGeometryDeclarations::default();
        for declaration in &self.label_sources {
            if let Some(property) = HtmlTextProperty::named(declaration.prepared.property()) {
                geometry.set(property, declaration.prepared.value());
            }
        }
        geometry
    }

    pub(super) fn source_stroke_width_status(&self) -> FlowchartSourceFacetStatus {
        FlowchartSourceFacetStatus::from_parts(
            self.stroke_width.is_some(),
            self.admitted_stroke_width_value().is_some(),
        )
    }

    pub(super) fn admitted_stroke_width_value(&self) -> Option<f32> {
        self.stroke_width
            .as_deref()
            .and_then(admitted_flowchart_source_stroke_width)
    }

    /// An important declaration on the path outranks generated structural class CSS.
    #[cfg(any(feature = "layout-shoal", test))]
    pub(super) fn edge_important_stroke_width(&self) -> Option<&FlowchartStyleDeclaration> {
        self.edge_important_stroke_width.as_deref()
    }

    /// Mirror numeric source radii as attributes for SVG readers that ignore CSS geometry.
    pub(super) fn rectangle_source_radii(&self) -> [Option<f64>; 2] {
        self.radius_sources.each_ref().map(|source| {
            crate::mermaid_style::parse_svg_number_or_px(source.as_ref()?.value())
                .filter(|radius| *radius >= 0.0)
        })
    }

    pub(super) fn has_source_radius(&self) -> bool {
        self.radius_sources.iter().any(Option::is_some)
    }
}

fn marker_source_value(declaration: &FlowchartStyleDeclaration) -> &str {
    let Some((_, source_value)) = declaration.raw().split_once(':') else {
        return declaration.value();
    };
    let source_value = source_value.trim_end_matches(';');
    if !declaration.important() {
        return source_value;
    }

    let value = declaration.value();
    source_value
        .find(value)
        .map(|offset| &source_value[..offset + value.len()])
        .unwrap_or(value)
}

fn flowchart_edge_animation_declaration_active(
    declaration: &FlowchartStyleDeclaration,
) -> Option<bool> {
    match declaration.property() {
        "animation" => parse_flowchart_animation_shorthand(declaration.value()),
        "animation-name" => parse_flowchart_animation_name(declaration.value()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartAnimationNameComponent {
    Active,
    Inactive,
    CssWide,
}

const FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS: &[&str] = &["var", "env"];
const FLOWCHART_ANIMATION_FUNCTIONS: &[&str] = &[
    "linear",
    "cubic-bezier",
    "steps",
    "calc",
    "min",
    "max",
    "clamp",
    "scroll",
    "view",
];
const FLOWCHART_ANIMATION_SHORTHAND_KEYWORDS: &[&str] = &[
    "none",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "linear",
    "step-start",
    "step-end",
    "infinite",
    "normal",
    "reverse",
    "alternate",
    "alternate-reverse",
    "forwards",
    "backwards",
    "both",
    "running",
    "paused",
    "auto",
];

fn is_css_wide_keyword(value: &str) -> bool {
    flowchart_animation_identifier_matches(
        value,
        &["inherit", "initial", "revert", "revert-layer", "unset"],
    )
}

fn flowchart_animation_identifier_matches(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

fn parse_flowchart_animation_name(value: &str) -> Option<bool> {
    let mut parser = Parser::new(value);
    let names = parser
        .parse_comma_separated(parse_flowchart_animation_name_component)
        .ok()?;
    if names.len() != 1 && names.contains(&FlowchartAnimationNameComponent::CssWide) {
        return None;
    }
    Some(names.contains(&FlowchartAnimationNameComponent::Active))
}

fn parse_flowchart_animation_name_component<'i>(
    parser: &mut Parser<'i>,
) -> std::result::Result<FlowchartAnimationNameComponent, cssparser::ParseError<()>> {
    let token = parser.next()?.clone();
    let component = match token {
        Token::Ident(name) if name.eq_ignore_ascii_case("none") => {
            FlowchartAnimationNameComponent::Inactive
        }
        Token::Ident(name) if is_css_wide_keyword(name.as_ref()) => {
            FlowchartAnimationNameComponent::CssWide
        }
        Token::Ident(_) | Token::QuotedString(_) => FlowchartAnimationNameComponent::Active,
        Token::Function(name)
            if flowchart_animation_identifier_matches(
                name.as_ref(),
                FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS,
            ) =>
        {
            consume_flowchart_animation_function(parser)?;
            FlowchartAnimationNameComponent::Active
        }
        _ => return Err(cssparser::ParseError::custom(())),
    };
    parser.expect_exhausted()?;
    Ok(component)
}

fn parse_flowchart_animation_shorthand(value: &str) -> Option<bool> {
    let mut parser = Parser::new(value);
    parser
        .parse_comma_separated(parse_flowchart_single_animation)
        .ok()
        .map(|animations| animations.into_iter().any(std::convert::identity))
}

fn parse_flowchart_single_animation<'i>(
    parser: &mut Parser<'i>,
) -> std::result::Result<bool, cssparser::ParseError<()>> {
    let mut active_name = false;
    let mut name_seen = false;
    let mut component_count = 0usize;
    let mut css_wide = false;

    loop {
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => break,
            Err(error) => return Err(error.into()),
        };
        if matches!(token, Token::WhiteSpace(_)) {
            continue;
        }
        component_count = component_count.saturating_add(1);
        match token {
            Token::Ident(name) if is_css_wide_keyword(name.as_ref()) => {
                css_wide = true;
            }
            Token::Ident(name) if flowchart_animation_shorthand_keyword(name.as_ref()) => {}
            Token::Ident(_) | Token::QuotedString(_) => {
                if name_seen {
                    return Err(cssparser::ParseError::custom(()));
                }
                name_seen = true;
                active_name = true;
            }
            Token::Dimension { unit, .. }
                if unit.eq_ignore_ascii_case("s") || unit.eq_ignore_ascii_case("ms") => {}
            Token::Number { value, .. } if value.is_finite() && value >= 0.0 => {}
            Token::Function(name) => {
                let dynamic = flowchart_animation_identifier_matches(
                    name.as_ref(),
                    FLOWCHART_DYNAMIC_ANIMATION_FUNCTIONS,
                );
                let known = dynamic
                    || flowchart_animation_identifier_matches(
                        name.as_ref(),
                        FLOWCHART_ANIMATION_FUNCTIONS,
                    );
                if !known {
                    return Err(cssparser::ParseError::custom(()));
                }
                consume_flowchart_animation_function(parser)?;
                if dynamic {
                    active_name = true;
                }
            }
            _ => return Err(cssparser::ParseError::custom(())),
        }
    }

    if component_count == 0 || (css_wide && component_count != 1) {
        return Err(cssparser::ParseError::custom(()));
    }
    Ok(active_name)
}

fn consume_flowchart_animation_function<'i>(
    parser: &mut Parser<'i>,
) -> std::result::Result<(), cssparser::ParseError<()>> {
    parser.parse_nested_block(|nested| {
        while nested.next_including_whitespace().is_ok() {}
        Ok(())
    })
}

fn flowchart_animation_shorthand_keyword(value: &str) -> bool {
    flowchart_animation_identifier_matches(value, FLOWCHART_ANIMATION_SHORTHAND_KEYWORDS)
}

fn projected_joined_style_bytes(a: &[String], b: &[String]) -> Option<usize> {
    a.iter()
        .chain(b)
        .enumerate()
        .try_fold(0usize, |bytes, (index, part)| {
            bytes.checked_add(
                usize::from(index != 0).checked_add(super::super::util::escaped_xml_len(part))?,
            )
        })
}

#[cfg(test)]
pub(in crate::svg::parity) fn flowchart_compile_styles(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
) -> FlowchartCompiledStyles {
    let prepared_classes =
        FlowchartPreparedClassStyles::prepare(class_defs, classes.iter().map(String::as_str), None)
            .expect("unmetered source style preparation cannot fail");
    flowchart_compile_prepared_styles(
        &prepared_classes,
        classes,
        inline_styles_a,
        inline_styles_b,
        None,
    )
    .expect("unmetered source style compilation cannot fail")
}

pub(in crate::svg::parity::flowchart) fn flowchart_compile_prepared_styles(
    class_styles: &FlowchartPreparedClassStyles,
    classes: &[String],
    inline_styles_a: &[String],
    inline_styles_b: &[String],
    work_meter: Option<&crate::resources::OperationWorkMeter>,
) -> std::result::Result<FlowchartCompiledStyles, crate::resources::OperationWorkError> {
    flowchart_compile_prepared_styles_for_class_names(
        class_styles,
        classes.iter().map(String::as_str),
        inline_styles_a,
        inline_styles_b,
        work_meter,
    )
}

fn flowchart_compile_prepared_styles_for_class_names<'a>(
    class_styles: &FlowchartPreparedClassStyles,
    classes: impl Iterator<Item = &'a str>,
    inline_styles_a: &[String],
    inline_styles_b: &[String],
    work_meter: Option<&crate::resources::OperationWorkMeter>,
) -> std::result::Result<FlowchartCompiledStyles, crate::resources::OperationWorkError> {
    // Preserve Mermaid's insertion order from compileStyles() and styles2String(). Merman
    // also honors authored !important priority before emitting the winners as important CSS.
    type SemanticMap = IndexMap<String, LabelDeclaration, rustc_hash::FxBuildHasher>;
    fn set_semantic(map: &mut SemanticMap, prepared: Arc<FlowchartStyleDeclaration>, inline: bool) {
        crate::flowchart::flowchart_insert_style_by_spelling(
            map,
            prepared.property().to_owned(),
            LabelDeclaration { prepared, inline },
            |declaration| declaration.prepared.important(),
        );
    }

    #[derive(Default)]
    struct AnimationNameCascade {
        winner: Option<(bool, bool)>,
    }

    impl AnimationNameCascade {
        fn observe(
            &mut self,
            prepared: &FlowchartStyleDeclaration,
            work_meter: Option<&crate::resources::OperationWorkMeter>,
        ) -> std::result::Result<(), crate::resources::OperationWorkError> {
            if !matches!(prepared.property(), "animation" | "animation-name") {
                return Ok(());
            }
            charge_style_scan(work_meter, prepared.value().len())?;
            let Some(active) = flowchart_edge_animation_declaration_active(prepared) else {
                return Ok(());
            };
            if self
                .winner
                .is_some_and(|(_, important)| important && !prepared.important())
            {
                return Ok(());
            }
            self.winner = Some((active, prepared.important()));
            Ok(())
        }

        fn active(&self) -> Option<bool> {
            self.winner.map(|(active, _)| active)
        }
    }

    let mut semantic = SemanticMap::default();
    let mut inline_semantic = SemanticMap::default();
    let mut emission = IndexMap::default();
    let mut animation_name = AnimationNameCascade::default();
    let mut edge_class_declarations = Vec::new();
    let mut class_important_width = None;
    let mut inline_important_width = None;
    let mut has_source_label_foreground = false;

    for class in classes {
        let Some(declarations) = class_styles.classes.get(class) else {
            continue;
        };
        has_source_label_foreground |= declarations.has_rejected_color;
        charge_style_work(work_meter, declarations.declaration_count)?;
        for prepared in &declarations.declarations {
            if !crate::flowchart::flowchart_is_source_spelled_label_style_key(
                prepared.property_css(),
            ) {
                edge_class_declarations.push(Arc::clone(prepared));
                if prepared.property() == "stroke-width" && prepared.important() {
                    class_important_width = Some(Arc::clone(prepared));
                }
            }
            animation_name.observe(prepared, work_meter)?;
            crate::flowchart::flowchart_insert_style_by_spelling(
                &mut emission,
                prepared.property_css().to_owned(),
                Arc::clone(prepared),
                |declaration| declaration.important(),
            );
            set_semantic(&mut semantic, Arc::clone(prepared), false);
        }
    }

    for raw_group in inline_styles_a.iter().chain(inline_styles_b) {
        charge_style_scan(work_meter, raw_group.len())?;
        for raw in crate::flowchart::flowchart_split_mermaid_style_decls(raw_group) {
            charge_style_work(work_meter, 1)?;
            let Some(declaration) = FlowchartStyleDeclaration::parse(raw) else {
                has_source_label_foreground |= is_source_color(raw);
                continue;
            };
            let prepared = Arc::new(declaration);
            if prepared.property() == "stroke-width" && prepared.important() {
                inline_important_width = Some(Arc::clone(&prepared));
            }
            animation_name.observe(&prepared, work_meter)?;
            crate::flowchart::flowchart_insert_style_by_spelling(
                &mut emission,
                prepared.property_css().to_owned(),
                Arc::clone(&prepared),
                |declaration| declaration.important(),
            );
            set_semantic(&mut inline_semantic, Arc::clone(&prepared), true);
            set_semantic(&mut semantic, prepared, true);
        }
    }

    let mut node_style = String::new();
    let mut label_style = String::new();
    let mut label_div_decls = Vec::new();
    let mut rectangle_size = crate::flowchart::FlowchartRectangleCssSize::default();
    for declaration in emission.values() {
        rectangle_size.observe(declaration.property(), declaration.important(), || {
            declaration.svg_rect_length_px()
        });
    }
    for declaration in emission.into_values() {
        let property_css = declaration.property_css();
        let v = declaration.value();
        if crate::flowchart::flowchart_is_source_spelled_label_style_key(property_css) {
            if !label_style.is_empty() {
                label_style.push(';');
            }
            let _ = write!(&mut label_style, "{property_css}:{v} !important");
            label_div_decls.push((property_css.to_string(), v.to_string()));
        } else {
            if !declaration.important() && rectangle_size.has_important(declaration.property()) {
                continue;
            }
            if !node_style.is_empty() {
                node_style.push(';');
            }
            let _ = write!(&mut node_style, "{property_css}:{v} !important");
        }
    }

    let source = |property| semantic.get(property).map(|decl| &decl.prepared);
    let value = |property| source(property).map(|decl| decl.value().to_owned());
    let stroke_source = source("stroke").cloned();
    let inline_stroke_source = inline_semantic
        .get("stroke")
        .map(|decl| Arc::clone(&decl.prepared));
    let fill = value("fill");
    let opacity = |property| {
        source(property)
            .and_then(|declaration| {
                crate::theme::edge_label_bg::literal_opacity(declaration.value())
            })
            .unwrap_or(1.0)
    };
    let fill_opacity = opacity("fill-opacity") * opacity("opacity");
    let stroke = value("stroke");
    let stroke_width = value("stroke-width");
    let stroke_dasharray = value("stroke-dasharray");
    let radius_sources = [source("rx").cloned(), source("ry").cloned()];
    let has_source_filter = source("filter").is_some();
    let label_sources = semantic
        .into_values()
        .filter(|decl| {
            crate::flowchart::flowchart_is_source_spelled_label_style_key(
                decl.prepared.property_css(),
            )
        })
        .collect::<Vec<_>>();
    has_source_label_foreground |= label_sources
        .iter()
        .any(|decl| decl.prepared.property() == "color");

    Ok(FlowchartCompiledStyles {
        edge_class_declarations,
        edge_animation_active: animation_name.active(),
        node_style,
        rectangle_size: rectangle_size.size,
        label_style,
        label_div_decls,
        fill,
        fill_opacity,
        stroke,
        stroke_source,
        inline_stroke_source,
        stroke_width,
        stroke_dasharray,
        radius_sources,
        has_source_filter,
        has_source_label_foreground,
        edge_important_stroke_width: inline_important_width.clone().or(class_important_width),
        inline_important_stroke_width: inline_important_width,
        label_sources,
    })
}

fn is_source_color(raw: &str) -> bool {
    raw.split_once(':')
        .is_some_and(|(property, _)| property.trim() == "color")
}

pub(super) fn admitted_flowchart_source_stroke_width(value: &str) -> Option<f32> {
    let value = crate::mermaid_style::parse_svg_number_or_px(value)?;
    (value <= f32::MAX as f64).then_some(value as f32)
}

pub(in crate::svg::parity) fn flowchart_label_div_style_prefix(
    styles: &FlowchartCompiledStyles,
    color_as_rgb: bool,
) -> String {
    fn div_style_survives_mermaid_overrides(key: &str) -> bool {
        !matches!(key, "line-height" | "text-align" | "white-space")
    }

    let mut out = String::new();
    for (key, value) in &styles.label_div_decls {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() || !div_style_survives_mermaid_overrides(key) {
            continue;
        }
        if key == "color" {
            if color_as_rgb {
                let color = super::super::util::cssom_color_value(value);
                let _ = write!(&mut out, "color: {color} !important; ");
            } else {
                let _ = write!(
                    &mut out,
                    "color: {} !important; ",
                    value.to_ascii_lowercase()
                );
            }
        } else {
            let _ = write!(&mut out, "{key}: {value} !important; ");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_keyword_and_function_tables_are_ascii_case_insensitive() {
        assert!(flowchart_animation_shorthand_keyword("EaSe-In"));
        assert_eq!(
            parse_flowchart_animation_shorthand("2s LiNeAr(0, 1) BoTh"),
            Some(false)
        );
        assert_eq!(
            parse_flowchart_animation_shorthand("VaR(--animation-name) 2s EaSe-In"),
            Some(true)
        );
        assert_eq!(
            parse_flowchart_animation_name("EnV(--animation-name)"),
            Some(true)
        );
    }

    #[test]
    fn flowchart_shape_style_preserves_source_keys_with_a_canonical_semantic_winner() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"FILL:#ef4444".to_string(), r"stroke:#111827".to_string()],
            &[r"f\69ll:#22c55e".to_string()],
        );

        assert_eq!(styles.fill.as_deref(), Some("#22c55e"));
        assert_eq!(
            styles.node_style,
            r"FILL:#ef4444 !important;stroke:#111827 !important;f\69ll:#22c55e !important"
        );
    }

    #[test]
    fn escaped_custom_property_keeps_its_css_spelling() {
        let styles = flowchart_compile_styles(
            &IndexMap::new(),
            &[],
            &[r"--brand\:accent:#22c55e".to_string()],
            &[],
        );

        assert!(
            styles
                .node_style
                .contains(r"--brand\:accent:#22c55e !important")
        );
        assert!(!styles.node_style.contains("--brand:accent:"));
    }

    #[test]
    fn node_class_styles_reuse_parsing_and_preserve_each_node_cascade() {
        let definitions = IndexMap::from([
            ("node".into(), vec!["fill:#222222,color:#222222".into()]),
            ("default".into(), vec!["fill:#111111,stroke:#111111".into()]),
            ("hot".into(), vec!["fill:#aabbcc !important".into()]),
            ("cold".into(), vec!["fill:#ddeeff !important".into()]),
            ("rejected".into(), vec!["color:url(bad.svg)".into()]),
            ("unused".into(), vec!["fill:#ffffff".into()]),
        ]);
        let mut cache = FlowchartNodeClassStyles::new(&definitions);
        let assigned = cache.compile(&[], &[], true);
        assert_eq!(assigned.fill.as_deref(), Some("#222222"));
        assert_eq!(assigned.stroke.as_deref(), Some("#111111"));

        let repeated = cache.compile(
            &["hot", "missing", "cold", "hot"].map(str::to_owned),
            &["fill:#123456,stroke:#010203".into()],
            true,
        );
        assert_eq!(repeated.fill.as_deref(), Some("#aabbcc"));
        assert_eq!(repeated.stroke.as_deref(), Some("#010203"));

        let explicit = cache.compile(&["hot", "cold", "rejected"].map(str::to_owned), &[], false);
        assert_eq!(explicit.fill.as_deref(), Some("#ddeeff"));
        assert!(explicit.stroke.is_none());
        assert!(explicit.label_style.is_empty());
        assert!(explicit.has_source_label_foreground());
        assert_eq!(cache.prepared.classes.len(), 5);

        let shared_hot = &explicit.edge_class_declarations[0];
        assert_eq!(shared_hot.value(), "#aabbcc");
        assert_eq!(
            repeated
                .edge_class_declarations
                .iter()
                .filter(|declaration| Arc::ptr_eq(declaration, shared_hot))
                .count(),
            2
        );
    }

    #[test]
    fn class_style_work_rejections_follow_definition_order() {
        let class_defs = IndexMap::from([
            ("z".to_string(), vec!["fill:red".to_string()]),
            ("unbound".to_string(), vec!["x".repeat(1024)]),
            ("a".to_string(), vec![format!("fill:{}", "x".repeat(128))]),
        ]);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 7)
                .expect("work limit"),
        );

        let error = FlowchartPreparedClassStyles::prepare(
            &class_defs,
            ["a", "missing", "z", "a"],
            Some(&meter),
        )
        .expect_err("second selected definition must exceed the budget");

        let crate::resources::OperationWorkError::ResourceLimitExceeded(error) = error else {
            panic!("expected structured resource rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, 11);
        assert_eq!(error.max, 7);
        assert_eq!(meter.used(), 7);
    }
}
