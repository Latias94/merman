use std::collections::{BTreeMap, HashMap, HashSet};

use merman_model::OperationControl;
use merman_render::__private::{
    EffectColorSpace, EffectComposition, EffectInput, MAX_NATIVE_SHADOW_STAGES,
    NativeSvgFilterApplication, NativeSvgFilterReceipt, NativeSvgFilterUnits, NativeSvgShadowStage,
};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const TYPED_FILTER_ID_MARKER: &str = "-theme-effect-";

#[derive(Debug)]
struct FilterDefinition {
    units: NativeSvgFilterUnits,
    region: [f32; 4],
    color_space: EffectColorSpace,
    composition: EffectComposition,
    stages: Vec<DropShadowDefinition>,
}

#[derive(Debug)]
struct DropShadowDefinition {
    input: EffectInput,
    offset: [f32; 2],
    std_deviation: [f32; 2],
    color_css: String,
}

#[derive(Debug)]
struct OpenFilter {
    depth: usize,
    id: Option<String>,
    units: Option<NativeSvgFilterUnits>,
    region: Option<[f32; 4]>,
    color_space: Option<EffectColorSpace>,
    composition: EffectComposition,
    independent_merge_started: bool,
    independent_merge_complete: bool,
    stages: Vec<DropShadowDefinition>,
    expanded_elements: Vec<Vec<String>>,
    expanded_offset_omitted: bool,
    valid: bool,
}

#[derive(Debug)]
struct DefinitionSlot {
    occurrences: usize,
    definition: Option<FilterDefinition>,
}

#[derive(Default)]
struct RawReferenceProof {
    open_sources: Vec<RawSource>,
    sources: HashMap<String, bool>,
    targets: HashSet<String>,
}

struct RawSource {
    ids: Vec<String>,
    plain: bool,
    reusable: bool,
    has_typed_reference: bool,
}

impl RawReferenceProof {
    fn start(
        &mut self,
        reader: &NsReader<&[u8]>,
        element: &BytesStart<'_>,
        has_typed_reference: bool,
    ) -> Result<(), ()> {
        let local_name = element.local_name();
        // Admit literal geometry/text sources, including HTML fallback's nested SVG.
        // References inside a source need a separate expansion proof and fail closed.
        let mut source = RawSource {
            ids: Vec::new(),
            plain: is_svg_namespace(reader, element)
                && matches!(
                    local_name.as_ref(),
                    "g" | "svg"
                        | "rect"
                        | "path"
                        | "circle"
                        | "ellipse"
                        | "line"
                        | "polyline"
                        | "polygon"
                        | "text"
                        | "tspan"
                        | "title"
                        | "desc"
                ),
            reusable: matches!(
                local_name.as_ref(),
                "marker" | "pattern" | "clipPath" | "mask" | "symbol"
            ),
            has_typed_reference,
        };
        let instantiates_source = matches!(local_name.as_ref(), "use" | "feImage");
        let mut href = None;
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| ())?;
            if attribute.key.as_namespace_binding().is_some() {
                continue;
            }
            let (namespace, name) = reader.resolver().resolve_attribute(attribute.key);
            // XML use expansion reads local-name identity before presentation filtering.
            // Keep a finite literal identity surface rather than guessing a different target.
            match name.as_ref() {
                "id" if !matches!(namespace, ResolveResult::Unbound) => return Err(()),
                "href"
                    if !matches!(namespace, ResolveResult::Unbound)
                        && !matches!(&namespace, ResolveResult::Bound(uri)
                            if uri.as_ref() == XLINK_NAMESPACE) =>
                {
                    return Err(());
                }
                _ => {}
            }
            if !is_usvg_attribute_namespace(&namespace) {
                continue;
            }
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ())?;
            match name.as_ref() {
                "id" => {
                    // usvg's XML use lookup and converted resource lookup project IDs
                    // differently. Namespaced or duplicate source IDs are not certified.
                    source.plain &= matches!(namespace, ResolveResult::Unbound);
                    source.ids.push(value.into_owned());
                }
                "href" => {
                    source.plain = false;
                    if instantiates_source {
                        if href
                            .as_deref()
                            .is_some_and(|previous| previous != value.as_ref())
                        {
                            return Err(());
                        }
                        href = Some(value.into_owned());
                    }
                }
                "filter" | "clip-path" | "mask" | "marker" | "marker-start" | "marker-mid"
                | "marker-end" => source.plain &= value.trim() == "none",
                "fill" | "stroke" => {
                    source.plain &= !value.to_ascii_lowercase().contains("url");
                }
                "style" => source.plain &= plain_reference_style(&value),
                _ => {}
            }
        }
        source.plain &= !has_typed_reference && source.ids.len() <= 1;
        if instantiates_source {
            let target = href
                .as_deref()
                .and_then(|href| href.strip_prefix('#'))
                .ok_or(())?;
            if target.is_empty()
                || !target.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
                })
            {
                return Err(());
            }
            self.targets.insert(target.to_string());
        }
        self.open_sources.push(source);
        Ok(())
    }

    fn end(&mut self) -> Option<()> {
        let source = self.open_sources.pop()?;
        if source.reusable && source.has_typed_reference {
            return None;
        }
        for id in source.ids {
            self.sources
                .entry(id)
                .and_modify(|plain| *plain = false)
                .or_insert(source.plain);
        }
        if let Some(parent) = self.open_sources.last_mut() {
            parent.plain &= source.plain;
            parent.has_typed_reference |= source.has_typed_reference;
        }
        Some(())
    }

    fn matches(&self) -> bool {
        self.open_sources.is_empty()
            && self
                .targets
                .iter()
                .all(|target| self.sources.get(target) == Some(&true))
    }
}

fn plain_reference_style(style: &str) -> bool {
    // Only literal writer declarations are admitted; this is not a CSS resolver.
    !style.to_ascii_lowercase().contains("url")
        && style
            .split(';')
            .filter(|part| !part.trim().is_empty())
            .all(|part| {
                let Some((property, value)) = part.split_once(':') else {
                    return false;
                };
                let property = property.trim();
                !property.is_empty()
                    && property
                        .bytes()
                        .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
                    && (!property.eq_ignore_ascii_case("filter")
                        || value
                            .trim()
                            .strip_suffix("!important")
                            .unwrap_or(value.trim())
                            .trim()
                            == "none")
            })
}

pub(super) fn preflight_native_filter_receipt(
    svg: &str,
    tree: &usvg::Tree,
    viewport: usvg::Rect,
    control: &OperationControl,
) -> crate::Result<Option<NativeSvgFilterReceipt>> {
    crate::export_checkpoint(control)?;
    let receipt = (|| {
        let shadows = parse_raw_drop_shadows(svg, control)?;
        if !resolved_tree_matches(tree, &shadows, viewport, control) {
            return None;
        }
        NativeSvgFilterReceipt::from_applications(shadows)
    })();
    crate::export_checkpoint(control)?;
    Ok(receipt)
}

fn parse_raw_drop_shadows(
    svg: &str,
    control: &OperationControl,
) -> Option<Vec<NativeSvgFilterApplication>> {
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut open_filters = Vec::<OpenFilter>::new();
    let mut open_style: Option<(usize, String)> = None;
    let mut definitions = BTreeMap::<String, DefinitionSlot>::new();
    let mut references = BTreeMap::<String, usize>::new();
    let mut reference_proof = RawReferenceProof::default();

    loop {
        crate::export_checkpoint(control).ok()?;
        match reader.read_event().ok()? {
            Event::Start(element) => {
                if open_style.is_some() {
                    return None;
                }
                // usvg discovers stylesheets by local name, in every namespace.
                if element.local_name().as_ref() == "style" {
                    open_style = Some((depth, String::new()));
                }
                let typed = record_typed_reference(&reader, &element, &mut references).ok()?;
                reference_proof.start(&reader, &element, typed).ok()?;
                observe_filter_child(&reader, &element, depth, &mut open_filters).ok()?;
                if is_svg_element(&reader, &element, "filter") {
                    open_filters.push(parse_filter(&reader, &element, depth).ok()?);
                }
                depth = depth.checked_add(1)?;
            }
            Event::Empty(element) => {
                if open_style.is_some() {
                    return None;
                }
                let typed = record_typed_reference(&reader, &element, &mut references).ok()?;
                reference_proof.start(&reader, &element, typed).ok()?;
                reference_proof.end()?;
                observe_filter_child(&reader, &element, depth, &mut open_filters).ok()?;
                if is_svg_element(&reader, &element, "filter") {
                    let filter = parse_filter(&reader, &element, depth).ok()?;
                    record_definition(filter, &mut definitions)?;
                }
            }
            Event::End(element) => {
                depth = depth.checked_sub(1)?;
                reference_proof.end()?;
                if open_style
                    .as_ref()
                    .is_some_and(|(style_depth, _)| *style_depth == depth)
                {
                    let (_, style) = open_style.take()?;
                    // This is finite writer-syntax admission, not partial CSS interpretation.
                    // Escapes can hide typed IDs; reject them without depending on CSS decoding.
                    if style.contains('\\') || style.contains(TYPED_FILTER_ID_MARKER) {
                        return None;
                    }
                }
                for filter in &mut open_filters {
                    if filter.composition == EffectComposition::Independent
                        && depth == filter.depth + 1
                        && element.local_name().as_ref() == "feMerge"
                    {
                        filter.independent_merge_complete =
                            filter.expanded_elements.len() == filter.stages.len() + 2;
                        filter.valid &= filter.independent_merge_complete;
                        filter.expanded_elements.clear();
                    } else if filter.color_space == Some(EffectColorSpace::Srgb)
                        && filter.composition == EffectComposition::Cascade
                        && depth == filter.depth + 1
                        && element.local_name().as_ref() == "feMerge"
                    {
                        if let Some(stage) = finish_expanded_shadow(filter) {
                            filter.stages.push(stage);
                        } else {
                            filter.valid = false;
                        }
                        filter.expanded_elements.clear();
                        filter.expanded_offset_omitted = false;
                    }
                }
                if open_filters
                    .last()
                    .is_some_and(|filter| filter.depth == depth)
                {
                    let filter = open_filters.pop()?;
                    record_definition(filter, &mut definitions)?;
                }
            }
            Event::Text(text) => {
                if let Some((_, style)) = &mut open_style {
                    style.push_str(text.as_ref());
                }
                if !text.as_ref().as_bytes().iter().all(u8::is_ascii_whitespace) {
                    invalidate_containing_filters(depth, &mut open_filters);
                }
            }
            Event::CData(text) => {
                if let Some((_, style)) = &mut open_style {
                    style.push_str(text.as_ref());
                }
                if !text.as_ref().as_bytes().iter().all(u8::is_ascii_whitespace) {
                    invalidate_containing_filters(depth, &mut open_filters);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some((_, style)) = &mut open_style {
                    if let Some(character) = reference.resolve_char_ref().ok()? {
                        style.push(character);
                    } else {
                        style.push_str(quick_xml::escape::resolve_predefined_entity(
                            reference.as_ref(),
                        )?);
                    }
                }
                invalidate_containing_filters(depth, &mut open_filters);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if depth != 0
        || !open_filters.is_empty()
        || open_style.is_some()
        || !reference_proof.matches()
        || references.is_empty()
        || definitions.len() != references.len()
    {
        return None;
    }

    let mut shadows = Vec::with_capacity(references.len());
    for (filter_id, reference_count) in references {
        crate::export_checkpoint(control).ok()?;
        if reference_count != 1 {
            return None;
        }
        let slot = definitions.get(&filter_id)?;
        if slot.occurrences != 1 {
            return None;
        }
        let definition = slot.definition.as_ref()?;
        let stages = definition
            .stages
            .iter()
            .map(|stage| {
                NativeSvgShadowStage::new(
                    stage.input,
                    stage.offset,
                    stage.std_deviation,
                    &stage.color_css,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        shadows.push(
            NativeSvgFilterApplication::new_with_units(
                filter_id,
                definition.units,
                definition.region,
                definition.color_space,
                stages,
                1,
            )?
            .with_composition(definition.composition)?,
        );
    }
    Some(shadows)
}

fn record_typed_reference(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    references: &mut BTreeMap<String, usize>,
) -> Result<bool, ()> {
    if !is_svg_namespace(reader, element) {
        return Ok(false);
    }

    let mut typed_filter = None;
    let mut style = None;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        if !is_usvg_attribute_namespace(&namespace)
            || !matches!(local_name.as_ref(), "filter" | "style")
        {
            continue;
        }
        // usvg projects these accepted namespaces to presentation-attribute local names.
        // The finite writer proof admits only the unqualified form, without precedence inference.
        if !matches!(namespace, ResolveResult::Unbound) {
            return Err(());
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| ())?;
        if local_name.as_ref() == "style" {
            // Admit the writer's literal CSS surface only, after XML entity normalization.
            // Even an unrelated escape prevents proving that no typed application is hidden.
            if value.contains('\\') {
                return Err(());
            }
            if style.replace(value.into_owned()).is_some() {
                return Err(());
            }
            continue;
        }
        let Some(filter_id) = exact_filter_reference(value.as_ref()) else {
            if value.contains(TYPED_FILTER_ID_MARKER) {
                return Err(());
            }
            continue;
        };
        if !filter_id.contains(TYPED_FILTER_ID_MARKER) {
            continue;
        }
        if typed_filter.replace(filter_id.to_string()).is_some() {
            return Err(());
        }
    }
    if let Some(style) = style
        && style.contains(TYPED_FILTER_ID_MARKER)
        && !inline_style_matches_typed_reference(&style, typed_filter.as_deref().ok_or(())?)
    {
        return Err(());
    }
    if let Some(filter_id) = typed_filter {
        let count = references.entry(filter_id).or_default();
        *count = count.checked_add(1).ok_or(())?;
        return Ok(true);
    }
    Ok(false)
}

fn inline_style_matches_typed_reference(style: &str, filter_id: &str) -> bool {
    // Admit only the writer's single mirrored filter declaration, without CSS cascade
    // inference. Multiple declarations, conflicting values, and style-only paint fail closed.
    if style.matches(TYPED_FILTER_ID_MARKER).count() != 1 {
        return false;
    }
    let mut matched = false;
    for declaration in style.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            if declaration.contains(TYPED_FILTER_ID_MARKER) {
                return false;
            }
            continue;
        };
        if !property.trim().eq_ignore_ascii_case("filter") {
            if declaration.contains(TYPED_FILTER_ID_MARKER) {
                return false;
            }
            continue;
        }
        let value = value.trim();
        let value = value.strip_suffix("!important").unwrap_or(value).trim_end();
        if matched || exact_filter_reference(value) != Some(filter_id) {
            return false;
        }
        matched = true;
    }
    matched
}

fn exact_filter_reference(value: &str) -> Option<&str> {
    let filter_id = value.strip_prefix("url(#")?.strip_suffix(')')?;
    (!filter_id.is_empty()).then_some(filter_id)
}

fn parse_filter(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    depth: usize,
) -> Result<OpenFilter, ()> {
    let mut id = None;
    let mut filter_units = None;
    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;
    let mut color_interpolation = None;
    let mut valid = true;

    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        if !matches!(namespace, ResolveResult::Unbound) {
            valid = false;
            continue;
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| ())?
            .into_owned();
        let accepted = match local_name.as_ref() {
            "id" => set_once(&mut id, value),
            "filterUnits" => set_once(&mut filter_units, value),
            "x" => set_once(&mut x, value),
            "y" => set_once(&mut y, value),
            "width" => set_once(&mut width, value),
            "height" => set_once(&mut height, value),
            "color-interpolation-filters" => set_once(&mut color_interpolation, value),
            _ => false,
        };
        valid &= accepted;
    }

    let units = match filter_units.as_deref() {
        Some("objectBoundingBox") => Some(NativeSvgFilterUnits::ObjectBoundingBox),
        Some("userSpaceOnUse") => Some(NativeSvgFilterUnits::UserSpaceOnUse),
        _ => None,
    };
    valid &= units.is_some();
    let color_space = match color_interpolation.as_deref() {
        Some("linearRGB") => Some(EffectColorSpace::LinearRgb),
        Some("sRGB") => Some(EffectColorSpace::Srgb),
        _ => None,
    };
    valid &= color_space.is_some();
    let region = parse_filter_region(x, y, width, height);
    valid &= region.is_some();

    Ok(OpenFilter {
        depth,
        id,
        units,
        region,
        color_space,
        composition: EffectComposition::Cascade,
        independent_merge_started: false,
        independent_merge_complete: false,
        stages: Vec::new(),
        expanded_elements: Vec::new(),
        expanded_offset_omitted: false,
        valid,
    })
}

fn parse_filter_region(
    x: Option<String>,
    y: Option<String>,
    width: Option<String>,
    height: Option<String>,
) -> Option<[f32; 4]> {
    let region = [
        parse_finite_number(x.as_deref()?)?,
        parse_finite_number(y.as_deref()?)?,
        parse_finite_number(width.as_deref()?)?,
        parse_finite_number(height.as_deref()?)?,
    ];
    (region[2] > 0.0 && region[3] > 0.0).then_some(region)
}

fn observe_filter_child(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    depth: usize,
    open_filters: &mut [OpenFilter],
) -> Result<(), ()> {
    let is_drop_shadow = is_svg_element(reader, element, "feDropShadow");
    for filter in open_filters {
        if depth <= filter.depth {
            continue;
        }
        if filter.stages.is_empty()
            && filter.expanded_elements.is_empty()
            && is_svg_element(reader, element, "feGaussianBlur")
            && element.attributes().any(|attribute| {
                attribute.ok().is_some_and(|attribute| {
                    attribute.key.as_ref() == "in" && attribute.value.as_ref() == "SourceAlpha"
                })
            })
        {
            filter.composition = EffectComposition::Independent;
        }
        if filter.composition == EffectComposition::Independent {
            observe_independent_shadow(reader, element, depth, filter)?;
        } else if filter.color_space == Some(EffectColorSpace::Srgb) {
            observe_expanded_shadow(reader, element, depth, filter)?;
        } else if depth == filter.depth + 1 && is_drop_shadow {
            if filter.stages.len() >= MAX_NATIVE_SHADOW_STAGES {
                filter.valid = false;
                continue;
            }
            if let Some(stage) = parse_drop_shadow(reader, element, filter.stages.len())? {
                filter.stages.push(stage);
            } else {
                filter.valid = false;
            }
        } else {
            filter.valid = false;
        }
    }
    Ok(())
}

// Recognize only the writer's sRGB shadow expansion, with an optional identity offset.
// The two merge nodes are the last two elements of each stage; this is not a general
// SVG filter-program parser.
fn observe_expanded_shadow(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    depth: usize,
    filter: &mut OpenFilter,
) -> Result<(), ()> {
    const ELEMENTS: [&str; 7] = [
        "feGaussianBlur",
        "feOffset",
        "feFlood",
        "feComposite",
        "feMerge",
        "feMergeNode",
        "feMergeNode",
    ];
    const ATTRIBUTES: [&[&str]; 7] = [
        &["in", "stdDeviation", "result"],
        &["in", "dx", "dy", "result"],
        &["flood-color", "result"],
        &["in", "in2", "operator", "result"],
        &["result"],
        &["in"],
        &["in"],
    ];
    let position = filter.expanded_elements.len();
    if position == 1 && is_svg_element(reader, element, "feFlood") {
        filter.expanded_offset_omitted = true;
    }
    let position = position + usize::from(filter.expanded_offset_omitted && position > 0);
    if filter.stages.len() >= MAX_NATIVE_SHADOW_STAGES
        || position >= ELEMENTS.len()
        || depth != filter.depth + if position < 5 { 1 } else { 2 }
        || !is_svg_element(reader, element, ELEMENTS[position])
    {
        filter.valid = false;
        return Ok(());
    }
    let names = ATTRIBUTES[position];
    if let Some(values) = exact_attribute_values(reader, element, names)? {
        filter.expanded_elements.push(values);
    } else {
        filter.valid = false;
    }
    Ok(())
}

fn exact_attribute_values(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    names: &[&str],
) -> Result<Option<Vec<String>>, ()> {
    let mut values = vec![None; names.len()];
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        let Some(index) = names.iter().position(|name| *name == local_name.as_ref()) else {
            return Ok(None);
        };
        if !matches!(namespace, ResolveResult::Unbound) || values[index].is_some() {
            return Ok(None);
        }
        values[index] = Some(
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ())?
                .into_owned(),
        );
    }
    Ok(values.into_iter().collect())
}

// Recognize only the independent stack emitted by Merman: bounded branches from SourceAlpha,
// followed by one ordered merge. Arbitrary SVG filter graphs remain outside this receipt.
fn observe_independent_shadow(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    depth: usize,
    filter: &mut OpenFilter,
) -> Result<(), ()> {
    if filter.independent_merge_complete {
        filter.valid = false;
        return Ok(());
    }
    if filter.independent_merge_started {
        let index = filter.expanded_elements.len().saturating_sub(1);
        let expected = if index < filter.stages.len() {
            format!("merman-shadow-{index}-shadow")
        } else {
            "SourceGraphic".to_owned()
        };
        if depth != filter.depth + 2
            || !is_svg_element(reader, element, "feMergeNode")
            || index > filter.stages.len()
        {
            filter.valid = false;
            return Ok(());
        }
        let values = exact_attribute_values(reader, element, &["in"])?;
        if let Some(values) = values.filter(|values| values[0] == expected) {
            filter.expanded_elements.push(values);
        } else {
            filter.valid = false;
        }
        return Ok(());
    }
    if filter.expanded_elements.is_empty() && is_svg_element(reader, element, "feMerge") {
        let values = exact_attribute_values(reader, element, &["result"])?;
        if depth == filter.depth + 1
            && !filter.stages.is_empty()
            && let Some(values) = values.filter(|values| values[0] == "merman-shadow-stack-result")
        {
            filter.independent_merge_started = true;
            filter.expanded_elements.push(values);
        } else {
            filter.valid = false;
        }
        return Ok(());
    }
    const ELEMENTS: [&str; 4] = ["feGaussianBlur", "feOffset", "feFlood", "feComposite"];
    const ATTRIBUTES: [&[&str]; 4] = [
        &["in", "stdDeviation", "result"],
        &["in", "dx", "dy", "result"],
        &["flood-color", "result"],
        &["in", "in2", "operator", "result"],
    ];
    let position = filter.expanded_elements.len();
    if position == 1 && is_svg_element(reader, element, "feFlood") {
        filter.expanded_offset_omitted = true;
    }
    let position = position + usize::from(filter.expanded_offset_omitted && position > 0);
    if filter.stages.len() >= MAX_NATIVE_SHADOW_STAGES
        || position >= ELEMENTS.len()
        || depth != filter.depth + 1
        || !is_svg_element(reader, element, ELEMENTS[position])
    {
        filter.valid = false;
        return Ok(());
    }
    if let Some(values) = exact_attribute_values(reader, element, ATTRIBUTES[position])? {
        filter.expanded_elements.push(values);
    } else {
        filter.valid = false;
        return Ok(());
    }
    if position == 3 {
        if let Some(stage) = finish_independent_shadow(filter) {
            filter.stages.push(stage);
        } else {
            filter.valid = false;
        }
        filter.expanded_elements.clear();
        filter.expanded_offset_omitted = false;
    }
    Ok(())
}

fn finish_independent_shadow(filter: &OpenFilter) -> Option<DropShadowDefinition> {
    let (blur, offset, flood, composite) = match filter.expanded_elements.as_slice() {
        [blur, offset, flood, composite] => (blur, Some(offset), flood, composite),
        [blur, flood, composite] => (blur, None, flood, composite),
        _ => return None,
    };
    let prefix = format!("merman-shadow-{}", filter.stages.len());
    let (offset_value, mask) = if let Some(offset) = offset {
        if offset[0] != blur[2] || offset[3] != format!("{prefix}-offset") {
            return None;
        }
        (
            [
                parse_finite_number(&offset[1])?,
                parse_finite_number(&offset[2])?,
            ],
            &offset[3],
        )
    } else {
        ([0.0; 2], &blur[2])
    };
    if blur[0] != "SourceAlpha"
        || blur[2] != format!("{prefix}-blur")
        || flood[1] != format!("{prefix}-flood")
        || composite[0] != flood[1]
        || &composite[1] != mask
        || composite[2] != "in"
        || composite[3] != format!("{prefix}-shadow")
    {
        return None;
    }
    Some(DropShadowDefinition {
        input: EffectInput::SourceGraphic,
        offset: offset_value,
        std_deviation: parse_std_deviation(&blur[1])?,
        color_css: flood[0].clone(),
    })
}

fn finish_expanded_shadow(filter: &OpenFilter) -> Option<DropShadowDefinition> {
    let (blur, offset, flood, composite, merge, shadow_node, source_node) =
        match filter.expanded_elements.as_slice() {
            [
                blur,
                offset,
                flood,
                composite,
                merge,
                shadow_node,
                source_node,
            ] => (
                blur,
                Some(offset),
                flood,
                composite,
                merge,
                shadow_node,
                source_node,
            ),
            [blur, flood, composite, merge, shadow_node, source_node] => (
                blur,
                None,
                flood,
                composite,
                merge,
                shadow_node,
                source_node,
            ),
            _ => return None,
        };
    let index = filter.stages.len();
    let prefix = format!("merman-shadow-{index}");
    let input = if blur[0] == "SourceGraphic" {
        EffectInput::SourceGraphic
    } else if index > 0 && blur[0] == format!("merman-shadow-{}-result", index - 1) {
        EffectInput::Previous
    } else {
        return None;
    };
    let (offset_value, mask) = if let Some(offset) = offset {
        if offset[0] != blur[2] || offset[3] != format!("{prefix}-offset") {
            return None;
        }
        (
            [
                parse_finite_number(&offset[1])?,
                parse_finite_number(&offset[2])?,
            ],
            &offset[3],
        )
    } else {
        ([0.0, 0.0], &blur[2])
    };
    if blur[2] != format!("{prefix}-blur")
        || flood[1] != format!("{prefix}-flood")
        || composite[0] != flood[1]
        || &composite[1] != mask
        || composite[2] != "in"
        || composite[3] != format!("{prefix}-shadow")
        || merge[0] != format!("{prefix}-result")
        || shadow_node[0] != composite[3]
        || source_node[0] != blur[0]
    {
        return None;
    }
    Some(DropShadowDefinition {
        input,
        offset: offset_value,
        std_deviation: parse_std_deviation(&blur[1])?,
        color_css: flood[0].clone(),
    })
}

fn parse_drop_shadow(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    stage_index: usize,
) -> Result<Option<DropShadowDefinition>, ()> {
    let mut input = None;
    let mut dx = None;
    let mut dy = None;
    let mut std_deviation = None;
    let mut flood_color = None;
    let mut valid = true;

    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        if !matches!(namespace, ResolveResult::Unbound) {
            valid = false;
            continue;
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| ())?
            .into_owned();
        let accepted = match local_name.as_ref() {
            "in" => set_once(&mut input, value),
            "dx" => set_once(&mut dx, value),
            "dy" => set_once(&mut dy, value),
            "stdDeviation" => set_once(&mut std_deviation, value),
            "flood-color" => set_once(&mut flood_color, value),
            _ => false,
        };
        valid &= accepted;
    }

    // Unknown result names fall back to the previous stage in usvg. Accept only the
    // explicit writer grammar so that this recovery cannot certify a different graph.
    let input = match input.as_deref() {
        Some("SourceGraphic") => Some(EffectInput::SourceGraphic),
        None if stage_index > 0 => Some(EffectInput::Previous),
        _ => None,
    };
    valid &= input.is_some();
    let offset = dx
        .as_deref()
        .and_then(parse_finite_number)
        .zip(dy.as_deref().and_then(parse_finite_number))
        .map(|(dx, dy)| [dx, dy]);
    let std_deviation = std_deviation.as_deref().and_then(parse_std_deviation);
    valid &= offset.is_some() && std_deviation.is_some() && flood_color.is_some();

    Ok(valid.then(|| DropShadowDefinition {
        input: input.expect("validated drop-shadow input"),
        offset: offset.expect("validated drop-shadow offset"),
        std_deviation: std_deviation.expect("validated drop-shadow standard deviation"),
        color_css: flood_color.expect("validated drop-shadow color"),
    }))
}

fn parse_std_deviation(value: &str) -> Option<[f32; 2]> {
    let mut values = svgtypes::NumberListParser::from(value);
    let first = values.next()?.ok()? as f32;
    let second = match values.next() {
        Some(value) => value.ok()? as f32,
        None => first,
    };
    values.next().is_none().then_some([first, second])
}

fn parse_finite_number(value: &str) -> Option<f32> {
    let value = value.trim().parse::<f32>().ok()?;
    value.is_finite().then_some(value)
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> bool {
    if slot.is_some() {
        false
    } else {
        *slot = Some(value);
        true
    }
}

fn invalidate_containing_filters(depth: usize, open_filters: &mut [OpenFilter]) {
    for filter in open_filters {
        if depth > filter.depth {
            filter.valid = false;
        }
    }
}

fn record_definition(
    filter: OpenFilter,
    definitions: &mut BTreeMap<String, DefinitionSlot>,
) -> Option<()> {
    let OpenFilter {
        id,
        units,
        region,
        color_space,
        composition,
        stages,
        expanded_elements,
        independent_merge_complete,
        valid,
        ..
    } = filter;
    let Some(id) = id else {
        return Some(());
    };
    if !id.contains(TYPED_FILTER_ID_MARKER) {
        return Some(());
    }
    let definition = if valid
        && !stages.is_empty()
        && expanded_elements.is_empty()
        && (composition == EffectComposition::Cascade || independent_merge_complete)
    {
        region
            .zip(color_space)
            .zip(units)
            .map(|((region, color_space), units)| FilterDefinition {
                units,
                region,
                color_space,
                composition,
                stages,
            })
    } else {
        None
    };
    let slot = definitions.entry(id).or_insert(DefinitionSlot {
        occurrences: 0,
        definition: None,
    });
    slot.occurrences = slot.occurrences.checked_add(1)?;
    if slot.occurrences == 1 {
        slot.definition = definition;
    } else {
        slot.definition = None;
    }
    Some(())
}

fn is_svg_element(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    expected_local_name: &str,
) -> bool {
    if !is_svg_namespace(reader, element) {
        return false;
    }
    let (_, local_name) = reader.resolver().resolve_element(element.name());
    local_name.as_ref() == expected_local_name
}

fn is_svg_namespace(reader: &NsReader<&[u8]>, element: &BytesStart<'_>) -> bool {
    let (namespace, _) = reader.resolver().resolve_element(element.name());
    matches!(namespace, ResolveResult::Unbound)
        || matches!(namespace, ResolveResult::Bound(namespace) if namespace.as_ref() == SVG_NAMESPACE)
}

fn is_usvg_attribute_namespace(namespace: &ResolveResult<'_>) -> bool {
    matches!(namespace, ResolveResult::Unbound)
        || matches!(namespace, ResolveResult::Bound(namespace) if matches!(namespace.as_ref(), SVG_NAMESPACE | XLINK_NAMESPACE | XML_NAMESPACE))
}

fn resolved_tree_matches(
    tree: &usvg::Tree,
    shadows: &[NativeSvgFilterApplication],
    viewport: usvg::Rect,
    control: &OperationControl,
) -> bool {
    let expected = shadows
        .iter()
        .map(|shadow| (shadow.filter_id(), shadow))
        .collect::<BTreeMap<_, _>>();
    let mut resolved_ids = HashSet::with_capacity(expected.len());
    for filter in tree.filters() {
        if crate::export_checkpoint(control).is_err() {
            return false;
        }
        let Some(shadow) = expected.get(filter.id()) else {
            if filter.id().contains(TYPED_FILTER_ID_MARKER) {
                return false;
            }
            continue;
        };
        if !resolved_ids.insert(filter.id()) || !resolved_filter_matches(filter, shadow) {
            return false;
        }
    }
    if resolved_ids.len() != expected.len() {
        return false;
    }

    let mut group_references = shadows
        .iter()
        .map(|shadow| (shadow.filter_id().to_string(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut visited = HashMap::<*const usvg::Group, Option<TextPaint>>::new();
    if visit_group(
        tree.root(),
        &expected,
        &mut group_references,
        &mut visited,
        viewport,
        false,
        control,
    )
    .is_none()
    {
        return false;
    }
    group_references.values().all(|count| *count == 1)
}

fn resolved_filter_matches(
    filter: &usvg::filter::Filter,
    application: &NativeSvgFilterApplication,
) -> bool {
    if application.composition() == EffectComposition::Independent {
        return resolved_independent_filter_matches(filter, application);
    }
    if application.color_space() == EffectColorSpace::Srgb {
        return resolved_expanded_filter_matches(filter, application);
    }
    if filter.primitives().len() != application.stages().len() {
        return false;
    }
    let color_space = match application.color_space() {
        EffectColorSpace::LinearRgb => usvg::filter::ColorInterpolation::LinearRGB,
        EffectColorSpace::Srgb => usvg::filter::ColorInterpolation::SRGB,
    };
    filter
        .primitives()
        .iter()
        .zip(application.stages())
        .enumerate()
        .all(|(index, (primitive, stage))| {
            if primitive.color_interpolation() != color_space {
                return false;
            }
            let usvg::filter::Kind::DropShadow(drop_shadow) = primitive.kind() else {
                return false;
            };
            let input_matches = match (stage.input(), drop_shadow.input()) {
                (EffectInput::SourceGraphic, usvg::filter::Input::SourceGraphic) => true,
                (EffectInput::Previous, usvg::filter::Input::Reference(result)) => index
                    .checked_sub(1)
                    .is_some_and(|previous| filter.primitives()[previous].result() == result),
                _ => false,
            };
            let [expected_dx, expected_dy] = stage.offset();
            let [expected_std_dev_x, expected_std_dev_y] = stage.std_deviation();
            let [red, green, blue, alpha] = stage.color_rgba();
            input_matches
                && same_f32(drop_shadow.dx(), expected_dx)
                && same_f32(drop_shadow.dy(), expected_dy)
                && same_f32(drop_shadow.std_dev_x().get(), expected_std_dev_x)
                && same_f32(drop_shadow.std_dev_y().get(), expected_std_dev_y)
                && drop_shadow.color() == usvg::Color::new_rgb(red, green, blue)
                && drop_shadow.opacity() == usvg::Opacity::new_u8(alpha)
        })
}

fn resolved_independent_filter_matches(
    filter: &usvg::filter::Filter,
    application: &NativeSvgFilterApplication,
) -> bool {
    use usvg::filter::{ColorInterpolation, CompositeOperator, Input, Kind};
    let color_space = match application.color_space() {
        EffectColorSpace::LinearRgb => ColorInterpolation::LinearRGB,
        EffectColorSpace::Srgb => ColorInterpolation::SRGB,
    };
    let mut remaining = filter.primitives();
    let mut shadows = Vec::with_capacity(application.stages().len() + 1);
    for (index, stage) in application.stages().iter().enumerate() {
        let count = if remaining
            .get(1)
            .is_some_and(|p| matches!(p.kind(), Kind::Offset(_)))
        {
            4
        } else {
            3
        };
        let Some((primitives, rest)) = remaining.split_at_checked(count) else {
            return false;
        };
        remaining = rest;
        let (blur, offset, flood, composite) = match primitives {
            [blur, offset, flood, composite] => (blur, Some(offset), flood, composite),
            [blur, flood, composite] => (blur, None, flood, composite),
            _ => return false,
        };
        let prefix = format!("merman-shadow-{index}");
        if primitives
            .iter()
            .any(|p| p.color_interpolation() != color_space)
            || [blur, flood, composite]
                .into_iter()
                .zip(["blur", "flood", "shadow"])
                .any(|(primitive, suffix)| primitive.result() != format!("{prefix}-{suffix}"))
        {
            return false;
        }
        let (
            Kind::GaussianBlur(blur_value),
            Kind::Flood(flood_value),
            Kind::Composite(composite_value),
        ) = (blur.kind(), flood.kind(), composite.kind())
        else {
            return false;
        };
        let [dx, dy] = stage.offset();
        let mask = if let Some(offset) = offset {
            let Kind::Offset(value) = offset.kind() else {
                return false;
            };
            if offset.result() != format!("{prefix}-offset")
                || offset.color_interpolation() != color_space
                || value.input() != &Input::Reference(blur.result().to_string())
                || !same_f32(value.dx(), dx)
                || !same_f32(value.dy(), dy)
            {
                return false;
            }
            offset.result()
        } else {
            if dx != 0.0 || dy != 0.0 {
                return false;
            }
            blur.result()
        };
        let [std_x, std_y] = stage.std_deviation();
        let [red, green, blue, alpha] = stage.color_rgba();
        if blur_value.input() != &Input::SourceAlpha
            || !same_f32(blur_value.std_dev_x().get(), std_x)
            || !same_f32(blur_value.std_dev_y().get(), std_y)
            || flood_value.color() != usvg::Color::new_rgb(red, green, blue)
            || flood_value.opacity() != usvg::Opacity::new_u8(alpha)
            || composite_value.input1() != &Input::Reference(flood.result().to_string())
            || composite_value.input2() != &Input::Reference(mask.to_string())
            || composite_value.operator() != CompositeOperator::In
        {
            return false;
        }
        shadows.push(Input::Reference(composite.result().to_string()));
    }
    shadows.push(Input::SourceGraphic);
    let [merge] = remaining else {
        return false;
    };
    let Kind::Merge(value) = merge.kind() else {
        return false;
    };
    merge.result() == "merman-shadow-stack-result"
        && merge.color_interpolation() == color_space
        && value.inputs() == shadows
}

fn resolved_expanded_filter_matches(
    filter: &usvg::filter::Filter,
    application: &NativeSvgFilterApplication,
) -> bool {
    use usvg::filter::{ColorInterpolation, CompositeOperator, Input, Kind};
    let mut remaining = filter.primitives();
    for (index, stage) in application.stages().iter().enumerate() {
        let count = if remaining
            .get(1)
            .is_some_and(|p| matches!(p.kind(), Kind::Offset(_)))
        {
            5
        } else {
            4
        };
        let Some((primitives, rest)) = remaining.split_at_checked(count) else {
            return false;
        };
        remaining = rest;
        let (blur, offset, flood, composite, merge) = match primitives {
            [blur, offset, flood, composite, merge] => {
                (blur, Some(offset), flood, composite, merge)
            }
            [blur, flood, composite, merge] => (blur, None, flood, composite, merge),
            _ => return false,
        };
        if primitives
            .iter()
            .any(|p| p.color_interpolation() != ColorInterpolation::SRGB)
        {
            return false;
        }
        let prefix = format!("merman-shadow-{index}");
        if [blur, flood, composite, merge]
            .into_iter()
            .zip(["blur", "flood", "shadow", "result"])
            .any(|(primitive, suffix)| primitive.result() != format!("{prefix}-{suffix}"))
        {
            return false;
        }
        let (
            Kind::GaussianBlur(blur_value),
            Kind::Flood(flood_value),
            Kind::Composite(composite_value),
            Kind::Merge(merge_value),
        ) = (blur.kind(), flood.kind(), composite.kind(), merge.kind())
        else {
            return false;
        };
        let input = match stage.input() {
            EffectInput::SourceGraphic => Input::SourceGraphic,
            EffectInput::Previous => {
                let Some(previous) = index.checked_sub(1) else {
                    return false;
                };
                Input::Reference(format!("merman-shadow-{previous}-result"))
            }
        };
        let [dx, dy] = stage.offset();
        let mask = if let Some(offset) = offset {
            let Kind::Offset(value) = offset.kind() else {
                return false;
            };
            if offset.result() != format!("{prefix}-offset")
                || value.input() != &Input::Reference(blur.result().to_string())
                || !same_f32(value.dx(), dx)
                || !same_f32(value.dy(), dy)
            {
                return false;
            }
            offset.result()
        } else {
            if dx != 0.0 || dy != 0.0 {
                return false;
            }
            blur.result()
        };
        let [std_x, std_y] = stage.std_deviation();
        let [red, green, blue, alpha] = stage.color_rgba();
        if blur_value.input() != &input
            || !same_f32(blur_value.std_dev_x().get(), std_x)
            || !same_f32(blur_value.std_dev_y().get(), std_y)
            || flood_value.color() != usvg::Color::new_rgb(red, green, blue)
            || flood_value.opacity() != usvg::Opacity::new_u8(alpha)
            || composite_value.input1() != &Input::Reference(flood.result().to_string())
            || composite_value.input2() != &Input::Reference(mask.to_string())
            || composite_value.operator() != CompositeOperator::In
            || merge_value.inputs() != [Input::Reference(composite.result().to_string()), input]
        {
            return false;
        }
    }
    remaining.is_empty()
}

fn same_f32(left: f32, right: f32) -> bool {
    let normalized_bits = |value: f32| {
        if value == 0.0 {
            0.0f32.to_bits()
        } else {
            value.to_bits()
        }
    };
    normalized_bits(left) == normalized_bits(right)
}

/// Text paint bounds in local coordinates plus filter restrictions for every paint kind.
/// Each subtree is visited once; independent siblings do not become one another's source.
#[derive(Clone, Copy, Default)]
struct TextPaint {
    bounds: Option<usvg::Rect>,
    restricted: bool,
    filter_restricted: bool,
    has_typed_application: bool,
}

impl TextPaint {
    fn include_filters(&mut self, other: &Self) {
        self.filter_restricted |= other.filter_restricted;
        self.has_typed_application |= other.has_typed_application;
    }

    fn include(&mut self, bounds: usvg::Rect) -> Option<()> {
        self.bounds = Some(match self.bounds {
            Some(current) => current.join(&bounds)?,
            None => bounds,
        });
        Some(())
    }
}

fn visit_group(
    group: &usvg::Group,
    expected: &BTreeMap<&str, &NativeSvgFilterApplication>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashMap<*const usvg::Group, Option<TextPaint>>,
    viewport: usvg::Rect,
    ancestor_clipped: bool,
    control: &OperationControl,
) -> Option<TextPaint> {
    crate::export_checkpoint(control).ok()?;
    let clipped = ancestor_clipped || group.clip_path().is_some() || group.mask().is_some();
    let group_ptr = std::ptr::from_ref(group);
    if let Some(paint) = visited.get(&group_ptr) {
        let mut paint = (*paint)?;
        // Cached typed paint cannot certify a restricted embedding without rechecking its
        // source. Preserve all application flags and fail closed rather than reuse that proof.
        if clipped && paint.has_typed_application {
            return None;
        }
        paint.restricted |= clipped;
        return Some(paint);
    }
    // Reuse restrictions on shared subroots; an in-progress entry cannot prove a cycle safe.
    visited.insert(group_ptr, None);
    let mut text_paint = TextPaint {
        restricted: clipped,
        ..TextPaint::default()
    };
    for node in group.children() {
        crate::export_checkpoint(control).ok()?;
        match node {
            usvg::Node::Group(child) => {
                let child_paint = visit_group(
                    child, expected, references, visited, viewport, clipped, control,
                )?;
                text_paint.include_filters(&child_paint);
                if let Some(bounds) = child_paint.bounds {
                    text_paint.include(bounds.transform(child.transform())?)?;
                    text_paint.restricted |= child_paint.restricted;
                }
            }
            usvg::Node::Text(text) => {
                // usvg derives this box from flattened outlines, unlike its SVG text-metrics
                // objectBoundingBox. SVG element transforms are already represented by groups.
                let bounds = text.stroke_bounding_box();
                text_paint.include(bounds)?;
            }
            _ => {}
        }
        if !matches!(node, usvg::Node::Group(_)) {
            let mut subroots_match = true;
            node.subroots(|subroot| {
                if subroots_match {
                    if let Some(paint) = visit_group(
                        subroot, expected, references, visited, viewport, true, control,
                    ) {
                        text_paint.include_filters(&paint);
                    } else {
                        subroots_match = false;
                    }
                }
            });
            if !subroots_match {
                return None;
            }
        }
    }

    if let Some(clip_path) = group.clip_path() {
        text_paint.include_filters(&visit_clip_path(
            clip_path, expected, references, visited, viewport, control,
        )?);
    }
    if let Some(mask) = group.mask() {
        text_paint.include_filters(&visit_mask(
            mask, expected, references, visited, viewport, control,
        )?);
    }

    let typed_filter = group
        .filters()
        .iter()
        .find(|filter| expected.contains_key(filter.id()));
    if let Some(filter) = typed_filter {
        // Typed groups keep their single exact filter and exclude untyped source effects.
        if group.filters().len() != 1 || clipped || text_paint.filter_restricted {
            return None;
        }
        // usvg can retain a filter group after discarding empty text or a missing font.
        // Such a definition is not an observed paint terminal.
        if group.children().is_empty() {
            return None;
        }
        let shadow = expected.get(filter.id())?;
        if !resolved_filter_region_matches(group, filter, shadow) {
            return None;
        }
        if let Some(ink) = text_paint.bounds {
            if text_paint.restricted {
                return None;
            }
            let [top, right, bottom, left] = shadow.paint_outsets()?;
            let paint = usvg::Rect::from_ltrb(
                (f64::from(ink.left()) - left) as f32,
                (f64::from(ink.top()) - top) as f32,
                (f64::from(ink.right()) + right) as f32,
                (f64::from(ink.bottom()) + bottom) as f32,
            )?;
            if !contains_with_roundoff(filter.rect().to_rect(), paint)
                || !contains_with_roundoff(viewport, paint.transform(group.abs_transform())?)
            {
                return None;
            }
            text_paint.bounds = Some(paint);
            text_paint.restricted = true;
        }
        let count = references.get_mut(filter.id())?;
        *count = count.checked_add(1)?;
        if *count != 1 {
            return None;
        }
        text_paint.has_typed_application = true;
    } else if !group.filters().is_empty() {
        // Unknown filters may alter or clip typed descendant paint. Carry the restriction
        // upward even without text bounds so a typed ancestor cannot consume their output.
        if text_paint.has_typed_application {
            return None;
        }
        text_paint.filter_restricted = true;
        for filter in group.filters() {
            for primitive in filter.primitives() {
                crate::export_checkpoint(control).ok()?;
                if let usvg::filter::Kind::Image(image) = primitive.kind()
                    && visit_group(
                        image.root(),
                        expected,
                        references,
                        visited,
                        viewport,
                        true,
                        control,
                    )?
                    .has_typed_application
                {
                    return None;
                }
            }
        }
    }

    visited.insert(group_ptr, Some(text_paint));
    Some(text_paint)
}

fn contains_with_roundoff(outer: usvg::Rect, inner: usvg::Rect) -> bool {
    // Account only for f32 arithmetic at each compared edge. A large canvas width must not
    // grant unrelated slack at its zero origin.
    [
        (inner.left(), outer.left()),
        (inner.top(), outer.top()),
        (outer.right(), inner.right()),
        (outer.bottom(), inner.bottom()),
    ]
    .into_iter()
    .all(|(value, minimum)| {
        let tolerance = value.abs().max(minimum.abs()).max(1.0) * f32::EPSILON * 4.0;
        value >= minimum - tolerance
    })
}

fn resolved_filter_region_matches(
    group: &usvg::Group,
    filter: &usvg::filter::Filter,
    shadow: &NativeSvgFilterApplication,
) -> bool {
    let actual = filter.rect();
    let [x, y, width, height] = shadow.region();
    let Some(mut expected) = usvg::NonZeroRect::from_xywh(x, y, width, height) else {
        return false;
    };
    if shadow.units() == NativeSvgFilterUnits::ObjectBoundingBox {
        let Some(object_bbox) = group.bounding_box().to_non_zero_rect() else {
            return false;
        };
        // usvg normalizes the fractional rectangle before scaling it to object coordinates.
        let Some(mapped) = usvg::NonZeroRect::from_xywh(
            expected.x() * object_bbox.width() + object_bbox.x(),
            expected.y() * object_bbox.height() + object_bbox.y(),
            expected.width() * object_bbox.width(),
            expected.height() * object_bbox.height(),
        ) else {
            return false;
        };
        expected = mapped;
    }
    // Compare the same native rectangle representation: xywh construction rounds right/bottom
    // to f32, so subtracting x/y need not recover the original serialized width/height bits.
    [actual.left(), actual.top(), actual.right(), actual.bottom()]
        .into_iter()
        .zip([
            expected.left(),
            expected.top(),
            expected.right(),
            expected.bottom(),
        ])
        .all(|(actual, expected)| same_f32(actual, expected))
}

fn visit_clip_path(
    clip_path: &usvg::ClipPath,
    expected: &BTreeMap<&str, &NativeSvgFilterApplication>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashMap<*const usvg::Group, Option<TextPaint>>,
    viewport: usvg::Rect,
    control: &OperationControl,
) -> Option<TextPaint> {
    let mut paint = visit_group(
        clip_path.root(),
        expected,
        references,
        visited,
        viewport,
        true,
        control,
    )?;
    if let Some(nested) = clip_path.clip_path() {
        paint.include_filters(&visit_clip_path(
            nested, expected, references, visited, viewport, control,
        )?);
    }
    Some(paint)
}

fn visit_mask(
    mask: &usvg::Mask,
    expected: &BTreeMap<&str, &NativeSvgFilterApplication>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashMap<*const usvg::Group, Option<TextPaint>>,
    viewport: usvg::Rect,
    control: &OperationControl,
) -> Option<TextPaint> {
    let mut paint = visit_group(
        mask.root(),
        expected,
        references,
        visited,
        viewport,
        true,
        control,
    )?;
    if let Some(nested) = mask.mask() {
        paint.include_filters(&visit_mask(
            nested, expected, references, visited, viewport, control,
        )?);
    }
    Some(paint)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST_ID: &str = "diagram-state-ready-theme-effect-hard-shadow";
    const SECOND_ID: &str = "diagram-state-done-theme-effect-hard-shadow";

    fn filter(id: &str, dx: i32, std_deviation: &str, color: &str) -> String {
        format!(
            r#"<filter id="{id}" filterUnits="objectBoundingBox" x="-0.2" y="-0.2" width="1.4" height="1.4" color-interpolation-filters="linearRGB"><feDropShadow in="SourceGraphic" dx="{dx}" dy="5" stdDeviation="{std_deviation}" flood-color="{color}"/></filter>"#
        )
    }

    fn exact_svg() -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100"><defs>{}{}</defs><g filter="url(#{FIRST_ID})"><rect x="10" y="10" width="50" height="30" fill="#fff"/></g><g filter="url(#{SECOND_ID})"><rect x="100" y="10" width="50" height="30" fill="#fff"/></g></svg>"##,
            filter(FIRST_ID, 4, "0", "#112233"),
            filter(SECOND_ID, -3, "8", "rgba(68, 85, 102, 0.5)"),
        )
    }

    fn parse_tree(svg: &str) -> usvg::Tree {
        usvg::Tree::from_str(svg, &usvg::Options::default()).expect("valid SVG test tree")
    }

    #[test]
    fn filter_certification_observes_cancellation_during_raw_xml_scan() {
        let svg = exact_svg();
        assert!(
            super::parse_raw_drop_shadows(&svg, &merman_model::OperationControl::new()).is_some()
        );
        let control = merman_model::OperationControl::new();
        control.cancel_after_checkpoints(3);

        assert!(super::parse_raw_drop_shadows(&svg, &control).is_none());
        assert!(control.is_cancelled());

        assert!(matches!(
            crate::export_checkpoint(&control),
            Err(crate::ExportError::Cancelled(cancelled))
                if cancelled.phase == merman_model::OperationPhase::Export
        ));
    }

    #[test]
    fn filter_certification_observes_cancellation_during_resolved_tree_walk() {
        let svg = exact_svg();
        let tree = parse_tree(&svg);
        let control = merman_model::OperationControl::new();
        let shadows = parse_raw_drop_shadows(&svg).expect("valid raw filter definitions");
        control.cancel_after_checkpoints(3);

        assert!(!resolved_tree_matches(
            &tree,
            &shadows,
            tree.size().to_rect(0.0, 0.0).unwrap(),
            &control,
        ));
        assert!(matches!(
            crate::export_checkpoint(&control),
            Err(crate::ExportError::Cancelled(cancelled))
                if cancelled.phase == merman_model::OperationPhase::Export
        ));
    }

    fn independent_svg(color_space: &str) -> String {
        let mut stages = String::new();
        for (index, dx, color) in [(0, -15, "#803020"), (1, 15, "#204080")] {
            stages.push_str(&format!(
                r#"<feGaussianBlur in="SourceAlpha" stdDeviation="0" result="merman-shadow-{index}-blur"/><feOffset in="merman-shadow-{index}-blur" dx="{dx}" dy="0" result="merman-shadow-{index}-offset"/><feFlood flood-color="{color}" result="merman-shadow-{index}-flood"/><feComposite in="merman-shadow-{index}-flood" in2="merman-shadow-{index}-offset" operator="in" result="merman-shadow-{index}-shadow"/>"#
            ));
        }
        stages.push_str(r#"<feMerge result="merman-shadow-stack-result"><feMergeNode in="merman-shadow-0-shadow"/><feMergeNode in="merman-shadow-1-shadow"/><feMergeNode in="SourceGraphic"/></feMerge>"#);
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs><filter id="{FIRST_ID}" filterUnits="objectBoundingBox" x="-1" y="-1" width="3" height="3" color-interpolation-filters="{color_space}">{stages}</filter></defs><g filter="url(#{FIRST_ID})"><rect x="50" y="30" width="50" height="30" fill="#fff"/></g></svg>"##
        )
    }

    #[test]
    fn independent_receipt_requires_exact_branches_and_merge_order() {
        for color_space in ["sRGB", "linearRGB"] {
            let svg = independent_svg(color_space);
            let tree = parse_tree(&svg);
            let receipt =
                preflight_native_filter_receipt(&svg, &tree).expect("independent stack receipt");
            assert_eq!(receipt.drop_shadow_count(), 2);
            for mutation in [
                svg.replacen("SourceAlpha", "SourceGraphic", 1),
                svg.replace(r#"in2="merman-shadow-1-offset""#, r#"in2="merman-shadow-0-offset""#),
                svg.replace(r#"<feMergeNode in="merman-shadow-0-shadow"/><feMergeNode in="merman-shadow-1-shadow"/>"#, r#"<feMergeNode in="merman-shadow-1-shadow"/><feMergeNode in="merman-shadow-0-shadow"/>"#),
                svg.replace(r#"<feMergeNode in="SourceGraphic"/>"#, ""),
                svg.replace("dx=\"15\"", "dx=\"16\""),
            ] {
                let changed_tree = parse_tree(&mutation);
                assert!(preflight_native_filter_receipt(&mutation, &tree).is_none());
                assert!(preflight_native_filter_receipt(&svg, &changed_tree).is_none());
            }
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    #[test]
    fn independent_branches_preserve_source_and_dark_shadow_colors() {
        for color_space in ["sRGB", "linearRGB"] {
            let svg = independent_svg(color_space);
            let tree = parse_tree(&svg);
            assert!(preflight_native_filter_receipt(&svg, &tree).is_some());
            let mut pixels = tiny_skia::Pixmap::new(200, 100).unwrap();
            resvg::render(
                &tree,
                tiny_skia::Transform::identity(),
                &mut pixels.as_mut(),
            );
            for (x, expected) in [
                (40, [128, 48, 32, 255]),
                (75, [255; 4]),
                (110, [32, 64, 128, 255]),
            ] {
                let color = pixels.pixel(x, 45).unwrap().demultiply();
                let actual = [color.red(), color.green(), color.blue(), color.alpha()];
                assert_eq!(actual[3], expected[3], "{color_space} alpha at x={x}");
                // The native backend uses 8-bit linearRGB intermediates: converting dark sRGB
                // channels to linear and back can differ by two levels. sRGB stays exact.
                let tolerance = if color_space == "linearRGB" { 2 } else { 0 };
                assert!(
                    actual[..3]
                        .iter()
                        .zip(&expected[..3])
                        .all(|(actual, expected)| actual.abs_diff(*expected) <= tolerance),
                    "{color_space} at x={x}: {actual:?}, expected {expected:?}"
                );
            }
        }
    }

    fn preflight_native_filter_receipt(
        svg: &str,
        tree: &usvg::Tree,
    ) -> Option<NativeSvgFilterReceipt> {
        super::preflight_native_filter_receipt(
            svg,
            tree,
            tree.size().to_rect(0.0, 0.0)?,
            &OperationControl::new(),
        )
        .expect("active filter certification")
    }

    fn parse_raw_drop_shadows(svg: &str) -> Option<Vec<NativeSvgFilterApplication>> {
        super::parse_raw_drop_shadows(svg, &OperationControl::new())
    }

    fn parse_text_tree(svg: &str) -> usvg::Tree {
        let mut options = usvg::Options::default();
        std::sync::Arc::make_mut(&mut options.fontdb).load_font_data(
            include_bytes!("../../merman-render/tests/fixtures/fonts/FontAwesome-4.6.3.otf")
                .to_vec(),
        );
        usvg::Tree::from_str(svg, &options).expect("valid text filter fixture")
    }

    fn text_shadow_svg(units: &str, region: &str, size: u32, transform: &str) -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}"><defs><filter id="{FIRST_ID}" filterUnits="{units}" {region} color-interpolation-filters="linearRGB"><feDropShadow in="SourceGraphic" dx="0" dy="0" stdDeviation="8" flood-color="#00f2ff"/></filter></defs><g transform="{transform}"><text x="80" y="80" font-family="FontAwesome" font-size="40" filter="url(#{FIRST_ID})">&#xf000;</text></g></svg>"##
        )
    }

    fn with_untyped_siblings(svg: &str) -> String {
        svg.replace(
            "</defs>",
            r##"<filter id="default-shadow"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06"/></filter></defs>"##,
        )
        .replace(
            "</svg>",
            r##"<g filter="url(#default-shadow)"><rect x="155" y="65" width="20" height="10"/></g><g filter="url(#default-shadow)"><rect x="180" y="80" width="10" height="15"/></g></svg>"##,
        )
    }

    #[test]
    fn independent_untyped_siblings_preserve_the_exact_typed_receipt() {
        for source in [
            exact_svg(),
            text_shadow_svg(
                "userSpaceOnUse",
                r#"x="0" y="0" width="200" height="200""#,
                512,
                "translate(40 60) rotate(15)",
            ),
        ] {
            let expected = preflight_native_filter_receipt(&source, &parse_text_tree(&source))
                .expect("the typed-only fixture must certify observed paint");
            let svg = with_untyped_siblings(&source);
            let tree = parse_text_tree(&svg);
            assert!(tree.filters().len() > expected.filter_count() as usize);
            assert_eq!(preflight_native_filter_receipt(&svg, &tree), Some(expected));
            assert_eq!(
                preflight_native_filter_receipt(&source, &tree),
                Some(expected),
                "independent non-typed paint does not participate in typed receipt identity"
            );
        }
    }

    fn assert_reinstantiated_typed_source_is_rejected(svg: &str, filter_count: usize) {
        let tree = parse_tree(svg);
        assert_eq!(tree.filters().len(), filter_count);
        assert!(
            tree.filters()
                .iter()
                .any(|filter| filter.id() == "node-theme-effect-shadow")
        );
        assert!(tree.filters().iter().any(|filter| {
            filter.id() != "ordinary" && !filter.id().contains(TYPED_FILTER_ID_MARKER)
        }));
        assert!(parse_raw_drop_shadows(svg).is_none());
        assert!(preflight_native_filter_receipt(svg, &tree).is_none());
    }

    #[test]
    fn use_cannot_repeat_a_visible_typed_source_under_an_ordinary_alias() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
  </defs>
  <g id="source" filter="url(#node-theme-effect-shadow)">
    <rect x="10" y="10" width="20" height="20"/>
  </g>
  <use href="#source" x="80"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 2);
    }

    #[test]
    fn fe_image_cannot_repeat_an_already_visible_typed_source() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
    <filter id="ordinary" filterUnits="userSpaceOnUse"
            x="0" y="0" width="200" height="100">
      <feImage href="#source"/>
    </filter>
  </defs>
  <g id="source" filter="url(#node-theme-effect-shadow)">
    <rect x="10" y="10" width="20" height="20"/>
  </g>
  <rect width="200" height="100" filter="url(#ordinary)"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 3);
    }

    #[test]
    fn namespaced_filter_cannot_hide_a_second_typed_application() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:s="http://www.w3.org/2000/svg" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
  </defs>
  <rect x="10" y="10" width="20" height="20"
        filter="url(#node-theme-effect-shadow)"/>
  <rect x="70" y="10" width="20" height="20"
        s:filter="url(#node-theme-effect-shadow)"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 2);
    }

    #[test]
    fn raw_href_namespace_cannot_select_a_different_source_than_the_proof() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:q="urn:receipt-review" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
    <g id="plain"><rect width="20" height="20"/></g>
  </defs>
  <g id="source" filter="url(#node-theme-effect-shadow)">
    <rect x="10" y="10" width="20" height="20"/>
  </g>
  <use q:href="#source" href="#plain" x="80"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 2);
    }

    #[test]
    fn raw_id_namespace_cannot_hide_a_filtered_reference_source() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:q="urn:receipt-review" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
  </defs>
  <g q:id="source" filter="url(#node-theme-effect-shadow)">
    <rect x="10" y="10" width="20" height="20"/>
  </g>
  <g id="source"><rect x="70" y="10" width="20" height="20"/></g>
  <use href="#source" x="80"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 2);
    }

    #[test]
    fn geometry_shadow_rejects_own_and_ancestor_clip_or_mask() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="userSpaceOnUse"
            x="0" y="0" width="64" height="64"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="4" dy="0"
                    stdDeviation="0" flood-color="#000000"/>
    </filter>
    <clipPath id="crop"><rect x="20" y="20" width="5" height="20"/></clipPath>
    <mask id="fade" x="0" y="0" width="64" height="64" maskUnits="userSpaceOnUse">
      <rect x="20" y="20" width="5" height="20" fill="white"/>
    </mask>
  </defs>
  <g transform="translate(1 1)">
    <g filter="url(#node-theme-effect-shadow)">
      <rect x="20" y="20" width="20" height="20"/>
    </g>
  </g>
</svg>"##;
        let expected = preflight_native_filter_receipt(source, &parse_tree(source)).unwrap();
        for restriction in [r#"clip-path="url(#crop)""#, r#"mask="url(#fade)""#] {
            for (name, original, replacement) in [
                (
                    "ancestor",
                    r#"<g transform="translate(1 1)">"#,
                    format!(r#"<g transform="translate(1 1)" {restriction}>"#),
                ),
                (
                    "own",
                    r#"<g filter="url(#node-theme-effect-shadow)">"#,
                    format!(r#"<g filter="url(#node-theme-effect-shadow)" {restriction}>"#),
                ),
            ] {
                let svg = source.replace(original, &replacement);
                let tree = parse_tree(&svg);
                assert_eq!(tree.filters().len(), 1, "{name}: {restriction}");
                assert!(
                    preflight_native_filter_receipt(&svg, &tree).is_none(),
                    "{name}: {restriction}"
                );
            }
            let sibling = source.replace(
                "</svg>",
                &format!(
                    r#"<g {restriction}><rect x="20" y="20" width="20" height="20"/></g></svg>"#
                ),
            );
            assert_eq!(
                preflight_native_filter_receipt(&sibling, &parse_tree(&sibling)),
                Some(expected),
                "independent sibling: {restriction}"
            );
        }
    }

    #[test]
    fn foreign_namespace_stylesheet_cannot_hide_a_second_typed_application() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:x="urn:receipt-review" width="200" height="100">
  <defs>
    <filter id="node-theme-effect-shadow" filterUnits="objectBoundingBox"
            x="-0.5" y="-0.5" width="2" height="2"
            color-interpolation-filters="linearRGB">
      <feDropShadow in="SourceGraphic" dx="2" dy="2"
                    stdDeviation="0" flood-color="#112233"/>
    </filter>
    <x:style>.repeat{filter:url(#node-theme-effect-shadow)}</x:style>
  </defs>
  <rect x="10" y="10" width="20" height="20"
        filter="url(#node-theme-effect-shadow)"/>
  <rect class="repeat" x="70" y="10" width="20" height="20"/>
</svg>"##;
        assert_reinstantiated_typed_source_is_rejected(svg, 2);
    }

    #[test]
    fn html_fallback_use_keeps_the_typed_filter_on_its_ancestor() {
        let source = text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(40 60)",
        );
        let expected = preflight_native_filter_receipt(&source, &parse_text_tree(&source)).unwrap();
        let start = source.find("<text ").unwrap();
        let end = source.find("</text>").unwrap() + "</text>".len();
        let filtered_text = &source[start..end];
        let text = filtered_text.replace(&format!(r#" filter="url(#{FIRST_ID})""#), "");
        let definition = format!(
            r##"<g id="fixture-foreignobject-label-0"><svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200" style="overflow: visible; opacity: 1; filter: none; transform: none;"><rect width="200" height="200" fill="none" stroke="none" opacity="0"/>{text}</svg></g>"##
        );
        for href in [
            r##"href="#fixture-foreignobject-label-0""##,
            r##"xlink:href="#fixture-foreignobject-label-0""##,
            r##"href="#fixture-foreignobject-label-0" xlink:href="#fixture-foreignobject-label-0""##,
            r##"href="#fixture-foreignobject-label-&#48;""##,
        ] {
            let svg = with_untyped_siblings(&source)
                .replace(
                    "<svg xmlns=",
                    r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink" xmlns="#,
                )
                .replace("</defs>", &format!("{definition}</defs>"))
                .replace(
                    filtered_text,
                    &format!(
                        r##"<g filter="url(#{FIRST_ID})" style="filter:url(#{FIRST_ID}) !important"><use x="0" y="0" {href} style="opacity: 1; display: inline; visibility: inherit; clip-path: none; mask: none; filter: none; transform: none;"/></g>"##
                    ),
                );
            assert_eq!(
                parse_raw_drop_shadows(&svg)
                    .expect("the fallback definition contains no filtered sibling")
                    .len(),
                1
            );
            assert_eq!(
                preflight_native_filter_receipt(&svg, &parse_text_tree(&svg)),
                Some(expected),
                "the nested SVG, invisible rect, and real text contain no typed filter: {href}"
            );
        }
    }

    #[test]
    fn references_to_nested_or_ambiguous_sources_fail_closed() {
        let source = exact_svg().replacen("<g filter=", r#"<g id="source" filter="#, 1);
        for definitions in [
            r##"<g id="forward"><use href="#source"/></g>"##,
            r##"<g id="forward"><rect width="20" height="20"/></g><g id="forward"><rect width="20" height="20"/></g>"##,
            r##"<g id="forward"><rect width="20" height="20"/></g><g s:id="forward"><rect width="20" height="20"/></g>"##,
        ] {
            let svg = source
                .replace(
                    "<svg xmlns=",
                    r#"<svg xmlns:s="http://www.w3.org/2000/svg" xmlns="#,
                )
                .replace("</defs>", &format!("{definitions}</defs>"))
                .replace("</svg>", r##"<use href="#forward"/></svg>"##);
            assert!(parse_raw_drop_shadows(&svg).is_none());
            assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn repeated_resources_do_not_admit_typed_descendants() {
        let source = exact_svg();
        for resource in ["marker", "pattern", "clipPath", "mask", "symbol"] {
            let svg = source
                .replacen("<g filter=", &format!("<{resource}><g filter="), 1)
                .replacen("</g>", &format!("</g></{resource}>"), 1);
            assert!(parse_raw_drop_shadows(&svg).is_none(), "{resource}");
            assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn backend_attribute_namespaces_and_all_stylesheet_fragments_fail_closed() {
        let source = exact_svg();
        for (prefix, namespace) in [
            ("s", SVG_NAMESPACE),
            ("xlink", XLINK_NAMESPACE),
            ("xml", XML_NAMESPACE),
        ] {
            let binding = if prefix == "xml" {
                String::new()
            } else {
                format!(r#" xmlns:{prefix}="{namespace}""#)
            };
            for attribute in [
                format!(r#"{prefix}:filter="url(#{FIRST_ID})""#),
                format!(r#"{prefix}:style="filter:url(#{FIRST_ID})""#),
            ] {
                let svg = source
                    .replace("<svg xmlns=", &format!("<svg{binding} xmlns="))
                    .replace(
                        "</svg>",
                        &format!(r#"<rect width="20" height="20" {attribute}/></svg>"#),
                    );
                assert!(parse_raw_drop_shadows(&svg).is_none(), "{attribute}");
                assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
            }
        }
        for style in [
            r#"<q:style>.repeat{filter:url(#hidden-theme&#45;effect-shadow)}</q:style>"#,
            r#"<q:style><![CDATA[.repeat{filter:url(#hidden-theme-effect-shadow)}]]></q:style>"#,
            r#"<q:style>.repeat{filter:url(#hidden-th&#92;65me-effect-shadow)}</q:style>"#,
        ] {
            let svg = source
                .replace("<svg xmlns=", r#"<svg xmlns:q="urn:receipt-review" xmlns="#)
                .replace("</defs>", &format!("{style}</defs>"));
            assert!(parse_raw_drop_shadows(&svg).is_none(), "{style}");
            assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn typed_reuse_via_inline_style_or_stylesheet_rejects_its_actual_resolved_tree() {
        let source = exact_svg();
        for (style, extra) in [
            (
                String::new(),
                format!(
                    r#"<rect x="60" y="65" width="30" height="15" style='filter:url(#{FIRST_ID})'/>"#
                ),
            ),
            (
                String::new(),
                format!(
                    r#"<rect x="60" y="65" width="30" height="15" transform="translate(5 3)" style='filter:url(#{FIRST_ID}) !important'/>"#
                ),
            ),
            (
                format!(r#"<style>.repeat{{filter:url(#{FIRST_ID})}}</style>"#),
                r#"<rect class="repeat" x="60" y="65" width="30" height="15"/>"#.to_string(),
            ),
            (
                format!(r#"<style><![CDATA[.repeat{{filter:url(#{FIRST_ID})}}]]></style>"#),
                r#"<rect class="repeat" x="60" y="65" width="30" height="15"/>"#.to_string(),
            ),
            (
                format!(r#"<style>.repeat{{filter:url(#{FIRST_ID})}}</style>"#)
                    .replace("-theme-effect-", "-theme&#45;effect-"),
                r#"<rect class="repeat" x="60" y="65" width="30" height="15"/>"#.to_string(),
            ),
        ] {
            let svg = source
                .replace("</defs>", &format!("{style}</defs>"))
                .replace("</svg>", &format!("{extra}</svg>"));
            let tree = parse_tree(&svg);
            assert_eq!(
                tree.filters().len(),
                3,
                "the extra real application must survive as a native bbox-specific alias"
            );
            assert!(
                tree.filters()
                    .iter()
                    .any(|filter| !filter.id().contains(TYPED_FILTER_ID_MARKER))
            );
            assert!(parse_raw_drop_shadows(&svg).is_none());
            assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
        }
    }

    #[test]
    fn escaped_typed_ids_in_inline_style_and_stylesheets_reject_their_actual_tree() {
        let source = exact_svg();
        for escaped_marker in [r"-th\65me-effect-", r"-th&#92;65me-effect-"] {
            let escaped_id = FIRST_ID.replace(TYPED_FILTER_ID_MARKER, escaped_marker);
            assert!(!escaped_id.contains(TYPED_FILTER_ID_MARKER));
            for (style, extra) in [
                (
                    String::new(),
                    format!(
                        r#"<rect x="60" y="65" width="30" height="15" style='filter:url(#{escaped_id})'/>"#
                    ),
                ),
                (
                    format!(r#"<style>.repeat{{filter:url(#{escaped_id})}}</style>"#),
                    r#"<rect class="repeat" x="60" y="65" width="30" height="15"/>"#.to_string(),
                ),
            ] {
                let svg = source
                    .replace("</defs>", &format!("{style}</defs>"))
                    .replace("</svg>", &format!("{extra}</svg>"));
                let tree = parse_tree(&svg);
                assert!(tree.filters().iter().any(|filter| filter.id() == FIRST_ID));
                assert!(
                    parse_raw_drop_shadows(&svg).is_none(),
                    "writer admission must reject CSS escapes independently of backend decoding"
                );
                assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
            }
        }
        let cdata = source.replace(
            "</defs>",
            r#"<style><![CDATA[.repeat{filter:url(#diagram-state-ready-th\65me-effect-hard-shadow)}]]></style></defs>"#,
        );
        assert!(parse_raw_drop_shadows(&cdata).is_none());
        assert!(preflight_native_filter_receipt(&cdata, &parse_tree(&cdata)).is_none());
    }

    #[test]
    fn neo_inline_typed_style_must_repeat_only_its_own_single_filter_attribute() {
        let source = exact_svg();
        let tree = parse_tree(&source);
        let expected = preflight_native_filter_receipt(&source, &tree).unwrap();
        let attribute = format!(r#"filter="url(#{FIRST_ID})""#);
        for style in [
            format!("filter:url(#{FIRST_ID})"),
            format!("fill:#fff;filter:url(#{FIRST_ID}) !important"),
            format!("filter: url(#{FIRST_ID}) !important; stroke: #000"),
        ] {
            for replacement in [
                format!(r#"{attribute} style='{style}'"#),
                format!(r#"style='{style}' {attribute}"#),
            ] {
                let svg = source.replacen(&attribute, &replacement, 1);
                assert_eq!(
                    preflight_native_filter_receipt(&svg, &parse_tree(&svg)),
                    Some(expected),
                    "Neo's same-element attr plus style is one application"
                );
            }
        }
        for style in [
            format!("filter:url(#{SECOND_ID}) !important"),
            format!("filter:url(#{FIRST_ID});filter:url(#{FIRST_ID})"),
            format!("filter:url(#{FIRST_ID});filter:none"),
            format!("filter:none !important;filter:url(#{FIRST_ID})"),
            format!("filter:url(#{FIRST_ID}) url(#{SECOND_ID})"),
            format!("filter:url(#{FIRST_ID});fill:url(#{FIRST_ID})"),
        ] {
            let svg = source.replacen(&attribute, &format!(r#"{attribute} style='{style}'"#), 1);
            assert!(parse_raw_drop_shadows(&svg).is_none(), "{style}");
            assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn untyped_ancestor_descendant_and_same_group_filters_reject_typed_geometry() {
        let source = with_untyped_siblings(&exact_svg());
        assert!(preflight_native_filter_receipt(&source, &parse_tree(&source)).is_some());
        let typed_group = format!(r#"<g filter="url(#{FIRST_ID})">"#);
        for opening in [
            format!(r#"<g filter="url(#default-shadow)">{typed_group}"#),
            format!(r#"{typed_group}<g filter="url(#default-shadow)">"#),
        ] {
            let svg = source
                .replacen(&typed_group, &opening, 1)
                .replacen("</g>", "</g></g>", 1);
            let tree = parse_tree(&svg);
            assert!(preflight_native_filter_receipt(&source, &tree).is_none());
            assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
        }
        for filters in [
            format!("url(#{FIRST_ID}) url(#default-shadow)"),
            format!("url(#default-shadow) url(#{FIRST_ID})"),
        ] {
            let svg = source.replacen(&typed_group, &format!(r#"<g filter="{filters}">"#), 1);
            assert!(preflight_native_filter_receipt(&source, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn untyped_ancestor_and_descendant_filters_cannot_certify_typed_label_paint() {
        let source = with_untyped_siblings(&text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(40 60)",
        ));
        assert!(preflight_native_filter_receipt(&source, &parse_text_tree(&source)).is_some());
        let clipped_parent = source.replace(
            "<g transform=",
            r##"<g filter="url(#default-shadow)" transform="##,
        );
        let filtered_child = source
            .replacen(&format!(r#" filter="url(#{FIRST_ID})""#), "", 1)
            .replace(
                "<text ",
                &format!(r##"<g filter="url(#{FIRST_ID})"><text filter="url(#default-shadow)" "##),
            )
            .replace("</text>", "</text></g>");
        for svg in [clipped_parent, filtered_child] {
            let tree = parse_text_tree(&svg);
            assert!(preflight_native_filter_receipt(&source, &tree).is_none());
            assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
        }
    }

    #[test]
    fn untyped_siblings_cannot_hide_missing_changed_or_unexpected_typed_ids() {
        let source = with_untyped_siblings(&exact_svg());
        let tree = parse_tree(&source);
        assert!(preflight_native_filter_receipt(&source, &tree).is_some());
        let missing = source.replace(&format!(r#" filter="url(#{SECOND_ID})""#), "");
        let changed = source.replacen("dx=\"4\"", "dx=\"6\"", 1);
        let aliased = source.replace(
            &format!(r#"filter="url(#{SECOND_ID})""#),
            &format!(r#"filter="url(#{FIRST_ID})""#),
        );
        let extra_id = "diagram-extra-theme-effect-shadow";
        let extra = source
            .replace(
                "</defs>",
                &format!("{}</defs>", filter(extra_id, 2, "0", "#112233")),
            )
            .replace(
                "</svg>",
                &format!(
                    r##"<g filter="url(#{extra_id})"><rect x="60" y="70" width="30" height="15"/></g></svg>"##
                ),
            );
        for svg in [missing, changed, aliased, extra] {
            assert!(preflight_native_filter_receipt(&source, &parse_tree(&svg)).is_none());
            assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
        }
        let duplicate_definition = source.replace(
            "</defs>",
            &format!("{}</defs>", filter(FIRST_ID, 4, "0", "#112233")),
        );
        let duplicate_reference = source.replace(
            "</svg>",
            &format!(
                r##"<g filter="url(#{FIRST_ID})"><rect x="60" y="70" width="30" height="15"/></g></svg>"##
            ),
        );
        for svg in [duplicate_definition, duplicate_reference] {
            assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
        }
    }

    #[test]
    fn untyped_filter_image_subroot_cannot_import_typed_paint() {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs>{}<g id="source" filter="url(#{FIRST_ID})"><rect x="10" y="10" width="50" height="30"/></g><filter id="default-shadow" filterUnits="userSpaceOnUse" x="0" y="0" width="200" height="100"><feImage href="#source"/></filter></defs><rect width="200" height="100" filter="url(#default-shadow)"/></svg>"##,
            filter(FIRST_ID, 4, "0", "#112233"),
        );
        let tree = parse_tree(&svg);
        assert!(tree.filters().iter().any(|filter| filter.id() == FIRST_ID));
        assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
    }

    #[test]
    fn shared_pattern_filter_restrictions_survive_an_earlier_sibling_visit() {
        for (pattern_filter, consumer_filter) in
            [("default-shadow", FIRST_ID), (FIRST_ID, "default-shadow")]
        {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs>{}<filter id="default-shadow"><feGaussianBlur stdDeviation="1"/></filter><pattern id="pattern" patternUnits="userSpaceOnUse" width="20" height="10"><g filter="url(#{pattern_filter})"><rect width="20" height="10"/></g></pattern></defs><rect x="100" y="60" width="20" height="10" fill="url(#pattern)"/><g filter="url(#{consumer_filter})"><rect x="10" y="10" width="50" height="30" fill="url(#pattern)"/></g></svg>"##,
                filter(FIRST_ID, 4, "0", "#112233"),
            );
            let tree = parse_tree(&svg);
            if pattern_filter == FIRST_ID {
                assert!(parse_raw_drop_shadows(&svg).is_none());
            } else {
                assert_eq!(parse_raw_drop_shadows(&svg).unwrap().len(), 1);
            }
            assert!(
                preflight_native_filter_receipt(&svg, &tree).is_none(),
                "a shared pattern remains interacting source paint: {pattern_filter} -> {consumer_filter}"
            );
        }
    }

    #[test]
    fn cached_typed_paint_cannot_reuse_an_unclipped_proof_in_a_restricted_context() {
        for source in [
            exact_svg(),
            text_shadow_svg(
                "userSpaceOnUse",
                r#"x="0" y="0" width="200" height="200""#,
                512,
                "translate(40 60)",
            ),
        ] {
            let tree = parse_text_tree(&source);
            let shadows = parse_raw_drop_shadows(&source).unwrap();
            let expected = shadows
                .iter()
                .map(|shadow| (shadow.filter_id(), shadow))
                .collect();
            let mut references = shadows
                .iter()
                .map(|shadow| (shadow.filter_id().to_string(), 0))
                .collect();
            let mut visited = HashMap::new();
            let viewport = tree.size().to_rect(0.0, 0.0).unwrap();
            let control = OperationControl::new();
            let paint = visit_group(
                tree.root(),
                &expected,
                &mut references,
                &mut visited,
                viewport,
                false,
                &control,
            )
            .expect("ordinary paint is certified before sharing its subroot");
            assert!(paint.has_typed_application);
            assert!(references.values().all(|count| *count == 1));
            assert!(
                visit_group(
                    tree.root(),
                    &expected,
                    &mut references,
                    &mut visited,
                    viewport,
                    true,
                    &control,
                )
                .is_none(),
                "clip, mask, and filter-image embeddings cannot reuse an unclipped typed proof"
            );
        }
    }

    #[test]
    fn cached_untyped_paint_preserves_current_clipping_and_filter_restrictions() {
        let svg = with_untyped_siblings(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs></defs></svg>"#,
        );
        let tree = parse_tree(&svg);
        let mut references = BTreeMap::new();
        let mut visited = HashMap::new();
        for clipped in [false, true] {
            let paint = visit_group(
                tree.root(),
                &BTreeMap::new(),
                &mut references,
                &mut visited,
                tree.size().to_rect(0.0, 0.0).unwrap(),
                clipped,
                &OperationControl::new(),
            )
            .expect("untyped cached paint still returns a restricted source summary");
            assert_eq!(paint.restricted, clipped);
            assert!(paint.filter_restricted);
            assert!(!paint.has_typed_application);
        }
    }

    #[test]
    fn fallback_font_line_advances_require_a_line_scaled_filter_allocation() {
        let mut options = usvg::Options::default();
        std::sync::Arc::make_mut(&mut options.fontdb).load_font_data(
            include_bytes!("../../merman-render/tests/fixtures/fonts/DejaVuSerif-NativeFilter.ttf")
                .to_vec(),
        );
        std::sync::Arc::make_mut(&mut options.fontdb).load_font_data(
            include_bytes!(
                "../../merman-render/tests/fixtures/fonts/DejaVuSansBold-NativeFilter.ttf"
            )
            .to_vec(),
        );
        // These are the emitted metrics for the failing Linux title, popup, and Flowchart labels.
        // The fixed-em allocation excludes real DejaVu outlines plus their halo. A reserve
        // proportional to the declared line metrics fits all three without changing the receipt.
        for (text, font_size, measured_width, anchor, family, weight) in [
            (
                "Request Volume",
                20.0_f32,
                112.28_f32,
                "middle",
                "DejaVu Serif",
                400,
            ),
            ("Documentation", 16.0, 95.36, "start", "DejaVu Serif", 400),
            ("Browse Products", 14.0, 98.56, "middle", "DejaVu Sans", 600),
        ] {
            let make_svg = |reserve: f32| {
                let origin = if anchor == "middle" { 256.0 } else { 128.0 };
                let measured_left = if anchor == "middle" {
                    origin - measured_width / 2.0
                } else {
                    origin
                };
                let left = measured_left - reserve - 30.0;
                let width = measured_width + 2.0 * (reserve + 30.0);
                format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="256" viewBox="0 0 512 256"><defs><filter id="{FIRST_ID}" filterUnits="userSpaceOnUse" x="{left}" y="64" width="{width}" height="128" color-interpolation-filters="linearRGB"><feDropShadow in="SourceGraphic" dx="0" dy="0" stdDeviation="10" flood-color="#00f2ff"/></filter></defs><text x="{origin}" y="128" text-anchor="{anchor}" dominant-baseline="middle" font-family="{family}" font-weight="{weight}" font-size="{font_size}" filter="url(#{FIRST_ID})">{text}</text></svg>"##
                )
            };
            let narrow = make_svg(font_size);
            let narrow_tree = usvg::Tree::from_str(&narrow, &options).unwrap();
            assert!(
                preflight_native_filter_receipt(&narrow, &narrow_tree).is_none(),
                "the old fixed-em allocation must reject overflowing {text} ink"
            );
            let allocated = make_svg(measured_width.max(font_size));
            let allocated_tree = usvg::Tree::from_str(&allocated, &options).unwrap();
            assert!(
                preflight_native_filter_receipt(&allocated, &allocated_tree).is_some(),
                "the line-scaled allocation must contain {text} ink and halo"
            );
            assert!(
                preflight_native_filter_receipt(&narrow, &allocated_tree).is_none(),
                "a different allocation must never certify the narrow source"
            );
        }
    }

    #[test]
    fn text_shadow_receipt_rejects_a_region_that_cuts_off_native_glyph_glow() {
        for (units, good, clipped) in [
            (
                "userSpaceOnUse",
                r#"x="0" y="0" width="200" height="200""#,
                r#"x="60" y="35" width="80" height="70""#,
            ),
            (
                "objectBoundingBox",
                r#"x="-2" y="-2" width="5" height="5""#,
                r#"x="0" y="0" width="1" height="1""#,
            ),
        ] {
            let svg = text_shadow_svg(units, good, 512, "translate(40 60) rotate(15)");
            let tree = parse_text_tree(&svg);
            assert!(
                preflight_native_filter_receipt(&svg, &tree).is_some(),
                "{units}"
            );
            let svg = text_shadow_svg(units, clipped, 512, "translate(40 60) rotate(15)");
            let tree = parse_text_tree(&svg);
            assert!(
                preflight_native_filter_receipt(&svg, &tree).is_none(),
                "{units}: matching filter attributes do not prove unclipped glyph paint"
            );
        }
    }

    #[test]
    fn text_shadow_receipt_rejects_root_clipping_after_nested_transform() {
        let region = r#"x="0" y="0" width="200" height="200""#;
        let good = text_shadow_svg("userSpaceOnUse", region, 512, "translate(150 20)");
        assert!(preflight_native_filter_receipt(&good, &parse_text_tree(&good)).is_some());
        let clipped = text_shadow_svg("userSpaceOnUse", region, 256, "translate(150 20)");
        assert!(
            preflight_native_filter_receipt(&clipped, &parse_text_tree(&clipped)).is_none(),
            "the root viewport must contain the transformed glyph glow"
        );
    }

    #[test]
    fn text_shadow_receipt_rejects_unobserved_ancestor_clipping_and_masks() {
        let region = r#"x="0" y="0" width="200" height="200""#;
        let source = text_shadow_svg("userSpaceOnUse", region, 512, "translate(40 60)");
        for (definition, attribute) in [
            (
                r#"<clipPath id="clip"><circle cx="80" cy="80" r="80"/></clipPath>"#,
                r#"clip-path="url(#clip)""#,
            ),
            (
                r#"<mask id="mask"><rect width="512" height="512" fill="white"/></mask>"#,
                r#"mask="url(#mask)""#,
            ),
        ] {
            let svg = source
                .replace("</defs>", &format!("{definition}</defs>"))
                .replace("<g transform=", &format!("<g {attribute} transform="));
            assert!(
                preflight_native_filter_receipt(&svg, &parse_text_tree(&svg)).is_none(),
                "{attribute}"
            );
        }
    }

    #[test]
    fn text_shadow_receipt_uses_transformed_ink_with_negative_viewbox_origin() {
        let source = text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(100 40) rotate(15) scale(1.25 0.75)",
        )
        .replace(r#"viewBox="0 0 512 512""#, r#"viewBox="-100 -120 512 512""#)
        .replace(
            r#"x="80" y="80" font-family"#,
            r#"x="100" y="100" text-anchor="end" dominant-baseline="middle" font-family"#,
        );
        assert!(preflight_native_filter_receipt(&source, &parse_text_tree(&source)).is_some());
        for text in ["", " "] {
            let empty = source.replace("&#xf000;", text);
            assert!(preflight_native_filter_receipt(&empty, &parse_text_tree(&empty)).is_none());
        }
        assert!(
            preflight_native_filter_receipt(&source, &parse_tree(&source)).is_none(),
            "a missing native font cannot certify a discarded text terminal"
        );
    }

    #[cfg(feature = "png")]
    #[test]
    fn text_shadow_receipt_uses_actual_content_crop_without_viewbox() {
        let source = text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(40 60)",
        )
        .replace(r#" viewBox="0 0 512 512""#, "")
        .replace(
            r#"width="512" height="512""#,
            r#"width="100%" style="max-width:512px""#,
        );
        let tree = parse_text_tree(&source);
        let reference_viewport = usvg::Rect::from_xywh(0.0, 0.0, 512.0, 512.0).unwrap();
        let expected = super::preflight_native_filter_receipt(
            &source,
            &tree,
            reference_viewport,
            &OperationControl::new(),
        )
        .expect("active filter certification")
        .expect("the full reference viewport contains the effect");
        let metadata =
            crate::parse_root_svg_metadata(&source, &crate::OperationControl::new()).unwrap();
        let (geometry, translated) = crate::raster_geometry_for_svg(metadata, &tree);
        assert!(translated);
        let actual_viewport = usvg::Rect::from_xywh(
            geometry.min_x,
            geometry.min_y,
            geometry.width,
            geometry.height,
        )
        .unwrap();
        assert_eq!(
            super::preflight_native_filter_receipt(
                &source,
                &tree,
                actual_viewport,
                &OperationControl::new(),
            )
            .expect("active filter certification"),
            Some(expected),
            "dimensionless content fitting retains the exact unclipped glyph-glow receipt"
        );
        assert!(
            super::preflight_native_filter_receipt(
                &source,
                &tree,
                tree.root().abs_stroke_bounding_box(),
                &OperationControl::new(),
            )
            .expect("active filter certification")
            .is_none(),
            "the old stroke-only content crop excludes glow and must never be certified"
        );
    }

    #[cfg(feature = "png")]
    #[test]
    fn text_shadow_receipt_preserves_explicit_viewport_without_viewbox() {
        let source = text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(40 60)",
        )
        .replace(r#" viewBox="0 0 512 512""#, "");
        let tree = parse_text_tree(&source);
        let expected = preflight_native_filter_receipt(&source, &tree)
            .expect("the explicit viewport contains actual glyph ink and glow");
        let metadata =
            crate::parse_root_svg_metadata(&source, &crate::OperationControl::new()).unwrap();
        let (geometry, translated) = crate::raster_geometry_for_svg(metadata, &tree);
        assert!(!translated);
        assert_eq!((geometry.min_x, geometry.min_y), (0.0, 0.0));
        assert_eq!((geometry.width, geometry.height), (512.0, 512.0));
        let actual_viewport = usvg::Rect::from_xywh(
            geometry.min_x,
            geometry.min_y,
            geometry.width,
            geometry.height,
        )
        .unwrap();
        assert_eq!(
            super::preflight_native_filter_receipt(
                &source,
                &tree,
                actual_viewport,
                &OperationControl::new(),
            )
            .expect("active filter certification"),
            Some(expected),
            "raster preparation preserves the exact unclipped text-shadow receipt"
        );
    }

    #[test]
    fn text_shadow_receipt_uses_outline_bounds_instead_of_font_metrics() {
        fn first_text(group: &usvg::Group) -> Option<&usvg::Text> {
            group.children().iter().find_map(|node| match node {
                usvg::Node::Text(text) => Some(text.as_ref()),
                usvg::Node::Group(child) => first_text(child),
                _ => None,
            })
        }
        let source = text_shadow_svg(
            "userSpaceOnUse",
            r#"x="0" y="0" width="200" height="200""#,
            512,
            "translate(40 60)",
        )
        .replace("&#xf000;", "&#xf111;");
        let tree = parse_text_tree(&source);
        let text =
            first_text(tree.root()).expect("the supplied test font must produce real glyphs");
        let ink = text.stroke_bounding_box();
        assert_ne!(
            ink,
            text.bounding_box(),
            "this fixture distinguishes outline and metrics bounds"
        );
        let exact = format!(
            r#"x="{}" y="{}" width="{}" height="{}""#,
            ink.left() - 32.0,
            ink.top() - 32.0,
            ink.width() + 64.0,
            ink.height() + 64.0
        );
        let source = source.replace(r#"x="0" y="0" width="200" height="200""#, &exact);
        let exact_tree = parse_text_tree(&source);
        assert!(
            preflight_native_filter_receipt(&source, &exact_tree).is_some(),
            "a region containing actual glyph ink plus four sigma must not require a font-metrics box"
        );
    }

    #[test]
    fn fractional_object_region_follows_both_native_rounding_steps() {
        let source = text_shadow_svg(
            "objectBoundingBox",
            r#"x="-0.7" y="-1.1" width="2.7" height="3.1""#,
            512,
            "translate(40 60)",
        )
        .replace(r#"stdDeviation="8""#, r#"stdDeviation="0""#);
        let tree = parse_text_tree(&source);
        assert!(preflight_native_filter_receipt(&source, &tree).is_some());
        let changed = source.replace(r#"width="2.7""#, r#"width="2.8""#);
        assert!(preflight_native_filter_receipt(&changed, &tree).is_none());
    }

    #[test]
    fn containment_roundoff_does_not_hide_clipping_at_a_large_canvas_origin() {
        let canvas = usvg::Rect::from_xywh(0.0, 0.0, 1_000_000.0, 1_000_000.0).unwrap();
        let clipped = usvg::Rect::from_xywh(-0.1, 10.0, 20.0, 20.0).unwrap();
        assert!(!contains_with_roundoff(canvas, clipped));
        let edge = usvg::Rect::from_xywh(10.0, 10.0, 20.0, 20.0).unwrap();
        let rounded =
            usvg::Rect::from_ltrb(10.0_f32.next_down(), 10.0, 30.0_f32.next_up(), 30.0).unwrap();
        assert!(contains_with_roundoff(edge, rounded));
    }

    #[test]
    fn exact_hard_and_soft_shadow_applications_pass() {
        let svg = exact_svg();
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("exact receipt");

        assert_eq!(receipt.drop_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 2);
    }

    #[test]
    fn user_space_filters_preserve_line_geometry_and_nested_coordinates() {
        for path in ["M 40 60 L 140 60", "M 80 30 L 80 120"] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="220" height="190"><defs><filter id="{FIRST_ID}" filterUnits="userSpaceOnUse" x="15" y="5" width="150" height="140" color-interpolation-filters="linearRGB"><feDropShadow in="SourceGraphic" dx="0" dy="0" stdDeviation="6" flood-color="#00f2ff"/></filter></defs><g transform="translate(13 17)"><path d="{path}" stroke="#fff" stroke-width="2" filter="url(#{FIRST_ID})"/></g></svg>"##
            );
            let tree = parse_tree(&svg);
            let applications = parse_raw_drop_shadows(&svg).unwrap();
            assert_eq!(
                applications[0].units(),
                NativeSvgFilterUnits::UserSpaceOnUse
            );
            let receipt =
                preflight_native_filter_receipt(&svg, &tree).expect("line filter survives usvg");
            assert_eq!(receipt.reference_count(), 1);
            let changed = svg.replacen("x=\"15\"", "x=\"16\"", 1);
            assert!(preflight_native_filter_receipt(&changed, &tree).is_none());
            let wrong_units = svg.replace(
                "filterUnits=\"userSpaceOnUse\"",
                "filterUnits=\"objectBoundingBox\"",
            );
            assert!(preflight_native_filter_receipt(&wrong_units, &tree).is_none());

            #[cfg(feature = "png")]
            {
                let mut pixels = tiny_skia::Pixmap::new(220, 190).unwrap();
                resvg::render(
                    &tree,
                    tiny_skia::Transform::identity(),
                    &mut pixels.as_mut(),
                );
                // Sample outside the white path, where only the cyan shadow can paint.
                let (x, y) = if path.starts_with("M 40") {
                    (93, 83)
                } else {
                    (99, 87)
                };
                let pixel = pixels.pixel(x, y).unwrap();
                assert!(
                    pixel.alpha() > 0 && pixel.blue() > pixel.red(),
                    "line glow must paint native pixels: {pixel:?}"
                );
            }
        }
    }

    #[test]
    fn duplicate_shared_id_fails() {
        let svg = exact_svg().replace(
            &format!("filter=\"url(#{SECOND_ID})\""),
            &format!("filter=\"url(#{FIRST_ID})\""),
        );
        let tree = parse_tree(&svg);

        assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
    }

    #[test]
    fn raw_and_resolved_dx_color_or_region_mismatch_fails() {
        let svg = exact_svg();
        let tree = parse_tree(&svg);
        let wrong_dx = svg.replacen("dx=\"4\"", "dx=\"6\"", 1);
        let wrong_color = svg.replacen("#112233", "#abcdef", 1);
        let wrong_region = svg.replacen("x=\"-0.2\"", "x=\"-0.1\"", 1);
        let wrong_std_deviation = svg.replacen("stdDeviation=\"8\"", "stdDeviation=\"4\"", 1);

        assert!(preflight_native_filter_receipt(&wrong_dx, &tree).is_none());
        assert!(preflight_native_filter_receipt(&wrong_color, &tree).is_none());
        assert!(preflight_native_filter_receipt(&wrong_region, &tree).is_none());
        assert!(preflight_native_filter_receipt(&wrong_std_deviation, &tree).is_none());
    }

    #[test]
    fn invalid_stddev_and_extra_primitive_fail() {
        let negative_stddev = exact_svg().replacen("stdDeviation=\"8\"", "stdDeviation=\"-1\"", 1);
        let tree = parse_tree(&negative_stddev);
        assert!(preflight_native_filter_receipt(&negative_stddev, &tree).is_none());

        let extra_primitive = exact_svg().replacen(
            "</filter>",
            r##"<feFlood flood-color="#000000"/></filter>"##,
            1,
        );
        let tree = parse_tree(&extra_primitive);
        assert!(preflight_native_filter_receipt(&extra_primitive, &tree).is_none());
    }

    fn srgb_stage(index: usize, input: &str, dx: i32, color: &str) -> String {
        format!(
            r#"<feGaussianBlur in="{input}" stdDeviation="0" result="merman-shadow-{index}-blur"/><feOffset in="merman-shadow-{index}-blur" dx="{dx}" dy="0" result="merman-shadow-{index}-offset"/><feFlood flood-color="{color}" result="merman-shadow-{index}-flood"/><feComposite in="merman-shadow-{index}-flood" in2="merman-shadow-{index}-offset" operator="in" result="merman-shadow-{index}-shadow"/><feMerge result="merman-shadow-{index}-result"><feMergeNode in="merman-shadow-{index}-shadow"/><feMergeNode in="{input}"/></feMerge>"#
        )
    }

    fn srgb_svg(composed: bool) -> String {
        let mut stages = srgb_stage(0, "SourceGraphic", 60, "#111827");
        if composed {
            stages.push_str(&srgb_stage(1, "merman-shadow-0-result", 20, "#334455"));
        }
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs><filter id="{FIRST_ID}" filterUnits="objectBoundingBox" x="-1" y="-1" width="4" height="4" color-interpolation-filters="sRGB">{stages}</filter></defs><g filter="url(#{FIRST_ID})"><rect x="10" y="10" width="50" height="30" fill="#fff"/></g></svg>"##
        )
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    #[test]
    fn srgb_expanded_template_preserves_dark_color_pixels() {
        let svg = srgb_svg(false);
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("expanded sRGB receipt");
        assert_eq!(receipt.drop_shadow_count(), 1);
        let mut pixmap = tiny_skia::Pixmap::new(200, 100).expect("pixmap");
        resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let color = pixmap.pixel(90, 20).expect("inside shadow").demultiply();
        assert_eq!(
            [color.red(), color.green(), color.blue(), color.alpha()],
            [17, 24, 39, 255]
        );
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    #[test]
    fn srgb_identity_offset_elision_preserves_pixels_and_receipts() {
        for offsets in [[(0, 0), (0, 0)], [(0, 0), (0, 6)], [(4, 0), (0, 0)]] {
            for second_input in ["SourceGraphic", "merman-shadow-0-result"] {
                let mut stages = String::new();
                for (index, (dx, dy)) in offsets.into_iter().enumerate() {
                    let input = if index == 0 {
                        "SourceGraphic"
                    } else {
                        second_input
                    };
                    stages.push_str(
                        &srgb_stage(index, input, dx, "rgba(17, 24, 39, 0.5)")
                            .replace("dy=\"0\"", &format!("dy=\"{dy}\""))
                            .replace("stdDeviation=\"0\"", "stdDeviation=\"2\""),
                    );
                }
                let full = format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><defs><filter id="{FIRST_ID}" filterUnits="objectBoundingBox" x="-1" y="-1" width="4" height="4" color-interpolation-filters="sRGB">{stages}</filter></defs><g filter="url(#{FIRST_ID})"><rect x="30" y="25" width="50" height="30" fill="#fff"/></g></svg>"##
                );
                let mut compact = full.clone();
                for (index, offset) in offsets.into_iter().enumerate() {
                    if offset == (0, 0) {
                        compact = compact.replace(
                            &format!(r#"<feOffset in="merman-shadow-{index}-blur" dx="0" dy="0" result="merman-shadow-{index}-offset"/>"#),
                            "",
                        ).replace(
                            &format!(r#"in2="merman-shadow-{index}-offset""#),
                            &format!(r#"in2="merman-shadow-{index}-blur""#),
                        );
                    }
                }
                assert_ne!(compact, full);
                let full_tree = parse_tree(&full);
                let compact_tree = parse_tree(&compact);
                let expected = preflight_native_filter_receipt(&full, &full_tree).unwrap();
                assert_eq!(
                    preflight_native_filter_receipt(&compact, &compact_tree),
                    Some(expected)
                );
                assert_eq!(
                    preflight_native_filter_receipt(&full, &compact_tree),
                    Some(expected)
                );
                assert_eq!(
                    preflight_native_filter_receipt(&compact, &full_tree),
                    Some(expected)
                );
                let mut full_pixels = tiny_skia::Pixmap::new(200, 100).unwrap();
                let mut compact_pixels = tiny_skia::Pixmap::new(200, 100).unwrap();
                resvg::render(
                    &full_tree,
                    tiny_skia::Transform::identity(),
                    &mut full_pixels.as_mut(),
                );
                resvg::render(
                    &compact_tree,
                    tiny_skia::Transform::identity(),
                    &mut compact_pixels.as_mut(),
                );
                assert_eq!(full_pixels.data(), compact_pixels.data());

                let omitted = offsets.iter().position(|offset| *offset == (0, 0)).unwrap();
                let wrong_mask = compact.replace(
                    &format!(r#"in2="merman-shadow-{omitted}-blur""#),
                    &format!(r#"in2="merman-shadow-{omitted}-offset""#),
                );
                assert!(
                    preflight_native_filter_receipt(&wrong_mask, &parse_tree(&wrong_mask))
                        .is_none()
                );
                let translated = full.replace(
                    &format!(r#"in="merman-shadow-{omitted}-blur" dx="0" dy="0""#),
                    &format!(r#"in="merman-shadow-{omitted}-blur" dx="1" dy="0""#),
                );
                assert_ne!(translated, full);
                assert!(preflight_native_filter_receipt(&translated, &compact_tree).is_none());
                assert!(
                    preflight_native_filter_receipt(&compact, &parse_tree(&translated)).is_none()
                );
            }
        }
    }

    #[test]
    fn srgb_direct_drop_shadow_is_not_certified() {
        let svg = exact_svg().replace(
            "color-interpolation-filters=\"linearRGB\"",
            "color-interpolation-filters=\"sRGB\"",
        );
        assert!(preflight_native_filter_receipt(&svg, &parse_tree(&svg)).is_none());
    }

    #[test]
    fn srgb_composed_template_and_mismatched_stage_wiring() {
        let svg = srgb_svg(true);
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("composed sRGB receipt");
        assert_eq!(receipt.drop_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 1);
        let single_svg = srgb_svg(false);
        let single_tree = parse_tree(&single_svg);
        let single_receipt = preflight_native_filter_receipt(&single_svg, &single_tree)
            .expect("removing a complete stage remains a valid single-stage application");
        assert_ne!(receipt, single_receipt);
        assert!(preflight_native_filter_receipt(&svg, &single_tree).is_none());
        assert!(preflight_native_filter_receipt(&single_svg, &tree).is_none());
        let wrong_order = svg.replacen(
            r#"<feMergeNode in="merman-shadow-0-shadow"/><feMergeNode in="SourceGraphic"/>"#,
            r#"<feMergeNode in="SourceGraphic"/><feMergeNode in="merman-shadow-0-shadow"/>"#,
            1,
        );
        let wrong_previous = svg.replace("in=\"merman-shadow-0-result\"", "in=\"missing-result\"");
        let missing_composite = svg.replacen(r#"<feComposite in="merman-shadow-0-flood" in2="merman-shadow-0-offset" operator="in" result="merman-shadow-0-shadow"/>"#, "", 1);
        let wrong_space = svg.replace(
            "color-interpolation-filters=\"sRGB\"",
            "color-interpolation-filters=\"linearRGB\"",
        );
        for mutation in [wrong_order, wrong_previous, missing_composite, wrong_space] {
            assert_ne!(mutation, svg);
            let mutated_tree = parse_tree(&mutation);
            assert!(preflight_native_filter_receipt(&mutation, &mutated_tree).is_none());
            assert!(preflight_native_filter_receipt(&svg, &mutated_tree).is_none());
        }
    }

    fn composed_svg() -> String {
        let second_stage = r##"<feDropShadow dx="0" dy="0" stdDeviation="8" flood-color="rgba(0, 242, 255, 0.3)"/>"##;
        exact_svg().replacen("</filter>", &format!("{second_stage}</filter>"), 1)
    }

    #[test]
    fn ordered_shadow_chain_passes_and_receipt_preserves_each_stage() {
        let svg = composed_svg();
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("composed receipt");
        assert_eq!(receipt.drop_shadow_count(), 3);
        assert_eq!(receipt.reference_count(), 2);

        let single_svg = exact_svg();
        let single_tree = parse_tree(&single_svg);
        let single = preflight_native_filter_receipt(&single_svg, &single_tree)
            .expect("deleting a legal stage remains observable");
        assert_ne!(receipt, single);
        assert!(preflight_native_filter_receipt(&single_svg, &tree).is_none());
        assert!(preflight_native_filter_receipt(&svg, &single_tree).is_none());
    }

    #[test]
    fn composed_stage_order_and_explicit_source_change_receipt() {
        let svg = composed_svg();
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("composed receipt");
        let first_stage = r##"<feDropShadow in="SourceGraphic" dx="4" dy="5" stdDeviation="0" flood-color="#112233"/>"##;
        let second_stage = r##"<feDropShadow dx="0" dy="0" stdDeviation="8" flood-color="rgba(0, 242, 255, 0.3)"/>"##;
        let reordered = svg.replace(
            &format!("{first_stage}{second_stage}"),
            &format!(
                "{}{}",
                second_stage.replacen("<feDropShadow", "<feDropShadow in=\"SourceGraphic\"", 1),
                first_stage.replace(" in=\"SourceGraphic\"", "")
            ),
        );
        assert_ne!(svg, reordered);
        let reordered_tree = parse_tree(&reordered);
        let reordered_receipt = preflight_native_filter_receipt(&reordered, &reordered_tree)
            .expect("reordered legal chain");
        assert_ne!(receipt, reordered_receipt);
        assert!(preflight_native_filter_receipt(&reordered, &tree).is_none());
        assert!(preflight_native_filter_receipt(&svg, &reordered_tree).is_none());

        let source = svg.replacen(
            "<feDropShadow dx=",
            "<feDropShadow in=\"SourceGraphic\" dx=",
            1,
        );
        let source_tree = parse_tree(&source);
        let source_receipt = preflight_native_filter_receipt(&source, &source_tree)
            .expect("explicit SourceGraphic on a later stage");
        assert_ne!(receipt, source_receipt);
        assert!(preflight_native_filter_receipt(&source, &tree).is_none());
        assert!(preflight_native_filter_receipt(&svg, &source_tree).is_none());
    }

    #[test]
    fn shadow_chain_is_not_limited_to_two_stages() {
        let stage = r##"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="#00f2ff"/>"##;
        let svg = exact_svg().replacen("</filter>", &format!("{}</filter>", stage.repeat(4)), 1);
        let receipt = preflight_native_filter_receipt(&svg, &parse_tree(&svg))
            .expect("five-stage application and independent single-stage application");
        assert_eq!(receipt.drop_shadow_count(), 6);
        assert_eq!(receipt.reference_count(), 2);
    }

    #[test]
    fn raw_shadow_chain_obeys_the_shared_primitive_bound() {
        let stage = r##"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="#00f2ff"/>"##;
        let svg = exact_svg().replacen(
            "</filter>",
            &format!("{}</filter>", stage.repeat(MAX_NATIVE_SHADOW_STAGES - 1)),
            1,
        );
        let applications = parse_raw_drop_shadows(&svg).expect("exact stage hard cap");
        assert!(
            applications
                .iter()
                .any(|application| application.stages().len() == MAX_NATIVE_SHADOW_STAGES)
        );
        let excessive = svg.replacen("</filter>", &format!("{stage}</filter>"), 1);
        assert!(parse_raw_drop_shadows(&excessive).is_none());
        let empty = exact_svg().replace(
            r##"<feDropShadow in="SourceGraphic" dx="4" dy="5" stdDeviation="0" flood-color="#112233"/>"##,
            "",
        );
        assert!(parse_raw_drop_shadows(&empty).is_none());
    }

    #[test]
    fn per_stage_color_space_override_cannot_hide_in_resolved_tree() {
        let svg = composed_svg();
        let overridden = svg.replacen(
            "<feDropShadow dx=",
            "<feDropShadow color-interpolation-filters=\"sRGB\" dx=",
            1,
        );
        let tree = parse_tree(&overridden);
        assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
        assert!(preflight_native_filter_receipt(&overridden, &tree).is_none());
    }

    #[test]
    fn shadow_chain_rejects_unknown_reference_fallback_and_non_shadow_stage() {
        for input in ["missing-result", "Previous", "SourceAlpha"] {
            let svg = composed_svg().replacen(
                "<feDropShadow dx=",
                &format!("<feDropShadow in=\"{input}\" dx="),
                1,
            );
            let tree = parse_tree(&svg);
            assert!(
                preflight_native_filter_receipt(&svg, &tree).is_none(),
                "{input}"
            );
        }
        let no_source = composed_svg().replacen(" in=\"SourceGraphic\"", "", 1);
        assert!(preflight_native_filter_receipt(&no_source, &parse_tree(&no_source)).is_none());
        let extra = composed_svg().replacen(
            "</filter>",
            "<feGaussianBlur stdDeviation=\"2\"/></filter>",
            1,
        );
        assert!(preflight_native_filter_receipt(&extra, &parse_tree(&extra)).is_none());
    }

    #[test]
    fn extra_resolved_filter_and_usvg_alias_fail() {
        let svg = exact_svg();
        let extra_filter_tree = parse_tree(&svg.replacen(
            &format!("filter=\"url(#{FIRST_ID})\""),
            &format!("filter=\"url(#{FIRST_ID}) url(#{SECOND_ID})\""),
            1,
        ));
        assert!(preflight_native_filter_receipt(&svg, &extra_filter_tree).is_none());

        let alias_tree = parse_tree(&svg.replace(
            &format!("filter=\"url(#{SECOND_ID})\""),
            &format!("filter=\"url(#{FIRST_ID})\""),
        ));
        assert!(preflight_native_filter_receipt(&svg, &alias_tree).is_none());
    }

    #[test]
    fn extra_unreferenced_typed_definition_and_resolved_units_mismatch_fail() {
        let svg = exact_svg();
        let extra_definition = svg.replacen(
            "</defs>",
            &format!(
                "{}</defs>",
                filter("diagram-state-unused-theme-effect-shadow", 2, "1", "#000")
            ),
            1,
        );
        let tree = parse_tree(&extra_definition);
        assert!(preflight_native_filter_receipt(&extra_definition, &tree).is_none());

        let wrong_units_tree = parse_tree(&svg.replacen(
            "filterUnits=\"objectBoundingBox\"",
            "filterUnits=\"userSpaceOnUse\"",
            1,
        ));
        assert!(preflight_native_filter_receipt(&svg, &wrong_units_tree).is_none());
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_localization_proof_requires_cap_budget_and_outer_group_equality() {
        let control = merman_model::OperationControl::new();
        for svg in [composed_svg(), with_untyped_siblings(&composed_svg())] {
            let tree = parse_tree(&svg);
            let receipt = preflight_native_filter_receipt(&svg, &tree).expect("exact receipt");
            let (exact_plan, _) = crate::plan_pdf_filter_images(
                &tree,
                1.0,
                1.0,
                crate::PdfFilterImageLimit::unbounded(),
                &control,
            )
            .expect("actual PDF group and pixel inventory");
            assert_eq!(receipt.reference_count(), 2);
            assert_eq!(
                exact_plan.filtered_groups,
                if svg.contains("default-shadow") { 4 } else { 2 }
            );
            assert!(
                crate::pdf_native_filter_fully_localized(
                    &tree,
                    1.0,
                    1.0,
                    exact_plan,
                    Some(receipt),
                    &control,
                )
                .expect("all typed and independent default groups are fully localized")
            );
            for changed_plan in [
                crate::PdfFilterImagePlan {
                    limited: true,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    filtered_groups: exact_plan.filtered_groups - 1,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    filtered_groups: exact_plan.filtered_groups + 1,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    requested_scale: 2.0,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    effective_scale: 0.5,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    requested_image_pixels: exact_plan.requested_image_pixels - 1,
                    ..exact_plan
                },
                crate::PdfFilterImagePlan {
                    effective_image_pixels: exact_plan.effective_image_pixels - 1,
                    ..exact_plan
                },
            ] {
                assert!(
                    !crate::pdf_native_filter_fully_localized(
                        &tree,
                        1.0,
                        1.0,
                        changed_plan,
                        Some(receipt),
                        &control,
                    )
                    .expect("a mismatched PDF inventory cannot certify localization"),
                    "{changed_plan:?}"
                );
            }
            assert!(
                !crate::pdf_native_filter_fully_localized(
                    &tree,
                    1.0,
                    100.0,
                    crate::PdfFilterImagePlan {
                        requested_scale: 100.0,
                        effective_scale: 100.0,
                        ..exact_plan
                    },
                    Some(receipt),
                    &control,
                )
                .expect("per-group side caps still prevent full localization")
            );
            assert!(
                !crate::pdf_native_filter_fully_localized(
                    &tree, 1.0, 1.0, exact_plan, None, &control,
                )
                .expect("a missing typed receipt cannot prove localization")
            );
            let (limited_plan, _) = crate::plan_pdf_filter_images(
                &tree,
                1.0,
                1.0,
                crate::PdfFilterImageLimit::new(Some(exact_plan.requested_image_pixels / 2)),
                &control,
            )
            .expect("a smaller aggregate budget can retain every group at lower sampling");
            assert!(limited_plan.limited);
            assert!(
                !crate::pdf_native_filter_fully_localized(
                    &tree,
                    1.0,
                    1.0,
                    limited_plan,
                    Some(receipt),
                    &control,
                )
                .expect("an actual budget reduction cannot prove full localization")
            );
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_localization_rejects_an_untyped_ancestor_and_capped_independent_sibling() {
        let control = OperationControl::new();
        let source = with_untyped_siblings(&composed_svg());
        let source_tree = parse_tree(&source);
        let (source_plan, _) = crate::plan_pdf_filter_images(
            &source_tree,
            1.0,
            1.0,
            crate::PdfFilterImageLimit::unbounded(),
            &control,
        )
        .expect("original sibling regions");
        let changed_region = source.replace(
            r#"<filter id="default-shadow">"#,
            r#"<filter id="default-shadow" filterUnits="userSpaceOnUse" x="0" y="0" width="500" height="10">"#,
        );
        let tree = parse_tree(&changed_region);
        let receipt = preflight_native_filter_receipt(&changed_region, &tree);
        assert_eq!(
            receipt,
            preflight_native_filter_receipt(&source, &source_tree),
            "changing an independent sibling region preserves typed receipt identity"
        );
        assert!(
            !crate::pdf_native_filter_fully_localized(
                &tree,
                1.0,
                1.0,
                source_plan,
                receipt,
                &control,
            )
            .expect("a frozen plan must account for the new sibling region's actual pixels")
        );
        let ancestor = source
            .replacen(
                &format!(r#"<g filter="url(#{FIRST_ID})">"#),
                &format!(r#"<g filter="url(#default-shadow)"><g filter="url(#{FIRST_ID})">"#),
                1,
            )
            .replacen("</g>", "</g></g>", 1);
        let tree = parse_tree(&ancestor);
        let receipt = preflight_native_filter_receipt(&ancestor, &tree);
        assert!(receipt.is_none());
        let (plan, _) = crate::plan_pdf_filter_images(
            &tree,
            1.0,
            1.0,
            crate::PdfFilterImageLimit::unbounded(),
            &control,
        )
        .expect("an ancestor can still have a finite PDF image plan");
        assert!(
            !crate::pdf_native_filter_fully_localized(&tree, 1.0, 1.0, plan, receipt, &control)
                .expect("a finite plan cannot bypass typed ancestor rejection")
        );

        let capped_sibling = source.replace(
            r#"<filter id="default-shadow">"#,
            r#"<filter id="default-shadow" filterUnits="userSpaceOnUse" x="0" y="0" width="6000" height="10">"#,
        );
        let tree = parse_tree(&capped_sibling);
        let receipt = preflight_native_filter_receipt(&capped_sibling, &tree);
        assert!(receipt.is_some(), "the typed paint remains independent");
        let (plan, _) = crate::plan_pdf_filter_images(
            &tree,
            1.0,
            1.0,
            crate::PdfFilterImageLimit::unbounded(),
            &control,
        )
        .expect("the backend side cap produces a finite pixel plan");
        assert!(!plan.limited);
        assert!(
            !crate::pdf_native_filter_fully_localized(&tree, 1.0, 1.0, plan, receipt, &control,)
                .expect("an independent non-typed region must also retain its requested sampling")
        );
    }
}
