#[cfg(test)]
use crate::resources::{RenderResourcePolicy, ResourceLimitId};
use crate::svg_xml::SvgStructureMetrics;
use crate::svg_xml::{
    SVG_NAMESPACE, SvgXmlVisitor, is_svg_element_namespace, resolve_xml_reference_value,
};
#[cfg(test)]
use crate::svg_xml::{XML_VALIDATION_PASS, validate_xml_characters, xml_validation_error};
use crate::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use svgtypes::{FuncIRI, IRI, Length, SimplePathSegment, SimplifyingPathParser};

use super::SvgValidation;
use super::resource_closure::{SvgResourceClosure, SvgResourceClosureBuilder};
use super::text_tracking::{SvgFontSeal, SvgTextContentTracker};
use crate::svg_safety::{
    CssValidationError, is_data_url, matches_active_svg_element,
    parsed_attribute_violates_resvg_contract, validate_resvg_css_declaration_list_with_checkpoints,
    validate_resvg_css_stylesheet_with_checkpoints,
};
use crate::svg_scan::checkpoint_loop;

const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const VALIDATION_PASS: &str = "validate-resvg-compatible-svg";

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ReferenceAnalysisWork {
    dependency_nodes: usize,
    tree_edges: usize,
    postorder_slots: usize,
    occurrence_slots: usize,
    occurrence_edges: usize,
}

#[cfg(test)]
thread_local! {
    static REFERENCE_ANALYSIS_WORK: std::cell::Cell<Option<ReferenceAnalysisWork>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(test)]
fn record_reference_analysis_work(update: impl FnOnce(&mut ReferenceAnalysisWork)) {
    if let Some(mut work) = REFERENCE_ANALYSIS_WORK.get() {
        update(&mut work);
        REFERENCE_ANALYSIS_WORK.set(Some(work));
    }
}

#[cfg(test)]
fn measure_reference_analysis_work<T>(operation: impl FnOnce() -> T) -> (T, ReferenceAnalysisWork) {
    struct Restore(Option<ReferenceAnalysisWork>);
    impl Drop for Restore {
        fn drop(&mut self) {
            REFERENCE_ANALYSIS_WORK.set(self.0);
        }
    }

    let _restore = Restore(REFERENCE_ANALYSIS_WORK.replace(Some(Default::default())));
    let output = operation();
    let work = REFERENCE_ANALYSIS_WORK
        .get()
        .expect("reference analysis scope is active");
    (output, work)
}

/// Validates XML at operations that explicitly consume an opaque SVG document.
#[cfg(test)]
fn validate_well_formed_svg(svg: &str, limits: RenderResourcePolicy) -> Result<()> {
    let mut checkpoint = || Ok(());
    validate_well_formed_svg_with_checkpoint(svg, limits, &mut checkpoint)
}

#[cfg(test)]
pub(crate) fn validate_well_formed_svg_with_checkpoint(
    svg: &str,
    limits: RenderResourcePolicy,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    validate_well_formed_svg_with_controls(svg, checkpoint, &mut |elements, _tree_depth| {
        limits.check_svg_element_count(elements).map_err(Into::into)
    })
    .map(|_| ())
}

pub fn validate_well_formed_svg_with_controls(
    svg: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
    check_structure: &mut impl FnMut(usize, usize) -> Result<()>,
) -> Result<SvgStructureMetrics> {
    #[cfg(any(test, feature = "test-support"))]
    super::test_support::record_xml_walk(svg);
    crate::svg_xml::validate_well_formed_svg_with_controls(svg, checkpoint, check_structure)
}

fn visit_svg_references(
    svg: &str,
    checkpoint: &mut dyn FnMut() -> Result<()>,
    check_structure: &mut dyn FnMut(usize, usize) -> Result<()>,
    observe_element: &mut dyn FnMut(ValidatedElement, usize) -> Result<()>,
) -> Result<SvgStructureMetrics> {
    #[cfg(any(test, feature = "test-support"))]
    super::test_support::record_xml_walk(svg);
    crate::svg_xml::visit_well_formed_svg_with_controls(
        svg,
        checkpoint,
        check_structure,
        &mut ReferenceVisitor(observe_element),
    )
}

struct ReferenceVisitor<'a>(&'a mut dyn FnMut(ValidatedElement, usize) -> Result<()>);

impl SvgXmlVisitor for ReferenceVisitor<'_> {
    type Attributes<'xml> = ReferenceAttributes<'xml>;
    type Element = ValidatedElement;

    fn begin<'xml>(
        &mut self,
        local_name: &'xml str,
        is_svg: bool,
    ) -> Option<Self::Attributes<'xml>> {
        Some(ReferenceAttributes::new(local_name, is_svg))
    }

    fn attribute<'xml>(
        &mut self,
        references: &mut Self::Attributes<'xml>,
        namespace: ResolveResult<'_>,
        name: &str,
        value: Cow<'xml, str>,
        checkpoint: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let is_unbound = matches!(&namespace, ResolveResult::Unbound);
        let is_xlink = matches!(
            &namespace,
            ResolveResult::Bound(namespace) if namespace.as_ref() == XLINK_NAMESPACE
        );
        references.observe_raw_attribute(name, is_unbound, is_xlink, &value);
        if usvg_consumes_attribute_namespace(namespace)? {
            references.observe(name, &value, checkpoint)?;
        }
        Ok(())
    }

    fn finish(&mut self, references: Self::Attributes<'_>) -> Self::Element {
        references.finish()
    }

    fn element(&mut self, element: Self::Element, depth: usize) -> Result<()> {
        (self.0)(element, depth)
    }
}

/// Validates the terminal XML contract consumed by usvg/resvg.
///
/// This check deliberately does not claim that an SVG is safe to insert into a browser DOM. DOM
/// embedding needs a separate browser-oriented policy for navigation, network access, and HTML
/// integration.
#[cfg(test)]
pub(crate) fn validate_resvg_compatible_svg(
    svg: &str,
    limits: RenderResourcePolicy,
) -> Result<TerminalSvgValidation> {
    validate_resvg_compatible_svg_inner(svg, limits)
}

#[cfg(test)]
fn validate_resvg_compatible_svg_inner(
    svg: &str,
    limits: RenderResourcePolicy,
) -> Result<TerminalSvgValidation> {
    let mut checkpoint = || Ok(());
    let raw_structure =
        validate_well_formed_svg_with_controls(svg, &mut checkpoint, &mut |elements, _| {
            limits.check_svg_element_count(elements).map_err(Into::into)
        })?;
    validate_resvg_compatible_svg_after_xml_with_structure(
        svg,
        raw_structure,
        &mut checkpoint,
        &mut |elements, tree_depth| {
            limits
                .check_svg_structure(elements, tree_depth)
                .map_err(Into::into)
        },
        &mut |value| {
            limits
                .check_svg_backend_coordinate_magnitude(value)
                .map_err(Into::into)
        },
    )
}

impl SvgValidation<'_> {
    /// Validates XML structure and the operation's element allowance.
    pub fn validate_xml(self, svg: &str) -> Result<SvgStructureMetrics> {
        validate_well_formed_svg_with_controls(
            svg,
            &mut || self.checkpoint(),
            &mut |elements, _tree_depth| {
                self.resource_policy()
                    .check_svg_element_count(elements)
                    .map_err(|error| self.terminate_resource_error(error))
            },
        )
    }

    /// Validates and accounts for an SVG before native consumption.
    pub fn validate_native(self, svg: &str) -> Result<TerminalSvgValidation> {
        validate_resvg_svg_for_consumer(svg, self, true)
    }

    /// Observes native compatibility without terminating on backend-only constraints.
    pub fn observe_native_compatibility(self, svg: &str) -> Result<TerminalSvgValidation> {
        validate_resvg_svg_for_consumer(svg, self, false)
    }

    /// Checks the explicit budgets of an opaque SVG without granting native compatibility.
    pub fn check_resource_budget(self, svg: &str) -> Result<()> {
        check_svg_resource_budget_with_controls(
            svg,
            &mut || self.checkpoint(),
            &mut |elements, _| self.preflight_svg_element_count(elements),
        )
    }
}

fn validate_resvg_svg_for_consumer(
    svg: &str,
    execution: SvgValidation<'_>,
    native_required: bool,
) -> Result<TerminalSvgValidation> {
    #[cfg(any(test, feature = "test-support"))]
    super::test_support::record_terminal_validation(svg);
    let mut checkpoint = || execution.checkpoint();
    let mut check_structure = |elements, tree_depth| {
        if native_required {
            execution.preflight_svg_structure(elements, tree_depth)
        } else {
            execution.preflight_svg_element_count(elements)?;
            execution
                .resource_policy()
                .check_svg_structure(elements, tree_depth)
                .map_err(Into::into)
        }
    };
    let raw_structure =
        validate_well_formed_svg_with_controls(svg, &mut checkpoint, &mut check_structure)?;
    validate_resvg_compatible_svg_after_xml_with_structure(
        svg,
        raw_structure,
        &mut checkpoint,
        &mut check_structure,
        &mut |value| {
            execution
                .resource_policy()
                .check_svg_backend_coordinate_magnitude(value)
                .map_err(|error| {
                    if native_required {
                        execution.terminate_resource_error(error)
                    } else {
                        error.into()
                    }
                })
        },
    )
}

fn check_svg_resource_budget_with_controls(
    svg: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
    check_structure: &mut impl FnMut(usize, usize) -> Result<()>,
) -> Result<()> {
    let mut references = SvgReferenceFacts::default();
    let mut stack = Vec::new();
    let raw_structure =
        visit_svg_references(svg, checkpoint, check_structure, &mut |element, depth| {
            stack.truncate(depth);
            let index = references.append(stack.last().copied(), element);
            stack.push(index);
            Ok(())
        })?;
    let summary = match analyze_svg_reference_facts_with_checkpoints::<false, _>(
        references,
        raw_structure,
        checkpoint,
    ) {
        Ok(summary) => summary,
        // Cyclic references do not prevent ordinary browser SVG publication. Native admission
        // separately requires a complete expansion plan before backend work.
        Err(ReferencePlanningError::Invalid(_)) => return checkpoint(),
        Err(ReferencePlanningError::Checkpoint(error)) => return Err(error),
    };
    checkpoint()?;
    check_structure(summary.structure.elements, summary.structure.max_tree_depth)
}

/// Reuses the ordinary XML reference collector for occurrence validation. This does not
/// certify native compatibility or replace the finalizer's resource-policy checks.
pub fn plan_svg_occurrences_for_receipt(svg: &str) -> Result<SvgReferencePlan> {
    let mut facts = SvgReferenceFacts::default();
    let mut stack = Vec::new();
    let mut checkpoint = || Ok(());
    let structure = visit_svg_references(
        svg,
        &mut checkpoint,
        &mut |_, _| Ok(()),
        &mut |element, depth| {
            stack.truncate(depth);
            let index = facts.append(stack.last().copied(), element);
            stack.push(index);
            Ok(())
        },
    )?;
    match analyze_svg_reference_facts_with_checkpoints::<true, _>(facts, structure, &mut checkpoint)
    {
        Ok(analysis) => Ok(analysis.into_detailed_plan()),
        Err(ReferencePlanningError::Invalid(error)) => Err(validation_error(error)),
        Err(ReferencePlanningError::Checkpoint(error)) => Err(error),
    }
}

/// Reference-expansion work retained by a sealed resvg-compatible SVG.
///
/// The occurrence slice is ordered by source-document element encounter order. Exporters use it
/// to charge each inline resource once for every `<use>`-expanded instance before asking usvg to
/// decode that resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgReferencePlan {
    expanded_elements: usize,
    max_tree_depth: usize,
    raw_element_occurrences: Box<[usize]>,
}

impl SvgReferencePlan {
    /// Returns the element count after same-document `<use>` expansion.
    pub const fn expanded_elements(&self) -> usize {
        self.expanded_elements
    }

    /// Returns the maximum resolved XML-tree depth after `<use>` expansion.
    pub const fn max_tree_depth(&self) -> usize {
        self.max_tree_depth
    }

    /// Returns expanded instance counts for source elements in document encounter order.
    pub fn raw_element_occurrences(&self) -> &[usize] {
        &self.raw_element_occurrences
    }
}

#[inline(never)]
fn validate_resvg_compatible_svg_after_xml_with_structure(
    svg: &str,
    raw_structure: SvgStructureMetrics,
    mut checkpoint: &mut dyn FnMut() -> Result<()>,
    check_structure: &mut dyn FnMut(usize, usize) -> Result<()>,
    check_coordinate: &mut dyn FnMut(f64) -> Result<()>,
) -> Result<TerminalSvgValidation> {
    #[cfg(any(test, feature = "test-support"))]
    super::test_support::record_terminal_xml_walk(svg);
    let mut reader = NsReader::from_str(svg);
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut style_text = None::<OpenStyle>;
    let mut references = SvgReferenceFacts::default();
    let mut reference_stack = Vec::new();
    let mut resource_closure = SvgResourceClosureBuilder::default();
    let mut inline_raster_sources = Vec::new();
    let mut text_content = SvgTextContentTracker::default();
    let mut fallback_css_residual = false;

    loop {
        checkpoint()?;
        let event = reader.read_event();
        checkpoint()?;
        let event = event.map_err(|error| validation_error(format!("invalid XML: {error}")))?;
        let is_empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if style_text.is_some() {
                    return Err(validation_error(
                        "a <style> element contains nested XML elements",
                    ));
                }
                let is_root = depth == 0;
                reject_additional_root(is_root, root_seen, root_closed)?;
                let mut validated = validate_element_after_xml(
                    &element,
                    reader.resolver(),
                    is_root,
                    svg,
                    &mut resource_closure,
                    checkpoint,
                    check_coordinate,
                )?;
                let parent = reference_stack.last().copied();
                if !is_empty && validated.is_svg_element && element.local_name().as_ref() == "text"
                {
                    text_content.observe_start(depth);
                }
                if validated.local_text_reference.is_some() {
                    text_content.observe_character();
                }
                let opens_style = validated.is_style;
                fallback_css_residual |= validated.fallback_css_residual;
                let raster_href = validated.inline_raster_href.take();
                let index = references.append(parent, validated);
                if let Some(href) = raster_href {
                    inline_raster_sources.push((index, href));
                }
                if is_root {
                    root_seen = true;
                    root_closed = is_empty;
                }
                if is_empty {
                    if opens_style {
                        validate_style_text("", &mut checkpoint)?;
                    }
                } else {
                    depth += 1;
                    if opens_style {
                        style_text = Some(OpenStyle { css: String::new() });
                    }
                    reference_stack.push(index);
                }
            }
            Event::End(element) => {
                let (namespace, _) = reader.resolver().resolve_element(element.name());
                reject_unknown_namespace(namespace)?;
                let local_name = element.local_name();
                let element_name = local_name.as_ref();
                if let Some(style) = style_text.take() {
                    if !element_name.eq_ignore_ascii_case("style") {
                        return Err(validation_error(
                            "a <style> element contains nested XML elements",
                        ));
                    }
                    validate_style_text(&style.css, &mut checkpoint)?;
                    resource_closure
                        .observe_stylesheet_urls(&style.css)
                        .map_err(validation_error)?;
                }
                reference_stack
                    .pop()
                    .ok_or_else(|| validation_error("an end tag has no matching reference node"))?;
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| validation_error("an end tag has no matching start tag"))?;
                text_content.observe_end(depth);
                if depth == 0 {
                    root_closed = true;
                }
            }
            Event::Text(text) => {
                let text = text.xml10_content();
                checkpoint()?;
                text_content.observe_content(&text);
                if let Some(style) = style_text.as_mut() {
                    style.css.push_str(&text);
                } else {
                    let text_outside_root = depth == 0 && !text.trim().is_empty();
                    checkpoint()?;
                    if text_outside_root {
                        return Err(validation_error("text is not allowed outside the SVG root"));
                    }
                }
            }
            Event::CData(text) => {
                let text = text.xml10_content();
                checkpoint()?;
                text_content.observe_content(&text);
                if let Some(style) = style_text.as_mut() {
                    style.css.push_str(&text);
                } else {
                    let cdata_outside_root = depth == 0 && !text.trim().is_empty();
                    checkpoint()?;
                    if cdata_outside_root {
                        return Err(validation_error(
                            "CDATA is not allowed outside the SVG root",
                        ));
                    }
                }
            }
            Event::GeneralRef(reference) => {
                let value = resolve_xml_reference_value(&reference);
                checkpoint()?;
                let value = value.map_err(validation_error)?;
                text_content.observe_character();
                if let Some(style) = style_text.as_mut() {
                    style.css.push(value);
                } else if depth == 0 && !value.is_ascii_whitespace() {
                    return Err(validation_error(
                        "character references are not allowed outside the SVG root",
                    ));
                }
            }
            Event::PI(_) => {
                return Err(validation_error(
                    "processing instructions are not accepted by the resvg-safe contract",
                ));
            }
            Event::DocType(_) => {
                return Err(validation_error(
                    "document type declarations are not accepted by the resvg-safe contract",
                ));
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }

    if !root_seen {
        return Err(validation_error(
            "the document does not contain an SVG root",
        ));
    }
    if !root_closed || depth != 0 || style_text.is_some() {
        return Err(validation_error("the SVG root is not closed"));
    }
    checkpoint()?;
    let reference_plan = match analyze_svg_reference_facts_with_checkpoints::<true, _>(
        references,
        raw_structure,
        &mut checkpoint,
    ) {
        Ok(plan) => plan.into_detailed_plan(),
        Err(ReferencePlanningError::Invalid(error)) => {
            return Err(validation_error(error));
        }
        Err(ReferencePlanningError::Checkpoint(error)) => return Err(error),
    };
    checkpoint()?;
    check_structure(
        reference_plan.expanded_elements(),
        reference_plan.max_tree_depth(),
    )?;
    let resource_closure = resource_closure.finish().map_err(validation_error)?;
    let text_elements = text_content.text_element_count();
    let font_seal = SvgFontSeal::from_text_count(text_elements);
    Ok(TerminalSvgValidation {
        reference_plan,
        resource_closure,
        inline_raster_sources: InlineRasterSources(inline_raster_sources.into_boxed_slice()),
        text_elements,
        font_seal,
        fallback_css_residual,
    })
}

struct OpenStyle {
    css: String,
}

/// A normalized href usually borrows the SVG input. Keep only its range in the sealed artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InlineRasterHref {
    Source(Range<usize>),
    Normalized(Box<str>),
}

impl InlineRasterHref {
    fn new(source: &str, value: &str) -> Self {
        if let Some(start) = value.as_ptr().addr().checked_sub(source.as_ptr().addr())
            && let Some(end) = start.checked_add(value.len())
            && end <= source.len()
        {
            return Self::Source(start..end);
        }
        Self::Normalized(value.into())
    }

    fn as_str<'a>(&'a self, source: &'a str) -> &'a str {
        match self {
            Self::Source(range) => &source[range.clone()],
            Self::Normalized(value) => value,
        }
    }
}

/// Inline raster references collected from the validated document.
#[derive(Debug, Clone, Default)]
pub struct InlineRasterSources(Box<[(usize, InlineRasterHref)]>);

impl InlineRasterSources {
    /// Iterates resource occurrences in the same SVG bytes that were validated.
    pub fn iter<'a>(&'a self, svg: &'a str) -> impl Iterator<Item = (usize, &'a str)> + 'a {
        self.0
            .iter()
            .map(move |(index, href)| (*index, href.as_str(svg)))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct TerminalSvgValidation {
    pub reference_plan: SvgReferencePlan,
    pub resource_closure: SvgResourceClosure,
    pub inline_raster_sources: InlineRasterSources,
    pub text_elements: usize,
    pub font_seal: SvgFontSeal,
    pub fallback_css_residual: bool,
}

#[cfg(test)]
impl std::ops::Deref for TerminalSvgValidation {
    type Target = SvgReferencePlan;

    fn deref(&self) -> &Self::Target {
        &self.reference_plan
    }
}

fn reject_additional_root(is_root: bool, root_seen: bool, root_closed: bool) -> Result<()> {
    if is_root && (root_seen || root_closed) {
        return Err(validation_error(
            "the document contains more than one root element",
        ));
    }
    Ok(())
}

struct ValidatedElement {
    fallback_css_residual: bool,
    is_svg_element: bool,
    is_style: bool,
    is_marker: bool,
    may_repeat_per_element: bool,
    inline_raster_href: Option<InlineRasterHref>,
    local_text_reference: Option<String>,
    ids: ElementIds,
    references: [Option<ElementReference>; 4],
}

#[derive(Default)]
struct ElementIds {
    parsed: Option<String>,
    raw: RawElementId,
}

#[derive(Default)]
enum RawElementId {
    #[default]
    Absent,
    Parsed,
    Other(String),
}

impl ElementIds {
    fn observe_raw(&mut self, value: &str) {
        if matches!(self.raw, RawElementId::Absent) {
            self.raw = RawElementId::Other(value.to_owned());
        }
    }

    fn observe(&mut self, value: &str) {
        let parsed = self.parsed.get_or_insert_with(|| value.to_owned());
        if matches!(&self.raw, RawElementId::Other(raw) if raw == parsed) {
            self.raw = RawElementId::Parsed;
        }
    }

    fn raw_value(&self) -> Option<&str> {
        match &self.raw {
            RawElementId::Absent => None,
            RawElementId::Parsed => self.parsed.as_deref(),
            RawElementId::Other(value) => Some(value),
        }
    }
}

struct ReferenceIdentity {
    element: usize,
    is_marker: bool,
    ids: ElementIds,
}

struct SourceReference {
    source: usize,
    reference: ElementReference,
}

/// Preorder topology and sparse expansion facts for one immutable XML generation.
/// A document without expansion retains no child lists, reference lists, or dependency graph.
#[derive(Default)]
struct SvgReferenceFacts {
    parents: Vec<usize>,
    identities: Vec<ReferenceIdentity>,
    references: Vec<SourceReference>,
    effects: Vec<usize>,
}

impl SvgReferenceFacts {
    fn append(&mut self, parent: Option<usize>, element: ValidatedElement) -> usize {
        let index = self.parents.len();
        self.parents.push(parent.unwrap_or(usize::MAX));
        if element.ids.parsed.is_some() || element.ids.raw_value().is_some() {
            self.identities.push(ReferenceIdentity {
                element: index,
                is_marker: element.is_marker,
                ids: element.ids,
            });
        }
        self.references
            .extend(
                element
                    .references
                    .into_iter()
                    .flatten()
                    .map(|reference| SourceReference {
                        source: index,
                        reference,
                    }),
            );
        if element.may_repeat_per_element {
            self.effects.push(index);
        }
        index
    }
}

/// One edge arena, with contiguous adjacency ranges for real nodes and transparent ID groups.
pub(super) struct ReferenceDependencyGraph {
    ranges: Vec<std::ops::Range<usize>>,
    edges: Vec<(usize, usize)>,
    real_nodes: usize,
}

impl ReferenceDependencyGraph {
    #[cfg(test)]
    pub(super) fn new(dependencies: Vec<Vec<(usize, usize)>>, real_nodes: usize) -> Self {
        match Self::with_checkpoints(dependencies, real_nodes, &mut || {
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(graph) => graph,
            Err(error) => match error {},
        }
    }

    pub(super) fn with_checkpoints<E>(
        dependencies: Vec<Vec<(usize, usize)>>,
        real_nodes: usize,
        checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
    ) -> std::result::Result<Self, E> {
        checkpoint()?;
        let mut edge_count = 0;
        for (iteration, node_edges) in dependencies.iter().enumerate() {
            checkpoint_loop(iteration, checkpoint)?;
            edge_count += node_edges.len();
        }
        let mut ranges = Vec::with_capacity(dependencies.len());
        let mut edges = Vec::with_capacity(edge_count);
        let mut edge_iteration = 0usize;
        for (iteration, node_edges) in dependencies.into_iter().enumerate() {
            checkpoint_loop(iteration, checkpoint)?;
            let start = edges.len();
            for edge in node_edges {
                checkpoint_loop(edge_iteration, checkpoint)?;
                edge_iteration = edge_iteration.saturating_add(1);
                edges.push(edge);
            }
            ranges.push(start..edges.len());
        }
        checkpoint()?;
        Ok(Self {
            ranges,
            edges,
            real_nodes,
        })
    }

    fn dependencies(&self, index: usize) -> &[(usize, usize)] {
        &self.edges[self.ranges[index].clone()]
    }
}

#[derive(Clone, Copy)]
enum ReferenceTargetKind {
    UseElement,
    ParsedElement,
    Marker,
}

struct ElementReference {
    target: String,
    multiplicity: usize,
    target_kind: ReferenceTargetKind,
}

pub(super) enum ReferencePlanningError<E> {
    Invalid(String),
    Checkpoint(E),
}

pub(super) type ReferencePlanningResult<T, E> = std::result::Result<T, ReferencePlanningError<E>>;

#[inline(never)]
fn validate_element_after_xml(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_root: bool,
    source: &str,
    resource_closure: &mut SvgResourceClosureBuilder,
    mut checkpoint: &mut dyn FnMut() -> Result<()>,
    check_coordinate: &mut dyn FnMut(f64) -> Result<()>,
) -> Result<ValidatedElement> {
    let (namespace, local_name) = resolver.resolve_element(element.name());
    let is_svg_element = is_svg_element_namespace(&namespace);
    validate_namespace(namespace, is_root)?;
    let element_name = local_name.as_ref();
    if is_root && element_name != "svg" {
        return Err(validation_error("the document root is not an SVG element"));
    }
    if matches_active_svg_element(element_name) {
        return Err(validation_error(format!(
            "active element <{element_name}> survived terminal sanitization"
        )));
    }
    if is_svg_element && element_name.eq_ignore_ascii_case("foreignObject") {
        return Err(validation_error(
            "a <foreignObject> element is not supported by the standalone portable SVG contract",
        ));
    }

    let mut reference_attributes = ReferenceAttributes::new(element_name, is_svg_element);
    let mut root_width_seen = false;
    let mut root_height_seen = false;
    let mut fallback_css_residual = false;
    let mut inline_text_path = None;
    let is_anchor = element_name.eq_ignore_ascii_case("a");
    // The caller already validated this immutable element, including lexical and expanded
    // attribute-name uniqueness. Keep parsing and all target checks without rebuilding that set.
    for attribute in element.attributes().with_checks(false) {
        checkpoint()?;
        let attribute = attribute
            .map_err(|error| validation_error(format!("invalid XML attribute: {error}")))?;
        let qualified_name = attribute.key.as_ref();
        checkpoint()?;
        let value = attribute.normalized_value(XmlVersion::Implicit1_0);
        checkpoint()?;
        let value = value
            .map_err(|error| validation_error(format!("invalid XML attribute value: {error}")))?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }

        let (attribute_namespace, local_name) = resolver.resolve_attribute(attribute.key);
        let is_unbound_attribute = matches!(&attribute_namespace, ResolveResult::Unbound);
        let is_xlink_attribute = matches!(
            &attribute_namespace,
            ResolveResult::Bound(namespace) if namespace.as_ref() == XLINK_NAMESPACE
        );
        let consumes_namespace = usvg_consumes_attribute_namespace(attribute_namespace);
        checkpoint()?;
        reference_attributes.observe_raw_attribute(
            local_name.as_ref(),
            is_unbound_attribute,
            is_xlink_attribute,
            &value,
        );
        if !consumes_namespace? {
            continue;
        }
        let semantic_name = local_name.as_ref();
        checkpoint()?;
        if is_svg_element
            && is_unbound_attribute
            && qualified_name == "data-merman-fallback-css-residual"
            && value == "true"
        {
            // An authored marker can only request a warning or stricter fidelity rejection.
            // It cannot grant compatibility, font, or resource evidence.
            fallback_css_residual = true;
        }
        if is_unbound_attribute && qualified_name == "data-merman-typed-fonts" {
            return Err(validation_error(
                "embedded theme font resources are not supported",
            ));
        }
        let violates_contract =
            parsed_attribute_violates_resvg_contract(element_name, qualified_name, &value);
        checkpoint()?;
        if violates_contract {
            return Err(validation_error(format!(
                "attribute {qualified_name:?} on <{element_name}> violates the resvg-safe contract"
            )));
        }
        if is_svg_element {
            check_native_coordinate_attribute(
                element_name,
                semantic_name,
                &value,
                is_root,
                &mut checkpoint,
                check_coordinate,
            )?;
        }
        if is_root && semantic_name == "width" && !root_width_seen {
            root_width_seen = true;
            let dimension = validate_positive_root_dimension("width", &value);
            checkpoint()?;
            dimension?;
        }
        if is_root && semantic_name == "height" && !root_height_seen {
            root_height_seen = true;
            let dimension = validate_positive_root_dimension("height", &value);
            checkpoint()?;
            dimension?;
        }
        if semantic_name == "style" {
            match validate_resvg_css_declaration_list_with_checkpoints(&value, &mut checkpoint) {
                Ok(()) => {}
                Err(CssValidationError::Invalid(error)) => {
                    checkpoint()?;
                    return Err(validation_error(format!(
                        "invalid style attribute on <{element_name}>: {error}"
                    )));
                }
                Err(CssValidationError::Checkpoint(error)) => return Err(error),
            }
            resource_closure
                .observe_inline_css_urls(&value)
                .map_err(validation_error)?;
        } else if is_url_function_attribute(semantic_name) {
            resource_closure
                .observe_inline_css_urls(&value)
                .map_err(validation_error)?;
        }
        reference_attributes.observe(semantic_name, &value, checkpoint)?;
        if semantic_name == "src" || (semantic_name == "href" && !is_svg_element && !is_anchor) {
            observe_terminal_resource_url(resource_closure, &value);
        }
        if is_svg_element && element_name == "textPath" && semantic_name == "path" {
            inline_text_path.get_or_insert(value);
        }
    }
    if let Some(error) = reference_attributes.marker_error.take() {
        return Err(error);
    }
    let mut inline_raster_href = None;
    if let Some(href) = reference_attributes
        .effective_href()
        .filter(|href| !href.is_empty())
        && !element_name.eq_ignore_ascii_case("use")
        && element_name != "tref"
    {
        if matches!(element_name, "image" | "feImage") && is_data_url(href) {
            resource_closure.observe_inline_data_resource();
            inline_raster_href = Some(InlineRasterHref::new(source, href));
        } else if match inline_text_path {
            Some(path) => !inline_text_path_is_usable(&path, &mut checkpoint)?,
            None => true,
        } {
            observe_terminal_resource_url(resource_closure, href);
        }
    }
    let mut validated = reference_attributes.finish();
    validated.fallback_css_residual = fallback_css_residual;
    validated.inline_raster_href = inline_raster_href;
    if let Some(id) = &validated.ids.parsed {
        resource_closure.observe_parsed_fragment_id(id);
    }
    if let Some(id) = validated.ids.raw_value() {
        resource_closure.observe_raw_use_fragment_id(id);
    }
    if let Some(target) = &validated.local_text_reference {
        resource_closure.observe_raw_use_fragment_reference(target);
    }
    // Raw links use the effective href. Presentation resources resolve against parsed IDs,
    // which may differ from the first local-name XML ID.
    for reference in validated.references.iter().flatten() {
        if matches!(reference.target_kind, ReferenceTargetKind::UseElement) {
            resource_closure.observe_raw_use_fragment_reference(&reference.target);
        }
    }
    Ok(validated)
}

/// Owns the attribute precedence shared by native validation and ordinary SVG budgeting.
/// Unbounded marker forms are retained as native-contract errors; ordinary output remains
/// unverified and only charges the references that this collector can analyze.
struct ReferenceAttributes<'a> {
    element_name: &'a str,
    is_svg_element: bool,
    ids: ElementIds,
    raw_href: Option<Cow<'a, str>>,
    xlink_href: Option<Cow<'a, str>>,
    geometry_source_len: Option<usize>,
    marker_start: Option<String>,
    marker_start_seen: bool,
    marker_mid: Option<String>,
    marker_mid_seen: bool,
    marker_end: Option<String>,
    marker_end_seen: bool,
    marker_error: Option<Error>,
}

impl<'a> ReferenceAttributes<'a> {
    fn new(element_name: &'a str, is_svg_element: bool) -> Self {
        Self {
            element_name,
            is_svg_element,
            ids: ElementIds::default(),
            raw_href: None,
            xlink_href: None,
            geometry_source_len: None,
            marker_start: None,
            marker_start_seen: false,
            marker_mid: None,
            marker_mid_seen: false,
            marker_end: None,
            marker_end_seen: false,
            marker_error: None,
        }
    }

    #[inline]
    #[expect(
        clippy::ptr_arg,
        reason = "Retained hrefs preserve source borrows from the normalized XML value"
    )]
    fn observe_raw_attribute(
        &mut self,
        name: &str,
        is_unbound: bool,
        is_xlink: bool,
        value: &Cow<'a, str>,
    ) {
        // The legacy tref parser falls back to the first raw local name. Other SVG links
        // accept only the unprefixed and XLink forms.
        match name {
            "id" => self.ids.observe_raw(value),
            "href" if self.is_svg_element && !self.element_name.eq_ignore_ascii_case("a") => {
                if is_unbound || self.element_name == "tref" {
                    self.raw_href.get_or_insert_with(|| value.clone());
                }
                if is_xlink {
                    self.xlink_href.get_or_insert_with(|| value.clone());
                }
            }
            _ => {}
        }
    }

    #[inline]
    fn observe(
        &mut self,
        semantic_name: &str,
        value: &str,
        checkpoint: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        match semantic_name {
            "id" if self.is_svg_element => self.ids.observe(value),
            "d" if self.is_svg_element && self.element_name == "path" => {
                self.geometry_source_len.get_or_insert(value.len());
            }
            "points"
                if self.is_svg_element && matches!(self.element_name, "polyline" | "polygon") =>
            {
                self.geometry_source_len.get_or_insert(value.len());
            }
            "marker-start" | "marker-mid" | "marker-end" => {
                self.observe_marker(semantic_name, value, checkpoint)?;
            }
            _ => {}
        }
        Ok(())
    }

    #[inline(never)]
    fn observe_marker(
        &mut self,
        semantic_name: &str,
        value: &str,
        checkpoint: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let marker_slot = if semantic_name == "marker-start" && !self.marker_start_seen {
            self.marker_start_seen = true;
            Some(&mut self.marker_start)
        } else if semantic_name == "marker-mid" && !self.marker_mid_seen {
            self.marker_mid_seen = true;
            Some(&mut self.marker_mid)
        } else if semantic_name == "marker-end" && !self.marker_end_seen {
            self.marker_end_seen = true;
            Some(&mut self.marker_end)
        } else {
            None
        };
        if let Some(slot) = marker_slot {
            let target = same_document_marker_target(value);
            checkpoint()?;
            match target {
                Ok(Some(target)) if is_marker_capable_svg_element(self.element_name) => {
                    *slot = Some(target.to_owned());
                }
                Ok(None) => {}
                Ok(Some(_)) => {
                    self.marker_error.get_or_insert_with(|| {
                        validation_error(format!(
                            "marker references on <{}> cannot be bounded before usvg parsing",
                            self.element_name
                        ))
                    });
                }
                Err(error) => {
                    self.marker_error.get_or_insert(error);
                }
            }
        }
        Ok(())
    }

    fn effective_href(&self) -> Option<&str> {
        let raw = self.raw_href.as_deref();
        let xlink = self.xlink_href.as_deref();
        if self.element_name == "tref" {
            xlink.or(raw)
        } else {
            raw.or(xlink)
        }
    }

    fn finish(self) -> ValidatedElement {
        let target = self.effective_href().and_then(same_document_href_target);
        let local_text_reference = if self.element_name == "tref" {
            target.map(str::to_owned)
        } else {
            None
        };
        let target_kind = if self.element_name.eq_ignore_ascii_case("use") {
            Some(ReferenceTargetKind::UseElement)
        } else if self.element_name.eq_ignore_ascii_case("feImage") {
            Some(ReferenceTargetKind::ParsedElement)
        } else {
            None
        };
        let expansion = target_kind
            .zip(target)
            .map(|(target_kind, target)| ElementReference {
                target: target.to_owned(),
                multiplicity: 1,
                target_kind,
            });
        let marker = |target, multiplicity| ElementReference {
            target,
            multiplicity,
            target_kind: ReferenceTargetKind::Marker,
        };
        ValidatedElement {
            fallback_css_residual: false,
            is_svg_element: self.is_svg_element,
            is_style: self.is_svg_element && self.element_name.eq_ignore_ascii_case("style"),
            is_marker: self.is_svg_element && self.element_name.eq_ignore_ascii_case("marker"),
            may_repeat_per_element: self.is_svg_element
                && matches!(self.element_name, "filter" | "mask" | "clipPath"),
            inline_raster_href: None,
            local_text_reference,
            ids: self.ids,
            references: [
                expansion,
                self.marker_start.map(|target| marker(target, 1)),
                self.marker_mid.map(|target| {
                    marker(
                        target,
                        marker_mid_instance_upper_bound(
                            self.element_name,
                            self.geometry_source_len.unwrap_or(0),
                        ),
                    )
                }),
                self.marker_end.map(|target| marker(target, 1)),
            ],
        }
    }
}

fn is_url_function_attribute(name: &str) -> bool {
    matches!(
        name,
        "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "cursor"
            | "background"
            | "background-image"
    )
}

fn observe_terminal_resource_url(resources: &mut SvgResourceClosureBuilder, value: &str) {
    if let Some(fragment) = same_document_href_target(value) {
        resources.observe_parsed_fragment_reference(fragment);
    } else if value
        .trim_start()
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        resources.observe_inline_data_resource();
    }
}

/// Backend coordinate limits belong to native consumption, rather than ordinary SVG output.
/// Inspect the concrete root and canvas values while their attributes are already being read.
fn check_native_coordinate_attribute(
    element_name: &str,
    name: &str,
    value: &str,
    is_root: bool,
    checkpoint: &mut dyn FnMut() -> Result<()>,
    check_coordinate: &mut dyn FnMut(f64) -> Result<()>,
) -> Result<()> {
    if is_root
        && name == "viewBox"
        && let Ok(view_box) = value.parse::<svgtypes::ViewBox>()
    {
        for value in [
            view_box.x,
            view_box.y,
            view_box.w,
            view_box.h,
            view_box.x + view_box.w,
            view_box.y + view_box.h,
        ] {
            checkpoint()?;
            check_coordinate(value)?;
        }
    }
    let gradient_coordinate = match element_name {
        "linearGradient" => matches!(name, "x1" | "y1" | "x2" | "y2"),
        "radialGradient" => matches!(name, "cx" | "cy" | "r" | "fx" | "fy" | "fr"),
        _ => false,
    };
    if (is_root && matches!(name, "width" | "height")) || gradient_coordinate {
        // Relative lengths are resolved by the backend. Root and canvas producers emit
        // concrete SVG coordinates here; percentages must not be treated as pixel counts.
        if let Ok(length) = value.parse::<Length>()
            && matches!(
                length.unit,
                svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px
            )
        {
            check_coordinate(length.number)?;
        }
    }
    if matches!(name, "transform" | "gradientTransform" | "patternTransform") {
        for transform in svgtypes::TransformListParser::from(value) {
            checkpoint()?;
            let translation = match transform {
                Ok(svgtypes::TransformListToken::Translate { tx, ty }) => Some((tx, ty)),
                Ok(svgtypes::TransformListToken::Matrix { e, f, .. }) => Some((e, f)),
                _ => None,
            };
            if let Some((x, y)) = translation {
                check_coordinate(x)?;
                check_coordinate(y)?;
            }
        }
    }
    if is_root && name == "style" {
        let mut declarations = cssparser::Parser::new(value);
        while !declarations.is_exhausted() {
            checkpoint()?;
            let result: std::result::Result<(), cssparser::ParseError<Error>> = declarations
                .parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
                    let property = declaration.expect_ident_cloned()?;
                    if !property.eq_ignore_ascii_case("max-width") {
                        return Ok(());
                    }
                    declaration.expect_colon()?;
                    let start = declaration.position();
                    let _ = declaration.next()?;
                    let literal = declaration.slice_from(start);
                    if let Some(value) = crate::mermaid_style::parse_svg_number_or_px(literal) {
                        check_coordinate(value).map_err(cssparser::ParseError::custom)?;
                    }
                    Ok(())
                });
            if let Err(cssparser::ParseError {
                kind: cssparser::ParseErrorKind::Custom(error),
                ..
            }) = result
            {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn validate_positive_root_dimension(name: &str, value: &str) -> Result<()> {
    let Ok(length) = value.parse::<Length>() else {
        return Ok(());
    };
    if length.number.is_finite() && length.number > 0.0 {
        return Ok(());
    }
    Err(validation_error(format!(
        "root SVG {name} must be a positive length"
    )))
}

fn same_document_href_target(value: &str) -> Option<&str> {
    IRI::from_str(value).ok().map(|iri| iri.0)
}

fn inline_text_path_is_usable(
    value: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    // Match usvg's PathBuilder admission without building a second path. The parser may yield
    // a valid prefix before an error. Consecutive moves overwrite each other and a lone move
    // yields no path. The retained control points must fit in finite f32 bounds.
    let mut pending_move = None;
    let mut has_segment = false;
    let mut bounds = [(f32::INFINITY, f32::NEG_INFINITY); 2];
    let mut observe_point = |(x, y): (f64, f64)| {
        let point = [x as f32, y as f32];
        if !point.into_iter().all(f32::is_finite) {
            return false;
        }
        for ((min, max), coordinate) in bounds.iter_mut().zip(point) {
            *min = min.min(coordinate);
            *max = max.max(coordinate);
        }
        true
    };
    for (index, segment) in SimplifyingPathParser::from(value).enumerate() {
        checkpoint_loop(index, checkpoint)?;
        let Ok(segment) = segment else {
            break;
        };
        let points = match segment {
            SimplePathSegment::MoveTo { x, y } => {
                pending_move = Some((x, y));
                continue;
            }
            SimplePathSegment::LineTo { x, y } => [Some((x, y)), None, None],
            SimplePathSegment::Quadratic { x1, y1, x, y } => [Some((x1, y1)), Some((x, y)), None],
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => [Some((x1, y1)), Some((x2, y2)), Some((x, y))],
            SimplePathSegment::ClosePath => [None; 3],
        };
        has_segment = true;
        if !pending_move
            .take()
            .into_iter()
            .chain(points.into_iter().flatten())
            .all(&mut observe_point)
        {
            return Ok(false);
        }
    }
    checkpoint()?;
    Ok(has_segment
        && pending_move.into_iter().all(observe_point)
        && bounds
            .into_iter()
            .all(|(min, max)| f64::from(max) - f64::from(min) < f64::from(f32::MAX)))
}

fn same_document_marker_target(value: &str) -> Result<Option<&str>> {
    if value == "none" {
        return Ok(None);
    }
    FuncIRI::from_str(value)
        .map(|iri| Some(iri.0))
        .map_err(|_| validation_error("marker reference is not a bounded same-document url()"))
}

fn is_marker_capable_svg_element(element_name: &str) -> bool {
    matches!(
        element_name,
        "path" | "line" | "polyline" | "polygon" | "rect" | "circle" | "ellipse"
    )
}

fn marker_mid_instance_upper_bound(element_name: &str, geometry_source_len: usize) -> usize {
    match element_name {
        // One arc command may be lowered to multiple cubic segments. Four per source byte remains
        // a conservative bound even for compressed implicit command repetition.
        "path" => geometry_source_len.saturating_mul(4).saturating_add(16),
        "polyline" | "polygon" => geometry_source_len.saturating_add(4),
        "line" => 2,
        "rect" | "circle" | "ellipse" => 16,
        _ => 0,
    }
}

struct SvgReferenceAnalysis {
    structure: SvgStructureMetrics,
    raw_element_occurrences: Box<[usize]>,
}

impl SvgReferenceAnalysis {
    fn into_detailed_plan(self) -> SvgReferencePlan {
        SvgReferencePlan {
            expanded_elements: self.structure.elements,
            max_tree_depth: self.structure.max_tree_depth,
            raw_element_occurrences: self.raw_element_occurrences,
        }
    }
}

fn analyze_svg_reference_facts_with_checkpoints<const DETAILED: bool, E>(
    facts: SvgReferenceFacts,
    structure: SvgStructureMetrics,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> ReferencePlanningResult<SvgReferenceAnalysis, E> {
    checkpoint().map_err(ReferencePlanningError::Checkpoint)?;
    debug_assert_eq!(facts.parents.len(), structure.elements);
    if facts.references.is_empty() && facts.effects.is_empty() {
        // This decision uses every actual XML reference and effect, including missing targets.
        // IDs alone cannot expand an element. No topology or identity index is materialized.
        let raw_element_occurrences = if DETAILED {
            #[cfg(test)]
            record_reference_analysis_work(|work| work.occurrence_slots += structure.elements);
            vec![1; structure.elements].into_boxed_slice()
        } else {
            Box::default()
        };
        checkpoint().map_err(ReferencePlanningError::Checkpoint)?;
        return Ok(SvgReferenceAnalysis {
            structure,
            raw_element_occurrences,
        });
    }
    let (graph, effects) = build_reference_fact_dependencies(facts, checkpoint)
        .map_err(ReferencePlanningError::Checkpoint)?;
    analyze_compact_reference_dependencies::<DETAILED, _>(&graph, &effects, checkpoint)
}

struct ParsedIdCandidates {
    first: (usize, bool),
    additional: Vec<(usize, bool)>,
}

impl ParsedIdCandidates {
    fn iter(&self) -> impl Iterator<Item = &(usize, bool)> {
        std::iter::once(&self.first).chain(&self.additional)
    }
}

fn build_reference_fact_dependencies<E>(
    facts: SvgReferenceFacts,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<(ReferenceDependencyGraph, Vec<usize>), E> {
    let SvgReferenceFacts {
        parents,
        identities,
        references,
        effects,
    } = facts;
    let real_nodes = parents.len();
    let mut use_ids = HashMap::new();
    let mut parsed_ids = HashMap::<&str, ParsedIdCandidates>::new();
    for (iteration, identity) in identities.iter().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if let Some(id) = identity.ids.raw_value() {
            // Raw XML lookup selects the first local-name ID in document order.
            use_ids.entry(id).or_insert(identity.element);
        }
        if let Some(id) = identity.ids.parsed.as_deref() {
            let candidate = (identity.element, identity.is_marker);
            match parsed_ids.entry(id) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().additional.push(candidate);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(ParsedIdCandidates {
                        first: candidate,
                        additional: Vec::new(),
                    });
                }
            }
        }
    }

    let mut graph = ReferenceDependencyGraph {
        ranges: vec![0..0; real_nodes],
        edges: Vec::new(),
        real_nodes,
    };
    let mut parsed_dependencies = HashMap::new();
    let mut marker_dependencies = HashMap::new();
    let mut resolved_references = Vec::with_capacity(references.len());
    for (iteration, SourceReference { source, reference }) in references.into_iter().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        let target = match reference.target_kind {
            ReferenceTargetKind::UseElement => use_ids.get(reference.target.as_str()).copied(),
            ReferenceTargetKind::ParsedElement => resolve_fact_id_dependency(
                &reference.target,
                false,
                &parsed_ids,
                &mut parsed_dependencies,
                &mut graph,
                checkpoint,
            )?,
            ReferenceTargetKind::Marker => resolve_fact_id_dependency(
                &reference.target,
                true,
                &parsed_ids,
                &mut marker_dependencies,
                &mut graph,
                checkpoint,
            )?,
        };
        if let Some(target) = target {
            graph.ranges[source].end += 1;
            resolved_references.push((source, target, reference.multiplicity));
        }
    }
    // No graph node retains strings or an ID index. Sparse XML facts are consumed here.
    drop(use_ids);
    drop(parsed_ids);
    drop(parsed_dependencies);
    drop(marker_dependencies);
    drop(identities);

    for (iteration, &parent) in parents.iter().enumerate().skip(1) {
        checkpoint_loop(iteration, checkpoint)?;
        graph.ranges[parent].end += 1;
    }
    let tree_start = graph.edges.len();
    let mut end = tree_start;
    for (iteration, range) in graph.ranges[..real_nodes].iter_mut().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        let count = range.end;
        *range = end..end + count;
        end += count;
    }
    graph.edges.resize(end, (0, 0));
    // Each actual tree edge is written directly to its final arena once, without child lists.
    for (index, parent) in parents.into_iter().enumerate().skip(1) {
        checkpoint_loop(index, checkpoint)?;
        graph.edges[graph.ranges[parent].start] = (index, 1);
        graph.ranges[parent].start += 1;
        #[cfg(test)]
        record_reference_analysis_work(|work| work.tree_edges += 1);
    }
    for (iteration, (source, target, multiplicity)) in resolved_references.into_iter().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        graph.edges[graph.ranges[source].start] = (target, multiplicity);
        graph.ranges[source].start += 1;
    }
    let mut start = tree_start;
    for (iteration, range) in graph.ranges[..real_nodes].iter_mut().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        range.start = start;
        start = range.end;
    }
    #[cfg(test)]
    record_reference_analysis_work(|work| work.dependency_nodes += graph.ranges.len());
    Ok((graph, effects))
}

fn resolve_fact_id_dependency<'a, E>(
    target: &str,
    marker_only: bool,
    parsed_ids: &HashMap<&'a str, ParsedIdCandidates>,
    resolved: &mut HashMap<&'a str, Option<usize>>,
    graph: &mut ReferenceDependencyGraph,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<Option<usize>, E> {
    checkpoint()?;
    let Some((id, candidates)) = parsed_ids.get_key_value(target) else {
        return Ok(None);
    };
    if candidates.additional.is_empty() {
        let (index, is_marker) = candidates.first;
        return Ok((!marker_only || is_marker).then_some(index));
    }
    if let Some(&dependency) = resolved.get(target) {
        return Ok(dependency);
    }
    let mut candidates = candidates
        .iter()
        .filter_map(|&(index, is_marker)| (!marker_only || is_marker).then_some(index));
    let dependency = match (candidates.next(), candidates.next()) {
        (None, _) => None,
        (Some(first), None) => Some(first),
        (Some(first), Some(second)) => {
            let index = graph.ranges.len();
            let start = graph.edges.len();
            graph.edges.extend([(first, 1), (second, 1)]);
            for (iteration, candidate) in candidates.enumerate() {
                checkpoint_loop(iteration, checkpoint)?;
                graph.edges.push((candidate, 1));
            }
            graph.ranges.push(start..graph.edges.len());
            Some(index)
        }
    };
    resolved.insert(*id, dependency);
    Ok(dependency)
}

pub(super) fn summarize_svg_reference_dependencies_with_checkpoints<E>(
    graph: &ReferenceDependencyGraph,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> ReferencePlanningResult<SvgStructureMetrics, E> {
    analyze_compact_reference_dependencies::<false, _>(graph, &[], checkpoint)
        .map(|analysis| analysis.structure)
}

fn analyze_compact_reference_dependencies<const DETAILED: bool, E>(
    graph: &ReferenceDependencyGraph,
    effects: &[usize],
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> ReferencePlanningResult<SvgReferenceAnalysis, E> {
    // User element budgets can exceed a native backend's ceiling. Count independently of that
    // ceiling so an observational compatibility failure cannot hide a resource-limit failure.
    let nodes_len = graph.ranges.len();
    let mut states = vec![0_u8; nodes_len];
    let mut expanded_elements = vec![0_usize; nodes_len];
    let mut expanded_depths = vec![0_usize; nodes_len];
    let mut postorder = if DETAILED {
        Vec::with_capacity(nodes_len)
    } else {
        Vec::new()
    };
    #[cfg(test)]
    record_reference_analysis_work(|work| {
        work.postorder_slots += if DETAILED { nodes_len } else { 0 };
    });
    let mut stack = vec![(0_usize, false)];
    let mut traversal_iteration = 0usize;
    let mut dependency_iteration = 0usize;

    while let Some((index, complete)) = stack.pop() {
        checkpoint_loop(traversal_iteration, checkpoint)
            .map_err(ReferencePlanningError::Checkpoint)?;
        traversal_iteration = traversal_iteration.saturating_add(1);
        if complete {
            let is_group = index >= graph.real_nodes;
            let mut elements = if is_group { 0_usize } else { 1 };
            let mut depth = 0_usize;
            for &(dependency, multiplicity) in graph.dependencies(index) {
                checkpoint_loop(dependency_iteration, checkpoint)
                    .map_err(ReferencePlanningError::Checkpoint)?;
                dependency_iteration = dependency_iteration.saturating_add(1);
                let dependency_elements =
                    expanded_elements[dependency].saturating_mul(multiplicity);
                elements = elements.saturating_add(dependency_elements);
                let dependency_depth = expanded_depths[dependency];
                depth = depth.max(if is_group {
                    dependency_depth
                } else {
                    dependency_depth.saturating_add(1)
                });
            }
            expanded_elements[index] = elements;
            expanded_depths[index] = depth;
            states[index] = 2;
            if DETAILED {
                postorder.push(index);
            }
            continue;
        }

        match states[index] {
            0 => {
                states[index] = 1;
                stack.push((index, true));
                for &(dependency, _) in graph.dependencies(index).iter().rev() {
                    checkpoint_loop(dependency_iteration, checkpoint)
                        .map_err(ReferencePlanningError::Checkpoint)?;
                    dependency_iteration = dependency_iteration.saturating_add(1);
                    match states[dependency] {
                        0 => stack.push((dependency, false)),
                        1 => {
                            checkpoint().map_err(ReferencePlanningError::Checkpoint)?;
                            return Err(ReferencePlanningError::Invalid(
                                "same-document SVG expansion references contain a cycle"
                                    .to_string(),
                            ));
                        }
                        2 => {}
                        _ => unreachable!("SVG reference traversal state is bounded"),
                    }
                }
            }
            1 => {
                checkpoint().map_err(ReferencePlanningError::Checkpoint)?;
                return Err(ReferencePlanningError::Invalid(
                    "same-document SVG expansion references contain a cycle".to_string(),
                ));
            }
            2 => {}
            _ => unreachable!("SVG reference traversal state is bounded"),
        }
    }

    let mut raw_element_occurrences = if DETAILED {
        vec![0_usize; nodes_len]
    } else {
        Vec::new()
    };
    #[cfg(test)]
    record_reference_analysis_work(|work| {
        work.occurrence_slots += if DETAILED { nodes_len } else { 0 };
    });
    if DETAILED {
        raw_element_occurrences[0] = 1;
    }
    let application_upper_bound = expanded_elements[0];
    for (iteration, &index) in effects.iter().enumerate() {
        checkpoint_loop(iteration, checkpoint).map_err(ReferencePlanningError::Checkpoint)?;
        // The XML owner includes every real node under the SVG root. After cycle checking,
        // an extra root edge to an effect cannot change non-root costs or topological order.
        debug_assert!(index != 0 && states[index] == 2);
        // Charge each filter/mask/clip-path once per baseline expanded element, preserving
        // the conservative CSS-independent bound without evaluating the graph a second time.
        expanded_elements[0] = expanded_elements[0]
            .saturating_add(expanded_elements[index].saturating_mul(application_upper_bound));
        expanded_depths[0] = expanded_depths[0].max(expanded_depths[index].saturating_add(1));
        if DETAILED {
            raw_element_occurrences[index] =
                raw_element_occurrences[index].saturating_add(application_upper_bound);
        }
    }
    let mut occurrence_dependency_iteration = 0usize;
    for (iteration, index) in postorder.into_iter().rev().enumerate() {
        checkpoint_loop(iteration, checkpoint).map_err(ReferencePlanningError::Checkpoint)?;
        let occurrences = raw_element_occurrences[index];
        for &(dependency, multiplicity) in graph.dependencies(index) {
            checkpoint_loop(occurrence_dependency_iteration, checkpoint)
                .map_err(ReferencePlanningError::Checkpoint)?;
            occurrence_dependency_iteration = occurrence_dependency_iteration.saturating_add(1);
            #[cfg(test)]
            record_reference_analysis_work(|work| {
                work.occurrence_edges += 1;
            });
            let added = occurrences.saturating_mul(multiplicity);
            raw_element_occurrences[dependency] =
                raw_element_occurrences[dependency].saturating_add(added);
        }
    }
    raw_element_occurrences.truncate(graph.real_nodes);

    Ok(SvgReferenceAnalysis {
        structure: SvgStructureMetrics {
            elements: expanded_elements[0],
            max_tree_depth: expanded_depths[0],
        },
        raw_element_occurrences: raw_element_occurrences.into_boxed_slice(),
    })
}

// usvg maps attributes from these namespaces to SVG attribute ids by local name.
fn usvg_consumes_attribute_namespace(namespace: ResolveResult<'_>) -> Result<bool> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "attribute uses an unknown namespace prefix {prefix:?}"
        ))),
        ResolveResult::Unbound => Ok(true),
        ResolveResult::Bound(namespace) => {
            let namespace = namespace.as_ref();
            Ok(namespace == SVG_NAMESPACE
                || namespace == XLINK_NAMESPACE
                || namespace == XML_NAMESPACE)
        }
    }
}

fn validate_namespace(namespace: ResolveResult<'_>, is_root: bool) -> Result<()> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "element uses an unknown namespace prefix {prefix:?}"
        ))),
        ResolveResult::Bound(namespace) if is_root && namespace.as_ref() != SVG_NAMESPACE => Err(
            validation_error("the root element uses a non-SVG namespace"),
        ),
        ResolveResult::Unbound | ResolveResult::Bound(_) => Ok(()),
    }
}

fn reject_unknown_namespace(namespace: ResolveResult<'_>) -> Result<()> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "element uses an unknown namespace prefix {prefix:?}"
        ))),
        ResolveResult::Unbound | ResolveResult::Bound(_) => Ok(()),
    }
}

fn validate_style_text(css: &str, checkpoint: &mut impl FnMut() -> Result<()>) -> Result<()> {
    match validate_resvg_css_stylesheet_with_checkpoints(css, checkpoint) {
        Ok(()) => Ok(()),
        Err(CssValidationError::Invalid(error)) => {
            checkpoint()?;
            Err(validation_error(format!(
                "invalid <style> content: {error}"
            )))
        }
        Err(CssValidationError::Checkpoint(error)) => Err(error),
    }
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess(VALIDATION_PASS, message)
}

#[cfg(test)]
mod tests;
