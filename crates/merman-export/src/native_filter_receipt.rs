use std::collections::{BTreeMap, HashSet};

use merman_render::__private::{NativeSvgFilterReceipt, NativeSvgHardShadow};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

const SVG_NAMESPACE: &[u8] = b"http://www.w3.org/2000/svg";
const TYPED_FILTER_ID_MARKER: &str = "-theme-effect-";

#[derive(Debug)]
struct FilterDefinition {
    region: [f32; 4],
    primitive: DropShadowDefinition,
}

#[derive(Debug)]
struct DropShadowDefinition {
    offset: [f32; 2],
    std_deviation: [f32; 2],
    color_css: String,
}

#[derive(Debug)]
struct OpenFilter {
    depth: usize,
    id: Option<String>,
    region: Option<[f32; 4]>,
    primitive: Option<DropShadowDefinition>,
    primitive_seen: bool,
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
) -> Option<NativeSvgFilterReceipt> {
    let shadows = parse_raw_drop_shadows(svg)?;
    if !resolved_tree_matches(tree, &shadows) {
        return None;
    }
    NativeSvgFilterReceipt::from_drop_shadows(shadows)
}

fn parse_raw_drop_shadows(svg: &str) -> Option<Vec<NativeSvgHardShadow>> {
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
                if is_svg_element(&reader, &element, b"filter") {
                    open_filters.push(parse_filter(&reader, &element, depth).ok()?);
                }
                depth = depth.checked_add(1)?;
            }
            Event::Empty(element) => {
                record_typed_reference(&reader, &element, &mut references).ok()?;
                observe_filter_child(&reader, &element, depth, &mut open_filters).ok()?;
                if is_svg_element(&reader, &element, b"filter") {
                    let filter = parse_filter(&reader, &element, depth).ok()?;
                    record_definition(filter, &mut definitions)?;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1)?;
                if open_filters
                    .last()
                    .is_some_and(|filter| filter.depth == depth)
                {
                    let filter = open_filters.pop()?;
                    record_definition(filter, &mut definitions)?;
                }
            }
            Event::Text(text) => {
                if !text.as_ref().iter().all(u8::is_ascii_whitespace) {
                    invalidate_containing_filters(depth, &mut open_filters);
                }
            }
            Event::CData(text) => {
                if !text.as_ref().iter().all(u8::is_ascii_whitespace) {
                    invalidate_containing_filters(depth, &mut open_filters);
                }
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
        shadows.push(NativeSvgHardShadow::new(
            filter_id,
            definition.region,
            definition.primitive.offset,
            definition.primitive.std_deviation,
            &definition.primitive.color_css,
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
        if !matches!(namespace, ResolveResult::Unbound) || local_name.as_ref() != b"filter" {
            continue;
        }
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
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
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|_| ())?
            .into_owned();
        let accepted = match local_name.as_ref() {
            b"id" => set_once(&mut id, value),
            b"filterUnits" => set_once(&mut filter_units, value),
            b"x" => set_once(&mut x, value),
            b"y" => set_once(&mut y, value),
            b"width" => set_once(&mut width, value),
            b"height" => set_once(&mut height, value),
            b"color-interpolation-filters" => set_once(&mut color_interpolation, value),
            _ => false,
        };
        valid &= accepted;
    }

    valid &= filter_units.as_deref() == Some("objectBoundingBox");
    valid &= color_interpolation.as_deref() == Some("linearRGB");
    let region = parse_filter_region(x, y, width, height);
    valid &= region.is_some();

    Ok(OpenFilter {
        depth,
        id,
        region,
        primitive: None,
        primitive_seen: false,
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
    let is_drop_shadow = is_svg_element(reader, element, b"feDropShadow");
    for filter in open_filters {
        if depth <= filter.depth {
            continue;
        }
        if depth == filter.depth + 1 && is_drop_shadow {
            if filter.primitive_seen {
                filter.valid = false;
                continue;
            }
            filter.primitive_seen = true;
            filter.primitive = parse_drop_shadow(reader, element)?;
            filter.valid &= filter.primitive.is_some();
        } else {
            filter.valid = false;
        }
    }
    Ok(())
}

fn parse_drop_shadow(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
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
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|_| ())?
            .into_owned();
        let accepted = match local_name.as_ref() {
            b"in" => set_once(&mut input, value),
            b"dx" => set_once(&mut dx, value),
            b"dy" => set_once(&mut dy, value),
            b"stdDeviation" => set_once(&mut std_deviation, value),
            b"flood-color" => set_once(&mut flood_color, value),
            _ => false,
        };
        valid &= accepted;
    }

    valid &= input.as_deref() == Some("SourceGraphic");
    let offset = dx
        .as_deref()
        .and_then(parse_finite_number)
        .zip(dy.as_deref().and_then(parse_finite_number))
        .map(|(dx, dy)| [dx, dy]);
    let std_deviation = std_deviation.as_deref().and_then(parse_std_deviation);
    valid &= offset.is_some() && std_deviation.is_some() && flood_color.is_some();

    Ok(valid.then(|| DropShadowDefinition {
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
        region,
        primitive,
        valid,
        ..
    } = filter;
    let Some(id) = id else {
        return Some(());
    };
    if !id.contains(TYPED_FILTER_ID_MARKER) {
        return Some(());
    }
    let definition = if valid {
        region
            .zip(primitive)
            .map(|(region, primitive)| FilterDefinition { region, primitive })
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
    expected_local_name: &[u8],
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

fn resolved_tree_matches(tree: &usvg::Tree, shadows: &[NativeSvgHardShadow]) -> bool {
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
    if !visit_group(tree.root(), &expected, &mut group_references, &mut visited) {
        return false;
    }
    group_references.values().all(|count| *count == 1)
}

fn resolved_filter_matches(filter: &usvg::filter::Filter, shadow: &NativeSvgHardShadow) -> bool {
    let [primitive] = filter.primitives() else {
        return false;
    };
    if primitive.color_interpolation() != usvg::filter::ColorInterpolation::LinearRGB {
        return false;
    }
    let usvg::filter::Kind::DropShadow(drop_shadow) = primitive.kind() else {
        return false;
    };
    let [expected_dx, expected_dy] = shadow.offset();
    let [expected_std_dev_x, expected_std_dev_y] = shadow.std_deviation();
    let [red, green, blue, alpha] = shadow.color_rgba();
    matches!(drop_shadow.input(), usvg::filter::Input::SourceGraphic)
        && same_f32(drop_shadow.dx(), expected_dx)
        && same_f32(drop_shadow.dy(), expected_dy)
        && same_f32(drop_shadow.std_dev_x().get(), expected_std_dev_x)
        && same_f32(drop_shadow.std_dev_y().get(), expected_std_dev_y)
        && drop_shadow.color() == usvg::Color::new_rgb(red, green, blue)
        && drop_shadow.opacity() == usvg::Opacity::new_u8(alpha)
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

fn visit_group(
    group: &usvg::Group,
    expected: &BTreeMap<&str, &NativeSvgHardShadow>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashSet<*const usvg::Group>,
) -> bool {
    if !visited.insert(std::ptr::from_ref(group)) {
        return true;
    }
    if !group.filters().is_empty() {
        let [filter] = group.filters() else {
            return false;
        };
        let Some(shadow) = expected.get(filter.id()) else {
            return false;
        };
        if !resolved_filter_region_matches(group, filter, shadow) {
            return false;
        }
        let Some(count) = references.get_mut(filter.id()) else {
            return false;
        };
        let Some(next_count) = count.checked_add(1) else {
            return false;
        };
        *count = next_count;
        if *count != 1 {
            return false;
        }
    }

    if let Some(clip_path) = group.clip_path() {
        if !visit_clip_path(clip_path, expected, references, visited) {
            return false;
        }
    }
    if let Some(mask) = group.mask()
        && !visit_mask(mask, expected, references, visited)
    {
        return false;
    }

    for node in group.children() {
        if let usvg::Node::Group(child) = node {
            if !visit_group(child, expected, references, visited) {
                return false;
            }
        } else {
            let mut subroots_match = true;
            node.subroots(|subroot| {
                if subroots_match {
                    subroots_match = visit_group(subroot, expected, references, visited);
                }
            });
            if !subroots_match {
                return false;
            }
        }
    }
    true
}

fn resolved_filter_region_matches(
    group: &usvg::Group,
    filter: &usvg::filter::Filter,
    shadow: &NativeSvgHardShadow,
) -> bool {
    let Some(object_bbox) = group.bounding_box().to_non_zero_rect() else {
        return false;
    };
    let [x, y, width, height] = shadow.region();
    let expected = [
        x * object_bbox.width() + object_bbox.x(),
        y * object_bbox.height() + object_bbox.y(),
        width * object_bbox.width(),
        height * object_bbox.height(),
    ];
    let actual = filter.rect();
    [actual.x(), actual.y(), actual.width(), actual.height()]
        .into_iter()
        .zip(expected)
        .all(|(actual, expected)| same_f32(actual, expected))
}

fn visit_clip_path(
    clip_path: &usvg::ClipPath,
    expected: &BTreeMap<&str, &NativeSvgHardShadow>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashSet<*const usvg::Group>,
) -> bool {
    visit_group(clip_path.root(), expected, references, visited)
        && clip_path
            .clip_path()
            .is_none_or(|nested| visit_clip_path(nested, expected, references, visited))
}

fn visit_mask(
    mask: &usvg::Mask,
    expected: &BTreeMap<&str, &NativeSvgHardShadow>,
    references: &mut BTreeMap<String, usize>,
    visited: &mut HashSet<*const usvg::Group>,
) -> bool {
    visit_group(mask.root(), expected, references, visited)
        && mask
            .mask()
            .is_none_or(|nested| visit_mask(nested, expected, references, visited))
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
    fn exact_hard_and_soft_shadow_applications_pass() {
        let svg = exact_svg();
        let tree = parse_tree(&svg);
        let receipt = preflight_native_filter_receipt(&svg, &tree).expect("exact receipt");

        assert_eq!(receipt.drop_shadow_count(), 2);
        assert_eq!(receipt.reference_count(), 2);
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

    #[test]
    fn srgb_filter_interpolation_fails_native_color_preservation_proof() {
        let svg = exact_svg().replace(
            "color-interpolation-filters=\"linearRGB\"",
            "color-interpolation-filters=\"sRGB\"",
        );
        let tree = parse_tree(&svg);

        assert!(preflight_native_filter_receipt(&svg, &tree).is_none());
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
        let svg = exact_svg();
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
