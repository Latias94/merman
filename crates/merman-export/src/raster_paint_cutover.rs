use sha2::{Digest as _, Sha256};

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

/// Opaque exporter-owned evidence for one solid/transparent paint pair.
///
/// The exporter resolves the final SVG through `usvg`, identifies the unique control paint on
/// renderable paths, derives an underlay reference by replacing that control paint with
/// `transparent`, and compares the actual transparent render with that reference. The receipt is
/// deliberately family-neutral; the Merman facade binds it to renderer-owned route receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterPaintCutoverReceipt {
    facet: RasterPaintCutoverFacet,
    control_rgb: [u8; 3],
    solid_source_digest: [u8; 32],
    transparent_source_digest: [u8; 32],
    underlay_source_digest: [u8; 32],
    underlay_path_tree_digest: [u8; 32],
    transparent_path_tree_digest: [u8; 32],
    solid_effect_tree_digest: [u8; 32],
    underlay_effect_tree_digest: [u8; 32],
    transparent_effect_tree_digest: [u8; 32],
    transparent_geometry_compatible: bool,
    target_geometry_digest: [u8; 32],
    target_path_count: usize,
    solid_control_pixels: usize,
    changed_control_pixels: usize,
    requested_changed_control_pixels: usize,
    changed_pixels: usize,
    changed_outside_target_pixels: usize,
    transparent_control_pixels: usize,
    unchanged_control_pixels: usize,
    transparent_reference_mismatches: usize,
    digest: [u8; 32],
}

impl RasterPaintCutoverReceipt {
    fn seal(facts: RasterPaintCutoverFacts) -> Option<Self> {
        let mut receipt = Self {
            facet: facts.facet,
            control_rgb: facts.control_rgb,
            solid_source_digest: facts.solid_source_digest,
            transparent_source_digest: facts.transparent_source_digest,
            underlay_source_digest: facts.underlay_source_digest,
            underlay_path_tree_digest: facts.underlay_path_tree_digest,
            transparent_path_tree_digest: facts.transparent_path_tree_digest,
            solid_effect_tree_digest: facts.solid_effect_tree_digest,
            underlay_effect_tree_digest: facts.underlay_effect_tree_digest,
            transparent_effect_tree_digest: facts.transparent_effect_tree_digest,
            transparent_geometry_compatible: facts.transparent_geometry_compatible,
            target_geometry_digest: facts.target_geometry_digest,
            target_path_count: facts.target_path_count,
            solid_control_pixels: facts.solid_control_pixels,
            changed_control_pixels: facts.changed_control_pixels,
            requested_changed_control_pixels: facts.requested_changed_control_pixels,
            changed_pixels: facts.changed_pixels,
            changed_outside_target_pixels: facts.changed_outside_target_pixels,
            transparent_control_pixels: facts.transparent_control_pixels,
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
            && self.transparent_geometry_compatible
            && self.target_geometry_digest != [0; 32]
            && self.target_path_count > 0
            && self.solid_control_pixels > 0
            && self.changed_control_pixels > 0
            && self.requested_changed_control_pixels > 0
            && self.changed_pixels > 0
            && self.changed_outside_target_pixels == 0
            && self.transparent_control_pixels == 0
            && self.transparent_reference_mismatches == 0
            && self.digest == self.canonical_digest()
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, b"merman.raster-paint-cutover-receipt.v6");
        update_len_prefixed(&mut hasher, self.facet.id());
        hasher.update(self.control_rgb);
        hasher.update(self.solid_source_digest);
        hasher.update(self.transparent_source_digest);
        hasher.update(self.underlay_source_digest);
        hasher.update(self.underlay_path_tree_digest);
        hasher.update(self.transparent_path_tree_digest);
        hasher.update(self.solid_effect_tree_digest);
        hasher.update(self.underlay_effect_tree_digest);
        hasher.update(self.transparent_effect_tree_digest);
        hasher.update([u8::from(self.transparent_geometry_compatible)]);
        hasher.update(self.target_geometry_digest);
        update_usize(&mut hasher, self.target_path_count);
        update_usize(&mut hasher, self.solid_control_pixels);
        update_usize(&mut hasher, self.changed_control_pixels);
        update_usize(&mut hasher, self.requested_changed_control_pixels);
        update_usize(&mut hasher, self.changed_pixels);
        update_usize(&mut hasher, self.changed_outside_target_pixels);
        update_usize(&mut hasher, self.transparent_control_pixels);
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
    facet: RasterPaintCutoverFacet,
    control_css: &str,
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
    run_recursive_svg_backend(&control, move |backend_control| {
        encode_pair_on_backend_stack(
            &solid_svg,
            &transparent_svg,
            &options,
            backend_control,
            facet,
            &control_css,
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
    facet: RasterPaintCutoverFacet,
    control_css: &str,
    control_rgb: [u8; 3],
) -> Result<EncodedRasterPaintCutoverPair> {
    let solid_source = native_export_svg(solid_svg);
    let transparent_source = native_export_svg(transparent_svg);
    let underlay_source = replace_control_paint(solid_source, control_css, control_rgb)?;

    let solid = prepare_raster_source_on_backend_stack(solid_svg, solid_source, options, control)?;
    let solid_placement = RasterPlacement::from_prepared(&solid);
    let solid_tree = observe_paint_tree(&solid.tree, control_rgb, facet)?;
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
    let solid_bytes = solid_pixmap
        .encode_png()
        .map_err(|_| ExportError::PngEncode)?;
    export_checkpoint(control)?;

    let underlay =
        prepare_raster_source_on_backend_stack(solid_svg, &underlay_source, options, control)?;
    require_same_placement(solid_placement, RasterPlacement::from_prepared(&underlay))?;
    let underlay_tree = observe_paint_tree(&underlay.tree, control_rgb, facet)?;
    require_no_opaque_control_targets(&underlay_tree)?;
    require_solid_underlay_path_compatibility(&solid_tree, &underlay_tree)?;
    let underlay_pixmap = underlay.render_pixmap(underlay.matte, control)?;

    let solid_delta = observe_solid_delta(
        &solid_pixmap,
        &underlay_pixmap,
        &solid_tree.targets,
        &solid_tree.requested_targets,
        solid_placement,
        control_rgb,
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
    let transparent_tree = observe_paint_tree(&transparent.tree, control_rgb, facet)?;
    require_no_opaque_control_targets(&transparent_tree)?;
    if transparent_tree.effect_tree_digest != underlay_tree.effect_tree_digest {
        return Err(ExportError::RasterPaintCutover(
            "underlay and transparent SVG effect contexts differ",
        ));
    }
    require_transparent_path_compatibility(&underlay_tree, &transparent_tree, &solid_tree)?;
    let transparent_report = transparent.report_for_output(RasterOutputKind::Png);
    let transparent_pixmap = transparent.render_pixmap(transparent.matte, control)?;
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
    let transparent_control_pixels = count_control_pixels_in_regions(
        &transparent_pixmap,
        &solid_tree.targets,
        solid_placement,
        control_rgb,
    )?;
    if transparent_control_pixels != 0 {
        return Err(ExportError::RasterPaintCutover(
            "transparent PNG retained the solid control paint",
        ));
    }
    let transparent_bytes = transparent_pixmap
        .encode_png()
        .map_err(|_| ExportError::PngEncode)?;
    export_checkpoint(control)?;

    let receipt = RasterPaintCutoverReceipt::seal(RasterPaintCutoverFacts {
        facet,
        control_rgb,
        solid_source_digest: Sha256::digest(solid_source.as_bytes()).into(),
        transparent_source_digest: Sha256::digest(transparent_source.as_bytes()).into(),
        underlay_source_digest: Sha256::digest(underlay_source.as_bytes()).into(),
        underlay_path_tree_digest: underlay_tree.path_tree_digest,
        transparent_path_tree_digest: transparent_tree.path_tree_digest,
        solid_effect_tree_digest: solid_tree.effect_tree_digest,
        underlay_effect_tree_digest: underlay_tree.effect_tree_digest,
        transparent_effect_tree_digest: transparent_tree.effect_tree_digest,
        transparent_geometry_compatible: true,
        target_geometry_digest: solid_tree.target_geometry_digest,
        target_path_count: solid_tree.targets.len(),
        solid_control_pixels: solid_delta.control_pixels,
        changed_pixels: solid_delta.changed_pixels,
        changed_control_pixels: solid_delta.changed_control_pixels,
        requested_changed_control_pixels: solid_delta.requested_changed_control_pixels,
        changed_outside_target_pixels: solid_delta.changed_outside_target_pixels,
        transparent_control_pixels,
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
    semantic_paint_binding: SemanticPaintBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticPaintBinding {
    Native,
    FillFromStroke,
    FillAndStrokeFromStroke,
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
    region_bits: [u32; 4],
}

impl TargetPath {
    fn region(&self) -> [f32; 4] {
        self.region_bits.map(f32::from_bits)
    }
}

impl PathObservation {
    fn paint(&self, facet: RasterPaintCutoverFacet) -> Option<PaintObservation> {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                SemanticPaintBinding::Native => self.fill,
                SemanticPaintBinding::FillFromStroke
                | SemanticPaintBinding::FillAndStrokeFromStroke => self.stroke,
            },
            RasterPaintCutoverFacet::Stroke => match self.semantic_paint_binding {
                SemanticPaintBinding::FillFromStroke => None,
                SemanticPaintBinding::Native | SemanticPaintBinding::FillAndStrokeFromStroke => {
                    self.stroke
                }
            },
        }
    }

    fn geometry(&self, facet: RasterPaintCutoverFacet) -> Option<[u8; 32]> {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                SemanticPaintBinding::Native => Some(self.fill_geometry_digest),
                SemanticPaintBinding::FillFromStroke
                | SemanticPaintBinding::FillAndStrokeFromStroke => self.stroke_geometry_digest,
            },
            RasterPaintCutoverFacet::Stroke => match self.semantic_paint_binding {
                SemanticPaintBinding::FillFromStroke => None,
                SemanticPaintBinding::Native | SemanticPaintBinding::FillAndStrokeFromStroke => {
                    self.stroke_geometry_digest
                }
            },
        }
    }

    fn region(&self, facet: RasterPaintCutoverFacet) -> [u32; 4] {
        match facet {
            RasterPaintCutoverFacet::Fill => match self.semantic_paint_binding {
                SemanticPaintBinding::Native => self.fill_region_bits,
                SemanticPaintBinding::FillFromStroke
                | SemanticPaintBinding::FillAndStrokeFromStroke => self.stroke_region_bits,
            },
            RasterPaintCutoverFacet::Stroke => self.stroke_region_bits,
        }
    }
}

fn observe_paint_tree(
    tree: &usvg::Tree,
    control_rgb: [u8; 3],
    requested_facet: RasterPaintCutoverFacet,
) -> Result<PaintTreeObservation> {
    let mut paths = Vec::new();
    let mut effect_hasher = Sha256::new();
    update_len_prefixed(&mut effect_hasher, b"merman.raster-paint-effect-tree.v1");
    hash_tree_clip_paths(tree, &mut effect_hasher)?;
    collect_group_paths(tree.root(), &mut paths, &mut effect_hasher)?;
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
        b"merman.raster-paint-target-geometry.v1",
    );
    let mut requested_targets = Vec::new();
    for (path_index, path) in paths.iter().enumerate() {
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
        target_hasher.update([u8::from(fill_control), u8::from(stroke_control)]);
        for coordinate in region_bits {
            target_hasher.update(coordinate.to_be_bytes());
        }
        targets.push(TargetPath { region_bits });
        target_facets_by_path[path_index] = [fill_control, stroke_control];

        let requested_region_bits = match requested_facet {
            RasterPaintCutoverFacet::Fill if fill_control => {
                Some(path.region(RasterPaintCutoverFacet::Fill))
            }
            RasterPaintCutoverFacet::Stroke if stroke_control => {
                Some(path.region(RasterPaintCutoverFacet::Stroke))
            }
            RasterPaintCutoverFacet::Fill | RasterPaintCutoverFacet::Stroke => None,
        };
        if let Some(requested_region_bits) = requested_region_bits {
            requested_targets.push(TargetPath {
                region_bits: requested_region_bits,
            });
        }
    }
    update_usize(&mut target_hasher, targets.len());

    Ok(PaintTreeObservation {
        path_tree_digest,
        shape_tree_digest,
        effect_tree_digest,
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

fn collect_group_paths(
    group: &usvg::Group,
    paths: &mut Vec<PathObservation>,
    effect_hasher: &mut Sha256,
) -> Result<()> {
    collect_group_paths_with_transform(
        group,
        paths,
        None,
        SemanticPaintBinding::Native,
        effect_hasher,
    )
}

fn collect_group_paths_with_transform(
    group: &usvg::Group,
    paths: &mut Vec<PathObservation>,
    extra_transform: Option<tiny_skia::Transform>,
    inherited_binding: SemanticPaintBinding,
    effect_hasher: &mut Sha256,
) -> Result<()> {
    hash_group_effects(group, effect_hasher)?;
    let group_binding = semantic_paint_binding_from_id(group.id()).unwrap_or(inherited_binding);
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect_group_paths_with_transform(
                child,
                paths,
                extra_transform,
                group_binding,
                effect_hasher,
            )?,
            usvg::Node::Path(path) => {
                paths.push(observe_path_with_binding(
                    path,
                    extra_transform,
                    group_binding,
                )?);
            }
            // `usvg` keeps the glyph paths in a text node's flattened group in local
            // coordinates. The text node's absolute translation is applied by `resvg` during
            // rendering, but is not present on those flattened paths themselves.
            usvg::Node::Text(text) => collect_group_paths_with_transform(
                text.flattened(),
                paths,
                Some(text.abs_transform()),
                group_binding,
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

fn hash_group_effects(group: &usvg::Group, hasher: &mut Sha256) -> Result<()> {
    if group.mask().is_some() || !group.filters().is_empty() {
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
    hash_group_effects(group, hasher)?;
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
    observe_path_with_binding(path, extra_transform, SemanticPaintBinding::Native)
}

fn observe_path_with_binding(
    path: &usvg::Path,
    extra_transform: Option<tiny_skia::Transform>,
    inherited_binding: SemanticPaintBinding,
) -> Result<PathObservation> {
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
    update_len_prefixed(&mut path_hasher, b"merman.raster-paint-path-structure.v2");
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

    let semantic_paint_binding =
        semantic_paint_binding_from_id(path.id()).unwrap_or(inherited_binding);

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
    })
}

fn semantic_paint_binding_from_id(id: &str) -> Option<SemanticPaintBinding> {
    if id.ends_with(merman_render::svg::RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX) {
        Some(SemanticPaintBinding::FillAndStrokeFromStroke)
    } else if id.ends_with(merman_render::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX) {
        Some(SemanticPaintBinding::FillFromStroke)
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
    placement: RasterPlacement,
    control_rgb: [u8; 3],
) -> Result<SolidDeltaObservation> {
    if solid.width() != underlay.width() || solid.height() != underlay.height() {
        return Err(ExportError::RasterPaintCutover(
            "solid and underlay pixmap dimensions differ",
        ));
    }
    let regions = pixel_regions(targets, placement)?;
    let requested_regions = pixel_regions(requested_targets, placement)?;
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
        let is_control = pixel_near_rgb(*solid_pixel, control_rgb, COLOR_TOLERANCE);
        let changed = solid_pixel != underlay_pixel;
        let inside_any_region = regions.iter().any(|region| region.contains(x, y));
        if is_control {
            control_pixels = control_pixels.saturating_add(1);
            if changed {
                changed_control_pixels = changed_control_pixels.saturating_add(1);
                if requested_regions.iter().any(|region| region.contains(x, y)) {
                    requested_changed_control_pixels =
                        requested_changed_control_pixels.saturating_add(1);
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

fn count_control_pixels_in_regions(
    pixmap: &tiny_skia::Pixmap,
    targets: &[TargetPath],
    placement: RasterPlacement,
    control_rgb: [u8; 3],
) -> Result<usize> {
    let regions = pixel_regions(targets, placement)?;
    let width = pixmap.width() as usize;
    Ok(pixmap
        .pixels()
        .iter()
        .enumerate()
        .filter(|(index, pixel)| {
            let x = (*index % width) as u32;
            let y = (*index / width) as u32;
            regions.iter().any(|region| region.contains(x, y))
                && pixel_near_rgb(**pixel, control_rgb, COLOR_TOLERANCE)
        })
        .count())
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
    facet: RasterPaintCutoverFacet,
    control_rgb: [u8; 3],
    solid_source_digest: [u8; 32],
    transparent_source_digest: [u8; 32],
    underlay_source_digest: [u8; 32],
    underlay_path_tree_digest: [u8; 32],
    transparent_path_tree_digest: [u8; 32],
    solid_effect_tree_digest: [u8; 32],
    underlay_effect_tree_digest: [u8; 32],
    transparent_effect_tree_digest: [u8; 32],
    transparent_geometry_compatible: bool,
    target_geometry_digest: [u8; 32],
    target_path_count: usize,
    solid_control_pixels: usize,
    changed_control_pixels: usize,
    requested_changed_control_pixels: usize,
    changed_pixels: usize,
    changed_outside_target_pixels: usize,
    transparent_control_pixels: usize,
    unchanged_control_pixels: usize,
    transparent_reference_mismatches: usize,
}

fn hash_rect(hasher: &mut Sha256, rect: usvg::Rect) {
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
        encode_png_paint_cutover_pair_controlled(
            &compatible_svg(solid),
            &compatible_svg(transparent),
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
            facet,
            control_css,
        )
    }

    fn assert_cutover_error(result: Result<EncodedRasterPaintCutoverPair>) {
        assert!(matches!(result, Err(ExportError::RasterPaintCutover(_))));
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
