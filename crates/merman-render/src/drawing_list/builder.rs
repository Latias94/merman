use crate::environment::RenderSession;
use crate::{Error, Result};
use merman_core::OperationPhase;
#[cfg(test)]
use merman_display_list::DrawingListLimits;
use merman_display_list::{
    CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument, DrawingListError,
    DrawingListFootprint, DrawingListPolicy, DrawingResource, LinearGradientResource, PathResource,
    PathSegment, PathStyle, ResourceId, SemanticAnnotation, TextObligation, TextRun, Viewport,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// Operation-local owner for one bounded DrawingList candidate.
///
/// The backing collections never escape this module. Adapters can deepen this interface when
/// they introduce additional resource or scope kinds without weakening the final protocol
/// validator.
pub(crate) struct DrawingListBuilder<'a> {
    budget: super::DocumentBudget,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    usage: DrawingListFootprint,
    scopes: Vec<BuildScope>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> DrawingListBuilder<'a> {
    pub(crate) fn new(
        policy: DrawingListPolicy,
        limits: impl Into<super::DocumentBudget>,
        session: &'a RenderSession,
    ) -> Self {
        Self {
            budget: limits.into(),
            session,
            policy,
            usage: DrawingListFootprint::default(),
            scopes: Vec::new(),
            resources: Vec::new(),
            commands: Vec::new(),
            semantics: Vec::new(),
        }
    }

    pub(crate) fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Adds a command that does not own variable-sized drawing payload.
    ///
    /// Paths and text must use their bounded entry points so callers cannot allocate those
    /// payloads before the relevant DrawingList limit is checked.
    pub(crate) fn push_control(&mut self, command: DrawingCommand) -> Result<()> {
        if matches!(
            &command,
            DrawingCommand::DrawPath { .. }
                | DrawingCommand::DrawImage { .. }
                | DrawingCommand::DrawText { .. }
                | DrawingCommand::DrawRasterSubtree { .. }
                | DrawingCommand::ClipPath { .. }
        ) {
            return Err(contract_error(
                "DrawingList payload command bypassed its bounded builder entry point",
            ));
        }
        let scope_change = self.preview_scope_change(&command)?;
        let mut next = self.usage;
        checked_increment(&mut next.commands, 1, "command count")?;
        if matches!(scope_change, ScopeChange::Push(_)) {
            next.max_nesting_depth = next.max_nesting_depth.max(
                self.scopes
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| contract_error("DrawingList nesting depth overflows usize"))?,
            );
        }
        self.preflight(next)?;

        self.commands
            .try_reserve(1)
            .map_err(|_| allocation_failed("commands"))?;
        if matches!(scope_change, ScopeChange::Push(_)) {
            self.scopes
                .try_reserve(1)
                .map_err(|_| allocation_failed("command scopes"))?;
        }
        self.apply_scope_change(scope_change);
        self.usage = next;
        self.commands.push(command);
        Ok(())
    }

    pub(crate) fn push_semantic(&mut self, semantic: SemanticAnnotation) -> Result<()> {
        let mut next = self.usage;
        checked_increment(&mut next.semantics, 1, "semantic count")?;
        self.preflight(next)?;
        self.semantics
            .try_reserve(1)
            .map_err(|_| allocation_failed("semantics"))?;
        self.usage = next;
        self.semantics.push(semantic);
        Ok(())
    }

    /// Adds an already-materialized path without cloning its segment storage.
    ///
    /// Use `draw_path_with` when input can amplify into a variable-sized path.
    pub(crate) fn draw_path(
        &mut self,
        id: ResourceId,
        segments: Vec<PathSegment>,
        style: merman_display_list::PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(contract_error("DrawingList path has no geometry"));
        }
        let mut next = self.usage;
        checked_increment(&mut next.resources, 1, "resource count")?;
        checked_increment(&mut next.commands, 1, "command count")?;
        checked_increment(
            &mut next.path_segments,
            segments.len(),
            "path segment count",
        )?;
        if let Some(stroke) = &style.stroke {
            checked_increment(
                &mut next.stroke_dash_entries,
                stroke.dash_array.len(),
                "stroke dash entry count",
            )?;
        }
        self.preflight(next)?;

        self.resources
            .try_reserve(1)
            .map_err(|_| allocation_failed("resources"))?;
        self.commands
            .try_reserve(1)
            .map_err(|_| allocation_failed("commands"))?;
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        self.usage = next;
        Ok(())
    }

    /// Adds one path resource with multiple paint commands, preserving source painter order.
    ///
    /// Some SVG families paint a shape fill and stroke as separate elements while sharing the
    /// same geometry. Keeping that layering in the protocol avoids changing DOM order merely to
    /// satisfy the bounded builder.
    pub(crate) fn draw_path_layers(
        &mut self,
        id: ResourceId,
        segments: Vec<PathSegment>,
        styles: Vec<PathStyle>,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(contract_error("DrawingList path has no geometry"));
        }
        if styles.is_empty() {
            return Err(contract_error("DrawingList path has no paint commands"));
        }
        let mut next = self.usage;
        checked_increment(&mut next.resources, 1, "resource count")?;
        checked_increment(&mut next.commands, styles.len(), "command count")?;
        checked_increment(
            &mut next.path_segments,
            segments.len(),
            "path segment count",
        )?;
        for style in &styles {
            if let Some(stroke) = &style.stroke {
                checked_increment(
                    &mut next.stroke_dash_entries,
                    stroke.dash_array.len(),
                    "stroke dash entry count",
                )?;
            }
        }
        self.preflight(next)?;

        self.resources
            .try_reserve(1)
            .map_err(|_| allocation_failed("resources"))?;
        self.commands
            .try_reserve(styles.len())
            .map_err(|_| allocation_failed("commands"))?;
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        for style in styles {
            self.commands.push(DrawingCommand::DrawPath {
                path: id.clone(),
                style,
            });
        }
        self.usage = next;
        Ok(())
    }

    /// Adds a path resource used by a clip command without emitting a visible paint command.
    ///
    /// Clip paths have the same variable-sized resource footprint as painted paths, so they
    /// must be admitted through the bounded builder instead of being appended directly by an
    /// adapter.
    pub(crate) fn draw_clip_path(
        &mut self,
        id: ResourceId,
        segments: Vec<PathSegment>,
        fill_rule: merman_display_list::FillRule,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(contract_error("DrawingList clip path has no geometry"));
        }
        let mut next = self.usage;
        checked_increment(&mut next.resources, 1, "resource count")?;
        checked_increment(&mut next.commands, 1, "command count")?;
        checked_increment(
            &mut next.path_segments,
            segments.len(),
            "path segment count",
        )?;
        next.max_nesting_depth = next.max_nesting_depth.max(
            self.scopes
                .len()
                .checked_add(1)
                .ok_or_else(|| contract_error("DrawingList nesting depth overflows usize"))?,
        );
        self.preflight(next)?;

        self.resources
            .try_reserve(1)
            .map_err(|_| allocation_failed("resources"))?;
        self.commands
            .try_reserve(1)
            .map_err(|_| allocation_failed("commands"))?;
        self.scopes
            .try_reserve(1)
            .map_err(|_| allocation_failed("command scopes"))?;
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands.push(DrawingCommand::ClipPath {
            path: id,
            fill_rule,
        });
        self.scopes.push(BuildScope::Clip);
        self.usage = next;
        Ok(())
    }

    /// Admits each generated segment before growing the path's backing storage.
    ///
    /// Resource and command budgets are checked before invoking the producer. A rejected path
    /// leaves the committed footprint, resources, and commands unchanged.
    pub(crate) fn draw_path_with(
        &mut self,
        id: ResourceId,
        style: PathStyle,
        emit: impl FnOnce(&mut dyn FnMut(PathSegment) -> Result<()>) -> Result<()>,
    ) -> Result<()> {
        let mut next = self.usage;
        checked_increment(&mut next.resources, 1, "resource count")?;
        checked_increment(&mut next.commands, 1, "command count")?;
        if let Some(stroke) = &style.stroke {
            checked_increment(
                &mut next.stroke_dash_entries,
                stroke.dash_array.len(),
                "stroke dash entry count",
            )?;
        }
        self.preflight(next)?;
        self.resources
            .try_reserve(1)
            .map_err(|_| allocation_failed("resources"))?;
        self.commands
            .try_reserve(1)
            .map_err(|_| allocation_failed("commands"))?;

        let mut segments = Vec::new();
        let mut rejected = false;
        emit(&mut |segment| {
            if rejected {
                return Err(contract_error(
                    "DrawingList path producer ignored a rejected segment",
                ));
            }
            let result = (|| {
                checked_increment(&mut next.path_segments, 1, "path segment count")?;
                self.preflight(next)?;
                segments
                    .try_reserve(1)
                    .map_err(|_| allocation_failed("path segments"))?;
                segments.push(segment);
                Ok(())
            })();
            rejected = result.is_err();
            result
        })?;
        if rejected {
            return Err(contract_error(
                "DrawingList path producer ignored a rejected segment",
            ));
        }
        if segments.is_empty() {
            return Err(contract_error("DrawingList path has no geometry"));
        }
        self.session.checkpoint(OperationPhase::Emit)?;
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        self.usage = next;
        Ok(())
    }

    /// Adds a linear gradient after charging its resource and stop footprint.
    pub(crate) fn push_linear_gradient(&mut self, gradient: LinearGradientResource) -> Result<()> {
        let mut next = self.usage;
        checked_increment(&mut next.resources, 1, "resource count")?;
        checked_increment(
            &mut next.gradient_stops,
            gradient.stops.len(),
            "gradient stop count",
        )?;
        self.preflight(next)?;
        self.resources
            .try_reserve(1)
            .map_err(|_| allocation_failed("resources"))?;
        self.resources
            .push(DrawingResource::LinearGradient(gradient));
        self.usage = next;
        Ok(())
    }

    /// Copies host text only after its cumulative byte budget and projected work have passed.
    pub(crate) fn draw_host_text(
        &mut self,
        text: &str,
        make_run: impl FnOnce(String) -> TextRun,
    ) -> Result<()> {
        self.draw_host_text_parts(&[text], |text| Ok(make_run(text)))
    }

    /// Admits borrowed text fragments before joining or measuring them. A fallible host
    /// measurement callback cannot commit a command after cancellation or an invalid result.
    pub(crate) fn draw_host_text_parts(
        &mut self,
        parts: &[&str],
        make_run: impl FnOnce(String) -> Result<TextRun>,
    ) -> Result<()> {
        self.draw_host_text_iter(parts.iter().copied(), make_run)
    }

    /// Streams repeatable borrowed fragments without materializing a fragment array. This
    /// keeps whitespace-normalized labels bounded before either joining or host measurement.
    pub(crate) fn draw_host_text_iter<'s>(
        &mut self,
        parts: impl Iterator<Item = &'s str> + Clone,
        make_run: impl FnOnce(String) -> Result<TextRun>,
    ) -> Result<()> {
        let mut projected = self.usage;
        checked_increment(&mut projected.commands, 1, "command count")?;
        self.preflight(projected)?;
        let mut text_bytes = 0;
        for part in parts.clone() {
            checked_increment(&mut text_bytes, part.len(), "text byte count")?;
            checked_increment(&mut projected.text_bytes, part.len(), "text byte count")?;
            self.preflight(projected)?;
        }
        self.commands
            .try_reserve(1)
            .map_err(|_| allocation_failed("commands"))?;

        let mut owned = String::new();
        owned
            .try_reserve_exact(text_bytes)
            .map_err(|_| allocation_failed("text"))?;
        for part in parts.clone() {
            owned.push_str(part);
        }
        let run = make_run(owned)?;
        self.session.checkpoint(OperationPhase::Emit)?;
        if !run.text.bytes().eq(parts.flat_map(|part| part.bytes())) {
            return Err(contract_error(
                "DrawingList host-text builder changed the admitted text payload",
            ));
        }
        if !matches!(&run.obligation, TextObligation::HostText { .. }) {
            return Err(contract_error(
                "DrawingList host-text builder received a non-host text obligation",
            ));
        }
        if let Some(stroke) = &run.style.stroke
            && !stroke.dash_array.is_empty()
        {
            checked_increment(
                &mut projected.stroke_dash_entries,
                stroke.dash_array.len(),
                "stroke dash entry count",
            )?;
            self.preflight(projected)?;
        }
        self.usage = projected;
        self.commands.push(DrawingCommand::draw_text(run));
        Ok(())
    }

    pub(crate) fn finish(
        self,
        viewport: Viewport,
        extensions: BTreeMap<String, Value>,
    ) -> Result<DrawingListDocument> {
        self.session.checkpoint(OperationPhase::Emit)?;
        if !self.scopes.is_empty() {
            return Err(contract_error(
                "DrawingList builder contains unbalanced command scopes",
            ));
        }
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport,
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions,
        };
        self.budget.validate(&document, self.session)?;
        let actual = document.footprint().map_err(Error::DrawingListContract)?;
        if actual != self.usage {
            return Err(contract_error(format!(
                "DrawingList builder footprint diverged from the validated document: projected={:?} actual={actual:?}",
                self.usage
            )));
        }
        Ok(document)
    }

    fn preflight(&self, usage: DrawingListFootprint) -> Result<()> {
        // Keep construction rejection local; the family boundary latches protocol terminals.
        if let super::DocumentBudget::DrawingList(limits) = self.budget {
            usage
                .check_limits(&limits)
                .map_err(Error::DrawingListContract)?;
        }
        let work = usage.work_units().map_err(Error::DrawingListContract)?;
        self.session
            .work_meter()
            .preflight_at(work, OperationPhase::Emit)
            .map_err(Into::into)
    }

    fn preview_scope_change(&self, command: &DrawingCommand) -> Result<ScopeChange> {
        match command {
            DrawingCommand::Save => Ok(ScopeChange::Push(BuildScope::Save)),
            DrawingCommand::BeginSemanticGroup { .. } => {
                Ok(ScopeChange::Push(BuildScope::Semantic))
            }
            DrawingCommand::Restore => self.preview_restore(),
            DrawingCommand::EndSemanticGroup => self.preview_scope_pop(BuildScope::Semantic),
            DrawingCommand::BeginLayer { .. }
            | DrawingCommand::EndLayer
            | DrawingCommand::ClipPath { .. } => Err(contract_error(
                "DrawingList builder scope kind is not admitted by the current family cohort",
            )),
            _ => Ok(ScopeChange::None),
        }
    }

    fn preview_scope_pop(&self, expected: BuildScope) -> Result<ScopeChange> {
        match self.scopes.last().copied() {
            Some(actual) if actual == expected => Ok(ScopeChange::Pop),
            _ => Err(contract_error(
                "DrawingList command scope does not match the open builder scope",
            )),
        }
    }

    fn preview_restore(&self) -> Result<ScopeChange> {
        let Some(save_index) = self
            .scopes
            .iter()
            .rposition(|scope| *scope == BuildScope::Save)
        else {
            return Err(contract_error(
                "DrawingList restore does not match an open save scope",
            ));
        };
        if self.scopes[save_index + 1..]
            .iter()
            .any(|scope| *scope != BuildScope::Clip)
        {
            return Err(contract_error(
                "DrawingList restore crosses an open semantic group",
            ));
        }
        Ok(ScopeChange::PopState {
            clips: self.scopes.len() - save_index - 1,
        })
    }

    fn apply_scope_change(&mut self, change: ScopeChange) {
        match change {
            ScopeChange::None => {}
            ScopeChange::Push(scope) => self.scopes.push(scope),
            ScopeChange::Pop => {
                self.scopes.pop();
            }
            ScopeChange::PopState { clips } => {
                for _ in 0..clips {
                    self.scopes.pop();
                }
                self.scopes.pop();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildScope {
    Save,
    Semantic,
    Clip,
}

#[derive(Debug, Clone, Copy)]
enum ScopeChange {
    None,
    Push(BuildScope),
    Pop,
    PopState { clips: usize },
}

fn checked_increment(target: &mut usize, delta: usize, label: &'static str) -> Result<()> {
    *target = target
        .checked_add(delta)
        .ok_or_else(|| contract_error(format!("DrawingList {label} overflows usize")))?;
    Ok(())
}

fn allocation_failed(collection: &'static str) -> Error {
    Error::DrawingListAllocationFailed { collection }
}

fn contract_error(message: impl Into<String>) -> Error {
    Error::DrawingListContract(DrawingListError::InvalidDocument(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use merman_core::OperationControl;
    use merman_display_list::{
        Color, FillRule, MeasurementProvenance, Paint, PathStyle, Point, Rect, TextAnchor,
        TextBaseline, TextDirection, TextStyle,
    };

    fn make_session(environment: &RenderEnvironment, control: OperationControl) -> RenderSession {
        environment
            .begin_session_with_control(control)
            .expect("deterministic session should be available")
    }

    fn path_style() -> PathStyle {
        PathStyle {
            fill_rule: FillRule::NonZero,
            fill: Some(Paint::solid(Color::rgba(0, 0, 0, 255))),
            stroke: None,
        }
    }

    fn host_text_run(text: String) -> TextRun {
        TextRun {
            text,
            origin: Point::new(0.0, 0.0),
            bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
            style: TextStyle {
                font: merman_display_list::FontDescriptor {
                    families: vec!["sans-serif".to_string()],
                    weight: 400,
                    style: merman_display_list::FontStyle::Normal,
                    postscript_name: None,
                    resource: None,
                },
                font_size: 1.0,
                letter_spacing: 0.0,
                line_height: 1.0,
                fill: Paint::solid(Color::rgba(0, 0, 0, 255)),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: TextAnchor::Start,
            baseline: TextBaseline::Alphabetic,
            direction: TextDirection::Auto,
            language: None,
            obligation: TextObligation::HostText {
                measurement: MeasurementProvenance::DeterministicFallback {
                    profile: "test".to_string(),
                },
            },
        }
    }

    #[test]
    fn path_limit_rejects_before_resource_or_command_commit() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits {
                max_path_segments: 1,
                ..DrawingListLimits::default()
            },
            &session,
        );
        let error = builder
            .draw_path(
                ResourceId::new("path"),
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(0.0, 0.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(1.0, 0.0),
                    },
                ],
                path_style(),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "path_segments",
                actual: 2,
                maximum: 1,
            })
        ));
        assert!(builder.resources.is_empty());
        assert!(builder.commands.is_empty());
    }

    #[test]
    fn incremental_path_checks_command_and_resource_limits_before_emission() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        for limits in [
            DrawingListLimits {
                max_commands: 0,
                ..DrawingListLimits::default()
            },
            DrawingListLimits {
                max_resources: 0,
                ..DrawingListLimits::default()
            },
        ] {
            let mut builder =
                DrawingListBuilder::new(DrawingListPolicy::AllowRasterSubtree, limits, &session);
            let mut called = false;
            let error = builder
                .draw_path_with(ResourceId::new("path"), path_style(), |_| {
                    called = true;
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(
                error,
                Error::DrawingListContract(DrawingListError::ResourceLimit { .. })
            ));
            assert!(!called);
            assert!(builder.resources.is_empty());
            assert_eq!(builder.command_count(), 0);
            assert_eq!(builder.usage, DrawingListFootprint::default());
        }
    }

    #[test]
    fn incremental_path_limit_stops_emission_without_committing_partial_geometry() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits {
                max_path_segments: 2,
                ..DrawingListLimits::default()
            },
            &session,
        );
        let mut attempted = 0;
        let error = builder
            .draw_path_with(ResourceId::new("path"), path_style(), |emit| {
                for _ in 0..100 {
                    attempted += 1;
                    emit(PathSegment::MoveTo {
                        to: Point::new(0.0, 0.0),
                    })?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "path_segments",
                actual: 3,
                maximum: 2,
            })
        ));
        assert_eq!(attempted, 3);
        assert!(builder.resources.is_empty());
        assert_eq!(builder.command_count(), 0);
        assert_eq!(builder.usage, DrawingListFootprint::default());
    }

    #[test]
    fn incremental_path_cancellation_discards_admitted_segments() {
        let environment = RenderEnvironment::deterministic();
        let control = OperationControl::new();
        let session = make_session(&environment, control.clone());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits::default(),
            &session,
        );
        let mut attempted = 0;
        let error = builder
            .draw_path_with(ResourceId::new("path"), path_style(), |emit| {
                for _ in 0..100 {
                    attempted += 1;
                    emit(PathSegment::MoveTo {
                        to: Point::new(0.0, 0.0),
                    })?;
                    control.cancel();
                }
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(error, Error::Cancelled(_)));
        assert_eq!(attempted, 2);
        assert!(builder.resources.is_empty());
        assert_eq!(builder.command_count(), 0);
        assert_eq!(builder.usage, DrawingListFootprint::default());
    }

    #[test]
    fn text_and_nesting_limits_reject_only_the_prospective_command() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let limits = DrawingListLimits {
            max_text_bytes: 4,
            max_nesting_depth: 2,
            ..DrawingListLimits::default()
        };
        let mut builder =
            DrawingListBuilder::new(DrawingListPolicy::AllowRasterSubtree, limits, &session);
        builder.push_control(DrawingCommand::Save).unwrap();
        builder
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "document".to_string(),
            })
            .unwrap();
        let nesting_error = builder
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: "nested".to_string(),
            })
            .unwrap_err();
        assert!(matches!(
            nesting_error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "nesting_depth",
                actual: 3,
                maximum: 2,
            })
        ));
        assert_eq!(builder.commands.len(), 2);

        let session = make_session(&environment, OperationControl::new());
        let mut builder =
            DrawingListBuilder::new(DrawingListPolicy::AllowRasterSubtree, limits, &session);
        builder.draw_host_text("four", host_text_run).unwrap();
        let text_error = builder.draw_host_text("x", host_text_run).unwrap_err();
        assert!(matches!(
            text_error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "text_bytes",
                actual: 5,
                maximum: 4,
            })
        ));
        assert_eq!(builder.commands.len(), 1);
    }

    #[test]
    fn cancellation_does_not_commit_the_rejected_command() {
        let environment = RenderEnvironment::deterministic();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let session = make_session(&environment, control);
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits::default(),
            &session,
        );

        builder.push_control(DrawingCommand::Save).unwrap();
        let error = builder
            .push_control(DrawingCommand::SetOpacity { opacity: 1.0 })
            .unwrap_err();
        assert!(matches!(error, Error::Cancelled(_)));
        assert_eq!(builder.commands.len(), 1);
    }

    #[test]
    fn cancellation_during_finish_validation_returns_no_document() {
        let environment = RenderEnvironment::deterministic();
        let control = OperationControl::new();
        let session = make_session(&environment, control.clone());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits::default(),
            &session,
        );
        builder
            .draw_path_with(ResourceId::new("path"), path_style(), |emit| {
                for index in 0..128 {
                    emit(PathSegment::MoveTo {
                        to: Point::new(index as f64, 0.0),
                    })?;
                }
                Ok(())
            })
            .unwrap();
        // Start cancellation after construction, while the validator is traversing this path.
        control.cancel_after_checkpoints(16);
        let result = builder.finish(
            Viewport::new(Rect::new(0.0, 0.0, 128.0, 1.0)),
            BTreeMap::new(),
        );
        assert!(matches!(result, Err(Error::Cancelled(_))));
    }

    #[test]
    fn borrowed_text_iterator_stops_before_scanning_the_remaining_fragments() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::VectorOnly,
            DrawingListLimits {
                max_text_bytes: 3,
                ..Default::default()
            },
            &session,
        );
        let visited = std::cell::Cell::new(0);
        let parts = ["ab"; 100]
            .into_iter()
            .inspect(|_| visited.set(visited.get() + 1));
        let error = builder
            .draw_host_text_iter(parts, |_| panic!("must reject before measurement"))
            .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "text_bytes",
                actual: 4,
                maximum: 3,
            })
        ));
        assert_eq!(visited.get(), 2);
        assert!(builder.commands.is_empty());

        builder.budget = DrawingListLimits {
            max_commands: 0,
            ..DrawingListLimits::default()
        }
        .into();
        let parts = ["unused"]
            .into_iter()
            .inspect(|_| panic!("command budget must precede text scanning"));
        assert!(
            builder
                .draw_host_text_iter(parts, |text| Ok(host_text_run(text)))
                .is_err()
        );
    }

    #[test]
    fn text_fragments_are_admitted_before_the_fallible_callback() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::VectorOnly,
            DrawingListLimits {
                max_text_bytes: 6,
                ..DrawingListLimits::default()
            },
            &session,
        );
        let parts = ["é", " [3]"];
        let error = builder
            .draw_host_text_parts(&parts, |_| Err(contract_error("measurement failed")))
            .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::InvalidDocument(_))
        ));
        assert_eq!(builder.usage, DrawingListFootprint::default());
        assert!(builder.commands.is_empty());

        builder
            .draw_host_text_parts(&parts, |text| Ok(host_text_run(text)))
            .unwrap();
        assert_eq!(builder.usage.text_bytes, 6);
        let DrawingCommand::DrawText { run } = &builder.commands[0] else {
            panic!("expected text")
        };
        assert_eq!(run.text, "é [3]");
        let error = builder
            .draw_host_text_parts(&["x"], |_| {
                panic!("over-budget text must not invoke measurement")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::ResourceLimit {
                resource: "text_bytes",
                actual: 7,
                maximum: 6,
            })
        ));
        assert_eq!(builder.commands.len(), 1);
    }

    #[test]
    fn cancellation_in_host_text_callback_does_not_commit_text() {
        let environment = RenderEnvironment::deterministic();
        let control = OperationControl::new();
        let session = make_session(&environment, control.clone());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::VectorOnly,
            DrawingListLimits::default(),
            &session,
        );
        let result = builder.draw_host_text("cancelled", |text| {
            control.cancel();
            host_text_run(text)
        });
        assert!(matches!(result, Err(Error::Cancelled(_))));
        assert!(builder.commands.is_empty());
        assert_eq!(builder.usage, DrawingListFootprint::default());
    }

    #[test]
    fn finish_rejects_internal_footprint_drift() {
        let environment = RenderEnvironment::deterministic();
        let session = make_session(&environment, OperationControl::new());
        let mut builder = DrawingListBuilder::new(
            DrawingListPolicy::AllowRasterSubtree,
            DrawingListLimits::default(),
            &session,
        );
        builder.usage.commands = 1;

        let error = builder
            .finish(
                Viewport::new(Rect::new(0.0, 0.0, 1.0, 1.0)),
                BTreeMap::new(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(DrawingListError::InvalidDocument(message))
                if message.contains("footprint diverged")
        ));
    }
}
