use std::collections::{BTreeMap, HashSet};

use merman_render::__private::{
    EffectColorSpace, EffectInput, MAX_NATIVE_SHADOW_STAGES, NativeSvgFilterApplication,
    NativeSvgFilterReceipt, NativeSvgFilterUnits, NativeSvgShadowStage,
};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const TYPED_FILTER_ID_MARKER: &str = "-theme-effect-";

#[derive(Debug)]
struct FilterDefinition {
    units: NativeSvgFilterUnits,
    region: [f32; 4],
    color_space: EffectColorSpace,
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

pub(super) fn preflight_native_filter_receipt(
    svg: &str,
    tree: &usvg::Tree,
    viewport: usvg::Rect,
) -> Option<NativeSvgFilterReceipt> {
    let shadows = parse_raw_drop_shadows(svg)?;
    if !resolved_tree_matches(tree, &shadows, viewport) {
        return None;
    }
    NativeSvgFilterReceipt::from_applications(shadows)
}

fn parse_raw_drop_shadows(svg: &str) -> Option<Vec<NativeSvgFilterApplication>> {
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut open_filters = Vec::<OpenFilter>::new();
    let mut definitions = BTreeMap::<String, DefinitionSlot>::new();
    let mut references = BTreeMap::<String, usize>::new();

    loop {
        match reader.read_event().ok()? {
            Event::Start(element) => {
                record_typed_reference(&reader, &element, &mut references).ok()?;
                observe_filter_child(&reader, &element, depth, &mut open_filters).ok()?;
                if is_svg_element(&reader, &element, "filter") {
                    open_filters.push(parse_filter(&reader, &element, depth).ok()?);
                }
                depth = depth.checked_add(1)?;
            }
            Event::Empty(element) => {
                record_typed_reference(&reader, &element, &mut references).ok()?;
                observe_filter_child(&reader, &element, depth, &mut open_filters).ok()?;
                if is_svg_element(&reader, &element, "filter") {
                    let filter = parse_filter(&reader, &element, depth).ok()?;
                    record_definition(filter, &mut definitions)?;
                }
            }
            Event::End(element) => {
                depth = depth.checked_sub(1)?;
                for filter in &mut open_filters {
                    if filter.color_space == Some(EffectColorSpace::Srgb)
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
            Event::Text(text) if !text.as_ref().as_bytes().iter().all(u8::is_ascii_whitespace) => {
                invalidate_containing_filters(depth, &mut open_filters);
            }
            Event::CData(text) if !text.as_ref().as_bytes().iter().all(u8::is_ascii_whitespace) => {
                invalidate_containing_filters(depth, &mut open_filters);
            }
            Event::GeneralRef(_) => invalidate_containing_filters(depth, &mut open_filters),
            Event::Eof => break,
            _ => {}
        }
    }

    if depth != 0
        || !open_filters.is_empty()
        || references.is_empty()
        || definitions.len() != references.len()
    {
        return None;
    }

    let mut shadows = Vec::with_capacity(references.len());
    for (filter_id, reference_count) in references {
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
        shadows.push(NativeSvgFilterApplication::new_with_units(
            filter_id,
            definition.units,
            definition.region,
            definition.color_space,
            stages,
            1,
        )?);
    }
    Some(shadows)
}

fn record_typed_reference(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    references: &mut BTreeMap<String, usize>,
) -> Result<(), ()> {
    if !is_svg_namespace(reader, element) {
        return Ok(());
    }

    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        if !matches!(namespace, ResolveResult::Unbound) || local_name.as_ref() != "filter" {
            continue;
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| ())?;
        let Some(filter_id) = exact_filter_reference(value.as_ref()) else {
            continue;
        };
        if !filter_id.contains(TYPED_FILTER_ID_MARKER) {
            continue;
        }
        let count = references.entry(filter_id.to_string()).or_default();
        *count = count.checked_add(1).ok_or(())?;
    }
    Ok(())
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
        if filter.color_space == Some(EffectColorSpace::Srgb) {
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
    let mut values = vec![None; names.len()];
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| ())?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
        let Some(index) = names.iter().position(|name| *name == local_name.as_ref()) else {
            filter.valid = false;
            return Ok(());
        };
        if !matches!(namespace, ResolveResult::Unbound) || values[index].is_some() {
            filter.valid = false;
            return Ok(());
        }
        values[index] = Some(
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ())?
                .into_owned(),
        );
    }
    if let Some(values) = values.into_iter().collect::<Option<Vec<_>>>() {
        filter.expanded_elements.push(values);
    } else {
        filter.valid = false;
    }
    Ok(())
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
        stages,
        expanded_elements,
        valid,
        ..
    } = filter;
    let Some(id) = id else {
        return Some(());
    };
    if !id.contains(TYPED_FILTER_ID_MARKER) {
        return Some(());
    }
    let definition = if valid && !stages.is_empty() && expanded_elements.is_empty() {
        region
            .zip(color_space)
            .zip(units)
            .map(|((region, color_space), units)| FilterDefinition {
                units,
                region,
                color_space,
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

fn resolved_tree_matches(
    tree: &usvg::Tree,
    shadows: &[NativeSvgFilterApplication],
    viewport: usvg::Rect,
) -> bool {
    if tree.filters().len() != shadows.len() {
        return false;
    }
    let expected = shadows
        .iter()
        .map(|shadow| (shadow.filter_id(), shadow))
        .collect::<BTreeMap<_, _>>();
    let mut resolved_ids = HashSet::with_capacity(tree.filters().len());
    for filter in tree.filters() {
        let Some(shadow) = expected.get(filter.id()) else {
            return false;
        };
        if !resolved_ids.insert(filter.id()) || !resolved_filter_matches(filter, shadow) {
            return false;
        }
    }

    let mut group_references = shadows
        .iter()
        .map(|shadow| (shadow.filter_id().to_string(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut visited = HashSet::<*const usvg::Group>::new();
    if visit_group(
        tree.root(),
        &expected,
        &mut group_references,
        &mut visited,
        viewport,
        false,
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

/// Text-only paint summary in the current group's local coordinates. Each subtree is visited
/// once; ancestor effects cannot accidentally certify a previously filtered or clipped label.
#[derive(Default)]
struct TextPaint {
    bounds: Option<usvg::Rect>,
    restricted: bool,
}

impl TextPaint {
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
    visited: &mut HashSet<*const usvg::Group>,
    viewport: usvg::Rect,
    ancestor_clipped: bool,
) -> Option<TextPaint> {
    if !visited.insert(std::ptr::from_ref(group)) {
        return Some(TextPaint::default());
    }
    let clipped = ancestor_clipped || group.clip_path().is_some() || group.mask().is_some();
    let mut text_paint = TextPaint {
        restricted: clipped,
        ..TextPaint::default()
    };
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => {
                let child_paint =
                    visit_group(child, expected, references, visited, viewport, clipped)?;
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
                    subroots_match =
                        visit_group(subroot, expected, references, visited, viewport, true)
                            .is_some();
                }
            });
            if !subroots_match {
                return None;
            }
        }
    }

    if !group.filters().is_empty() {
        // usvg can retain a filter group after discarding empty text or a missing font.
        // Such a definition is not an observed paint terminal.
        if group.children().is_empty() {
            return None;
        }
        let [filter] = group.filters() else {
            return None;
        };
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
    }

    if let Some(clip_path) = group.clip_path()
        && !visit_clip_path(clip_path, expected, references, visited, viewport)
    {
        return None;
    }
    if let Some(mask) = group.mask()
        && !visit_mask(mask, expected, references, visited, viewport)
    {
        return None;
    }
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
    visited: &mut HashSet<*const usvg::Group>,
    viewport: usvg::Rect,
) -> bool {
    visit_group(
        clip_path.root(),
        expected,
        references,
        visited,
        viewport,
        true,
    )
    .is_some()
        && clip_path
            .clip_path()
            .is_none_or(|nested| visit_clip_path(nested, expected, references, visited, viewport))
}

fn visit_mask(
    mask: &usvg::Mask,
    expected: &BTreeMap<&str, &NativeSvgFilterApplication>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashSet<*const usvg::Group>,
    viewport: usvg::Rect,
) -> bool {
    visit_group(mask.root(), expected, references, visited, viewport, true).is_some()
        && mask
            .mask()
            .is_none_or(|nested| visit_mask(nested, expected, references, visited, viewport))
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

    fn preflight_native_filter_receipt(
        svg: &str,
        tree: &usvg::Tree,
    ) -> Option<NativeSvgFilterReceipt> {
        super::preflight_native_filter_receipt(svg, tree, tree.size().to_rect(0.0, 0.0)?)
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
        .replace(r#" viewBox="0 0 512 512""#, "");
        let tree = parse_text_tree(&source);
        assert!(
            preflight_native_filter_receipt(&source, &tree).is_some(),
            "the declared viewport contains the effect"
        );
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
        assert!(
            super::preflight_native_filter_receipt(&source, &tree, actual_viewport).is_none(),
            "the PNG content crop excludes glow, even though the declared SVG viewport contains it"
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
                    Some(expected.clone())
                );
                assert_eq!(
                    preflight_native_filter_receipt(&full, &compact_tree),
                    Some(expected.clone())
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
        let svg = composed_svg();
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("exact receipt");
        let exact_plan = crate::PdfFilterImagePlan {
            filtered_groups: 2,
            requested_scale: 1.0,
            effective_scale: 1.0,
            requested_image_pixels: 1,
            effective_image_pixels: 1,
            limited: false,
        };
        let control = merman_core::OperationControl::new();

        assert!(
            crate::pdf_native_filter_fully_localized(
                &tree,
                1.0,
                1.0,
                exact_plan,
                Some(receipt),
                &control,
            )
            .expect("exact localization proof")
        );
        assert!(
            !crate::pdf_native_filter_fully_localized(
                &tree,
                1.0,
                1.0,
                crate::PdfFilterImagePlan {
                    limited: true,
                    ..exact_plan
                },
                Some(receipt),
                &control,
            )
            .expect("limited plan should be evaluated")
        );
        assert!(
            !crate::pdf_native_filter_fully_localized(
                &tree,
                1.0,
                1.0,
                crate::PdfFilterImagePlan {
                    filtered_groups: 1,
                    ..exact_plan
                },
                Some(receipt),
                &control,
            )
            .expect("group mismatch should be evaluated")
        );
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
            .expect("scale mismatch should be evaluated")
        );
    }
}
