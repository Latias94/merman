use super::*;
use crate::canvas::Canvas;
use crate::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};
use std::cell::Cell;

mod admission;
mod components;
mod layered;
mod render_model;

struct TestRelationAdapter {
    summary_reason: Cell<Option<LayeredRelationSummaryReason>>,
    overlap: TestRelationOverlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestRelationOverlap {
    None,
    Route,
    Overlay,
}

trait TestRelationEndpoints {
    fn source_id(&self) -> &'static str;
    fn target_id(&self) -> &'static str;
}

impl TestRelationEndpoints for (&'static str, &'static str) {
    fn source_id(&self) -> &'static str {
        self.0
    }

    fn target_id(&self) -> &'static str {
        self.1
    }
}

struct NonCloneTestRelation {
    source_id: &'static str,
    target_id: &'static str,
}

impl TestRelationEndpoints for NonCloneTestRelation {
    fn source_id(&self) -> &'static str {
        self.source_id
    }

    fn target_id(&self) -> &'static str {
        self.target_id
    }
}

impl<R> RelationComponentAdapter<R> for TestRelationAdapter
where
    R: TestRelationEndpoints,
{
    fn build_edges(&self, relation: &R) -> LayeredRelationEdge {
        LayeredRelationEdge::new(relation.source_id(), relation.target_id(), 0, 0)
    }

    fn is_self_relation(&self, relation: &R) -> bool {
        relation.source_id() == relation.target_id()
    }

    fn self_loop_metrics(
        &self,
        _relation: &R,
        _resources: &ResourceContext,
    ) -> Result<RelationSelfLoopMetrics> {
        Ok(RelationSelfLoopMetrics::new(1, 0, 0, 1, None, '-', '|'))
    }

    fn self_loop_rows(
        &self,
        _relation: &R,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopRows> {
        let line = RelationGraphLine::try_with_role(
            "-",
            AsciiColorRole::EdgeLine,
            TerminalWidthProfile::Unicode,
            resources,
        )?;
        Ok(RelationSelfLoopRows::new(
            line.clone(),
            Vec::new(),
            line,
            '-',
            '|',
        ))
    }

    fn horizontal_relation_style(
        &self,
        _relation: &R,
        _source_side: RelationPortSide,
        _target_side: RelationPortSide,
        _resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle> {
        Ok(HorizontalRelationStyle::new(
            HorizontalRelationEndpoint::new(None, None),
            HorizontalRelationEndpoint::new(None, None),
            None,
            '-',
            '|',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
        ))
    }

    fn layered_horizontal_gap(&self) -> usize {
        1
    }

    fn layered_route_style(&self, _relation: &R) -> Result<LayeredRelationRouteStyle> {
        if self.overlap == TestRelationOverlap::Route {
            return Ok(LayeredRelationRouteStyle::new(
                'X',
                'X',
                RelationLineChars::new(['X', 'X', 'X', 'X'], 'X'),
                LayeredRelationRouteProfile::new(1, 0, 1, 0, 0),
            ));
        }

        Ok(LayeredRelationRouteStyle::new(
            '-',
            '-',
            RelationLineChars::new(['-', '-', '-', '-'], '+'),
            LayeredRelationRouteProfile::class(),
        ))
    }

    fn layered_relation_overlays(
        &self,
        _relation: &R,
        _geometry: &LayeredRelationRouteGeometry,
        _resources: &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>> {
        if self.overlap == TestRelationOverlap::Overlay {
            return Ok(vec![RelationOverlay::glyph(
                _geometry.source_x(),
                0,
                'X',
                AsciiColorRole::EdgeLine,
            )]);
        }

        Ok(Vec::new())
    }

    fn plan_vertical_region<'plan>(
        &self,
        boxes: &[&'plan RelationGraphBox],
        relation: &'plan R,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        let top = find_box_ref(boxes, relation.source_id()).ok_or_else(|| {
            <Self as RelationComponentAdapter<R>>::layered_error(
                self,
                LayeredRelationError::MissingEndpoint,
            )
        })?;
        let bottom = find_box_ref(boxes, relation.target_id()).ok_or_else(|| {
            <Self as RelationComponentAdapter<R>>::layered_error(
                self,
                LayeredRelationError::MissingEndpoint,
            )
        })?;
        let plan =
            RelationStackPlan::try_new(top, bottom, &[], resources, |_center, resources| {
                resources.grid_extent(0, 0)
            })?;
        Ok(RelationRegionPlan::Vertical {
            plan,
            rows: Box::new(|_center, _resources| Ok(Vec::new())),
        })
    }

    fn plan_parallel_region<'plan>(
        &self,
        boxes: Vec<&'plan RelationGraphBox>,
        _relations: Vec<&'plan R>,
        _options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        Ok(RelationRegionPlan::BoxStrip(RelationBoxStripPlan::stacked(
            boxes, resources,
        )?))
    }

    fn build_summary_row(
        &self,
        _relation: &R,
        reason: LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow> {
        self.summary_reason.set(Some(reason));
        Ok(RelationGraphSummaryRow::new("A", "-->", "B"))
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        AsciiError::UnsupportedFeature {
            diagram_type: "test",
            feature: match error {
                LayeredRelationError::MissingEndpoint => "missing endpoint",
                LayeredRelationError::UnrelatedBoxes => "unrelated boxes",
                LayeredRelationError::Crossing => "crossing",
            },
        }
    }
}

fn test_resources(options: &AsciiRenderOptions) -> ResourceContext {
    ResourceContext::new(options.resources)
}

fn options_with_grid_limit(max: usize) -> AsciiRenderOptions {
    AsciiRenderOptions::ascii()
        .with_resource_limit(AsciiResourceLimitId::MaxGridCells, max)
        .expect("test grid limit should be valid")
}

fn assert_grid_limit(error: AsciiError, actual: usize, max: usize) {
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxGridCells
                && details.actual == actual
                && details.max == max
    ));
}

fn admission_test_boxes() -> Vec<RelationGraphBox> {
    vec![
        RelationGraphBox::new(
            "a".to_string(),
            vec!["aaa".to_string(), "aaa".to_string()],
            3,
        ),
        RelationGraphBox::new(
            "b".to_string(),
            vec!["bbbb".to_string(), "bbbb".to_string(), "bbbb".to_string()],
            4,
        ),
    ]
}
