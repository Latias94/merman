use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

use super::*;

const COLOR_TOLERANCE: u8 = 8;
const REGION_PADDING_PX: i32 = 2;

/// Paint facet bound into one route-local raster observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterPaintCutoverFacet {
    Fill,
    Stroke,
}

impl RasterPaintCutoverFacet {
    const fn id(self) -> &'static [u8] {
        match self {
            Self::Fill => b"fill",
            Self::Stroke => b"stroke",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Fill => 0,
            Self::Stroke => 1,
        }
    }
}

/// SVG channels admitted by a renderer-owned route's raster contract.
///
/// A semantic route can control text fill and line stroke in the same diagram. A
/// partial config override may leave either channel as its only visible consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterPaintCutoverChannels {
    Only(RasterPaintCutoverFacet),
    FillOrStroke,
}

impl RasterPaintCutoverChannels {
    const fn id(self) -> &'static [u8] {
        match self {
            Self::Only(facet) => facet.id(),
            Self::FillOrStroke => b"fill-or-stroke",
        }
    }

    fn terminal_facet(self) -> Result<RasterPaintCutoverFacet> {
        match self {
            Self::Only(facet) => Ok(facet),
            Self::FillOrStroke => Err(ExportError::RasterPaintCutover(
                "renderer terminal bindings require one semantic facet",
            )),
        }
    }
}

/// Renderer-owned mapping from a semantic theme facet to its emitted SVG paint channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RasterPaintSemanticBinding {
    Native,
    FillFromStroke,
    FillAndStrokeFromStroke,
}

impl RasterPaintSemanticBinding {
    const fn id(self) -> &'static [u8] {
        match self {
            Self::Native => b"native",
            Self::FillFromStroke => b"fill-from-stroke",
            Self::FillAndStrokeFromStroke => b"fill-and-stroke-from-stroke",
        }
    }
}

/// Exact SVG terminal selected by the renderer for one raster paint proof.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RasterPaintTerminalBinding {
    terminal_id: String,
    binding: RasterPaintSemanticBinding,
    lifeline_geometry: RasterPaintLifelineGeometry,
}

impl RasterPaintTerminalBinding {
    /// Binds one renderer-owned Sequence lifeline to its finalized SVG `line` terminal.
    ///
    /// Geometry is normalized with the same small-number and near-integer rules used by the SVG
    /// writer. The exporter later reparses the finalized SVG and checks authored coordinates and
    /// final cascade-resolved stroke width, together with `data-et="life-line"` and the absence
    /// of terminal transforms.
    pub fn line_lifeline(
        terminal_id: impl Into<String>,
        binding: RasterPaintSemanticBinding,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke_width: f64,
    ) -> Option<Self> {
        Self::line_lifeline_with_effective_width(
            terminal_id,
            binding,
            x1,
            y1,
            x2,
            y2,
            stroke_width,
            stroke_width,
        )
    }

    pub fn line_lifeline_with_effective_width(
        terminal_id: impl Into<String>,
        binding: RasterPaintSemanticBinding,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        authored_stroke_width: f64,
        effective_stroke_width: f64,
    ) -> Option<Self> {
        let terminal_id = terminal_id.into();
        let lifeline_geometry = RasterPaintLifelineGeometry::new(
            x1,
            y1,
            x2,
            y2,
            authored_stroke_width,
            effective_stroke_width,
        )?;
        (!terminal_id.is_empty()).then_some(Self {
            terminal_id,
            binding,
            lifeline_geometry,
        })
    }

    pub fn terminal_id(&self) -> &str {
        &self.terminal_id
    }

    pub const fn binding(&self) -> RasterPaintSemanticBinding {
        self.binding
    }

    fn update_digest(&self, hasher: &mut Sha256) {
        update_len_prefixed(hasher, self.terminal_id.as_bytes());
        update_len_prefixed(hasher, self.binding.id());
        update_len_prefixed(hasher, b"line");
        update_len_prefixed(hasher, b"life-line");
        self.lifeline_geometry.update_digest(hasher);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RasterPaintLifelineGeometry {
    x1_bits: u64,
    y1_bits: u64,
    x2_bits: u64,
    y2_bits: u64,
    authored_stroke_width_bits: u64,
    effective_stroke_width_bits: u64,
}

impl RasterPaintLifelineGeometry {
    fn new(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        authored_stroke_width: f64,
        effective_stroke_width: f64,
    ) -> Option<Self> {
        let x1 = normalize_terminal_coordinate(x1)?;
        let y1 = normalize_terminal_coordinate(y1)?;
        let x2 = normalize_terminal_coordinate(x2)?;
        let y2 = normalize_terminal_coordinate(y2)?;
        let authored_stroke_width = normalize_terminal_coordinate(authored_stroke_width)?;
        let effective_stroke_width = normalize_terminal_coordinate(effective_stroke_width)?;
        if x1 != x2 || y2 <= y1 || authored_stroke_width <= 0.0 || effective_stroke_width <= 0.0 {
            return None;
        }
        Some(Self {
            x1_bits: x1.to_bits(),
            y1_bits: y1.to_bits(),
            x2_bits: x2.to_bits(),
            y2_bits: y2.to_bits(),
            authored_stroke_width_bits: authored_stroke_width.to_bits(),
            effective_stroke_width_bits: effective_stroke_width.to_bits(),
        })
    }

    fn update_digest(self, hasher: &mut Sha256) {
        hasher.update(self.x1_bits.to_be_bytes());
        hasher.update(self.y1_bits.to_be_bytes());
        hasher.update(self.x2_bits.to_be_bytes());
        hasher.update(self.y2_bits.to_be_bytes());
        hasher.update(self.authored_stroke_width_bits.to_be_bytes());
        hasher.update(self.effective_stroke_width_bits.to_be_bytes());
    }

    fn x1(self) -> f64 {
        f64::from_bits(self.x1_bits)
    }

    fn y1(self) -> f64 {
        f64::from_bits(self.y1_bits)
    }

    fn x2(self) -> f64 {
        f64::from_bits(self.x2_bits)
    }

    fn y2(self) -> f64 {
        f64::from_bits(self.y2_bits)
    }

    fn authored_stroke_width(self) -> f64 {
        f64::from_bits(self.authored_stroke_width_bits)
    }

    fn effective_stroke_width(self) -> f64 {
        f64::from_bits(self.effective_stroke_width_bits)
    }
}

fn normalize_terminal_coordinate(value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let mut value = if value.abs() < 1e-9 { 0.0 } else { value };
    let nearest = value.round();
    if (value - nearest).abs() < 1e-6 {
        value = nearest;
    }
    if value == -0.0 {
        value = 0.0;
    }
    Some(value)
}

/// Opaque exporter-owned evidence for one solid/transparent paint pair.
///
/// The exporter resolves the final SVG through `usvg`, identifies the unique control paint on
/// renderable paths, derives an underlay reference by replacing that control paint with
/// `transparent`, and compares the actual transparent render with that reference. The receipt is
/// deliberately family-neutral; the Merman facade binds it to renderer-owned route receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterPaintCutoverReceipt {
    channels: RasterPaintCutoverChannels,
    control_rgb: [u8; 3],
    solid_source_digest: [u8; 32],
    transparent_source_digest: [u8; 32],
    underlay_source_digest: [u8; 32],
    underlay_path_tree_digest: [u8; 32],
    transparent_path_tree_digest: [u8; 32],
    solid_effect_tree_digest: [u8; 32],
    underlay_effect_tree_digest: [u8; 32],
    transparent_effect_tree_digest: [u8; 32],
    terminal_bindings_digest: [u8; 32],
    transparent_geometry_compatible: bool,
    target_geometry_digest: [u8; 32],
    target_path_count: usize,
    solid_control_pixels: usize,
    changed_control_pixels: usize,
    requested_changed_control_pixels: usize,
    changed_pixels: usize,
    changed_outside_target_pixels: usize,
    unchanged_control_pixels: usize,
    transparent_reference_mismatches: usize,
    digest: [u8; 32],
}

impl RasterPaintCutoverReceipt {
    fn seal(facts: RasterPaintCutoverFacts) -> Option<Self> {
        let mut receipt = Self {
            channels: facts.channels,
            control_rgb: facts.control_rgb,
            solid_source_digest: facts.solid_source_digest,
            transparent_source_digest: facts.transparent_source_digest,
            underlay_source_digest: facts.underlay_source_digest,
            underlay_path_tree_digest: facts.underlay_path_tree_digest,
            transparent_path_tree_digest: facts.transparent_path_tree_digest,
            solid_effect_tree_digest: facts.solid_effect_tree_digest,
            underlay_effect_tree_digest: facts.underlay_effect_tree_digest,
            transparent_effect_tree_digest: facts.transparent_effect_tree_digest,
            terminal_bindings_digest: facts.terminal_bindings_digest,
            transparent_geometry_compatible: facts.transparent_geometry_compatible,
            target_geometry_digest: facts.target_geometry_digest,
            target_path_count: facts.target_path_count,
            solid_control_pixels: facts.solid_control_pixels,
            changed_control_pixels: facts.changed_control_pixels,
            requested_changed_control_pixels: facts.requested_changed_control_pixels,
            changed_pixels: facts.changed_pixels,
            changed_outside_target_pixels: facts.changed_outside_target_pixels,
            unchanged_control_pixels: facts.unchanged_control_pixels,
            transparent_reference_mismatches: facts.transparent_reference_mismatches,
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        receipt.proves_semantics().then_some(receipt)
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub fn proves_semantics(&self) -> bool {
        self.digest != [0; 32]
            && self.solid_source_digest != [0; 32]
            && self.transparent_source_digest != [0; 32]
            && self.underlay_source_digest != [0; 32]
            && self.underlay_path_tree_digest != [0; 32]
            && self.transparent_path_tree_digest != [0; 32]
            && self.solid_effect_tree_digest != [0; 32]
            && self.underlay_effect_tree_digest != [0; 32]
            && self.underlay_effect_tree_digest == self.solid_effect_tree_digest
            && self.transparent_effect_tree_digest == self.underlay_effect_tree_digest
            && self.terminal_bindings_digest != [0; 32]
            && self.transparent_geometry_compatible
            && self.target_geometry_digest != [0; 32]
            && self.target_path_count > 0
            && self.solid_control_pixels > 0
            && self.changed_control_pixels > 0
            && self.requested_changed_control_pixels > 0
            && self.changed_pixels > 0
            && self.changed_outside_target_pixels == 0
            && self.transparent_reference_mismatches == 0
            && self.digest == self.canonical_digest()
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, b"merman.raster-paint-cutover-receipt.v9");
        update_len_prefixed(&mut hasher, self.channels.id());
        hasher.update(self.control_rgb);
        hasher.update(self.solid_source_digest);
        hasher.update(self.transparent_source_digest);
        hasher.update(self.underlay_source_digest);
        hasher.update(self.underlay_path_tree_digest);
        hasher.update(self.transparent_path_tree_digest);
        hasher.update(self.solid_effect_tree_digest);
        hasher.update(self.underlay_effect_tree_digest);
        hasher.update(self.transparent_effect_tree_digest);
        hasher.update(self.terminal_bindings_digest);
        hasher.update([u8::from(self.transparent_geometry_compatible)]);
        hasher.update(self.target_geometry_digest);
        update_usize(&mut hasher, self.target_path_count);
        update_usize(&mut hasher, self.solid_control_pixels);
        update_usize(&mut hasher, self.changed_control_pixels);
        update_usize(&mut hasher, self.requested_changed_control_pixels);
        update_usize(&mut hasher, self.changed_pixels);
        update_usize(&mut hasher, self.changed_outside_target_pixels);
        update_usize(&mut hasher, self.unchanged_control_pixels);
        update_usize(&mut hasher, self.transparent_reference_mismatches);
        hasher.finalize().into()
    }
}

/// Encoded PNG pair and its route-local raster observation.
pub struct EncodedRasterPaintCutoverPair {
    solid_bytes: Vec<u8>,
    solid_report: RasterExportReport,
    transparent_bytes: Vec<u8>,
    transparent_report: RasterExportReport,
    receipt: RasterPaintCutoverReceipt,
}

impl EncodedRasterPaintCutoverPair {
    pub fn into_parts(
        self,
    ) -> (
        (Vec<u8>, RasterExportReport),
        (Vec<u8>, RasterExportReport),
        RasterPaintCutoverReceipt,
    ) {
        (
            (self.solid_bytes, self.solid_report),
            (self.transparent_bytes, self.transparent_report),
            self.receipt,
        )
    }
}

/// Encodes a route witness pair and seals raster-local paint evidence.
///
/// This workspace-only API accepts only terminally validated SVG artifacts. The caller supplies
/// the canonical solid control paint selected by the private KTD17 witness contract; the exporter
/// proves that the paint is visible only inside its resolved path geometry and that the actual
/// transparent output is pixel-identical to the same finalized SVG with that paint removed.
pub fn encode_png_paint_cutover_pair_controlled(
    solid_svg: &ResvgCompatibleSvg,
    transparent_svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
    channels: RasterPaintCutoverChannels,
    control_css: &str,
    terminal_bindings: &[RasterPaintTerminalBinding],
) -> Result<EncodedRasterPaintCutoverPair> {
    export_checkpoint(&control)?;
    let control_color = parse_export_color(control_css)
        .map_err(|_| ExportError::RasterPaintCutover("control paint is not a valid color"))?;
    if !control_color.is_opaque() {
        return Err(ExportError::RasterPaintCutover(
            "control paint must be opaque",
        ));
    }

    let solid_svg = solid_svg.clone();
    let transparent_svg = transparent_svg.clone();
    let options = options.clone();
    let control_css = control_css.to_owned();
    let terminal_bindings = terminal_bindings.to_vec();
    run_recursive_svg_backend(&control, move |backend_control| {
        encode_pair_on_backend_stack(
            &solid_svg,
            &transparent_svg,
            &options,
            backend_control,
            channels,
            &control_css,
            &terminal_bindings,
            [
                control_color.red(),
                control_color.green(),
                control_color.blue(),
            ],
        )
    })
}

fn encode_pair_on_backend_stack(
    solid_svg: &ResvgCompatibleSvg,
    transparent_svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: &OperationControl,
    channels: RasterPaintCutoverChannels,
    control_css: &str,
    terminal_bindings: &[RasterPaintTerminalBinding],
    control_rgb: [u8; 3],
) -> Result<EncodedRasterPaintCutoverPair> {
    let solid_source = native_export_svg(solid_svg);
    let transparent_source = native_export_svg(transparent_svg);
    let underlay_source = if terminal_bindings.is_empty() {
        replace_control_paint(solid_source, control_css, control_rgb)?
    } else {
        suppress_renderer_terminals(solid_source, channels.terminal_facet()?, terminal_bindings)?
    };
    if !terminal_bindings.is_empty() {
        validate_renderer_terminals(
            transparent_source,
            channels.terminal_facet()?,
            terminal_bindings,
        )?;
    }

    let solid = prepare_raster_source_on_backend_stack(solid_svg, solid_source, options, control)?;
    let solid_placement = RasterPlacement::from_prepared(&solid);
    let solid_tree = observe_paint_tree(&solid.tree, control_rgb, channels, terminal_bindings)?;
    if solid_tree.targets.is_empty() {
        return Err(ExportError::RasterPaintCutover(
            "solid SVG contains no renderable control-painted path",
        ));
    }
    if solid_tree.requested_targets.is_empty() {
        return Err(ExportError::RasterPaintCutover(
            "solid SVG contains no control-painted path for the requested facet",
        ));
    }
    let solid_report = solid.report_for_output(RasterOutputKind::Png);
    let solid_pixmap = solid.render_pixmap(solid.matte, control)?;
    // The rendered pixmap and owned observation are the only solid-side values needed after
    // preparation. Release the parsed tree before preparing the underlay and transparent trees.
    drop(solid);
    let solid_bytes = solid_pixmap
        .encode_png()
        .map_err(|_| ExportError::PngEncode)?;
    export_checkpoint(control)?;

    let underlay =
        prepare_raster_source_on_backend_stack(solid_svg, &underlay_source, options, control)?;
    require_same_placement(solid_placement, RasterPlacement::from_prepared(&underlay))?;
    let underlay_tree =
        observe_paint_tree(&underlay.tree, control_rgb, channels, terminal_bindings)?;
    require_no_opaque_control_targets(&underlay_tree)?;
    require_solid_underlay_path_compatibility(&solid_tree, &underlay_tree)?;
    let underlay_pixmap = underlay.render_pixmap(underlay.matte, control)?;
    drop(underlay);

    let solid_delta = observe_solid_delta(
        &solid_pixmap,
        &underlay_pixmap,
        &solid_tree.targets,
        &solid_tree.requested_targets,
        terminal_bindings,
        solid_placement,
    )?;
    drop(solid_pixmap);

    let transparent = prepare_raster_source_on_backend_stack(
        transparent_svg,
        transparent_source,
        options,
        control,
    )?;
    require_same_placement(
        solid_placement,
        RasterPlacement::from_prepared(&transparent),
    )?;
    let transparent_tree =
        observe_paint_tree(&transparent.tree, control_rgb, channels, terminal_bindings)?;
    require_no_opaque_control_targets(&transparent_tree)?;
    if transparent_tree.effect_tree_digest != underlay_tree.effect_tree_digest {
        return Err(ExportError::RasterPaintCutover(
            "underlay and transparent SVG effect contexts differ",
        ));
    }
    require_transparent_path_compatibility(&underlay_tree, &transparent_tree, &solid_tree)?;
    let transparent_report = transparent.report_for_output(RasterOutputKind::Png);
    let transparent_pixmap = transparent.render_pixmap(transparent.matte, control)?;
    drop(transparent);
    let transparent_reference_mismatches = transparent_pixmap
        .pixels()
        .iter()
        .zip(underlay_pixmap.pixels())
        .filter(|(actual, expected)| actual != expected)
        .count();
    if transparent_reference_mismatches != 0 {
        return Err(ExportError::RasterPaintCutover(
            "transparent PNG differs from the solid SVG underlay reference",
        ));
    }
    drop(underlay_pixmap);
    let transparent_bytes = transparent_pixmap
        .encode_png()
        .map_err(|_| ExportError::PngEncode)?;
    export_checkpoint(control)?;

    let receipt = RasterPaintCutoverReceipt::seal(RasterPaintCutoverFacts {
        channels,
        control_rgb,
        solid_source_digest: Sha256::digest(solid_source.as_bytes()).into(),
        transparent_source_digest: Sha256::digest(transparent_source.as_bytes()).into(),
        underlay_source_digest: Sha256::digest(underlay_source.as_bytes()).into(),
        underlay_path_tree_digest: underlay_tree.path_tree_digest,
        transparent_path_tree_digest: transparent_tree.path_tree_digest,
        solid_effect_tree_digest: solid_tree.effect_tree_digest,
        underlay_effect_tree_digest: underlay_tree.effect_tree_digest,
        transparent_effect_tree_digest: transparent_tree.effect_tree_digest,
        terminal_bindings_digest: solid_tree.terminal_bindings_digest,
        transparent_geometry_compatible: true,
        target_geometry_digest: solid_tree.target_geometry_digest,
        target_path_count: solid_tree.targets.len(),
        solid_control_pixels: solid_delta.control_pixels,
        changed_pixels: solid_delta.changed_pixels,
        changed_control_pixels: solid_delta.changed_control_pixels,
        requested_changed_control_pixels: solid_delta.requested_changed_control_pixels,
        changed_outside_target_pixels: solid_delta.changed_outside_target_pixels,
        unchanged_control_pixels: solid_delta.unchanged_control_pixels,
        transparent_reference_mismatches,
    })
    .ok_or(ExportError::RasterPaintCutover(
        "raster paint facts did not satisfy the receipt contract",
    ))?;

    Ok(EncodedRasterPaintCutoverPair {
        solid_bytes,
        solid_report,
        transparent_bytes,
        transparent_report,
        receipt,
    })
}

fn replace_control_paint(source: &str, control_css: &str, control_rgb: [u8; 3]) -> Result<String> {
    let [red, green, blue] = control_rgb;
    let variants = [
        control_css.to_owned(),
        format!("#{red:02x}{green:02x}{blue:02x}"),
        format!("#{red:02X}{green:02X}{blue:02X}"),
        format!("rgb({red}, {green}, {blue})"),
        format!("rgb({red},{green},{blue})"),
        format!("rgb({red} {green} {blue})"),
        format!("rgba({red}, {green}, {blue}, 1)"),
        format!("rgba({red}, {green}, {blue}, 1.0)"),
    ];
    if !variants.iter().any(|variant| source.contains(variant)) {
        return Err(ExportError::RasterPaintCutover(
            "solid SVG does not contain the canonical control paint token",
        ));
    }
    let mut underlay = source.to_owned();
    for variant in variants {
        underlay = underlay.replace(&variant, "transparent");
    }
    Ok(underlay)
}

fn suppress_renderer_terminals(
    source: &str,
    facet: RasterPaintCutoverFacet,
    terminal_bindings: &[RasterPaintTerminalBinding],
) -> Result<String> {
    process_renderer_terminals(source, facet, terminal_bindings, true)?.ok_or(
        ExportError::RasterPaintCutover("failed to materialize raster proof underlay"),
    )
}

fn validate_renderer_terminals(
    source: &str,
    facet: RasterPaintCutoverFacet,
    terminal_bindings: &[RasterPaintTerminalBinding],
) -> Result<()> {
    process_renderer_terminals(source, facet, terminal_bindings, false).map(|_| ())
}

struct RendererTerminalXmlState {
    terminal: RasterPaintTerminalBinding,
    property: &'static str,
    match_count: usize,
}

fn process_renderer_terminals(
    source: &str,
    facet: RasterPaintCutoverFacet,
    terminal_bindings: &[RasterPaintTerminalBinding],
    write_underlay: bool,
) -> Result<Option<String>> {
    use quick_xml::events::Event;
    use quick_xml::{Reader, Writer};

    let mut terminals = BTreeMap::<String, RendererTerminalXmlState>::new();
    for terminal in terminal_bindings {
        let property = match (terminal.binding, facet) {
            (RasterPaintSemanticBinding::Native, RasterPaintCutoverFacet::Fill) => "fill",
            (RasterPaintSemanticBinding::Native, RasterPaintCutoverFacet::Stroke)
            | (RasterPaintSemanticBinding::FillFromStroke, RasterPaintCutoverFacet::Fill)
            | (RasterPaintSemanticBinding::FillAndStrokeFromStroke, _) => "stroke",
            (RasterPaintSemanticBinding::FillFromStroke, RasterPaintCutoverFacet::Stroke) => {
                return Err(ExportError::RasterPaintCutover(
                    "renderer terminal binding does not admit the requested stroke facet",
                ));
            }
        };
        if terminals
            .insert(
                terminal.terminal_id.clone(),
                RendererTerminalXmlState {
                    terminal: terminal.clone(),
                    property,
                    match_count: 0,
                },
            )
            .is_some()
        {
            return Err(ExportError::RasterPaintCutover(
                "renderer terminal bindings contain a duplicate id",
            ));
        }
    }

    let mut reader = Reader::from_str(source);
    reader.config_mut().check_end_names = true;
    let mut writer = write_underlay.then(|| Writer::new(Vec::with_capacity(source.len())));
    let mut transform_stack = Vec::<bool>::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|_| ExportError::RasterPaintCutover("finalized SVG is not well-formed XML"))?;
        let ancestor_has_transform = transform_stack.last().copied().unwrap_or(false);
        let event = match event {
            Event::Start(element) => {
                let (element, has_transform) = inspect_terminal_element(
                    element,
                    &mut terminals,
                    ancestor_has_transform,
                    write_underlay,
                )?;
                transform_stack.push(ancestor_has_transform || has_transform);
                Event::Start(element)
            }
            Event::Empty(element) => {
                let (element, _) = inspect_terminal_element(
                    element,
                    &mut terminals,
                    ancestor_has_transform,
                    write_underlay,
                )?;
                Event::Empty(element)
            }
            Event::End(element) => {
                transform_stack
                    .pop()
                    .ok_or(ExportError::RasterPaintCutover(
                        "finalized SVG contains an unmatched closing element",
                    ))?;
                Event::End(element)
            }
            Event::Eof => break,
            other => other,
        };
        if let Some(writer) = writer.as_mut() {
            writer.write_event(event).map_err(|_| {
                ExportError::RasterPaintCutover("failed to materialize raster proof underlay")
            })?;
        }
    }

    if !transform_stack.is_empty() {
        return Err(ExportError::RasterPaintCutover(
            "finalized SVG contains an unclosed element",
        ));
    }
    if terminals.values().any(|state| state.match_count != 1) {
        return Err(ExportError::RasterPaintCutover(
            "renderer terminal id is missing or duplicated in finalized SVG",
        ));
    }
    writer
        .map(|writer| {
            String::from_utf8(writer.into_inner()).map_err(|_| {
                ExportError::RasterPaintCutover("raster proof underlay is not valid UTF-8")
            })
        })
        .transpose()
}

fn inspect_terminal_element(
    element: quick_xml::events::BytesStart<'_>,
    terminals: &mut BTreeMap<String, RendererTerminalXmlState>,
    ancestor_has_transform: bool,
    rewrite_style: bool,
) -> Result<(quick_xml::events::BytesStart<'static>, bool)> {
    use quick_xml::XmlVersion;

    let name = element.name().as_ref().to_owned();
    let mut attributes = Vec::<(String, String)>::new();
    let mut terminal_id = None::<String>;
    let mut has_transform = false;
    for attribute in element.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| {
            ExportError::RasterPaintCutover("finalized SVG contains an invalid attribute")
        })?;
        let key = attribute.key.as_ref().to_owned();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| ExportError::RasterPaintCutover("SVG attribute value is not valid XML"))?
            .into_owned();
        if key == "id" && terminals.contains_key(&value) {
            terminal_id = Some(value.clone());
        }
        has_transform |= key == "transform";
        attributes.push((key, value));
    }

    if let Some(terminal_id) = terminal_id {
        let state = terminals
            .get_mut(&terminal_id)
            .expect("terminal identity was checked above");
        state.match_count = state.match_count.saturating_add(1);
        if state.match_count != 1 {
            return Err(ExportError::RasterPaintCutover(
                "renderer terminal id is duplicated in finalized SVG",
            ));
        }
        validate_lifeline_terminal(
            &name,
            &attributes,
            ancestor_has_transform || has_transform,
            state.terminal.lifeline_geometry,
        )?;
        if rewrite_style {
            let override_declaration = format!("{}:transparent !important;", state.property);
            if let Some((_, style)) = attributes.iter_mut().find(|(key, _)| key == "style") {
                if !style.trim().is_empty() && !style.trim_end().ends_with(';') {
                    style.push(';');
                }
                style.push_str(&override_declaration);
            } else {
                attributes.push(("style".to_string(), override_declaration));
            }
        }
    }

    let mut rewritten = quick_xml::events::BytesStart::new(name);
    for (key, value) in &attributes {
        rewritten.push_attribute((key.as_str(), value.as_str()));
    }
    Ok((rewritten, has_transform))
}

fn validate_lifeline_terminal(
    element_name: &str,
    attributes: &[(String, String)],
    has_transform: bool,
    expected_geometry: RasterPaintLifelineGeometry,
) -> Result<()> {
    if element_name != "line" {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline terminal is not an SVG line",
        ));
    }
    if has_transform {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline terminal is transformed",
        ));
    }
    if terminal_attribute(attributes, "data-et") != Some("life-line") {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline terminal has the wrong semantic role",
        ));
    }
    if let Some(style) = terminal_attribute(attributes, "style") {
        if style_overrides_lifeline_geometry(style)? {
            return Err(ExportError::RasterPaintCutover(
                "renderer lifeline terminal has an inline style that can override geometry",
            ));
        }
    }
    let observed_geometry = RasterPaintLifelineGeometry::new(
        parse_terminal_length(attributes, "x1", false)?,
        parse_terminal_length(attributes, "y1", false)?,
        parse_terminal_length(attributes, "x2", false)?,
        parse_terminal_length(attributes, "y2", false)?,
        parse_terminal_length(attributes, "stroke-width", true)?,
        expected_geometry.effective_stroke_width(),
    )
    .ok_or(ExportError::RasterPaintCutover(
        "renderer lifeline terminal has invalid geometry",
    ))?;
    if observed_geometry.authored_stroke_width() != expected_geometry.authored_stroke_width()
        || observed_geometry.x1() != expected_geometry.x1()
        || observed_geometry.y1() != expected_geometry.y1()
        || observed_geometry.x2() != expected_geometry.x2()
        || observed_geometry.y2() != expected_geometry.y2()
    {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline terminal geometry differs from its sealed writer fact",
        ));
    }
    Ok(())
}

fn terminal_attribute<'a>(attributes: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
}

fn style_overrides_lifeline_geometry(style: &str) -> Result<bool> {
    use cssparser::{Delimiter, Parser};

    let mut parser = Parser::new(style);
    while !parser.is_exhausted() {
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;
            while declaration.next_including_whitespace().is_ok() {}
            Ok::<_, cssparser::ParseError<()>>(
                property.eq_ignore_ascii_case("transform")
                    || property.eq_ignore_ascii_case("x1")
                    || property.eq_ignore_ascii_case("y1")
                    || property.eq_ignore_ascii_case("x2")
                    || property.eq_ignore_ascii_case("y2")
                    || property.eq_ignore_ascii_case("stroke-width"),
            )
        });
        match declaration {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(_) => {
                return Err(ExportError::RasterPaintCutover(
                    "renderer lifeline terminal has an invalid inline style",
                ));
            }
        }
    }
    Ok(false)
}

fn parse_terminal_length(
    attributes: &[(String, String)],
    key: &str,
    allow_px_suffix: bool,
) -> Result<f64> {
    let raw = terminal_attribute(attributes, key).ok_or(ExportError::RasterPaintCutover(
        "renderer lifeline terminal is missing a required geometry attribute",
    ))?;
    let raw = raw.trim();
    let raw = if allow_px_suffix {
        raw.strip_suffix("px").unwrap_or(raw).trim()
    } else {
        raw
    };
    let value = raw.parse::<f64>().map_err(|_| {
        ExportError::RasterPaintCutover(
            "renderer lifeline terminal has a non-numeric geometry attribute",
        )
    })?;
    normalize_terminal_coordinate(value).ok_or(ExportError::RasterPaintCutover(
        "renderer lifeline terminal has a non-finite geometry attribute",
    ))
}

#[derive(Clone, Copy, PartialEq)]
struct RasterPlacement {
    width_px: u32,
    height_px: u32,
    effective_scale_bits: u64,
    min_x_bits: u32,
    min_y_bits: u32,
    translate_min_to_origin: bool,
}

impl RasterPlacement {
    fn from_prepared(prepared: &PreparedRaster) -> Self {
        Self {
            width_px: prepared.report.raster.width_px,
            height_px: prepared.report.raster.height_px,
            effective_scale_bits: prepared.report.raster.effective_scale.to_bits(),
            min_x_bits: prepared.geometry.min_x.to_bits(),
            min_y_bits: prepared.geometry.min_y.to_bits(),
            translate_min_to_origin: prepared.translate_min_to_origin,
        }
    }

    fn effective_scale(self) -> f64 {
        f64::from_bits(self.effective_scale_bits)
    }

    fn min_x(self) -> f32 {
        f32::from_bits(self.min_x_bits)
    }

    fn min_y(self) -> f32 {
        f32::from_bits(self.min_y_bits)
    }
}

fn require_same_placement(left: RasterPlacement, right: RasterPlacement) -> Result<()> {
    if left != right {
        return Err(ExportError::RasterPaintCutover(
            "solid and transparent raster placements differ",
        ));
    }
    Ok(())
}

struct PaintTreeObservation {
    path_tree_digest: [u8; 32],
    shape_tree_digest: [u8; 32],
    effect_tree_digest: [u8; 32],
    terminal_bindings_digest: [u8; 32],
    paths: Vec<PathObservation>,
    target_facets_by_path: Vec<[bool; 2]>,
    target_geometry_digest: [u8; 32],
    requested_targets: Vec<TargetPath>,
    targets: Vec<TargetPath>,
}

#[derive(Debug, Clone)]
struct PathObservation {
    path_digest: [u8; 32],
    shape_digest: [u8; 32],
    fill_geometry_digest: [u8; 32],
    stroke_geometry_digest: Option<[u8; 32]>,
    fill_region_bits: [u32; 4],
    stroke_region_bits: [u32; 4],
    visible: bool,
    fill: Option<PaintObservation>,
    stroke: Option<PaintObservation>,
    semantic_paint_binding: RasterPaintSemanticBinding,
    terminal_id: Option<String>,
    selected_by_renderer: bool,
    color_transfer: RasterPaintColorTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaintObservation {
    rgb: Option<[u8; 3]>,
    opacity_bits: u32,
}

impl PaintObservation {
    fn opacity(self) -> f32 {
        f32::from_bits(self.opacity_bits)
    }
}

#[derive(Debug, Clone)]
struct TargetPath {
    terminal_id: Option<String>,
    region_bits: [u32; 4],
    rendered_control_rgb: [u8; 3],
}

impl TargetPath {
    fn region(&self) -> [f32; 4] {
        self.region_bits.map(f32::from_bits)
    }
}

/// The only SVG group effect that route-local raster paint proofs may model.
///
/// `brightness(120%)` lowers to a single sRGB `feComponentTransfer` in the finalized SVG.
/// It changes paint values without changing geometry, alpha, or compositing. Everything else
/// remains deliberately unsupported so this module does not become a general filter engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RasterPaintColorTransfer {
    Identity,
    Brightness120Srgb,
}

impl RasterPaintColorTransfer {
    fn rendered_rgb(self, rgb: [u8; 3]) -> [u8; 3] {
        match self {
            Self::Identity => rgb,
            Self::Brightness120Srgb => {
                rgb.map(|channel| (f32::from(channel) * 1.2).clamp(0.0, f32::from(u8::MAX)) as u8)
            }
        }
    }

    const fn compose(self, child: Self) -> Option<Self> {
        match (self, child) {
            (Self::Identity, effect) | (effect, Self::Identity) => Some(effect),
            (Self::Brightness120Srgb, Self::Brightness120Srgb) => None,
        }
    }

    const fn is_identity(self) -> bool {
        matches!(self, Self::Identity)
    }

    const fn digest_tag(self) -> u8 {
        match self {
            Self::Identity => 0,
            Self::Brightness120Srgb => 1,
        }
    }
}

impl PathObservation {
    fn paint(&self, facet: RasterPaintCutoverFacet) -> Option<PaintObservation> {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                RasterPaintSemanticBinding::Native => self.fill,
                RasterPaintSemanticBinding::FillFromStroke
                | RasterPaintSemanticBinding::FillAndStrokeFromStroke => self.stroke,
            },
            RasterPaintCutoverFacet::Stroke => match self.semantic_paint_binding {
                RasterPaintSemanticBinding::FillFromStroke => None,
                RasterPaintSemanticBinding::Native
                | RasterPaintSemanticBinding::FillAndStrokeFromStroke => self.stroke,
            },
        }
    }

    fn geometry(&self, facet: RasterPaintCutoverFacet) -> Option<[u8; 32]> {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                RasterPaintSemanticBinding::Native => Some(self.fill_geometry_digest),
                RasterPaintSemanticBinding::FillFromStroke
                | RasterPaintSemanticBinding::FillAndStrokeFromStroke => {
                    self.stroke_geometry_digest
                }
            },
            RasterPaintCutoverFacet::Stroke => match self.semantic_paint_binding {
                RasterPaintSemanticBinding::FillFromStroke => None,
                RasterPaintSemanticBinding::Native
                | RasterPaintSemanticBinding::FillAndStrokeFromStroke => {
                    self.stroke_geometry_digest
                }
            },
        }
    }

    fn region(&self, facet: RasterPaintCutoverFacet) -> [u32; 4] {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                RasterPaintSemanticBinding::Native => self.fill_region_bits,
                RasterPaintSemanticBinding::FillFromStroke
                | RasterPaintSemanticBinding::FillAndStrokeFromStroke => self.stroke_region_bits,
            },
            RasterPaintCutoverFacet::Stroke => self.stroke_region_bits,
        }
    }
}

fn observe_paint_tree(
    tree: &usvg::Tree,
    control_rgb: [u8; 3],
    requested_channels: RasterPaintCutoverChannels,
    terminal_bindings: &[RasterPaintTerminalBinding],
) -> Result<PaintTreeObservation> {
    let mut paths = Vec::new();
    let mut terminal_resolver = TerminalBindingResolver::new(terminal_bindings)?;
    let mut effect_hasher = Sha256::new();
    update_len_prefixed(&mut effect_hasher, b"merman.raster-paint-effect-tree.v2");
    hash_tree_clip_paths(tree, &mut effect_hasher)?;
    collect_group_paths(
        tree.root(),
        &mut paths,
        &mut terminal_resolver,
        &mut effect_hasher,
    )?;
    let terminal_bindings_digest = terminal_resolver.finish()?;
    let effect_tree_digest = effect_hasher.finalize().into();
    let mut path_tree_hasher = Sha256::new();
    update_len_prefixed(&mut path_tree_hasher, b"merman.raster-paint-path-tree.v1");
    update_usize(&mut path_tree_hasher, paths.len());
    for path in &paths {
        path_tree_hasher.update(path.path_digest);
    }
    let path_tree_digest = path_tree_hasher.finalize().into();
    let mut shape_tree_hasher = Sha256::new();
    update_len_prefixed(&mut shape_tree_hasher, b"merman.raster-paint-shape-tree.v1");
    update_usize(&mut shape_tree_hasher, paths.len());
    for path in &paths {
        shape_tree_hasher.update(path.shape_digest);
    }
    let shape_tree_digest = shape_tree_hasher.finalize().into();

    let mut targets = Vec::new();
    let mut target_facets_by_path = vec![[false; 2]; paths.len()];
    let mut target_hasher = Sha256::new();
    update_len_prefixed(
        &mut target_hasher,
        b"merman.raster-paint-target-geometry.v2",
    );
    let mut requested_targets = Vec::new();
    for (path_index, path) in paths.iter().enumerate() {
        if !path.selected_by_renderer {
            continue;
        }
        let fill_control =
            paint_is_opaque_control(path.paint(RasterPaintCutoverFacet::Fill), control_rgb);
        let stroke_control =
            paint_is_opaque_control(path.paint(RasterPaintCutoverFacet::Stroke), control_rgb);
        if !fill_control && !stroke_control {
            continue;
        }
        let region_bits = union_control_regions(
            fill_control,
            path.region(RasterPaintCutoverFacet::Fill),
            stroke_control,
            path.region(RasterPaintCutoverFacet::Stroke),
        );
        update_usize(&mut target_hasher, path_index);
        target_hasher.update(path.path_digest);
        if let Some(terminal_id) = &path.terminal_id {
            update_len_prefixed(&mut target_hasher, b"terminal-owner");
            update_len_prefixed(&mut target_hasher, terminal_id.as_bytes());
        }
        target_hasher.update([u8::from(fill_control), u8::from(stroke_control)]);
        for coordinate in region_bits {
            target_hasher.update(coordinate.to_be_bytes());
        }
        let rendered_control_rgb = path.color_transfer.rendered_rgb(control_rgb);
        target_hasher.update(rendered_control_rgb);
        targets.push(TargetPath {
            terminal_id: path.terminal_id.clone(),
            region_bits,
            rendered_control_rgb,
        });
        target_facets_by_path[path_index] = [fill_control, stroke_control];

        let requested_region_bits = match requested_channels {
            RasterPaintCutoverChannels::FillOrStroke => Some(region_bits),
            RasterPaintCutoverChannels::Only(RasterPaintCutoverFacet::Fill) if fill_control => {
                Some(path.region(RasterPaintCutoverFacet::Fill))
            }
            RasterPaintCutoverChannels::Only(RasterPaintCutoverFacet::Stroke) if stroke_control => {
                Some(path.region(RasterPaintCutoverFacet::Stroke))
            }
            RasterPaintCutoverChannels::Only(_) => None,
        };
        if let Some(requested_region_bits) = requested_region_bits {
            requested_targets.push(TargetPath {
                terminal_id: path.terminal_id.clone(),
                region_bits: requested_region_bits,
                rendered_control_rgb,
            });
        }
    }
    update_usize(&mut target_hasher, targets.len());

    Ok(PaintTreeObservation {
        path_tree_digest,
        shape_tree_digest,
        effect_tree_digest,
        terminal_bindings_digest,
        paths,
        target_facets_by_path,
        target_geometry_digest: target_hasher.finalize().into(),
        requested_targets,
        targets,
    })
}

fn hash_tree_clip_paths(tree: &usvg::Tree, hasher: &mut Sha256) -> Result<()> {
    update_len_prefixed(hasher, b"tree-clip-paths");
    update_usize(hasher, tree.clip_paths().len());
    for clip_path in tree.clip_paths() {
        update_len_prefixed(hasher, clip_path.id().as_bytes());
        hash_clip_path(clip_path, hasher)?;
    }
    Ok(())
}

fn require_solid_underlay_path_compatibility(
    solid: &PaintTreeObservation,
    underlay: &PaintTreeObservation,
) -> Result<()> {
    require_same_path_shapes(solid, underlay)?;
    if solid.terminal_bindings_digest != underlay.terminal_bindings_digest {
        return Err(ExportError::RasterPaintCutover(
            "solid and underlay renderer terminal bindings differ",
        ));
    }
    if solid.effect_tree_digest != underlay.effect_tree_digest {
        return Err(ExportError::RasterPaintCutover(
            "solid and underlay SVG effect contexts differ",
        ));
    }
    for (path_index, (solid_path, underlay_path)) in
        solid.paths.iter().zip(&underlay.paths).enumerate()
    {
        if solid_path.visible != underlay_path.visible {
            return Err(ExportError::RasterPaintCutover(
                "solid and underlay path visibility differs",
            ));
        }
        let target_facets = solid.target_facets_by_path[path_index];
        for facet in [
            RasterPaintCutoverFacet::Fill,
            RasterPaintCutoverFacet::Stroke,
        ] {
            let target = target_facets[facet.index()];
            let solid_state = paint_state(solid_path.paint(facet));
            let underlay_state = paint_state(underlay_path.paint(facet));
            if solid_path.geometry(facet) != underlay_path.geometry(facet) {
                return Err(ExportError::RasterPaintCutover(
                    "solid and underlay paint geometry differs",
                ));
            }
            if target {
                if solid_state != PaintState::Active || underlay_state != PaintState::Suppressed {
                    return Err(ExportError::RasterPaintCutover(
                        "solid control paint was not reduced to a suppressed underlay paint",
                    ));
                }
            } else if solid_state != underlay_state
                || solid_path.paint(facet) != underlay_path.paint(facet)
            {
                return Err(ExportError::RasterPaintCutover(
                    "solid and underlay non-target paint differs",
                ));
            }
        }
    }
    Ok(())
}

fn require_transparent_path_compatibility(
    underlay: &PaintTreeObservation,
    transparent: &PaintTreeObservation,
    solid: &PaintTreeObservation,
) -> Result<()> {
    require_same_path_shapes(underlay, transparent)?;
    if underlay.terminal_bindings_digest != transparent.terminal_bindings_digest
        || underlay.terminal_bindings_digest != solid.terminal_bindings_digest
    {
        return Err(ExportError::RasterPaintCutover(
            "solid and transparent renderer terminal bindings differ",
        ));
    }
    for (path_index, (underlay_path, transparent_path)) in
        underlay.paths.iter().zip(&transparent.paths).enumerate()
    {
        let target_facets = solid.target_facets_by_path[path_index];
        let mut target_omission = false;
        for facet in [
            RasterPaintCutoverFacet::Fill,
            RasterPaintCutoverFacet::Stroke,
        ] {
            let target = target_facets[facet.index()];
            let underlay_state = paint_state(underlay_path.paint(facet));
            let transparent_state = paint_state(transparent_path.paint(facet));
            if target {
                if underlay_state != PaintState::Suppressed {
                    return Err(ExportError::RasterPaintCutover(
                        "underlay target paint is not suppressed",
                    ));
                }
                match transparent_state {
                    PaintState::Suppressed => {
                        if underlay_path.geometry(facet) != transparent_path.geometry(facet) {
                            return Err(ExportError::RasterPaintCutover(
                                "transparent target paint geometry differs",
                            ));
                        }
                    }
                    PaintState::Absent => target_omission = true,
                    PaintState::Active | PaintState::Partial => {
                        return Err(ExportError::RasterPaintCutover(
                            "transparent output retained a target paint",
                        ));
                    }
                }
            } else if underlay_state != transparent_state
                || underlay_path.geometry(facet) != transparent_path.geometry(facet)
                || underlay_path.paint(facet) != transparent_path.paint(facet)
            {
                return Err(ExportError::RasterPaintCutover(
                    "transparent output changed non-target paint geometry",
                ));
            }
        }

        if underlay_path.visible != transparent_path.visible
            && !(target_omission
                && underlay_path.visible
                && !transparent_path.visible
                && transparent_path
                    .paint(RasterPaintCutoverFacet::Fill)
                    .is_none()
                && transparent_path
                    .paint(RasterPaintCutoverFacet::Stroke)
                    .is_none())
        {
            return Err(ExportError::RasterPaintCutover(
                "transparent output changed path visibility",
            ));
        }
    }
    Ok(())
}

fn require_same_path_shapes(
    left: &PaintTreeObservation,
    right: &PaintTreeObservation,
) -> Result<()> {
    if left.paths.len() != right.paths.len() || left.shape_tree_digest != right.shape_tree_digest {
        return Err(ExportError::RasterPaintCutover(
            "raster path tree shape differs",
        ));
    }
    if left
        .paths
        .iter()
        .zip(&right.paths)
        .any(|(left, right)| left.shape_digest != right.shape_digest)
    {
        return Err(ExportError::RasterPaintCutover(
            "raster path geometry differs",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaintState {
    Absent,
    Suppressed,
    Active,
    Partial,
}

fn paint_state(paint: Option<PaintObservation>) -> PaintState {
    match paint {
        None => PaintState::Absent,
        Some(paint) if paint.opacity() == 0.0 => PaintState::Suppressed,
        Some(paint) if paint.opacity() == 1.0 => PaintState::Active,
        Some(_) => PaintState::Partial,
    }
}

fn union_control_regions(
    fill_control: bool,
    fill_region_bits: [u32; 4],
    stroke_control: bool,
    stroke_region_bits: [u32; 4],
) -> [u32; 4] {
    let first = match (fill_control, stroke_control) {
        (true, _) => fill_region_bits,
        (false, true) => stroke_region_bits,
        (false, false) => return [0; 4],
    };
    let [first_left, first_top, first_width, first_height] = first.map(f32::from_bits);
    let mut left = first_left;
    let mut top = first_top;
    let mut right = first_left + first_width;
    let mut bottom = first_top + first_height;
    if fill_control && stroke_control {
        let [region_left, region_top, region_width, region_height] =
            stroke_region_bits.map(f32::from_bits);
        left = left.min(region_left);
        top = top.min(region_top);
        right = right.max(region_left + region_width);
        bottom = bottom.max(region_top + region_height);
    }
    [left, top, right - left, bottom - top].map(f32::to_bits)
}

struct TerminalBindingState {
    terminal: RasterPaintTerminalBinding,
    matches: usize,
}

#[derive(Clone)]
struct ResolvedTerminalBinding {
    terminal_id: String,
    binding: RasterPaintSemanticBinding,
    lifeline_geometry: RasterPaintLifelineGeometry,
}

struct TerminalBindingResolver {
    bindings: BTreeMap<String, TerminalBindingState>,
}

impl TerminalBindingResolver {
    fn new(bindings: &[RasterPaintTerminalBinding]) -> Result<Self> {
        let mut canonical = BTreeMap::new();
        for binding in bindings {
            if binding.terminal_id.is_empty()
                || canonical
                    .insert(
                        binding.terminal_id.clone(),
                        TerminalBindingState {
                            terminal: binding.clone(),
                            matches: 0,
                        },
                    )
                    .is_some()
            {
                return Err(ExportError::RasterPaintCutover(
                    "renderer terminal bindings contain an empty or duplicate id",
                ));
            }
        }
        Ok(Self {
            bindings: canonical,
        })
    }

    fn has_explicit_bindings(&self) -> bool {
        !self.bindings.is_empty()
    }

    fn resolve(&mut self, id: &str) -> Result<Option<ResolvedTerminalBinding>> {
        let Some(state) = self.bindings.get_mut(id) else {
            return Ok(None);
        };
        state.matches = state.matches.saturating_add(1);
        if state.matches != 1 {
            return Err(ExportError::RasterPaintCutover(
                "renderer terminal id is not unique in finalized SVG",
            ));
        }
        Ok(Some(ResolvedTerminalBinding {
            terminal_id: id.to_owned(),
            binding: state.terminal.binding,
            lifeline_geometry: state.terminal.lifeline_geometry,
        }))
    }

    fn finish(self) -> Result<[u8; 32]> {
        if self.bindings.values().any(|state| state.matches != 1) {
            return Err(ExportError::RasterPaintCutover(
                "renderer terminal id is missing from finalized SVG",
            ));
        }
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, b"merman.raster-paint-terminal-bindings.v2");
        update_usize(&mut hasher, self.bindings.len());
        for state in self.bindings.into_values() {
            state.terminal.update_digest(&mut hasher);
        }
        Ok(hasher.finalize().into())
    }
}

fn collect_group_paths(
    group: &usvg::Group,
    paths: &mut Vec<PathObservation>,
    terminal_resolver: &mut TerminalBindingResolver,
    effect_hasher: &mut Sha256,
) -> Result<()> {
    let select_all = !terminal_resolver.has_explicit_bindings();
    collect_group_paths_with_transform(
        group,
        paths,
        None,
        RasterPaintColorTransfer::Identity,
        RasterPaintSemanticBinding::Native,
        None,
        select_all,
        terminal_resolver,
        effect_hasher,
    )
}

fn collect_group_paths_with_transform(
    group: &usvg::Group,
    paths: &mut Vec<PathObservation>,
    extra_transform: Option<tiny_skia::Transform>,
    inherited_color_transfer: RasterPaintColorTransfer,
    inherited_binding: RasterPaintSemanticBinding,
    inherited_terminal: Option<ResolvedTerminalBinding>,
    inherited_selection: bool,
    terminal_resolver: &mut TerminalBindingResolver,
    effect_hasher: &mut Sha256,
) -> Result<()> {
    let group_color_transfer = hash_group_effects(group, effect_hasher)?;
    let color_transfer = inherited_color_transfer
        .compose(group_color_transfer)
        .ok_or(ExportError::RasterPaintCutover(
            "raster paint proof does not support nested SVG color-transfer effects",
        ))?;
    let explicit_terminal = terminal_resolver.resolve(group.id())?;
    if inherited_selection
        && explicit_terminal.is_some()
        && terminal_resolver.has_explicit_bindings()
    {
        return Err(ExportError::RasterPaintCutover(
            "renderer terminal bindings overlap",
        ));
    }
    let marker_binding = (!terminal_resolver.has_explicit_bindings())
        .then(|| semantic_paint_binding_from_id(group.id()))
        .flatten();
    let group_binding = explicit_terminal
        .as_ref()
        .map(|terminal| terminal.binding)
        .or(marker_binding)
        .unwrap_or(inherited_binding);
    let group_terminal = explicit_terminal.clone().or(inherited_terminal);
    let group_selected = inherited_selection || explicit_terminal.is_some();
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect_group_paths_with_transform(
                child,
                paths,
                extra_transform,
                color_transfer,
                group_binding,
                group_terminal.clone(),
                group_selected,
                terminal_resolver,
                effect_hasher,
            )?,
            usvg::Node::Path(path) => {
                let explicit_terminal = terminal_resolver.resolve(path.id())?;
                if group_selected
                    && explicit_terminal.is_some()
                    && terminal_resolver.has_explicit_bindings()
                {
                    return Err(ExportError::RasterPaintCutover(
                        "renderer terminal bindings overlap",
                    ));
                }
                let marker_binding = (!terminal_resolver.has_explicit_bindings())
                    .then(|| semantic_paint_binding_from_id(path.id()))
                    .flatten();
                let path_binding = explicit_terminal
                    .as_ref()
                    .map(|terminal| terminal.binding)
                    .or(marker_binding)
                    .unwrap_or(group_binding);
                let path_terminal = explicit_terminal.clone().or(group_terminal.clone());
                paths.push(observe_path_with_binding(
                    path,
                    extra_transform,
                    path_binding,
                    path_terminal
                        .as_ref()
                        .map(|terminal| terminal.terminal_id.clone()),
                    group_selected || explicit_terminal.is_some(),
                    path_terminal.map(|terminal| terminal.lifeline_geometry),
                    color_transfer,
                )?);
            }
            // `usvg` keeps the glyph paths in a text node's flattened group in local
            // coordinates. The text node's absolute translation is applied by `resvg` during
            // rendering, but is not present on those flattened paths themselves.
            usvg::Node::Text(text) => collect_group_paths_with_transform(
                text.flattened(),
                paths,
                Some(text.abs_transform()),
                color_transfer,
                group_binding,
                group_terminal.clone(),
                group_selected,
                terminal_resolver,
                effect_hasher,
            )?,
            usvg::Node::Image(_) => {
                return Err(ExportError::RasterPaintCutover(
                    "raster paint proof does not support embedded images",
                ));
            }
        }
    }
    Ok(())
}

fn hash_group_effects(
    group: &usvg::Group,
    hasher: &mut Sha256,
) -> Result<RasterPaintColorTransfer> {
    if group.mask().is_some() {
        return Err(ExportError::RasterPaintCutover(
            "raster paint proof does not support SVG group effects",
        ));
    }
    update_len_prefixed(hasher, group.id().as_bytes());
    update_usize(hasher, group.children().len());
    hash_transform(hasher, group.abs_transform());
    hasher.update(group.opacity().get().to_bits().to_be_bytes());
    hasher.update([u8::from(group.isolate())]);
    hasher.update([blend_mode_tag(group.blend_mode())]);
    if let Some(clip_path) = group.clip_path() {
        update_len_prefixed(hasher, b"clip");
        update_len_prefixed(hasher, clip_path.id().as_bytes());
        hash_clip_path(clip_path, hasher)?;
    } else {
        update_len_prefixed(hasher, b"no-clip");
    }
    let color_transfer = supported_group_color_transfer(group)?;
    match color_transfer {
        RasterPaintColorTransfer::Identity => update_len_prefixed(hasher, b"no-color-filter"),
        RasterPaintColorTransfer::Brightness120Srgb => {
            update_len_prefixed(hasher, b"brightness-120-srgb");
            let filter = group
                .filters()
                .first()
                .ok_or(ExportError::RasterPaintCutover(
                    "raster paint proof lost its supported color-transfer filter",
                ))?;
            hash_supported_brightness_filter(filter, hasher)?;
        }
    }
    Ok(color_transfer)
}

fn supported_group_color_transfer(group: &usvg::Group) -> Result<RasterPaintColorTransfer> {
    match group.filters() {
        [] => Ok(RasterPaintColorTransfer::Identity),
        [filter] if is_brightness_120_srgb_filter(filter) => {
            Ok(RasterPaintColorTransfer::Brightness120Srgb)
        }
        _ => Err(ExportError::RasterPaintCutover(
            "raster paint proof does not support SVG group effects",
        )),
    }
}

fn is_brightness_120_srgb_filter(filter: &usvg::filter::Filter) -> bool {
    let [primitive] = filter.primitives() else {
        return false;
    };
    if primitive.color_interpolation() != usvg::filter::ColorInterpolation::SRGB {
        return false;
    }
    let usvg::filter::Kind::ComponentTransfer(component_transfer) = primitive.kind() else {
        return false;
    };
    matches!(
        component_transfer.input(),
        usvg::filter::Input::SourceGraphic
    ) && is_brightness_120_channel(component_transfer.func_r())
        && is_brightness_120_channel(component_transfer.func_g())
        && is_brightness_120_channel(component_transfer.func_b())
        && matches!(
            component_transfer.func_a(),
            usvg::filter::TransferFunction::Identity
        )
}

fn is_brightness_120_channel(function: &usvg::filter::TransferFunction) -> bool {
    matches!(
        function,
        usvg::filter::TransferFunction::Linear { slope, intercept }
            if slope.to_bits() == 1.2_f32.to_bits() && intercept.to_bits() == 0.0_f32.to_bits()
    )
}

fn hash_supported_brightness_filter(
    filter: &usvg::filter::Filter,
    hasher: &mut Sha256,
) -> Result<()> {
    if !is_brightness_120_srgb_filter(filter) {
        return Err(ExportError::RasterPaintCutover(
            "raster paint proof does not support SVG group effects",
        ));
    }
    update_len_prefixed(hasher, filter.id().as_bytes());
    hash_non_zero_rect(hasher, filter.rect());
    let primitive = filter
        .primitives()
        .first()
        .ok_or(ExportError::RasterPaintCutover(
            "raster paint proof lost its supported color-transfer primitive",
        ))?;
    update_len_prefixed(hasher, primitive.result().as_bytes());
    hash_non_zero_rect(hasher, primitive.rect());
    update_len_prefixed(hasher, b"component-transfer/source-graphic/srgb");
    let usvg::filter::Kind::ComponentTransfer(component_transfer) = primitive.kind() else {
        return Err(ExportError::RasterPaintCutover(
            "raster paint proof lost its supported color-transfer primitive",
        ));
    };
    for function in [
        component_transfer.func_r(),
        component_transfer.func_g(),
        component_transfer.func_b(),
    ] {
        let usvg::filter::TransferFunction::Linear { slope, intercept } = function else {
            return Err(ExportError::RasterPaintCutover(
                "raster paint proof lost its supported color-transfer channel",
            ));
        };
        hasher.update(slope.to_bits().to_be_bytes());
        hasher.update(intercept.to_bits().to_be_bytes());
    }
    update_len_prefixed(hasher, b"alpha-identity");
    Ok(())
}

fn blend_mode_tag(mode: usvg::BlendMode) -> u8 {
    match mode {
        usvg::BlendMode::Normal => 0,
        usvg::BlendMode::Multiply => 1,
        usvg::BlendMode::Screen => 2,
        usvg::BlendMode::Overlay => 3,
        usvg::BlendMode::Darken => 4,
        usvg::BlendMode::Lighten => 5,
        usvg::BlendMode::ColorDodge => 6,
        usvg::BlendMode::ColorBurn => 7,
        usvg::BlendMode::HardLight => 8,
        usvg::BlendMode::SoftLight => 9,
        usvg::BlendMode::Difference => 10,
        usvg::BlendMode::Exclusion => 11,
        usvg::BlendMode::Hue => 12,
        usvg::BlendMode::Saturation => 13,
        usvg::BlendMode::Color => 14,
        usvg::BlendMode::Luminosity => 15,
    }
}

fn hash_clip_path(clip_path: &usvg::ClipPath, hasher: &mut Sha256) -> Result<()> {
    hash_transform(hasher, clip_path.transform());
    if let Some(nested) = clip_path.clip_path() {
        update_len_prefixed(hasher, b"nested-clip");
        update_len_prefixed(hasher, nested.id().as_bytes());
        hash_clip_path(nested, hasher)?;
    } else {
        update_len_prefixed(hasher, b"no-nested-clip");
    }
    hash_group_tree(clip_path.root(), hasher)
}

fn hash_group_tree(group: &usvg::Group, hasher: &mut Sha256) -> Result<()> {
    if !hash_group_effects(group, hasher)?.is_identity() {
        return Err(ExportError::RasterPaintCutover(
            "raster paint proof does not support color-transfer filters in clip paths",
        ));
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => {
                update_len_prefixed(hasher, b"group");
                hash_group_tree(child, hasher)?;
            }
            usvg::Node::Path(path) => {
                update_len_prefixed(hasher, b"path");
                let observation = observe_path(path, None)?;
                hasher.update(observation.path_digest);
            }
            usvg::Node::Text(text) => {
                update_len_prefixed(hasher, b"text");
                hash_group_tree(text.flattened(), hasher)?;
            }
            usvg::Node::Image(_) => {
                return Err(ExportError::RasterPaintCutover(
                    "raster paint proof does not support images in clip paths",
                ));
            }
        }
    }
    Ok(())
}

fn observe_path(
    path: &usvg::Path,
    extra_transform: Option<tiny_skia::Transform>,
) -> Result<PathObservation> {
    observe_path_with_binding(
        path,
        extra_transform,
        RasterPaintSemanticBinding::Native,
        None,
        true,
        None,
        RasterPaintColorTransfer::Identity,
    )
}

fn observe_path_with_binding(
    path: &usvg::Path,
    extra_transform: Option<tiny_skia::Transform>,
    semantic_paint_binding: RasterPaintSemanticBinding,
    terminal_id: Option<String>,
    selected_by_renderer: bool,
    expected_lifeline_geometry: Option<RasterPaintLifelineGeometry>,
    color_transfer: RasterPaintColorTransfer,
) -> Result<PathObservation> {
    if let Some(expected_geometry) = expected_lifeline_geometry {
        validate_resolved_lifeline_path(path, extra_transform, expected_geometry)?;
    }
    let transform = path.abs_transform();
    let fill_bbox = map_rect(path.abs_bounding_box(), extra_transform)?;
    let stroke_bbox = map_rect(path.abs_stroke_bounding_box(), extra_transform)?;
    let mut shape_hasher = Sha256::new();
    update_len_prefixed(&mut shape_hasher, b"merman.raster-paint-path-shape.v2");
    hash_path_shape(
        &mut shape_hasher,
        transform,
        extra_transform,
        fill_bbox,
        path,
    );
    shape_hasher.update([match path.paint_order() {
        usvg::PaintOrder::FillAndStroke => 0,
        usvg::PaintOrder::StrokeAndFill => 1,
    }]);
    shape_hasher.update([match path.rendering_mode() {
        usvg::ShapeRendering::OptimizeSpeed => 0,
        usvg::ShapeRendering::CrispEdges => 1,
        usvg::ShapeRendering::GeometricPrecision => 2,
    }]);
    let shape_digest = shape_hasher.finalize().into();
    let fill_geometry_digest = fill_geometry_digest(path.fill());
    let stroke_geometry_digest = stroke_geometry_digest(path.stroke(), stroke_bbox);

    let mut path_hasher = Sha256::new();
    update_len_prefixed(&mut path_hasher, b"merman.raster-paint-path-structure.v3");
    path_hasher.update(shape_digest);
    path_hasher.update([u8::from(path.fill().is_some())]);
    path_hasher.update(fill_geometry_digest);
    match stroke_geometry_digest {
        Some(digest) => {
            path_hasher.update([1]);
            path_hasher.update(digest);
        }
        None => path_hasher.update([0]),
    }
    path_hasher.update([u8::from(path.is_visible())]);
    update_len_prefixed(&mut path_hasher, semantic_paint_binding.id());
    if let Some(terminal_id) = &terminal_id {
        update_len_prefixed(&mut path_hasher, b"terminal-owner");
        update_len_prefixed(&mut path_hasher, terminal_id.as_bytes());
    }
    path_hasher.update([u8::from(selected_by_renderer)]);
    path_hasher.update([color_transfer.digest_tag()]);

    Ok(PathObservation {
        path_digest: path_hasher.finalize().into(),
        shape_digest,
        fill_geometry_digest,
        stroke_geometry_digest,
        fill_region_bits: rect_bits(fill_bbox),
        stroke_region_bits: rect_bits(stroke_bbox),
        visible: path.is_visible(),
        fill: path.fill().map(observe_fill),
        stroke: path.stroke().map(observe_stroke),
        semantic_paint_binding,
        terminal_id,
        selected_by_renderer,
        color_transfer,
    })
}

fn validate_resolved_lifeline_path(
    path: &usvg::Path,
    extra_transform: Option<tiny_skia::Transform>,
    expected_geometry: RasterPaintLifelineGeometry,
) -> Result<()> {
    if extra_transform.is_some() {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path has an unexpected text transform",
        ));
    }
    let stroke = path.stroke().ok_or(ExportError::RasterPaintCutover(
        "renderer lifeline path has no resolved stroke",
    ))?;
    let observed_width = f64::from(stroke.width().get());
    if !approximately_equal(observed_width, expected_geometry.effective_stroke_width()) {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path has a different cascade-resolved stroke width",
        ));
    }

    let mut segments = path.data().segments();
    let Some(tiny_skia::PathSegment::MoveTo(start)) = segments.next() else {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path has no move-to segment",
        ));
    };
    let Some(tiny_skia::PathSegment::LineTo(end)) = segments.next() else {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path is not a single line",
        ));
    };
    if segments.next().is_some() {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path contains extra geometry",
        ));
    }

    let expected = [
        (expected_geometry.x1(), expected_geometry.y1()),
        (expected_geometry.x2(), expected_geometry.y2()),
    ];
    let observed = [
        (f64::from(start.x), f64::from(start.y)),
        (f64::from(end.x), f64::from(end.y)),
    ];
    if observed.into_iter().zip(expected).any(
        |((observed_x, observed_y), (expected_x, expected_y))| {
            !approximately_equal(observed_x, expected_x)
                || !approximately_equal(observed_y, expected_y)
        },
    ) {
        return Err(ExportError::RasterPaintCutover(
            "renderer lifeline path endpoints differ from its sealed writer fact",
        ));
    }
    Ok(())
}

fn approximately_equal(left: f64, right: f64) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= 1e-4 * scale
}

fn semantic_paint_binding_from_id(id: &str) -> Option<RasterPaintSemanticBinding> {
    if id.ends_with(merman_render::svg::RENDERER_SEMANTIC_NATIVE_PAINT_SUFFIX) {
        Some(RasterPaintSemanticBinding::Native)
    } else if id.ends_with(merman_render::svg::RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX) {
        Some(RasterPaintSemanticBinding::FillAndStrokeFromStroke)
    } else if id.ends_with(merman_render::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX) {
        Some(RasterPaintSemanticBinding::FillFromStroke)
    } else {
        None
    }
}

fn fill_geometry_digest(fill: Option<&usvg::Fill>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.raster-paint-fill-geometry.v1");
    match fill {
        Some(fill) => {
            hasher.update([1]);
            hasher.update([match fill.rule() {
                usvg::FillRule::NonZero => 0,
                usvg::FillRule::EvenOdd => 1,
            }]);
        }
        None => hasher.update([0]),
    }
    hasher.finalize().into()
}

fn stroke_geometry_digest(
    stroke: Option<&usvg::Stroke>,
    stroke_bbox: usvg::Rect,
) -> Option<[u8; 32]> {
    let stroke = stroke?;
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.raster-paint-stroke-geometry.v1");
    hash_rect(&mut hasher, stroke_bbox);
    hasher.update(stroke.width().get().to_bits().to_be_bytes());
    hasher.update(stroke.dashoffset().to_bits().to_be_bytes());
    hasher.update(stroke.miterlimit().get().to_bits().to_be_bytes());
    hasher.update([match stroke.linecap() {
        usvg::LineCap::Butt => 0,
        usvg::LineCap::Round => 1,
        usvg::LineCap::Square => 2,
    }]);
    hasher.update([match stroke.linejoin() {
        usvg::LineJoin::Miter => 0,
        usvg::LineJoin::MiterClip => 1,
        usvg::LineJoin::Round => 2,
        usvg::LineJoin::Bevel => 3,
    }]);
    match stroke.dasharray() {
        Some(dasharray) => {
            hasher.update([1]);
            update_usize(&mut hasher, dasharray.len());
            for value in dasharray {
                hasher.update(value.to_bits().to_be_bytes());
            }
        }
        None => hasher.update([0]),
    }
    Some(hasher.finalize().into())
}

fn hash_path_shape(
    hasher: &mut Sha256,
    transform: tiny_skia::Transform,
    extra_transform: Option<tiny_skia::Transform>,
    fill_bbox: usvg::Rect,
    path: &usvg::Path,
) {
    hash_transform(hasher, transform);
    if let Some(extra_transform) = extra_transform {
        hasher.update([1]);
        hash_transform(hasher, extra_transform);
    } else {
        hasher.update([0]);
    }
    hash_rect(hasher, fill_bbox);
    for segment in path.data().segments() {
        use tiny_skia::PathSegment;
        match segment {
            PathSegment::MoveTo(point) => {
                hasher.update([0]);
                hash_point(hasher, point);
            }
            PathSegment::LineTo(point) => {
                hasher.update([1]);
                hash_point(hasher, point);
            }
            PathSegment::QuadTo(control, point) => {
                hasher.update([2]);
                hash_point(hasher, control);
                hash_point(hasher, point);
            }
            PathSegment::CubicTo(first, second, point) => {
                hasher.update([3]);
                hash_point(hasher, first);
                hash_point(hasher, second);
                hash_point(hasher, point);
            }
            PathSegment::Close => hasher.update([4]),
        }
    }
}

fn hash_transform(hasher: &mut Sha256, transform: tiny_skia::Transform) {
    for value in [
        transform.sx,
        transform.kx,
        transform.ky,
        transform.sy,
        transform.tx,
        transform.ty,
    ] {
        hasher.update(value.to_bits().to_be_bytes());
    }
}

fn map_rect(rect: usvg::Rect, transform: Option<tiny_skia::Transform>) -> Result<usvg::Rect> {
    let Some(transform) = transform else {
        return Ok(rect);
    };
    let mut points = [
        tiny_skia::Point::from_xy(rect.left(), rect.top()),
        tiny_skia::Point::from_xy(rect.right(), rect.top()),
        tiny_skia::Point::from_xy(rect.left(), rect.bottom()),
        tiny_skia::Point::from_xy(rect.right(), rect.bottom()),
    ];
    transform.map_points(&mut points);
    let left = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    usvg::Rect::from_ltrb(left, top, right, bottom).ok_or(ExportError::RasterPaintCutover(
        "paint path transform produced invalid bounds",
    ))
}

fn observe_fill(fill: &usvg::Fill) -> PaintObservation {
    PaintObservation {
        rgb: paint_rgb(fill.paint()),
        opacity_bits: fill.opacity().get().to_bits(),
    }
}

fn observe_stroke(stroke: &usvg::Stroke) -> PaintObservation {
    PaintObservation {
        rgb: paint_rgb(stroke.paint()),
        opacity_bits: stroke.opacity().get().to_bits(),
    }
}

fn paint_rgb(paint: &usvg::Paint) -> Option<[u8; 3]> {
    match paint {
        usvg::Paint::Color(color) => Some([color.red, color.green, color.blue]),
        usvg::Paint::LinearGradient(_)
        | usvg::Paint::RadialGradient(_)
        | usvg::Paint::Pattern(_) => None,
    }
}

fn paint_is_opaque_control(paint: Option<PaintObservation>, control_rgb: [u8; 3]) -> bool {
    paint.is_some_and(|paint| paint.rgb == Some(control_rgb) && paint.opacity() == 1.0)
}

fn require_no_opaque_control_targets(observation: &PaintTreeObservation) -> Result<()> {
    if !observation.targets.is_empty() {
        return Err(ExportError::RasterPaintCutover(
            "transparent SVG retained an opaque control-painted path",
        ));
    }
    Ok(())
}

struct SolidDeltaObservation {
    control_pixels: usize,
    changed_control_pixels: usize,
    requested_changed_control_pixels: usize,
    changed_pixels: usize,
    changed_outside_target_pixels: usize,
    unchanged_control_pixels: usize,
}

fn observe_solid_delta(
    solid: &tiny_skia::Pixmap,
    underlay: &tiny_skia::Pixmap,
    targets: &[TargetPath],
    requested_targets: &[TargetPath],
    terminal_bindings: &[RasterPaintTerminalBinding],
    placement: RasterPlacement,
) -> Result<SolidDeltaObservation> {
    if solid.width() != underlay.width() || solid.height() != underlay.height() {
        return Err(ExportError::RasterPaintCutover(
            "solid and underlay pixmap dimensions differ",
        ));
    }
    let regions = pixel_regions(targets, placement)?;
    let control_regions = colored_pixel_regions(targets, placement)?;
    let requested_control_regions = colored_pixel_regions(requested_targets, placement)?;
    let mut terminal_proofs =
        terminal_pixel_proofs(requested_targets, terminal_bindings, placement)?;
    let mut control_pixels = 0usize;
    let mut changed_control_pixels = 0usize;
    let mut requested_changed_control_pixels = 0usize;
    let mut changed_pixels = 0usize;
    let mut changed_outside_target_pixels = 0usize;
    let mut unchanged_control_pixels = 0usize;
    let width = solid.width() as usize;
    for (index, (solid_pixel, underlay_pixel)) in
        solid.pixels().iter().zip(underlay.pixels()).enumerate()
    {
        let x = (index % width) as u32;
        let y = (index / width) as u32;
        let is_control = control_regions.iter().any(|target| {
            target.region.contains(x, y)
                && pixel_near_rgb(*solid_pixel, target.rendered_control_rgb, COLOR_TOLERANCE)
        });
        let changed = solid_pixel != underlay_pixel;
        let inside_any_region = regions.iter().any(|region| region.contains(x, y));
        if is_control {
            control_pixels = control_pixels.saturating_add(1);
            if changed {
                changed_control_pixels = changed_control_pixels.saturating_add(1);
                if requested_control_regions.iter().any(|target| {
                    target.region.contains(x, y)
                        && pixel_near_rgb(
                            *solid_pixel,
                            target.rendered_control_rgb,
                            COLOR_TOLERANCE,
                        )
                }) {
                    requested_changed_control_pixels =
                        requested_changed_control_pixels.saturating_add(1);
                }
                let mut matching_terminal = None;
                let mut ambiguous = false;
                for (index, proof) in terminal_proofs.iter().enumerate() {
                    if proof.regions.iter().any(|region| region.contains(x, y)) {
                        if matching_terminal.replace(index).is_some() {
                            ambiguous = true;
                            break;
                        }
                    }
                }
                if !ambiguous && let Some(index) = matching_terminal {
                    terminal_proofs[index].removed_control_pixel = true;
                }
            } else {
                unchanged_control_pixels = unchanged_control_pixels.saturating_add(1);
            }
        }
        if changed {
            changed_pixels = changed_pixels.saturating_add(1);
            if !inside_any_region {
                changed_outside_target_pixels = changed_outside_target_pixels.saturating_add(1);
            }
        }
    }
    if control_pixels == 0
        || changed_pixels == 0
        || changed_control_pixels == 0
        || requested_changed_control_pixels == 0
    {
        return Err(ExportError::RasterPaintCutover(
            "solid route produced no removed visible control-painted pixels for the requested facet",
        ));
    }
    if terminal_proofs
        .iter()
        .any(|proof| !proof.removed_control_pixel)
    {
        return Err(ExportError::RasterPaintCutover(
            "a renderer terminal produced no independently attributable removed control pixel",
        ));
    }
    if changed_outside_target_pixels != 0 {
        return Err(ExportError::RasterPaintCutover(
            "solid route changed pixels outside its resolved target geometry",
        ));
    }
    Ok(SolidDeltaObservation {
        control_pixels,
        changed_control_pixels,
        requested_changed_control_pixels,
        changed_pixels,
        changed_outside_target_pixels,
        unchanged_control_pixels,
    })
}

struct TerminalPixelProof {
    regions: Vec<PixelRegion>,
    removed_control_pixel: bool,
}

fn terminal_pixel_proofs(
    requested_targets: &[TargetPath],
    terminal_bindings: &[RasterPaintTerminalBinding],
    placement: RasterPlacement,
) -> Result<Vec<TerminalPixelProof>> {
    if terminal_bindings.is_empty() {
        return Ok(Vec::new());
    }
    let mut regions_by_terminal = BTreeMap::<String, Vec<PixelRegion>>::new();
    for terminal in terminal_bindings {
        if regions_by_terminal
            .insert(terminal.terminal_id.clone(), Vec::new())
            .is_some()
        {
            return Err(ExportError::RasterPaintCutover(
                "renderer terminal bindings contain a duplicate id",
            ));
        }
    }
    for target in requested_targets {
        let terminal_id = target
            .terminal_id
            .as_ref()
            .ok_or(ExportError::RasterPaintCutover(
                "renderer-selected target is missing its terminal owner",
            ))?;
        let regions =
            regions_by_terminal
                .get_mut(terminal_id)
                .ok_or(ExportError::RasterPaintCutover(
                    "renderer-selected target has an unknown terminal owner",
                ))?;
        regions.push(pixel_region(target.region(), placement)?);
    }
    if regions_by_terminal.values().any(Vec::is_empty) {
        return Err(ExportError::RasterPaintCutover(
            "renderer terminal has no control-painted target for the requested facet",
        ));
    }
    Ok(regions_by_terminal
        .into_values()
        .map(|regions| TerminalPixelProof {
            regions,
            removed_control_pixel: false,
        })
        .collect())
}

#[derive(Clone, Copy)]
struct PixelRegion {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl PixelRegion {
    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

fn pixel_regions(targets: &[TargetPath], placement: RasterPlacement) -> Result<Vec<PixelRegion>> {
    targets
        .iter()
        .map(|target| pixel_region(target.region(), placement))
        .collect()
}

#[derive(Clone, Copy)]
struct ColoredPixelRegion {
    region: PixelRegion,
    rendered_control_rgb: [u8; 3],
}

fn colored_pixel_regions(
    targets: &[TargetPath],
    placement: RasterPlacement,
) -> Result<Vec<ColoredPixelRegion>> {
    targets
        .iter()
        .map(|target| {
            Ok(ColoredPixelRegion {
                region: pixel_region(target.region(), placement)?,
                rendered_control_rgb: target.rendered_control_rgb,
            })
        })
        .collect()
}

fn pixel_region(region: [f32; 4], placement: RasterPlacement) -> Result<PixelRegion> {
    let [left, top, width, height] = region;
    if !region.into_iter().all(f32::is_finite) || width < 0.0 || height < 0.0 {
        return Err(ExportError::RasterPaintCutover(
            "target path has invalid resolved bounds",
        ));
    }
    let origin_x = if placement.translate_min_to_origin {
        placement.min_x()
    } else {
        0.0
    };
    let origin_y = if placement.translate_min_to_origin {
        placement.min_y()
    } else {
        0.0
    };
    let scale = placement.effective_scale();
    let raw_left = ((f64::from(left - origin_x) * scale).floor() as i64)
        .saturating_sub(i64::from(REGION_PADDING_PX));
    let raw_top = ((f64::from(top - origin_y) * scale).floor() as i64)
        .saturating_sub(i64::from(REGION_PADDING_PX));
    let raw_right = ((f64::from(left + width - origin_x) * scale).ceil() as i64)
        .saturating_add(i64::from(REGION_PADDING_PX));
    let raw_bottom = ((f64::from(top + height - origin_y) * scale).ceil() as i64)
        .saturating_add(i64::from(REGION_PADDING_PX));
    let left = raw_left.clamp(0, i64::from(placement.width_px)) as u32;
    let top = raw_top.clamp(0, i64::from(placement.height_px)) as u32;
    let right = raw_right.clamp(0, i64::from(placement.width_px)) as u32;
    let bottom = raw_bottom.clamp(0, i64::from(placement.height_px)) as u32;
    if left >= right || top >= bottom {
        return Err(ExportError::RasterPaintCutover(
            "target path resolves outside the raster viewport",
        ));
    }
    Ok(PixelRegion {
        left,
        top,
        right,
        bottom,
    })
}

fn pixel_near_rgb(pixel: tiny_skia::PremultipliedColorU8, rgb: [u8; 3], tolerance: u8) -> bool {
    let alpha = pixel.alpha();
    if alpha == 0 {
        return false;
    }
    let demultiply = |channel: u8| {
        ((u16::from(channel) * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255) as u8
    };
    [
        demultiply(pixel.red()),
        demultiply(pixel.green()),
        demultiply(pixel.blue()),
    ]
    .into_iter()
    .zip(rgb)
    .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
}

struct RasterPaintCutoverFacts {
    channels: RasterPaintCutoverChannels,
    control_rgb: [u8; 3],
    solid_source_digest: [u8; 32],
    transparent_source_digest: [u8; 32],
    underlay_source_digest: [u8; 32],
    underlay_path_tree_digest: [u8; 32],
    transparent_path_tree_digest: [u8; 32],
    solid_effect_tree_digest: [u8; 32],
    underlay_effect_tree_digest: [u8; 32],
    transparent_effect_tree_digest: [u8; 32],
    terminal_bindings_digest: [u8; 32],
    transparent_geometry_compatible: bool,
    target_geometry_digest: [u8; 32],
    target_path_count: usize,
    solid_control_pixels: usize,
    changed_control_pixels: usize,
    requested_changed_control_pixels: usize,
    changed_pixels: usize,
    changed_outside_target_pixels: usize,
    unchanged_control_pixels: usize,
    transparent_reference_mismatches: usize,
}

fn hash_rect(hasher: &mut Sha256, rect: usvg::Rect) {
    for value in [rect.x(), rect.y(), rect.width(), rect.height()] {
        hasher.update(value.to_bits().to_be_bytes());
    }
}

fn hash_non_zero_rect(hasher: &mut Sha256, rect: usvg::NonZeroRect) {
    for value in [rect.x(), rect.y(), rect.width(), rect.height()] {
        hasher.update(value.to_bits().to_be_bytes());
    }
}

fn rect_bits(rect: usvg::Rect) -> [u32; 4] {
    [rect.x(), rect.y(), rect.width(), rect.height()].map(f32::to_bits)
}

fn hash_point(hasher: &mut Sha256, point: tiny_skia::Point) {
    hasher.update(point.x.to_bits().to_be_bytes());
    hasher.update(point.y.to_bits().to_be_bytes());
}

fn update_usize(hasher: &mut Sha256, value: usize) {
    hasher.update(u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes());
}

fn update_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    update_usize(hasher, value.len());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compatible_svg(svg: &str) -> ResvgCompatibleSvg {
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        merman_render::svg::finalize_resvg_svg(svg, &session).expect("sealed SVG")
    }

    fn encode_pair(
        solid: &str,
        transparent: &str,
        facet: RasterPaintCutoverFacet,
        control_css: &str,
    ) -> Result<EncodedRasterPaintCutoverPair> {
        encode_pair_with_bindings(solid, transparent, facet, control_css, &[])
    }

    fn encode_pair_with_bindings(
        solid: &str,
        transparent: &str,
        facet: RasterPaintCutoverFacet,
        control_css: &str,
        terminal_bindings: &[RasterPaintTerminalBinding],
    ) -> Result<EncodedRasterPaintCutoverPair> {
        encode_png_paint_cutover_pair_controlled(
            &compatible_svg(solid),
            &compatible_svg(transparent),
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
            RasterPaintCutoverChannels::Only(facet),
            control_css,
            terminal_bindings,
        )
    }

    fn assert_cutover_error(result: Result<EncodedRasterPaintCutoverPair>) {
        match result {
            Err(ExportError::RasterPaintCutover(_)) => {}
            Err(other) => panic!("expected raster cutover error, got {other:?}"),
            Ok(_) => panic!("expected raster cutover error, got success"),
        }
    }

    fn lifeline_binding(id: &str, x: f64) -> RasterPaintTerminalBinding {
        RasterPaintTerminalBinding::line_lifeline(
            id,
            RasterPaintSemanticBinding::FillAndStrokeFromStroke,
            x,
            2.0,
            x,
            18.0,
            3.0,
        )
        .expect("valid lifeline binding")
    }

    fn lifeline_binding_with_widths(
        id: &str,
        x: f64,
        authored_stroke_width: f64,
        effective_stroke_width: f64,
    ) -> RasterPaintTerminalBinding {
        RasterPaintTerminalBinding::line_lifeline_with_effective_width(
            id,
            RasterPaintSemanticBinding::FillAndStrokeFromStroke,
            x,
            2.0,
            x,
            18.0,
            authored_stroke_width,
            effective_stroke_width,
        )
        .expect("valid lifeline binding")
    }

    #[test]
    fn renderer_sidecar_constructor_rejects_non_lifeline_geometry() {
        let binding = |x1, y1, x2, y2, stroke_width| {
            RasterPaintTerminalBinding::line_lifeline(
                "actor0",
                RasterPaintSemanticBinding::FillAndStrokeFromStroke,
                x1,
                y1,
                x2,
                y2,
                stroke_width,
            )
        };

        assert!(binding(10.0, 2.0, 11.0, 18.0, 3.0).is_none());
        assert!(binding(10.0, 18.0, 10.0, 2.0, 3.0).is_none());
        assert!(binding(10.0, 2.0, 10.0, 2.0, 3.0).is_none());
        assert!(binding(10.0, 2.0, 10.0, 18.0, 0.0).is_none());
    }

    #[test]
    fn fill_pair_seals_visible_control_and_exact_underlay_semantics() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="transparent"/></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("valid fill pair");
        let ((solid_png, _), (transparent_png, _), receipt) = pair.into_parts();

        assert!(solid_png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(transparent_png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_ne!(solid_png, transparent_png);
        assert!(receipt.proves_semantics());
        assert_ne!(receipt.digest(), [0; 32]);
    }

    fn encode_pair_with_channels(
        solid: &str,
        transparent: &str,
        channels: RasterPaintCutoverChannels,
    ) -> Result<EncodedRasterPaintCutoverPair> {
        encode_png_paint_cutover_pair_controlled(
            &compatible_svg(solid),
            &compatible_svg(transparent),
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
            channels,
            "#dc2626",
            &[],
        )
    }

    #[test]
    fn mixed_channel_contract_observes_either_or_both_visible_channels() {
        for (paint, single_facet) in [
            (r##"fill="#dc2626""##, Some(RasterPaintCutoverFacet::Fill)),
            (
                r##"fill="none" stroke="#dc2626" stroke-width="2""##,
                Some(RasterPaintCutoverFacet::Stroke),
            ),
            (
                r##"fill="#dc2626" stroke="#dc2626" stroke-width="2""##,
                None,
            ),
        ] {
            let solid = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" {paint}/></svg>"#
            );
            let transparent = solid.replace("#dc2626", "transparent");
            let pair = encode_pair_with_channels(
                &solid,
                &transparent,
                RasterPaintCutoverChannels::FillOrStroke,
            )
            .expect("mixed contract retains either native channel");
            assert!(pair.receipt.proves_semantics());
            if let Some(facet) = single_facet {
                let single = encode_pair_with_channels(
                    &solid,
                    &transparent,
                    RasterPaintCutoverChannels::Only(facet),
                )
                .expect("matching single channel");
                assert_ne!(
                    single.receipt.digest(),
                    pair.receipt.digest(),
                    "receipt binds the admitted channel contract"
                );
                let wrong = match facet {
                    RasterPaintCutoverFacet::Fill => RasterPaintCutoverFacet::Stroke,
                    RasterPaintCutoverFacet::Stroke => RasterPaintCutoverFacet::Fill,
                };
                assert_cutover_error(encode_pair_with_channels(
                    &solid,
                    &transparent,
                    RasterPaintCutoverChannels::Only(wrong),
                ));
            }
        }
    }

    #[test]
    fn mixed_channel_contract_rejects_invisible_or_incomplete_transparent_outputs() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626" stroke="#dc2626" stroke-width="2"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let unchanged_stroke = solid.replacen("#dc2626", "transparent", 1);
        let shifted = transparent.replace(r#"x="4""#, r#"x="5""#);
        for invalid in [solid, unchanged_stroke.as_str(), shifted.as_str()] {
            assert_cutover_error(encode_pair_with_channels(
                solid,
                invalid,
                RasterPaintCutoverChannels::FillOrStroke,
            ));
        }
        let invisible = solid.replace("<rect ", r#"<rect opacity="0" "#);
        let invisible_transparent = invisible.replace("#dc2626", "transparent");
        assert_cutover_error(encode_pair_with_channels(
            &invisible,
            &invisible_transparent,
            RasterPaintCutoverChannels::FillOrStroke,
        ));
    }

    #[test]
    fn mixed_channel_contract_cannot_replace_renderer_terminal_semantics() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let result = encode_png_paint_cutover_pair_controlled(
            &compatible_svg(solid),
            &compatible_svg(&transparent),
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
            RasterPaintCutoverChannels::FillOrStroke,
            "#dc2626",
            &[lifeline_binding("actor0", 10.0)],
        );
        assert_cutover_error(result);
    }

    #[test]
    fn stroke_pair_seals_only_the_requested_paint_facet() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#f8fafc" stroke="#2563eb" stroke-width="2"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#f8fafc" stroke="transparent" stroke-width="2"/></svg>"##;

        let pair = encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Stroke,
            "#2563eb",
        )
        .expect("valid stroke pair");
        let (_, _, receipt) = pair.into_parts();

        assert!(receipt.proves_semantics());
    }

    #[test]
    fn stroke_witness_rejects_control_paint_owned_only_by_fill() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#2563eb"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="transparent"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Stroke,
            "#2563eb",
        ));
    }

    #[test]
    fn fill_witness_rejects_control_paint_owned_only_by_stroke() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="none" stroke="#dc2626" stroke-width="2"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="none" stroke="transparent" stroke-width="2"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn renderer_owned_fill_marker_can_bind_fill_to_stroke_channel() {
        let solid = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path id="cluster{}" d="M4 4H16V16H4Z" fill="none" stroke="#dc2626" stroke-width="3"/></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX
        );
        let transparent = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path id="cluster{}" d="M4 4H16V16H4Z" fill="none" stroke="transparent" stroke-width="3"/></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX
        );

        let pair = encode_pair(
            &solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        )
        .expect("renderer-owned semantic fill marker should be honored");
        let (_, _, receipt) = pair.into_parts();
        assert!(receipt.proves_semantics());
    }

    #[test]
    fn renderer_owned_fill_and_stroke_marker_can_bind_both_facets_to_stroke_channel() {
        let solid = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path id="lifeline{}" d="M10 2V18" fill="none" stroke="#dc2626" stroke-width="3"/></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX
        );
        let transparent = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path id="lifeline{}" d="M10 2V18" fill="none" stroke="transparent" stroke-width="3"/></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX
        );

        for facet in [
            RasterPaintCutoverFacet::Fill,
            RasterPaintCutoverFacet::Stroke,
        ] {
            let pair = encode_pair(&solid, &transparent, facet, "#dc2626")
                .expect("renderer-owned dual semantic marker should be honored");
            let (_, _, receipt) = pair.into_parts();
            assert!(receipt.proves_semantics());
        }
    }

    #[test]
    fn renderer_owned_native_marker_scopes_a_current_color_fill_terminal() {
        let solid = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>.icon{{color:#dc2626}}</style><g id="icon{}" class="icon"><path fill="currentColor" d="M4 4H16V16H4Z"/></g></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_NATIVE_PAINT_SUFFIX
        );
        let transparent = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>.icon{{color:transparent}}</style><g id="icon{}" class="icon"><path fill="currentColor" d="M4 4H16V16H4Z"/></g></svg>"##,
            merman_render::svg::RENDERER_SEMANTIC_NATIVE_PAINT_SUFFIX
        );

        let pair = encode_pair(
            &solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        )
        .expect("renderer-owned native marker should scope the currentColor fill terminal");
        let (_, _, receipt) = pair.into_parts();
        assert!(receipt.proves_semantics());
    }

    #[test]
    fn renderer_sidecar_binds_an_exact_terminal_without_changing_public_svg() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="transparent" stroke-width="3"/></svg>"##;
        let bindings = [lifeline_binding("actor0", 10.0)];

        for facet in [
            RasterPaintCutoverFacet::Fill,
            RasterPaintCutoverFacet::Stroke,
        ] {
            let pair = encode_pair_with_bindings(solid, transparent, facet, "#dc2626", &bindings)
                .expect("exact renderer sidecar should select the lifeline");
            let (_, _, receipt) = pair.into_parts();
            assert!(receipt.proves_semantics());
        }
    }

    #[test]
    fn renderer_sidecar_binds_the_cascade_resolved_lifeline_width() {
        let solid = r##"<svg id="sequence" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>#sequence .actor-line{stroke-width:2px;}</style><line id="actor0" class="actor-line" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="0.5"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [lifeline_binding_with_widths("actor0", 10.0, 0.5, 2.0)];

        for facet in [
            RasterPaintCutoverFacet::Fill,
            RasterPaintCutoverFacet::Stroke,
        ] {
            let pair = encode_pair_with_bindings(solid, &transparent, facet, "#dc2626", &bindings)
                .expect("stylesheet width should be validated through the resolved path");
            assert!(pair.into_parts().2.proves_semantics());
        }
    }

    #[test]
    fn renderer_sidecar_rejects_a_mismatched_cascade_resolved_lifeline_width() {
        let solid = r##"<svg id="sequence" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>#sequence .actor-line{stroke-width:2px;}</style><line id="actor0" class="actor-line" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="0.5"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [lifeline_binding_with_widths("actor0", 10.0, 0.5, 3.0)];

        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Stroke,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn renderer_sidecar_parses_inline_style_declaration_boundaries() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3" style="font-family:&quot;a;stroke-width:99&quot;"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [lifeline_binding("actor0", 10.0)];

        let pair = encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Stroke,
            "#dc2626",
            &bindings,
        )
        .expect("a semicolon inside a CSS string is not a declaration boundary");
        assert!(pair.into_parts().2.proves_semantics());

        let geometry_override = solid.replace(
            "font-family:&quot;a;stroke-width:99&quot;",
            "stroke-width:4",
        );
        assert_cutover_error(encode_pair_with_bindings(
            &geometry_override,
            &transparent,
            RasterPaintCutoverFacet::Stroke,
            "#dc2626",
            &bindings,
        ));

        assert!(style_overrides_lifeline_geometry("stroke-width").is_err());
    }

    #[test]
    fn renderer_sidecar_rejects_missing_or_duplicate_terminals() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="6" y1="2" x2="6" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/><line id="actor0" x1="14" y1="2" x2="14" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let duplicate = [lifeline_binding("actor0", 6.0)];
        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &duplicate,
        ));

        let missing = [lifeline_binding("actor1", 10.0)];
        assert_cutover_error(encode_pair_with_bindings(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="transparent" stroke-width="3"/></svg>"##,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &missing,
        ));
    }

    #[test]
    fn renderer_sidecar_allows_unrelated_control_paint_inside_selected_region() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/><rect x="9" y="8" width="3" height="3" fill="#dc2626"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="transparent" stroke-width="3"/><rect x="9" y="8" width="3" height="3" fill="#dc2626"/></svg>"##;
        let bindings = [lifeline_binding("actor0", 10.0)];

        let pair = encode_pair_with_bindings(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        )
        .expect("unrelated same-color paint is not a selected terminal residual");
        assert!(pair.into_parts().2.proves_semantics());
    }

    #[test]
    fn renderer_sidecar_rejects_same_color_outside_the_selected_terminal() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="6" y1="2" x2="6" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/><line id="other" x1="14" y1="2" x2="14" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [lifeline_binding("actor0", 6.0)];

        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn renderer_sidecar_rejects_wrong_lifeline_role_tag_or_geometry() {
        let transparent_line = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="transparent" stroke-width="3"/></svg>"##;
        let bindings = [lifeline_binding("actor0", 10.0)];

        let wrong_role = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="participant" stroke="#dc2626" stroke-width="3"/></svg>"##;
        assert_cutover_error(encode_pair_with_bindings(
            wrong_role,
            transparent_line,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));

        let wrong_tag = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect id="actor0" x="10" y="2" width="1" height="16" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        assert_cutover_error(encode_pair_with_bindings(
            wrong_tag,
            transparent_line,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));

        let wrong_geometry = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="11" y1="2" x2="11" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></svg>"##;
        assert_cutover_error(encode_pair_with_bindings(
            wrong_geometry,
            transparent_line,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn renderer_sidecar_rejects_transformed_lifeline_terminals() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g transform="translate(1 0)"><line id="actor0" x1="10" y1="2" x2="10" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></g></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [lifeline_binding("actor0", 10.0)];

        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn renderer_sidecar_rejects_an_invisible_terminal_hidden_by_another() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><line id="actor0" x1="6" y1="2" x2="6" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/><g opacity="0"><line id="actor1" x1="14" y1="2" x2="14" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></g></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [
            lifeline_binding("actor0", 6.0),
            lifeline_binding("actor1", 14.0),
        ];

        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn renderer_sidecar_rejects_a_completely_clipped_terminal() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><clipPath id="hidden"><rect x="0" y="0" width="1" height="1"/></clipPath></defs><line id="actor0" x1="6" y1="2" x2="6" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/><g clip-path="url(#hidden)"><line id="actor1" x1="14" y1="2" x2="14" y2="18" data-et="life-line" stroke="#dc2626" stroke-width="3"/></g></svg>"##;
        let transparent = solid.replace("#dc2626", "transparent");
        let bindings = [
            lifeline_binding("actor0", 6.0),
            lifeline_binding("actor1", 14.0),
        ];

        assert_cutover_error(encode_pair_with_bindings(
            solid,
            &transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
            &bindings,
        ));
    }

    #[test]
    fn opaque_transparent_witness_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></svg>"##;
        let opaque = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#111827"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            opaque,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn geometry_drift_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></svg>"##;
        let moved = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="transparent"/><rect x="5" y="4" width="2" height="2" fill="#0f172a"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            moved,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn invisible_geometry_drift_fails_closed_even_when_pixels_match() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></svg>"##;
        let moved_transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="5" y="4" width="12" height="12" fill="transparent"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            moved_transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn unrelated_raster_change_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="2" width="6" height="6" fill="#dc2626"/><rect x="12" y="12" width="6" height="6" fill="#0f172a"/></svg>"##;
        let changed = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="2" width="6" height="6" fill="transparent"/><rect x="12" y="12" width="6" height="6" fill="#f8fafc"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            changed,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn control_paint_on_both_facets_is_accounted_for() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#dc2626" stroke="#dc2626"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="transparent" stroke="transparent"/></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("a shared fill and stroke control paint remains a valid fill witness");
        let (_, _, receipt) = pair.into_parts();
        assert!(receipt.proves_semantics());
    }

    #[test]
    fn invisible_transparent_stroke_geometry_drift_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#f8fafc" stroke="#2563eb" stroke-width="2" stroke-dasharray="2 1"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="4" y="4" width="12" height="12" fill="#f8fafc" stroke="transparent" stroke-width="4" stroke-dasharray="1 2"/></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Stroke,
            "#2563eb",
        ));
    }

    #[test]
    fn clip_path_is_part_of_the_effect_receipt_and_can_be_used_by_a_witness() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><clipPath id="clip"><rect x="4" y="4" width="8" height="8"/></clipPath></defs><g clip-path="url(#clip)"><rect width="20" height="20" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><clipPath id="clip"><rect x="4" y="4" width="8" height="8"/></clipPath></defs><g clip-path="url(#clip)"><rect width="20" height="20" fill="transparent"/></g></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("a geometry-preserving clip path is supported");
        let (_, _, receipt) = pair.into_parts();
        assert!(receipt.proves_semantics());
    }

    #[test]
    fn clip_path_geometry_drift_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><clipPath id="clip"><rect x="4" y="4" width="8" height="8"/></clipPath></defs><g clip-path="url(#clip)"><rect width="20" height="20" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><clipPath id="clip"><rect x="5" y="4" width="8" height="8"/></clipPath></defs><g clip-path="url(#clip)"><rect width="20" height="20" fill="transparent"/></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn brightness_120_srgb_component_transfer_preserves_route_local_paint_proof() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("the known geometry-preserving timeline brightness effect is supported");
        assert!(pair.into_parts().2.proves_semantics());
    }

    #[test]
    fn stylesheet_brightness_120_preserves_route_local_paint_proof() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>.eventWrapper{filter:brightness(120%)}</style><g class="eventWrapper"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>.eventWrapper{filter:brightness(120%)}</style><g class="eventWrapper"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("the Timeline stylesheet brightness effect is supported");
        assert!(pair.into_parts().2.proves_semantics());
    }

    #[test]
    fn brightness_transfer_matches_resvg_component_transfer_quantization() {
        assert_eq!(
            RasterPaintColorTransfer::Brightness120Srgb.rendered_rgb([3, 127, 200]),
            [3, 152, 240]
        );
    }

    #[test]
    fn unsupported_brightness_component_transfer_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(110%)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(110%)"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn masked_brightness_component_transfer_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><mask id="mask"><rect width="20" height="20" fill="white"/></mask></defs><g mask="url(#mask)" style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><mask id="mask"><rect width="20" height="20" fill="white"/></mask></defs><g mask="url(#mask)" style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn nested_brightness_component_transfer_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(120%)"><g style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(120%)"><g style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn mismatched_brightness_context_fails_closed() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g style="filter:brightness(120%)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn unsupported_group_effects_fail_closed_before_raster_rendering() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><g filter="url(#blur)"><rect x="4" y="4" width="12" height="12" fill="#dc2626"/></g></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter></defs><g filter="url(#blur)"><rect x="4" y="4" width="12" height="12" fill="transparent"/></g></svg>"##;

        assert_cutover_error(encode_pair(
            solid,
            transparent,
            RasterPaintCutoverFacet::Fill,
            "#dc2626",
        ));
    }

    #[test]
    fn occluded_control_path_does_not_require_individual_pixels() {
        let solid = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="2" width="4" height="4" fill="#dc2626"/><rect x="10" y="10" width="6" height="6" fill="#dc2626"/><rect x="10" y="10" width="6" height="6" fill="#0f172a"/></svg>"##;
        let transparent = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><rect x="2" y="2" width="4" height="4" fill="transparent"/><rect x="10" y="10" width="6" height="6" fill="transparent"/><rect x="10" y="10" width="6" height="6" fill="#0f172a"/></svg>"##;

        let pair = encode_pair(solid, transparent, RasterPaintCutoverFacet::Fill, "#dc2626")
            .expect("occluded control paths remain a valid raster witness");

        let (_, _, receipt) = pair.into_parts();
        assert!(receipt.proves_semantics());
    }
}
