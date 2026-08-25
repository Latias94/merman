//! Blocking root viewport contracts that do not depend on exact browser bbox numerics.

use crate::util::{is_canonical_sha256, sha256_hex};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const SCHEMA_VERSION: u32 = 1;
const COMPARISON_REVISION: &str = "root-viewport-contract-v1";
const CATALOG_RELATIVE_PATH: &str = "_verification/deterministic-root-contracts.json";
const ROOT_RELATION_EPSILON_PX: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeterministicRootCatalog {
    schema_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
    comparison_revision: String,
    entries: Vec<DeterministicRootEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeterministicRootEntry {
    diagram: String,
    fixture: String,
    input_sha256: String,
    upstream_svg_sha256: String,
    root: RootViewportSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootViewportSignature {
    style: Option<String>,
    view_box: Option<String>,
    width: Option<String>,
    height: Option<String>,
}

impl RootViewportSignature {
    fn from_root(root: roxmltree::Node<'_, '_>) -> Self {
        Self {
            style: root.attribute("style").map(str::to_string),
            view_box: root.attribute("viewBox").map(str::to_string),
            width: root.attribute("width").map(str::to_string),
            height: root.attribute("height").map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DimensionPolicy {
    Missing,
    Responsive,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ViewBoxContract {
    width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaxWidthRelation {
    Missing,
    Independent,
    MatchesViewBox,
}

#[derive(Debug, Clone, PartialEq)]
struct ParsedRootContract {
    signature: RootViewportSignature,
    view_box: Option<ViewBoxContract>,
    width_policy: DimensionPolicy,
    height_policy: DimensionPolicy,
    max_width_relation: MaxWidthRelation,
    style_without_max_width: BTreeMap<String, String>,
}

static DETERMINISTIC_ROOT_CATALOG: OnceLock<Result<DeterministicRootCatalog, String>> =
    OnceLock::new();

pub(crate) fn validate_root_viewport_contract(
    diagram: &str,
    fixture: &str,
    input: &str,
    upstream_svg: &str,
    upstream_dom: &crate::svgdom::ParsedSvgDom<'_>,
    local_dom: &crate::svgdom::ParsedSvgDom<'_>,
) -> Result<(), String> {
    let upstream = parse_root_contract_from_dom(upstream_dom)
        .map_err(|error| format!("upstream {diagram}/{fixture}: {error}"))?;
    let local = parse_root_contract_from_dom(local_dom)
        .map_err(|error| format!("local {diagram}/{fixture}: {error}"))?;

    if upstream.view_box.is_some() != local.view_box.is_some() {
        return Err(format!(
            "root contract failed for {diagram}/{fixture}: viewBox presence changed"
        ));
    }
    if upstream.width_policy != local.width_policy {
        return Err(format!(
            "root contract failed for {diagram}/{fixture}: width policy changed: upstream={:?} local={:?}",
            upstream.width_policy, local.width_policy
        ));
    }
    if upstream.height_policy != local.height_policy {
        return Err(format!(
            "root contract failed for {diagram}/{fixture}: height policy changed: upstream={:?} local={:?}",
            upstream.height_policy, local.height_policy
        ));
    }
    if upstream.max_width_relation != local.max_width_relation {
        return Err(format!(
            "root contract failed for {diagram}/{fixture}: max-width/viewBox policy changed: upstream={:?} local={:?}",
            upstream.max_width_relation, local.max_width_relation
        ));
    }
    if upstream.style_without_max_width != local.style_without_max_width {
        return Err(format!(
            "root contract failed for {diagram}/{fixture}: non-numeric root style changed: upstream={:?} local={:?}",
            upstream.style_without_max_width, local.style_without_max_width
        ));
    }

    let catalog = deterministic_root_catalog()?;
    if let Some(entry) = catalog
        .entries
        .iter()
        .find(|entry| entry.diagram == diagram && entry.fixture == fixture)
    {
        if sha256_hex(input.as_bytes()) != entry.input_sha256 {
            return Err(format!(
                "deterministic root contract input drifted for {diagram}/{fixture}"
            ));
        }
        if sha256_hex(upstream_svg.as_bytes()) != entry.upstream_svg_sha256 {
            return Err(format!(
                "deterministic root contract upstream SVG drifted for {diagram}/{fixture}"
            ));
        }
        if upstream.signature != entry.root {
            return Err(format!(
                "deterministic root contract upstream signature drifted for {diagram}/{fixture}: expected {:?}, found {:?}",
                entry.root, upstream.signature
            ));
        }
        if local.signature != entry.root {
            return Err(format!(
                "deterministic root contract changed for {diagram}/{fixture}: expected {:?}, found {:?}",
                entry.root, local.signature
            ));
        }
    }
    Ok(())
}

fn deterministic_root_catalog() -> Result<&'static DeterministicRootCatalog, String> {
    match DETERMINISTIC_ROOT_CATALOG.get_or_init(load_deterministic_root_catalog) {
        Ok(catalog) => Ok(catalog),
        Err(error) => Err(error.clone()),
    }
}

fn catalog_path() -> PathBuf {
    crate::cmd::fixtures_root().join(CATALOG_RELATIVE_PATH)
}

fn load_deterministic_root_catalog() -> Result<DeterministicRootCatalog, String> {
    let path = catalog_path();
    let json = fs::read_to_string(&path).map_err(|error| {
        format!(
            "read deterministic root catalog {}: {error}",
            path.display()
        )
    })?;
    let catalog: DeterministicRootCatalog = serde_json::from_str(&json).map_err(|error| {
        format!(
            "parse deterministic root catalog {}: {error}",
            path.display()
        )
    })?;
    validate_deterministic_root_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_deterministic_root_catalog(catalog: &DeterministicRootCatalog) -> Result<(), String> {
    if catalog.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "deterministic root catalog schema {} is unsupported; expected {SCHEMA_VERSION}",
            catalog.schema_version
        ));
    }
    if catalog.mermaid_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        || catalog.mermaid_source_commit != crate::cmd::MERMAID_SOURCE_COMMIT
        || catalog.comparison_revision != COMPARISON_REVISION
    {
        return Err("deterministic root catalog reference contract drifted".to_string());
    }
    if catalog.entries.is_empty() {
        return Err("deterministic root catalog must not be empty".to_string());
    }

    let primary_diagrams = crate::cmd::primary_svg_matrix_diagrams().collect::<BTreeSet<_>>();
    let mut previous: Option<(&str, &str)> = None;
    for entry in &catalog.entries {
        let key = (entry.diagram.as_str(), entry.fixture.as_str());
        if previous.is_some_and(|previous| previous >= key) {
            return Err(format!(
                "deterministic root entries must be unique and sorted, found {}/{} after {:?}",
                entry.diagram, entry.fixture, previous
            ));
        }
        previous = Some(key);
        if !primary_diagrams.contains(entry.diagram.as_str()) {
            return Err(format!(
                "deterministic root {}/{} names a non-primary diagram",
                entry.diagram, entry.fixture
            ));
        }
        for (role, digest) in [
            ("input", entry.input_sha256.as_str()),
            ("upstream SVG", entry.upstream_svg_sha256.as_str()),
        ] {
            if !is_canonical_sha256(digest) {
                return Err(format!(
                    "deterministic root {}/{} {role} SHA-256 is invalid",
                    entry.diagram, entry.fixture
                ));
            }
        }

        let fixture_path = crate::cmd::fixtures_root()
            .join(&entry.diagram)
            .join(format!("{}.mmd", entry.fixture));
        let input = fs::read(&fixture_path)
            .map_err(|error| format!("read {}: {error}", fixture_path.display()))?;
        if sha256_hex(&input) != entry.input_sha256 {
            return Err(format!(
                "deterministic root {}/{} input SHA-256 drifted",
                entry.diagram, entry.fixture
            ));
        }

        let upstream_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(&entry.diagram)
            .join(format!("{}.svg", entry.fixture));
        let upstream_svg = fs::read_to_string(&upstream_path)
            .map_err(|error| format!("read {}: {error}", upstream_path.display()))?;
        if sha256_hex(upstream_svg.as_bytes()) != entry.upstream_svg_sha256 {
            return Err(format!(
                "deterministic root {}/{} upstream SVG SHA-256 drifted",
                entry.diagram, entry.fixture
            ));
        }
        let parsed = parse_root_contract(&upstream_svg).map_err(|error| {
            format!(
                "deterministic root {}/{} upstream contract is invalid: {error}",
                entry.diagram, entry.fixture
            )
        })?;
        if parsed.signature != entry.root {
            return Err(format!(
                "deterministic root {}/{} signature drifted",
                entry.diagram, entry.fixture
            ));
        }
    }
    Ok(())
}

fn parse_root_contract(svg: &str) -> Result<ParsedRootContract, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(svg.as_ref()).map_err(|error| error.to_string())?;
    parse_root_contract_from_root(document.root_element())
}

fn parse_root_contract_from_dom(
    document: &crate::svgdom::ParsedSvgDom<'_>,
) -> Result<ParsedRootContract, String> {
    parse_root_contract_from_root(document.root_element())
}

fn parse_root_contract_from_root(
    root: roxmltree::Node<'_, '_>,
) -> Result<ParsedRootContract, String> {
    if !root.has_tag_name(("http://www.w3.org/2000/svg", "svg")) && !root.has_tag_name("svg") {
        return Err("document root must be <svg>".to_string());
    }

    let view_box = root.attribute("viewBox").map(parse_view_box).transpose()?;
    let width_policy = parse_dimension_policy(root.attribute("width"), "width")?;
    let height_policy = parse_dimension_policy(root.attribute("height"), "height")?;
    let (max_width, style_without_max_width) = parse_root_style(root.attribute("style"))?;
    let max_width_relation = match (max_width, view_box) {
        (None, _) => MaxWidthRelation::Missing,
        (Some(_), None) => MaxWidthRelation::Independent,
        (Some(max_width), Some(view_box))
            if (max_width - view_box.width).abs() <= ROOT_RELATION_EPSILON_PX =>
        {
            MaxWidthRelation::MatchesViewBox
        }
        (Some(_), Some(_)) => MaxWidthRelation::Independent,
    };

    Ok(ParsedRootContract {
        signature: RootViewportSignature::from_root(root),
        view_box,
        width_policy,
        height_policy,
        max_width_relation,
        style_without_max_width,
    })
}

fn parse_view_box(raw: &str) -> Result<ViewBoxContract, String> {
    let values = raw
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|_| format!("invalid viewBox component {part:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let [x, y, width, height] = values.as_slice() else {
        return Err(format!(
            "viewBox must contain exactly four finite numbers: {raw:?}"
        ));
    };
    if ![x, y, width, height].iter().all(|value| value.is_finite()) {
        return Err(format!("viewBox must be finite: {raw:?}"));
    }
    if *width <= 0.0 || *height <= 0.0 {
        return Err(format!(
            "viewBox dimensions must be positive and finite: {raw:?}"
        ));
    }
    Ok(ViewBoxContract { width: *width })
}

fn parse_dimension_policy(raw: Option<&str>, name: &str) -> Result<DimensionPolicy, String> {
    let Some(raw) = raw.map(str::trim) else {
        return Ok(DimensionPolicy::Missing);
    };
    if raw == "100%" {
        return Ok(DimensionPolicy::Responsive);
    }
    let numeric = raw
        .strip_suffix("px")
        .unwrap_or(raw)
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("root {name} must be 100% or a finite positive number: {raw:?}"))?;
    if !numeric.is_finite() || numeric <= 0.0 {
        return Err(format!(
            "root {name} must be 100% or a finite positive number: {raw:?}"
        ));
    }
    Ok(DimensionPolicy::Fixed)
}

fn parse_root_style(raw: Option<&str>) -> Result<(Option<f64>, BTreeMap<String, String>), String> {
    let mut max_width = None;
    let mut remaining = BTreeMap::new();
    for declaration in raw.unwrap_or_default().split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("invalid root style declaration: {declaration:?}"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.is_empty() || value.is_empty() {
            return Err(format!("invalid root style declaration: {declaration:?}"));
        }
        if name == "max-width" {
            if max_width.is_some() {
                return Err("duplicate root max-width declaration".to_string());
            }
            let numeric = value
                .to_ascii_lowercase()
                .strip_suffix("px")
                .ok_or_else(|| format!("root max-width must use px: {value:?}"))?
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("invalid root max-width: {value:?}"))?;
            if !numeric.is_finite() || numeric <= 0.0 {
                return Err(format!(
                    "root max-width must be positive and finite: {value:?}"
                ));
            }
            max_width = Some(numeric);
        } else if remaining.insert(name.clone(), value.to_string()).is_some() {
            return Err(format!("duplicate root style declaration: {name}"));
        }
    }
    Ok((max_width, remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(upstream: &str, local: &str) -> Result<(), String> {
        let upstream = parse_root_contract(upstream)?;
        let local = parse_root_contract(local)?;
        if upstream.view_box.is_some() != local.view_box.is_some()
            || upstream.width_policy != local.width_policy
            || upstream.height_policy != local.height_policy
            || upstream.max_width_relation != local.max_width_relation
            || upstream.style_without_max_width != local.style_without_max_width
        {
            Err("root contract mismatch".to_string())
        } else {
            Ok(())
        }
    }

    #[test]
    fn browser_owned_bbox_numbers_can_move_without_weakening_root_policy() {
        let upstream = r#"<svg width="100%" viewBox="-50 -10 450 259" style="max-width: 450px; background-color: white;"><g/></svg>"#;
        let local = r#"<svg width="100%" viewBox="0.125 0.25 451.5 260.75" style="max-width: 451.5px; background-color: white;"><g/></svg>"#;
        validate(upstream, local).expect("browser bbox movement is diagnostic");
    }

    #[test]
    fn invalid_non_finite_or_non_positive_roots_fail_closed() {
        for svg in [
            r#"<svg width="100%" viewBox="0 0 NaN 20" style="max-width: 10px;"/>"#,
            r#"<svg width="100%" viewBox="0 0 0 20" style="max-width: 10px;"/>"#,
            r#"<svg width="100%" viewBox="0 0 10 -1" style="max-width: 10px;"/>"#,
            r#"<svg width="0" viewBox="0 0 10 20"/>"#,
            r#"<svg width="100%" viewBox="0 0 10 20" style="max-width: infinitypx;"/>"#,
        ] {
            assert!(parse_root_contract(svg).is_err(), "svg={svg}");
        }
    }

    #[test]
    fn root_strategy_mutations_are_blocking() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#;
        for local in [
            r#"<svg width="100%" style="max-width: 100px; background-color: white;"/>"#,
            r#"<svg width="100" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#,
            r#"<svg width="100%" height="50" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#,
            r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 80px; background-color: white;"/>"#,
            r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px; background-color: transparent;"/>"#,
        ] {
            assert!(validate(upstream, local).is_err(), "local={local}");
        }
    }

    #[test]
    fn deterministic_root_signature_keeps_origin_exact() {
        let expected = parse_root_contract(
            r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px;"/>"#,
        )
        .unwrap();
        let changed = parse_root_contract(
            r#"<svg width="100%" viewBox="-1 0 100 50" style="max-width: 100px;"/>"#,
        )
        .unwrap();
        assert_ne!(expected.signature, changed.signature);
    }

    #[test]
    fn deterministic_contract_catalog_is_bound_to_live_inputs_and_upstream_roots() {
        let catalog = load_deterministic_root_catalog().expect("deterministic root catalog");
        assert_eq!(catalog.entries.len(), 10);
        assert!(
            catalog
                .entries
                .iter()
                .any(|entry| entry.diagram == "flowchart" && entry.fixture == "basic")
        );

        let mut drifted = catalog.clone();
        drifted.entries[0].input_sha256 = "0".repeat(64);
        assert!(validate_deterministic_root_catalog(&drifted).is_err());
    }

    #[test]
    fn descendant_structure_and_identity_mutations_remain_blocking() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px;"><defs><marker id="arrow"/></defs><g data-id="node-a" clip-path="url(#clip)"><rect width="20" height="10"/><path marker-end="url(#arrow)"/><text>ready</text></g></svg>"#;
        for local in [
            upstream.replace("data-id=\"node-a\"", "data-id=\"node-b\""),
            upstream.replace("url(#arrow)", "url(#other)"),
            upstream.replace("url(#clip)", "none"),
            upstream.replace(r#"<rect width="20" height="10"/>"#, ""),
        ] {
            let profile =
                crate::svgdom::DomComparisonProfile::from_mode(crate::svgdom::DomMode::ParityRoot);
            let upstream =
                crate::svgdom::dom_signature_for_comparison(upstream, profile, 3).unwrap();
            let local = crate::svgdom::dom_signature_for_comparison(&local, profile, 3).unwrap();
            assert_ne!(upstream, local);
        }
    }
}
