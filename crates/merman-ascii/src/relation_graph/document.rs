use super::horizontal::{
    HorizontalRelationPaintPlan, RelationGraphHorizontalDirection, horizontal_box_strip_lines,
    horizontal_box_strip_ref_extent,
};
use super::layered::{
    LayeredRelationRoutePlan, LayeredRelationScene, LayeredRelationSummaryReason,
};
use super::summary::{
    RelationGraphSummaryRow, relation_summary_extent, relation_summary_lines_for_rows,
};
use super::{
    RelationGraphBox, RelationGraphLine, RelationParallelPlan, RelationSelfLoopPlan,
    RelationSelfLoopRows, RelationStackPlan, grid_overflow, layout_allocation_failed,
    stacked_box_ref_extent, stacked_box_ref_lines,
};
use crate::Result;
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{LogicalExtent, ResourceContext};
use crate::text::{StyledLine, display_width_with_profile};

/// An admitted document assembly. Its materializer is intentionally supplied as
/// a closure so no `Vec<RelationGraphLine>` can be allocated before the checked
/// aggregate extent succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationDocumentPlan {
    extent: LogicalExtent,
    has_section: bool,
}

impl RelationDocumentPlan {
    pub(crate) fn new(
        base: LogicalExtent,
        section: Option<LogicalExtent>,
        section_title_width: usize,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let base = resources.grid_extent(base.width(), base.height())?;
        let section = section
            .map(|section| resources.grid_extent(section.width(), section.height()))
            .transpose()?;
        let (extent, has_section) = match section {
            Some(section) => {
                let has_section = section.height() > 0;
                let separator = usize::from(base.height() > 0 && has_section);
                let height = resources.checked_grid_add(base.height(), separator)?;
                let height = resources.checked_grid_add(height, usize::from(has_section))?;
                let height = resources.checked_grid_add(height, section.height())?;
                let width = if has_section {
                    base.width().max(section.width()).max(section_title_width)
                } else {
                    base.width().max(section.width())
                };
                (resources.grid_extent(width, height)?, has_section)
            }
            None => (base, false),
        };
        Ok(Self {
            extent,
            has_section,
        })
    }

    pub(crate) const fn extent(self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn materialize(
        self,
        resources: &mut ResourceContext,
        build_base: impl FnOnce(&mut ResourceContext) -> Result<Vec<RelationGraphLine>>,
    ) -> Result<Vec<RelationGraphLine>> {
        debug_assert!(!self.has_section);
        let lines = build_base(resources)?;
        self.check_materialized_extent(&lines, resources)?;
        Ok(lines)
    }

    pub(crate) fn materialize_with_section(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        build_base: impl FnOnce(&mut ResourceContext) -> Result<Vec<RelationGraphLine>>,
        build_section: impl FnOnce(&mut ResourceContext) -> Result<Vec<RelationGraphLine>>,
    ) -> Result<Vec<RelationGraphLine>> {
        debug_assert!(self.has_section);
        let mut lines = build_base(resources)?;
        let additional = self
            .extent
            .height()
            .checked_sub(lines.len())
            .ok_or_else(|| resources.grid_overflow())?;
        lines
            .try_reserve_exact(additional)
            .map_err(|_| layout_allocation_failed())?;
        let section_lines = build_section(resources)?;
        if !section_lines.is_empty() {
            if !lines.is_empty() {
                lines.push(RelationGraphLine::try_plain(
                    "",
                    options.terminal_width_profile,
                    resources,
                )?);
            }
            lines.push(RelationGraphLine::try_with_role(
                "relations:",
                AsciiColorRole::MutedText,
                options.terminal_width_profile,
                resources,
            )?);
            lines.extend(section_lines);
        }
        self.check_materialized_extent(&lines, resources)?;
        Ok(lines)
    }

    fn check_materialized_extent(
        self,
        lines: &[RelationGraphLine],
        resources: &ResourceContext,
    ) -> Result<()> {
        let width = lines
            .iter()
            .map(RelationGraphLine::width)
            .max()
            .unwrap_or(0);
        let actual = resources.grid_extent(width, lines.len())?;
        if actual != self.extent {
            return Err(grid_overflow(resources));
        }
        Ok(())
    }
}

type RelationStackRowsMaterializer<'a> =
    Box<dyn FnOnce(usize, &ResourceContext) -> Result<Vec<RelationGraphLine>> + 'a>;
type RelationParallelRowsMaterializer<'a> =
    Box<dyn FnOnce(&mut ResourceContext) -> Result<Vec<Vec<RelationGraphLine>>> + 'a>;
type RelationSelfLoopRowsMaterializer<'a> =
    Box<dyn FnOnce(&ResourceContext) -> Result<Vec<RelationSelfLoopRows>> + 'a>;

/// One independently planned relation region. Geometry and fallback selection
/// are fixed before the root document admits its aggregate extent; only the
/// family-owned row materializers remain deferred.
pub(crate) enum RelationRegionPlan<'a> {
    Vertical {
        plan: RelationStackPlan<'a>,
        rows: RelationStackRowsMaterializer<'a>,
    },
    Parallel {
        plan: RelationParallelPlan<'a>,
        lanes: RelationParallelRowsMaterializer<'a>,
    },
    SelfLoops {
        plan: RelationSelfLoopPlan<'a>,
        rows: RelationSelfLoopRowsMaterializer<'a>,
    },
    Layered(LayeredRelationPaintPlan<'a>),
    Horizontal(HorizontalRelationPaintPlan<'a>),
    HorizontalStrip {
        regions: Vec<RelationRegionPlan<'a>>,
        gap: usize,
        extent: LogicalExtent,
    },
    BoxStrip(RelationBoxStripPlan<'a>),
    Summary(RelationSummaryPaintPlan<'a>),
}

impl RelationRegionPlan<'_> {
    pub(crate) fn extent(&self) -> LogicalExtent {
        match self {
            Self::Vertical { plan, .. } => plan.extent(),
            Self::Parallel { plan, .. } => plan.extent(),
            Self::SelfLoops { plan, .. } => plan.extent(),
            Self::Layered(plan) => plan.extent(),
            Self::Horizontal(plan) => plan.extent(),
            Self::HorizontalStrip { extent, .. } => *extent,
            Self::BoxStrip(plan) => plan.extent(),
            Self::Summary(plan) => plan.extent(),
        }
    }

    fn paint(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        match self {
            Self::Vertical { plan, rows } => plan.render_lines(resources, rows),
            Self::Parallel { plan, lanes } => plan.render_lines(resources, lanes),
            Self::SelfLoops { plan, rows } => plan.render_lines(resources, rows),
            Self::Layered(plan) => plan.paint(options, resources),
            Self::Horizontal(plan) => plan.paint(options, resources),
            Self::HorizontalStrip {
                regions,
                gap,
                extent,
            } => paint_horizontal_relation_strip(regions, gap, extent, options, resources),
            Self::BoxStrip(plan) => plan.paint(options.terminal_width_profile, resources),
            Self::Summary(plan) => plan.paint(options, resources),
        }
    }

    pub(super) fn is_summary(&self) -> bool {
        matches!(self, Self::Summary(_))
    }

    pub(super) fn horizontal_strip<'a>(
        mut regions: Vec<RelationRegionPlan<'a>>,
        gap: usize,
        resources: &ResourceContext,
    ) -> Result<RelationRegionPlan<'a>> {
        regions.retain(|region| region.extent().height() > 0);
        let regions_width = regions.iter().try_fold(0usize, |width, region| {
            resources.checked_grid_add(width, region.extent().width())
        })?;
        let gaps_width = resources.checked_grid_mul(gap, regions.len().saturating_sub(1))?;
        let width = resources.checked_grid_add(regions_width, gaps_width)?;
        let height = regions
            .iter()
            .map(RelationRegionPlan::extent)
            .map(LogicalExtent::height)
            .max()
            .unwrap_or(0);
        let extent = resources.grid_extent(width, height)?;
        Ok(RelationRegionPlan::HorizontalStrip {
            regions,
            gap,
            extent,
        })
    }
}

fn paint_horizontal_relation_strip(
    regions: Vec<RelationRegionPlan<'_>>,
    gap: usize,
    extent: LogicalExtent,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(regions.len())
        .map_err(|_| layout_allocation_failed())?;
    for region in regions {
        let expected = region.extent();
        let lines = region.paint(options, resources)?;
        if lines
            .iter()
            .any(|line| line.width_profile() != options.terminal_width_profile)
            || relation_lines_extent(&lines, resources)? != expected
        {
            return Err(grid_overflow(resources));
        }
        blocks.push((expected, lines));
    }

    let mut rows = Vec::new();
    rows.try_reserve_exact(extent.height())
        .map_err(|_| layout_allocation_failed())?;
    for row_index in 0..extent.height() {
        let mut row = StyledLine::with_resources(options.terminal_width_profile, resources);
        for (block_index, (block_extent, lines)) in blocks.iter().enumerate() {
            if block_index > 0 {
                row.try_push_spaces(gap)?;
            }
            let Some(line) = lines.get(row_index) else {
                row.try_push_spaces(block_extent.width())?;
                continue;
            };
            row.try_push_line(line.styled())?;
            row.try_push_spaces(
                block_extent
                    .width()
                    .checked_sub(line.width())
                    .ok_or_else(|| grid_overflow(resources))?,
            )?;
        }
        rows.push(RelationGraphLine::from_styled(row));
    }
    Ok(rows)
}

/// A vertically separated relation document admitted before any region creates
/// rows or a canvas.
pub(crate) struct RelationRenderPlan<'a> {
    regions: Vec<RelationRegionPlan<'a>>,
    extent: LogicalExtent,
}

impl<'a> RelationRenderPlan<'a> {
    pub(crate) fn try_new(
        mut regions: Vec<RelationRegionPlan<'a>>,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        regions.retain(|region| region.extent().height() > 0);
        let separators = regions.len().saturating_sub(1);
        let height = regions.iter().try_fold(separators, |height, region| {
            resources.checked_grid_add(height, region.extent().height())
        })?;
        let width = regions
            .iter()
            .map(RelationRegionPlan::extent)
            .map(LogicalExtent::width)
            .max()
            .unwrap_or(0);
        let extent = resources.grid_extent(width, height)?;
        resources.charge_layout_work(extent.cells())?;
        Ok(Self { regions, extent })
    }

    #[cfg(test)]
    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(crate) fn materialize(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let mut joined = Vec::new();
        joined
            .try_reserve_exact(self.extent.height())
            .map_err(|_| layout_allocation_failed())?;
        for region in self.regions {
            if !joined.is_empty() {
                joined.push(RelationGraphLine::try_plain(
                    "",
                    options.terminal_width_profile,
                    resources,
                )?);
            }
            let expected = region.extent();
            let lines = region.paint(options, resources)?;
            if lines
                .iter()
                .any(|line| line.width_profile() != options.terminal_width_profile)
                || relation_lines_extent(&lines, resources)? != expected
            {
                return Err(grid_overflow(resources));
            }
            joined.extend(lines);
        }
        if relation_lines_extent(&joined, resources)? != self.extent {
            return Err(grid_overflow(resources));
        }
        Ok(joined)
    }
}

pub(crate) enum RelationBoxStripPlan<'a> {
    Stacked {
        boxes: Vec<&'a RelationGraphBox>,
        extent: LogicalExtent,
    },
    Horizontal {
        boxes: Vec<&'a RelationGraphBox>,
        direction: RelationGraphHorizontalDirection,
        gap: usize,
        width_profile: TerminalWidthProfile,
        extent: LogicalExtent,
    },
}

impl<'a> RelationBoxStripPlan<'a> {
    pub(crate) fn stacked(
        boxes: Vec<&'a RelationGraphBox>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let extent = stacked_box_ref_extent(&boxes, resources)?;
        Ok(Self::Stacked { boxes, extent })
    }

    pub(crate) fn horizontal(
        boxes: Vec<&'a RelationGraphBox>,
        direction: RelationGraphHorizontalDirection,
        gap: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let extent = horizontal_box_strip_ref_extent(&boxes, gap, resources)?;
        Ok(Self::Horizontal {
            boxes,
            direction,
            gap,
            width_profile,
            extent,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        match self {
            Self::Stacked { extent, .. } | Self::Horizontal { extent, .. } => *extent,
        }
    }

    fn paint(
        self,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        match self {
            Self::Stacked { boxes, .. } => stacked_box_ref_lines(&boxes, width_profile, resources),
            Self::Horizontal {
                boxes,
                direction,
                gap,
                width_profile,
                ..
            } => horizontal_box_strip_lines(&boxes, direction, gap, width_profile, resources),
        }
    }
}

enum RelationSummaryBase<'a> {
    Stacked(Vec<&'a RelationGraphBox>),
    Horizontal {
        boxes: Vec<&'a RelationGraphBox>,
        direction: RelationGraphHorizontalDirection,
        gap: usize,
    },
}

pub(crate) struct RelationSummaryPaintPlan<'a> {
    base: RelationSummaryBase<'a>,
    rows: Vec<RelationGraphSummaryRow>,
    reason: Option<LayeredRelationSummaryReason>,
    document: RelationDocumentPlan,
}

impl<'a> RelationSummaryPaintPlan<'a> {
    pub(crate) fn stacked(
        boxes: Vec<&'a RelationGraphBox>,
        rows: Vec<RelationGraphSummaryRow>,
        reason: Option<LayeredRelationSummaryReason>,
        options: &AsciiRenderOptions,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let base_extent = stacked_box_ref_extent(&boxes, resources)?;
        let summary_extent = relation_summary_extent(&rows, reason, options, resources)?;
        let document = RelationDocumentPlan::new(
            base_extent,
            Some(summary_extent),
            display_width_with_profile("relations:", options.terminal_width_profile),
            resources,
        )?;
        Ok(Self {
            base: RelationSummaryBase::Stacked(boxes),
            rows,
            reason,
            document,
        })
    }

    pub(crate) fn horizontal(
        boxes: Vec<&'a RelationGraphBox>,
        direction: RelationGraphHorizontalDirection,
        gap: usize,
        rows: Vec<RelationGraphSummaryRow>,
        reason: Option<LayeredRelationSummaryReason>,
        options: &AsciiRenderOptions,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let base_extent = horizontal_box_strip_ref_extent(&boxes, gap, resources)?;
        let summary_extent = relation_summary_extent(&rows, reason, options, resources)?;
        let document = RelationDocumentPlan::new(
            base_extent,
            Some(summary_extent),
            display_width_with_profile("relations:", options.terminal_width_profile),
            resources,
        )?;
        Ok(Self {
            base: RelationSummaryBase::Horizontal {
                boxes,
                direction,
                gap,
            },
            rows,
            reason,
            document,
        })
    }

    pub(crate) const fn extent(&self) -> LogicalExtent {
        self.document.extent()
    }

    fn paint(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let Self {
            base,
            rows,
            reason,
            document,
        } = self;
        match base {
            RelationSummaryBase::Stacked(boxes) => document.materialize_with_section(
                options,
                resources,
                |resources| {
                    stacked_box_ref_lines(&boxes, options.terminal_width_profile, resources)
                },
                |resources| relation_summary_lines_for_rows(&rows, reason, options, resources),
            ),
            RelationSummaryBase::Horizontal {
                boxes,
                direction,
                gap,
            } => document.materialize_with_section(
                options,
                resources,
                |resources| {
                    horizontal_box_strip_lines(
                        &boxes,
                        direction,
                        gap,
                        options.terminal_width_profile,
                        resources,
                    )
                },
                |resources| relation_summary_lines_for_rows(&rows, reason, options, resources),
            ),
        }
    }
}

pub(crate) struct LayeredRelationPaintPlan<'a> {
    pub(super) scene: LayeredRelationScene<'a>,
    pub(super) routes: Vec<LayeredRelationRoutePlan>,
    pub(super) extent: LogicalExtent,
}

impl<'a> LayeredRelationPaintPlan<'a> {
    fn extent(&self) -> LogicalExtent {
        self.extent
    }

    pub(super) fn paint(
        self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationGraphLine>> {
        let Self {
            scene,
            routes,
            extent: _,
        } = self;
        let mut canvas = scene.canvas_with_boxes(options, resources)?;
        for route in &routes {
            route.draw_route_at(&mut canvas)?;
        }
        for route in &routes {
            route.draw_overlays_at(&mut canvas)?;
        }
        let styled_lines = canvas.into_styled_lines_preserving_extent()?;
        resources.charge_layout_work(styled_lines.len().max(1))?;
        let mut rendered = Vec::new();
        rendered
            .try_reserve_exact(styled_lines.len())
            .map_err(|_| layout_allocation_failed())?;
        rendered.extend(styled_lines.into_iter().map(RelationGraphLine::from_styled));
        Ok(rendered)
    }
}

pub(super) fn relation_lines_extent(
    lines: &[RelationGraphLine],
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let width = lines
        .iter()
        .map(RelationGraphLine::width)
        .max()
        .unwrap_or(0);
    resources.grid_extent(width, lines.len())
}
