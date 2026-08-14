use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use crate::{Error, Result};
use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, ParseError, ParseErrorKind, Parser, ParserInput,
    ParserState, QualifiedRuleParser, StyleSheetParser, Token,
};
use std::collections::{BTreeSet, HashMap};
use svgtypes::{Length, LengthUnit, NumberListParser};

use super::builtin::attr_sanitize::{is_safe_data_image_url, matches_active_svg_element};
use super::builtin::css_sanitize::{
    matches_external_image_function, validate_resvg_css_declaration_list,
    validate_resvg_css_stylesheet,
};
use super::final_validation::validate_well_formed_svg;
use super::final_validation::{ReferenceDependencyGraph, plan_svg_reference_dependencies};
use super::{is_css_value_attribute, is_svg_idref_attribute};
use crate::svg::parity::{C4_EXTERNAL_PERSON_IMG, C4_PERSON_IMG};

const STATIC_VALIDATION_PASS: &str = "validate-static-inline-svg";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const XHTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
const CSS_NESTING_HARD_LIMIT: u8 = 64;
const ROOT_DIMENSION_HARD_LIMIT_PX: f64 = 1_000_000.0;
const SVG_BYTES_PER_ROOT_PIXEL: usize = 16;

pub(super) fn validate_rustdoc_static_svg(svg: &str, limits: RenderResourcePolicy) -> Result<()> {
    validate_well_formed_svg(svg, limits)?;
    validate(svg, ForeignObjectPolicy::Reject, CssStage::Final, limits)
        .map_err(static_validation_error)
}

pub(super) fn validate_rustdoc_admission_svg(
    svg: &str,
    limits: RenderResourcePolicy,
) -> Result<()> {
    validate_well_formed_svg(svg, limits)?;
    validate(
        svg,
        ForeignObjectPolicy::AllowSafeXhtml,
        CssStage::Admission,
        limits,
    )
    .map_err(static_validation_error)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ForeignObjectPolicy {
    Reject,
    AllowSafeXhtml,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CssStage {
    Admission,
    Final,
}

fn validate(
    svg: &str,
    foreign_objects: ForeignObjectPolicy,
    css_stage: CssStage,
    limits: RenderResourcePolicy,
) -> std::result::Result<(), String> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| format!("rendered SVG is not valid XML: {error}"))?;
    let root = document.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("svg") {
        return Err("rendered document root is not <svg>".to_string());
    }
    let root_id = unnamespaced_attribute(root, "id");
    let root_dimension_limit = root_dimension_limit(limits);

    let mut ids = BTreeSet::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        if let Some(id) = unnamespaced_attribute(node, "id") {
            if id.is_empty() {
                return Err("rendered SVG contains an empty id".to_string());
            }
            if !ids.insert(id.to_string()) {
                return Err(format!("rendered SVG contains duplicate id {id:?}"));
            }
        }
    }

    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let element = node.tag_name().name();
        if let Some(namespace) = node.tag_name().namespace()
            && namespace != SVG_NAMESPACE
            && !(foreign_objects == ForeignObjectPolicy::AllowSafeXhtml
                && namespace == XHTML_NAMESPACE
                && is_inside_svg_foreign_object(node))
        {
            return Err(format!(
                "rendered SVG contains non-SVG element <{element}> in namespace {namespace:?}"
            ));
        }
        let allowed_foreign_object = foreign_objects == ForeignObjectPolicy::AllowSafeXhtml
            && element.eq_ignore_ascii_case("foreignObject")
            && node
                .tag_name()
                .namespace()
                .is_none_or(|namespace| namespace == SVG_NAMESPACE);
        if element.eq_ignore_ascii_case("base")
            || (matches_active_svg_element(element) && !allowed_foreign_object)
            || (is_inside_svg_foreign_object(node) && is_forbidden_xhtml_element(element))
        {
            return Err(format!(
                "rendered SVG contains forbidden <{element}> content"
            ));
        }

        for attribute in node.attributes() {
            let name = attribute.name();
            let value = attribute.value();
            if node == root && attribute.namespace().is_none() {
                if matches!(name.to_ascii_lowercase().as_str(), "width" | "height") {
                    validate_root_dimension(name, value, root_dimension_limit)?;
                } else if name.eq_ignore_ascii_case("viewBox") {
                    validate_root_view_box(value, root_dimension_limit)?;
                }
            }
            if name.eq_ignore_ascii_case("base") {
                return Err(format!(
                    "rendered SVG contains forbidden base attribute {name:?}"
                ));
            }
            if is_event_attribute(name) {
                return Err(format!("rendered SVG contains event attribute {name:?}"));
            }
            if is_forbidden_embedding_attribute(name)
                || (is_inside_svg_foreign_object(node) && is_forbidden_xhtml_attribute(name))
            {
                return Err(format!(
                    "rendered SVG contains forbidden embedding attribute {name:?}"
                ));
            }
            if name.eq_ignore_ascii_case("href") {
                if let Some(namespace) = attribute.namespace()
                    && namespace != XLINK_NAMESPACE
                {
                    return Err(format!(
                        "rendered SVG contains href in unsupported namespace {namespace:?}"
                    ));
                }
                validate_href(element, value, &ids)?;
            }
            if is_svg_idref_attribute(name) {
                validate_idrefs(name, value, &ids)?;
            }
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "src" | "data" | "poster"
            ) && !value.trim().is_empty()
            {
                return Err(format!(
                    "rendered SVG contains external resource attribute {name:?}"
                ));
            }
            if is_css_value_attribute(name) {
                validate_css(value, &ids, false)?;
                if node == root && name.eq_ignore_ascii_case("style") {
                    validate_root_layout_declaration_text(value, root_dimension_limit)?;
                }
                if css_stage == CssStage::Final && name.eq_ignore_ascii_case("style") {
                    validate_resvg_css_declaration_list(value).map_err(|error| {
                        format!("rendered SVG contains non-static CSS declarations: {error}")
                    })?;
                }
            }
        }

        if element.eq_ignore_ascii_case("style") {
            let mut css = String::new();
            for child in node.children() {
                if let Some(text) = child.text() {
                    css.push_str(text);
                }
            }
            validate_css(&css, &ids, true)?;
            let root_id = root_id
                .ok_or_else(|| "rendered SVG with embedded CSS must have a root id".to_string())?;
            validate_stylesheet_scope(
                &css,
                root_id,
                css_stage == CssStage::Admission,
                root_dimension_limit,
            )?;
            if css_stage == CssStage::Final {
                validate_resvg_css_stylesheet(&css)
                    .map_err(|error| format!("rendered SVG contains non-static CSS: {error}"))?;
            }
        }
    }
    validate_browser_reference_expansion(&document, limits)?;
    Ok(())
}

fn validate_browser_reference_expansion(
    document: &roxmltree::Document<'_>,
    limits: RenderResourcePolicy,
) -> std::result::Result<(), String> {
    let elements = document
        .descendants()
        .filter(roxmltree::Node::is_element)
        .collect::<Vec<_>>();
    let mut indices = HashMap::with_capacity(elements.len());
    let mut ids = HashMap::new();
    let mut dependencies = vec![Vec::new(); elements.len()];

    for (index, node) in elements.iter().copied().enumerate() {
        indices.insert(node.id(), index);
        if let Some(id) = unnamespaced_attribute(node, "id") {
            ids.insert(id, index);
        }
        if let Some(parent) = node.parent_element()
            && let Some(&parent_index) = indices.get(&parent.id())
        {
            dependencies[parent_index].push((index, 1));
        }
    }

    for (index, node) in elements.iter().copied().enumerate() {
        if !node.tag_name().name().eq_ignore_ascii_case("use")
            || node
                .tag_name()
                .namespace()
                .is_some_and(|namespace| namespace != SVG_NAMESPACE)
        {
            continue;
        }
        let href = unnamespaced_attribute(node, "href")
            .or_else(|| node.attribute((XLINK_NAMESPACE, "href")));
        let Some(target) = href.and_then(|value| value.trim().strip_prefix('#')) else {
            continue;
        };
        if let Some(&target_index) = ids.get(target) {
            dependencies[index].push((target_index, 1));
        }
    }

    let graph = ReferenceDependencyGraph::new(dependencies, elements.len());
    let plan = plan_svg_reference_dependencies(&graph)?;
    limits
        .check_svg_structure(plan.expanded_elements(), plan.max_tree_depth())
        .map_err(|error| error.to_string())
}

fn unnamespaced_attribute<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| attribute.value())
}

fn is_inside_svg_foreign_object(node: roxmltree::Node<'_, '_>) -> bool {
    node.ancestors().any(|ancestor| {
        ancestor.is_element()
            && ancestor
                .tag_name()
                .name()
                .eq_ignore_ascii_case("foreignObject")
            && ancestor
                .tag_name()
                .namespace()
                .is_none_or(|namespace| namespace == SVG_NAMESPACE)
    })
}

fn is_event_attribute(name: &str) -> bool {
    name.get(..2)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
}

fn is_forbidden_xhtml_element(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "audio"
            | "button"
            | "canvas"
            | "embed"
            | "form"
            | "iframe"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "object"
            | "picture"
            | "script"
            | "select"
            | "source"
            | "style"
            | "textarea"
            | "track"
            | "video"
    )
}

fn is_forbidden_xhtml_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "action"
            | "attributionsrc"
            | "autofocus"
            | "contenteditable"
            | "download"
            | "formaction"
            | "href"
            | "ping"
            | "poster"
            | "src"
            | "tabindex"
            | "target"
    )
}

fn is_forbidden_embedding_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "attributionsrc" | "download" | "ping"
    )
}

fn validate_href(
    element: &str,
    value: &str,
    ids: &BTreeSet<String>,
) -> std::result::Result<(), String> {
    let value = value.trim();
    if let Some(target) = value.strip_prefix('#') {
        return require_local_target(target, ids, "href");
    }

    if matches!(element.to_ascii_lowercase().as_str(), "image" | "feimage")
        && value.to_ascii_lowercase().starts_with("data:")
    {
        return validate_inline_raster(value);
    }

    if !element.eq_ignore_ascii_case("a") {
        return Err(format!(
            "rendered <{element}> references a non-local resource {value:?}"
        ));
    }

    let compact = value
        .chars()
        .filter(|character| !character.is_ascii_control() && !character.is_ascii_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if compact.starts_with("javascript:")
        || compact.starts_with("vbscript:")
        || compact.starts_with("data:")
        || compact.starts_with("file:")
        || compact.starts_with("//")
    {
        return Err(format!(
            "rendered SVG contains unsafe navigation href {value:?}"
        ));
    }
    Ok(())
}

fn validate_inline_raster(value: &str) -> std::result::Result<(), String> {
    if !is_safe_data_image_url(value) {
        return Err("rendered SVG contains an invalid inline raster data URL".to_string());
    }
    if matches!(value, C4_PERSON_IMG | C4_EXTERNAL_PERSON_IMG) {
        Ok(())
    } else {
        Err("rendered SVG contains an unrecognized inline raster asset".to_string())
    }
}

fn require_local_target(
    target: &str,
    ids: &BTreeSet<String>,
    context: &str,
) -> std::result::Result<(), String> {
    if target.is_empty() || !ids.contains(target) {
        return Err(format!(
            "rendered SVG {context} references missing local id {target:?}"
        ));
    }
    Ok(())
}

fn validate_idrefs(
    attribute: &str,
    value: &str,
    ids: &BTreeSet<String>,
) -> std::result::Result<(), String> {
    let mut references = value.split_ascii_whitespace().peekable();
    if references.peek().is_none() {
        return Err(format!(
            "rendered SVG contains an empty {attribute} reference"
        ));
    }
    for reference in references {
        require_local_target(reference, ids, attribute)?;
    }
    Ok(())
}

fn validate_css(
    css: &str,
    ids: &BTreeSet<String>,
    stylesheet: bool,
) -> std::result::Result<(), String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    validate_css_parser(&mut parser, ids, stylesheet, 0)
}

fn validate_stylesheet_scope(
    css: &str,
    root_id: &str,
    allow_discardable_animation: bool,
    root_dimension_limit: f64,
) -> std::result::Result<(), String> {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    validate_scoped_rule_list(
        &mut input,
        root_id,
        0,
        allow_discardable_animation,
        root_dimension_limit,
    )
    .map_err(|error| match error.kind {
        ParseErrorKind::Custom(message) => message,
        ParseErrorKind::Basic(error) => {
            format!("rendered SVG contains invalid scoped CSS: {error}")
        }
    })
}

#[derive(Clone, Copy)]
enum ScopedAtRule {
    Group,
    DiscardableAnimation,
}

struct ScopedRuleParser<'a> {
    root_id: &'a str,
    nesting: u8,
    allow_discardable_animation: bool,
    root_dimension_limit: f64,
}

impl<'i> AtRuleParser<'i> for ScopedRuleParser<'_> {
    type Prelude = ScopedAtRule;
    type AtRule = ();
    type Error = String;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::Prelude, ParseError<'i, Self::Error>> {
        consume_scoped_css_tokens(input, self.nesting)?;
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "container" | "document" | "layer" | "media" | "scope" | "supports"
        ) {
            Ok(ScopedAtRule::Group)
        } else if self.allow_discardable_animation
            && matches!(
                name.to_ascii_lowercase().as_str(),
                "keyframes" | "-webkit-keyframes"
            )
        {
            Ok(ScopedAtRule::DiscardableAnimation)
        } else {
            Err(input.new_custom_error(format!("rendered SVG contains forbidden CSS @{name} rule")))
        }
    }

    fn rule_without_block(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
    ) -> std::result::Result<Self::AtRule, ()> {
        Err(())
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::AtRule, ParseError<'i, Self::Error>> {
        match prelude {
            ScopedAtRule::Group => validate_scoped_rule_list(
                input,
                self.root_id,
                self.nesting + 1,
                self.allow_discardable_animation,
                self.root_dimension_limit,
            ),
            ScopedAtRule::DiscardableAnimation => {
                consume_scoped_css_tokens(input, self.nesting + 1)
            }
        }
    }
}

impl<'i> QualifiedRuleParser<'i> for ScopedRuleParser<'_> {
    type Prelude = SelectorScope;
    type QualifiedRule = ();
    type Error = String;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::Prelude, ParseError<'i, Self::Error>> {
        validate_selector_list_scope(input, self.root_id, self.nesting)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        if prelude.targets_root {
            validate_root_layout_declarations(input, self.nesting, self.root_dimension_limit)
        } else {
            consume_scoped_css_tokens(input, self.nesting)
        }
    }
}

#[derive(Clone, Copy)]
struct SelectorScope {
    targets_root: bool,
}

fn validate_scoped_rule_list<'i, 't>(
    input: &mut Parser<'i, 't>,
    root_id: &str,
    nesting: u8,
    allow_discardable_animation: bool,
    root_dimension_limit: f64,
) -> std::result::Result<(), ParseError<'i, String>> {
    if nesting >= CSS_NESTING_HARD_LIMIT {
        return Err(
            input.new_custom_error("rendered SVG CSS exceeds the nesting limit".to_string())
        );
    }
    let mut parser = ScopedRuleParser {
        root_id,
        nesting,
        allow_discardable_animation,
        root_dimension_limit,
    };
    for rule in StyleSheetParser::new(input, &mut parser) {
        rule.map_err(|(error, _)| error)?;
    }
    Ok(())
}

fn validate_selector_list_scope<'i, 't>(
    input: &mut Parser<'i, 't>,
    root_id: &str,
    nesting: u8,
) -> std::result::Result<SelectorScope, ParseError<'i, String>> {
    let mut expects_root = true;
    let mut saw_root = false;
    let mut current_targets_root = false;
    let mut any_targets_root = false;
    let mut pending_descendant = false;
    loop {
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => {
                if saw_root && !expects_root {
                    return Ok(SelectorScope {
                        targets_root: any_targets_root || current_targets_root,
                    });
                }
                return Err(input
                    .new_custom_error("rendered SVG contains an empty CSS selector".to_string()));
            }
            Err(error) => return Err(error.into()),
        };
        match token {
            Token::WhiteSpace(_) if saw_root => pending_descendant = true,
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::Comma if !expects_root => {
                any_targets_root |= current_targets_root;
                expects_root = true;
                saw_root = false;
                current_targets_root = false;
                pending_descendant = false;
            }
            Token::IDHash(id) | Token::Hash(id) if expects_root && id.as_ref() == root_id => {
                expects_root = false;
                saw_root = true;
                current_targets_root = true;
                pending_descendant = false;
            }
            Token::Delim('+') | Token::Delim('~') | Token::Delim('|') => {
                return Err(input.new_custom_error(
                    "rendered SVG CSS selector can escape the SVG root".to_string(),
                ));
            }
            Token::Delim('>') if !expects_root => {
                current_targets_root = false;
                pending_descendant = false;
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
                if !expects_root =>
            {
                if pending_descendant {
                    current_targets_root = false;
                    pending_descendant = false;
                }
                input
                    .parse_nested_block(|nested| consume_scoped_css_tokens(nested, nesting + 1))?;
            }
            _ if expects_root => {
                return Err(input.new_custom_error(format!(
                    "rendered SVG CSS selector must start with #{root_id}"
                )));
            }
            _ => {
                if pending_descendant {
                    current_targets_root = false;
                    pending_descendant = false;
                }
            }
        }
    }
}

fn validate_root_layout_declaration_text(
    css: &str,
    root_dimension_limit: f64,
) -> std::result::Result<(), String> {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    validate_root_layout_declarations(&mut input, 0, root_dimension_limit).map_err(|error| {
        match error.kind {
            ParseErrorKind::Custom(message) => message,
            ParseErrorKind::Basic(error) => {
                format!("rendered SVG contains invalid root CSS: {error}")
            }
        }
    })
}

fn validate_root_layout_declarations<'i, 't>(
    input: &mut Parser<'i, 't>,
    nesting: u8,
    root_dimension_limit: f64,
) -> std::result::Result<(), ParseError<'i, String>> {
    if nesting >= CSS_NESTING_HARD_LIMIT {
        return Err(
            input.new_custom_error("rendered SVG CSS exceeds the nesting limit".to_string())
        );
    }
    while !input.is_exhausted() {
        input.parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;
            let normalized = property.to_ascii_lowercase();
            if is_root_size_property(&normalized) {
                let value_start = declaration.position();
                consume_scoped_css_tokens(declaration, nesting + 1)?;
                let value = declaration.slice_from(value_start).trim().to_string();
                validate_root_dimension(&property, &value, root_dimension_limit)
                    .map_err(|message| declaration.new_custom_error(message))?;
            } else if is_allowed_root_presentation_property(&normalized) {
                consume_scoped_css_tokens(declaration, nesting + 1)?;
            } else {
                return Err(declaration
                    .new_custom_error(format!("rendered SVG root layout cannot set {property}")));
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn is_allowed_root_presentation_property(property: &str) -> bool {
    matches!(
        property,
        "background-color"
            | "color"
            | "cursor"
            | "direction"
            | "fill"
            | "fill-opacity"
            | "fill-rule"
            | "font"
            | "font-family"
            | "font-size"
            | "font-stretch"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "letter-spacing"
            | "opacity"
            | "paint-order"
            | "pointer-events"
            | "shape-rendering"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-opacity"
            | "stroke-width"
            | "text-anchor"
            | "text-decoration"
            | "text-rendering"
            | "unicode-bidi"
            | "visibility"
            | "white-space"
            | "word-spacing"
    )
}

fn is_root_size_property(property: &str) -> bool {
    matches!(
        property,
        "width"
            | "height"
            | "min-width"
            | "min-height"
            | "max-width"
            | "max-height"
            | "inline-size"
            | "block-size"
            | "min-inline-size"
            | "min-block-size"
            | "max-inline-size"
            | "max-block-size"
    )
}

fn root_dimension_limit(limits: RenderResourcePolicy) -> f64 {
    limits
        .value(ResourceLimitId::MaxSvgBytes)
        .map(|bytes| bytes / SVG_BYTES_PER_ROOT_PIXEL)
        .unwrap_or(ROOT_DIMENSION_HARD_LIMIT_PX as usize)
        .clamp(1, ROOT_DIMENSION_HARD_LIMIT_PX as usize) as f64
}

fn validate_root_dimension(
    property: &str,
    value: &str,
    limit: f64,
) -> std::result::Result<(), String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Ok(());
    }
    let length = value
        .parse::<Length>()
        .map_err(|_| format!("rendered SVG root layout has invalid {property} value {value:?}"))?;
    if !length.number.is_finite() || length.number <= 0.0 {
        return Err(format!(
            "rendered SVG root layout requires positive finite {property}"
        ));
    }
    match length.unit {
        LengthUnit::None | LengthUnit::Px if length.number <= limit => Ok(()),
        LengthUnit::Percent if length.number <= 100.0 => Ok(()),
        LengthUnit::None | LengthUnit::Px | LengthUnit::Percent => Err(format!(
            "rendered SVG root layout {property} exceeds its bounded size"
        )),
        _ => Err(format!(
            "rendered SVG root layout has unsupported {property} unit"
        )),
    }
}

fn validate_root_view_box(value: &str, limit: f64) -> std::result::Result<(), String> {
    let mut values = NumberListParser::from(value);
    let mut parsed = [0.0; 4];
    for slot in &mut parsed {
        *slot = values
            .next()
            .ok_or_else(|| "rendered SVG root layout has an invalid viewBox".to_string())?
            .map_err(|_| "rendered SVG root layout has an invalid viewBox".to_string())?;
    }
    if values.next().is_some()
        || parsed.iter().any(|value| !value.is_finite())
        || parsed[2] <= 0.0
        || parsed[3] <= 0.0
        || parsed[0].abs() > limit
        || parsed[1].abs() > limit
        || parsed[2] > limit
        || parsed[3] > limit
    {
        return Err("rendered SVG root layout has an unbounded viewBox".to_string());
    }
    Ok(())
}

fn consume_scoped_css_tokens<'i, 't>(
    input: &mut Parser<'i, 't>,
    nesting: u8,
) -> std::result::Result<(), ParseError<'i, String>> {
    if nesting >= CSS_NESTING_HARD_LIMIT {
        return Err(
            input.new_custom_error("rendered SVG CSS exceeds the nesting limit".to_string())
        );
    }
    loop {
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input.parse_nested_block(|nested| consume_scoped_css_tokens(nested, nesting + 1))?;
        }
    }
}

fn validate_css_parser<'i, 't>(
    input: &mut Parser<'i, 't>,
    ids: &BTreeSet<String>,
    stylesheet: bool,
    nesting: u8,
) -> std::result::Result<(), String> {
    if nesting >= CSS_NESTING_HARD_LIMIT {
        return Err("rendered SVG CSS exceeds the nesting limit".to_string());
    }
    loop {
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(format!("rendered SVG contains invalid CSS: {error:?}")),
        };
        match token {
            Token::AtKeyword(name) if stylesheet && name.eq_ignore_ascii_case("import") => {
                return Err("rendered SVG contains a forbidden CSS @import".to_string());
            }
            Token::UnquotedUrl(value) => validate_css_url(&value, ids, !stylesheet)?,
            Token::Function(name) => {
                if matches_external_image_function(&name) {
                    return Err(
                        "rendered SVG contains a forbidden CSS image-set function".to_string()
                    );
                }
                let is_url = name.eq_ignore_ascii_case("url");
                input
                    .parse_nested_block(|nested| {
                        if is_url {
                            validate_quoted_css_url(nested, ids, !stylesheet)
                        } else {
                            validate_css_parser(nested, ids, stylesheet, nesting + 1)
                        }
                        .map_err(|message| nested.new_custom_error::<String, String>(message))
                    })
                    .map_err(|error| nested_css_error(error, "function"))?;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                input
                    .parse_nested_block(|nested| {
                        validate_css_parser(nested, ids, stylesheet, nesting + 1)
                            .map_err(|message| nested.new_custom_error::<String, String>(message))
                    })
                    .map_err(|error| nested_css_error(error, "block"))?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err("rendered SVG contains an invalid CSS token".to_string());
            }
            _ => {}
        }
    }
}

fn nested_css_error(error: ParseError<'_, String>, context: &str) -> String {
    match error.kind {
        ParseErrorKind::Custom(message) => message,
        ParseErrorKind::Basic(error) => {
            format!("rendered SVG contains an invalid CSS {context}: {error}")
        }
    }
}

fn validate_quoted_css_url<'i, 't>(
    input: &mut Parser<'i, 't>,
    ids: &BTreeSet<String>,
    require_target: bool,
) -> std::result::Result<(), String> {
    let token = input
        .next_including_whitespace_and_comments()
        .map_err(|_| "rendered SVG contains an empty CSS URL".to_string())?
        .clone();
    let value = match token {
        Token::QuotedString(value) => value,
        Token::IDHash(value) => {
            return validate_local_css_target(&value, ids, require_target);
        }
        _ => return Err("rendered SVG contains a malformed CSS URL".to_string()),
    };
    validate_css_url(&value, ids, require_target)
}

fn validate_css_url(
    value: &str,
    ids: &BTreeSet<String>,
    require_target: bool,
) -> std::result::Result<(), String> {
    let Some(target) = value.trim().strip_prefix('#') else {
        return Err(format!(
            "rendered SVG contains a non-local CSS URL {value:?}"
        ));
    };
    validate_local_css_target(target, ids, require_target)
}

fn validate_local_css_target(
    target: &str,
    ids: &BTreeSet<String>,
    require_target: bool,
) -> std::result::Result<(), String> {
    if target.is_empty() {
        return Err("rendered SVG contains an empty local CSS URL".to_string());
    }
    if require_target {
        require_local_target(target, ids, "CSS URL")
    } else {
        Ok(())
    }
}

fn static_validation_error(message: impl Into<String>) -> Error {
    Error::SvgPostprocess {
        pass: STATIC_VALIDATION_PASS.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(svg: &str) -> Result<()> {
        validate_rustdoc_static_svg(svg, RenderResourcePolicy::trusted_native())
    }

    #[test]
    fn accepts_rebased_static_svg_and_safe_navigation() {
        validate(
            r##"<svg id="root" aria-label="prose url(https://example.test/not-a-resource)"><defs><path id="shape"/><linearGradient id="paint"/></defs><style>#root #shape{fill:url(#paint)}#root .optional{fill:url(#unused-theme-paint)}</style><use href="#shape"/><a href="https://example.test/docs"><text>Docs</text></a></svg>"##,
        )
        .unwrap();
    }

    #[test]
    fn rejects_active_and_external_svg_content() {
        for (svg, expected) in [
            (r#"<svg><script/></svg>"#, "forbidden <script>"),
            (r#"<svg><g onclick="x"/></svg>"#, "event attribute"),
            (
                r#"<svg><image href="https://example.test/a.png"/></svg>"#,
                "non-local resource",
            ),
            (
                r#"<svg><linearGradient href="https://example.test/p.svg#g"/></svg>"#,
                "non-local resource",
            ),
            (
                r#"<svg xml:base="https://example.test/"><path/></svg>"#,
                "base attribute",
            ),
            (
                r#"<svg><style>@import url(https://example.test/a.css)</style></svg>"#,
                "@import",
            ),
            (
                r#"<svg><path style="fill:url(#missing)"/></svg>"#,
                "missing local id",
            ),
            (
                r#"<svg id="root" aria-controls="missing"><path/></svg>"#,
                "missing local id",
            ),
            (
                r#"<svg><style>.x{fill:u\72l(https://example.test/a.svg)}</style></svg>"#,
                "non-local CSS URL",
            ),
            (
                r#"<svg><path background="url(https://tracker.test/a.png)"/></svg>"#,
                "non-local CSS URL",
            ),
            (
                r#"<svg><path background-image="url(https://tracker.test/a.png)"/></svg>"#,
                "non-local CSS URL",
            ),
            (
                r#"<svg><style>@\69mport 'https://example.test/a.css';</style></svg>"#,
                "@import",
            ),
            (
                r#"<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">label</div></foreignObject></svg>"#,
                "forbidden <foreignObject>",
            ),
            (
                r#"<svg><animate attributeName="x"/></svg>"#,
                "forbidden <animate>",
            ),
            (
                r#"<svg><style>.x{background-image:im\61ge-set(\"https://example.test/x.png\" 1x)}</style></svg>"#,
                "image-set",
            ),
            (
                r#"<svg><image href="data:image/svg+xml;base64,PHN2Zy8+"/></svg>"#,
                "invalid inline raster",
            ),
            (
                r#"<svg><image href="data:image/jpeg;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB"/></svg>"#,
                "unrecognized inline raster",
            ),
            (
                r#"<svg><a href="https://example.test" ping="https://tracker.test"/></svg>"#,
                "embedding attribute",
            ),
            (
                r#"<svg><foreignObject><input xmlns="http://www.w3.org/1999/xhtml" autofocus="autofocus"/></foreignObject></svg>"#,
                "forbidden <foreignObject>",
            ),
        ] {
            let error = validate(svg).expect_err("unsafe SVG");
            assert!(error.to_string().contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn admission_rejects_interactive_foreign_object_content() {
        let error = validate_rustdoc_admission_svg(
            r#"<svg><foreignObject><input xmlns="http://www.w3.org/1999/xhtml" autofocus="autofocus"/></foreignObject></svg>"#,
            RenderResourcePolicy::trusted_native(),
        )
        .expect_err("interactive foreignObject content");

        assert!(error.to_string().contains("forbidden <input>"), "{error}");
    }

    #[test]
    fn admission_rejects_external_background_resource_attributes() {
        for attribute in ["background", "background-image"] {
            let svg =
                format!(r#"<svg><path {attribute}="url(https://tracker.test/a.png)"/></svg>"#);
            let error =
                validate_rustdoc_admission_svg(&svg, RenderResourcePolicy::trusted_native())
                    .expect_err("external background resource");

            assert!(error.to_string().contains("non-local CSS URL"), "{error}");
        }
    }

    #[test]
    fn admission_allows_animation_only_for_the_staticization_stage() {
        let svg = r#"<svg id="root"><style>@keyframes dash{to{opacity:0}}#root .edge{animation:dash 1s}</style><path class="edge"/></svg>"#;

        validate_rustdoc_admission_svg(svg, RenderResourcePolicy::trusted_native()).unwrap();
        let error = validate(svg).expect_err("final animated CSS");
        assert!(error.to_string().contains("forbidden CSS"), "{error}");
    }

    #[test]
    fn rejects_css_that_can_affect_the_rustdoc_host() {
        for css in [
            "body{display:none}",
            "html{filter:blur(20px)}",
            "#root, body{display:none}",
            "#root + body{display:none}",
            "@font-face{font-family:host;src:url(#font)}",
        ] {
            let svg = format!(r#"<svg id="root"><style>{css}</style></svg>"#);
            let error = validate(&svg).expect_err("host-affecting CSS");
            assert!(
                error.to_string().contains("CSS selector")
                    || error.to_string().contains("forbidden CSS"),
                "{css}: {error}"
            );
        }

        validate(
            r#"<svg id="root"><style>@media (prefers-color-scheme: dark){#root{fill:black}#root .node{stroke:white}}</style><g class="node"/></svg>"#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_layout_escape_css_that_can_target_the_svg_root() {
        let validators: [fn(&str, RenderResourcePolicy) -> Result<()>; 2] =
            [validate_rustdoc_admission_svg, validate_rustdoc_static_svg];
        for svg in [
            r#"<svg id="root" style="position:fixed;inset:0;z-index:2147483647;pointer-events:auto"/>"#,
            r#"<svg id="root"><style>#root{position:fixed!important;inset:0;z-index:2147483647}</style></svg>"#,
            r#"<svg id="root"><style>@media (min-width:1px){#root{width:100vw;height:100vh}}</style></svg>"#,
            r#"<svg id="root"><style>#root.diagram{margin:-100px;transform:scale(100)}</style></svg>"#,
            r#"<svg id="root" width="100vw" height="100vh" style="pointer-events:auto"/>"#,
            r#"<svg id="root" width="999999999" height="999999999"/>"#,
            r#"<svg id="root" viewBox="0 0 999999999 1"/>"#,
            r#"<svg id="root" style="width:999999999px;height:999999999px"/>"#,
            r#"<svg id="root"><style>#root{width:calc(100% + 999999999px);height:999999999px;pointer-events:auto}</style></svg>"#,
            r#"<svg id="root" style="width:100%;padding:999999999px"/>"#,
            r#"<svg id="root"><style>#root{border:999999999px solid;aspect-ratio:1/999999999}</style></svg>"#,
        ] {
            for validator in validators {
                let error = validator(svg, RenderResourcePolicy::trusted_native())
                    .expect_err("root layout escape");
                assert!(
                    error.to_string().contains("SVG root layout"),
                    "{svg}: {error}"
                );
            }
        }
    }

    #[test]
    fn allows_layout_css_scoped_to_svg_descendants() {
        validate(
            r#"<svg id="root"><style>#root .node{position:absolute;z-index:2;transform:matrix(1,0,0,1,2,3)}</style><g class="node"/></svg>"#,
        )
        .unwrap();
        validate(r#"<svg id="root" width="100%" viewBox="0 0 681 400" style="max-width:681px"/>"#)
            .unwrap();
    }

    #[test]
    fn rejects_recursive_or_amplified_browser_use_expansion() {
        let validators: [fn(&str, RenderResourcePolicy) -> Result<()>; 2] =
            [validate_rustdoc_admission_svg, validate_rustdoc_static_svg];
        for svg in [
            r##"<svg><g id="loop"><use href="#loop"/></g></svg>"##,
            r##"<svg><g id="left"><use href="#right"/></g><g id="right"><use href="#left"/></g></svg>"##,
            r##"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><path id="safe"/><g id="loop"><use xlink:href="#safe" href="#loop"/></g></svg>"##,
        ] {
            for validator in validators {
                let error = validator(svg, RenderResourcePolicy::trusted_native())
                    .expect_err("recursive browser use expansion");
                assert!(error.to_string().contains("cycle"), "{svg}: {error}");
            }
        }

        let mut amplified = String::from(r#"<svg><defs><g id="level-0"><path/></g>"#);
        for level in 1..=6 {
            amplified.push_str(&format!(
                r##"<g id="level-{level}"><use href="#level-{}"/><use href="#level-{}"/></g>"##,
                level - 1,
                level - 1
            ));
        }
        amplified.push_str(r##"</defs><use href="#level-6"/></svg>"##);
        let limits = RenderResourcePolicy::trusted_native()
            .with_limit(ResourceLimitId::MaxSvgElements, 64)
            .unwrap();
        for validator in validators {
            let error = validator(&amplified, limits).expect_err("amplified use expansion");
            assert!(error.to_string().contains("max_svg_elements"), "{error}");
        }

        for validator in validators {
            validator(
                r##"<svg><defs><path id="shared"/></defs><use href="#shared"/><use href="#shared"/></svg>"##,
                RenderResourcePolicy::trusted_native(),
            )
            .unwrap();
        }
    }
}
