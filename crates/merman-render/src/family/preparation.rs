use super::*;

#[inline(never)]
fn prepare_pair<S, L>(
    semantic: S,
    layout: impl FnOnce(&S) -> Result<L>,
) -> Result<Box<FamilyPair<S, L>>> {
    let layout = layout(&semantic)?;
    Ok(Box::new(FamilyPair::new(semantic, layout)))
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
#[allow(
    clippy::too_many_arguments,
    reason = "Family preparation forwards the operation-owned environment, layout, and diagnostics."
)]
fn prepare_flowchart_artifact<L>(
    semantic: diagrams::flowchart::FlowchartModel,
    render_context: diagrams::flowchart::FlowchartRenderContext,
    prepared_text_layout: Option<&crate::text::PreparedTextLayout>,
    resolved_theme: Option<&ResolvedDiagramTheme>,
    math_backend: Option<&crate::math::ConfiguredMathBackend>,
    effective_config: &merman_core::MermaidConfig,
    typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    work_meter: Arc<crate::resources::OperationWorkMeter>,
    edge_style_plan: crate::svg::FlowchartEdgeStylePlan,
    svg_label_preparation: FlowchartSvgLabelPreparation,
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderContext,
        Option<&crate::flowchart::FlowchartSvgLabelSidecarBuilder>,
        &crate::svg::FlowchartEdgeStylePlan,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<L>>> {
    let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
        resolved_theme,
        effective_config,
        crate::svg::flowchart_node_label_fill_config_override(effective_config),
        work_meter.as_ref(),
    )?;
    let edge_theme =
        crate::flowchart::FlowchartEdgeThemeStyle::resolve(resolved_theme, work_meter.as_ref())?;
    let base_typography =
        crate::flowchart::FlowchartBaseTypographyPlan::resolve(resolved_theme, effective_config);
    let svg_label_sidecar = svg_label_preparation.0.then(|| {
        crate::flowchart::FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
            prepared_text_layout,
            resolved_theme,
            work_meter,
        )
        .with_base_typography(base_typography)
        .with_prepared_math_backend(math_backend, &prepared_theme.compatibility)
        .with_typography_config_ownership(typography_config_ownership)
        .with_edge_label_padding(edge_theme.edge_label_padding())
    });
    let layout = layout(
        &semantic,
        &render_context,
        svg_label_sidecar.as_ref(),
        &edge_style_plan,
    )?;
    let svg_label_sidecar = svg_label_sidecar
        .map(crate::flowchart::FlowchartSvgLabelSidecarBuilder::finish)
        .unwrap_or_default();
    if let Some(error) = svg_label_sidecar.prepared_work_error().cloned() {
        return Err(error.into());
    }
    if let Some(error) = svg_label_sidecar.prepared_resource_error().cloned() {
        return Err(error.into());
    }
    if let Some(error) = svg_label_sidecar.prepared_error().cloned() {
        return Err(error.into());
    }
    Ok(Box::new(FlowchartFamilyArtifact {
        pair: FamilyPair::new(semantic, layout),
        render_context,
        edge_style_plan,
        edge_theme,
        prepared_theme,
        svg_label_sidecar,
        theme_evidence: crate::flowchart::FlowchartThemeEvidenceRecorder::default(),
        effect_evidence: crate::diagram_theme::SvgShadowEvidenceRecorder::default(),
        expected_effect_applications: std::cell::Cell::new(0),
    }))
}

#[inline(never)]
#[cfg(feature = "diagram-mindmap")]
fn prepare_mindmap_family(
    model: diagrams::mindmap::MindmapDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let node_palette = crate::mindmap::MindmapNodePalettePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        execution.work_meter_ref(),
    )?;
    let layout = crate::mindmap::layout_mindmap_diagram_typed_with_work_meter(
        &model,
        &meta.effective_config,
        node_palette.font_family_css(),
        execution.text_measurer(),
        execution.math_renderer(),
        execution.work_meter(),
        #[cfg(feature = "layout-elk")]
        execution.elk_operation_seed(),
    )?;
    Ok(BuiltinFamilyArtifact::Mindmap(Box::new(
        MindmapFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            node_palette,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-sankey")]
fn prepare_sankey_family(
    model: diagrams::sankey::SankeyDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let layout = crate::sankey::layout_sankey_diagram_typed_with_work_meter(
        &model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::sankey::SankeyTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        layout.nodes.len(),
        execution.work_meter_ref(),
    )?;
    let node_palette = crate::sankey::SankeyNodePalettePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Sankey(Box::new(
        SankeyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            node_palette,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-block")]
fn prepare_block_family(
    model: diagrams::block::BlockDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::block::BlockTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    )?;
    let label_background_theme = crate::block::BlockLabelBackgroundPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &typography_theme.css_binding().edge_label_background,
        execution.work_meter_ref(),
    )?;
    let layout = crate::block::layout_block_diagram_typed_with_text_style(
        &model,
        typography_theme.padding(),
        typography_theme.css_binding().html_labels,
        typography_theme.text_style().clone(),
        execution.text_measurer(),
    )?;
    let edge_paint_theme = crate::block::BlockEdgePaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        &layout,
        execution.work_meter_ref(),
    )?;
    let marker_paint_theme = crate::block::BlockMarkerPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        &layout,
        execution.work_meter_ref(),
    )?;
    let node_label_paint_theme = crate::block::BlockNodeLabelPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        &layout,
        execution.work_meter_ref(),
    )?;
    let node_paint_theme = crate::block::BlockNodePaintThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        &layout,
        &node_label_paint_theme,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Block(Box::new(
        BlockFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            node_paint_theme,
            node_label_paint_theme,
            edge_paint_theme,
            marker_paint_theme,
            label_background_theme,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-railroad")]
fn prepare_railroad_family(
    model: diagrams::railroad::RailroadDiagramRenderModel,
    diagram_type: &str,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::railroad::RailroadTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let layout = crate::railroad::layout_railroad_diagram_typed_for_type_with_theme(
        &model,
        diagram_type,
        &typography_theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Railroad(Box::new(
        RailroadFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
fn prepare_error_family(
    model: diagrams::error_diagram::ErrorDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::error::ErrorTypographyThemePlan::resolve_with_message(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.text_measurer(),
        model.error_message.as_deref(),
    );
    let layout = crate::error::layout_error_diagram_typed(&model, &typography_theme)?;
    Ok(BuiltinFamilyArtifact::Error(Box::new(
        ErrorFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-info")]
fn prepare_info_family(
    model: diagrams::info::InfoDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::info::InfoTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    let layout = crate::info::layout_info_diagram_typed(&model, &typography_theme)?;
    Ok(BuiltinFamilyArtifact::Info(Box::new(InfoFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        typography_theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-cynefin")]
fn prepare_cynefin_family(
    model: diagrams::cynefin::CynefinDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::cynefin::CynefinTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.work_meter_ref(),
    )?;
    let layout = crate::cynefin::layout_cynefin_diagram_typed_with_theme(
        &model,
        meta.effective_config.as_value(),
        &typography_theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Cynefin(Box::new(
        CynefinFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-wardley")]
fn prepare_wardley_family(
    model: diagrams::wardley::WardleyDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::wardley::WardleyTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let layout = crate::wardley::layout_wardley_diagram_typed_with_theme(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &typography_theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Wardley(Box::new(
        WardleyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-state")]
fn prepare_state_family(
    model: diagrams::state::StateDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let label_sidecar = crate::state::StateLabelSidecarBuilder::new_with_work_meter(
        execution.prepared_text_layout(),
        execution.work_meter(),
    );
    if meta
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty())
    {
        label_sidecar.reject_unsupported("state_diagram_title_bbox_y");
    }
    let layout = crate::state::layout_state_diagram_typed_with_work_meter(
        &model,
        meta.effective_config.as_value(),
        execution
            .state_style_plan()
            .expect("State family layout requires an adapted style plan"),
        Some(&label_sidecar),
        execution,
    )?;
    let label_sidecar = label_sidecar.finish();
    if let Some(error) = label_sidecar.prepared_resource_error().cloned() {
        return Err(error.into());
    }
    if let Some(error) = label_sidecar.prepared_error().cloned() {
        return Err(error.into());
    }
    Ok(BuiltinFamilyArtifact::State(Box::new(
        StateFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            label_sidecar,
            effect_evidence: crate::diagram_theme::SvgShadowEvidenceRecorder::default(),
        },
    )))
}

#[inline(never)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn prepare_flowchart_family(
    model: diagrams::flowchart::FlowchartModel,
    render_context: diagrams::flowchart::FlowchartRenderContext,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
    svg_label_preparation: FlowchartSvgLabelPreparation,
) -> Result<BuiltinFamilyArtifact> {
    match execution.family_id() {
        DiagramFamilyId::SWIMLANE => {
            let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
                &model,
                &meta.effective_config,
                true,
                execution.work_meter_ref(),
            )?;
            Ok(BuiltinFamilyArtifact::Swimlane(prepare_flowchart_artifact(
                model,
                render_context,
                execution.prepared_text_layout(),
                execution.resolved_theme(),
                execution.math_backend(),
                &meta.effective_config,
                crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                execution.work_meter(),
                edge_style_plan,
                svg_label_preparation,
                |model, render_context, svg_label_sidecar, edge_style_plan| {
                    crate::swimlane::layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
                        model,
                        render_context,
                        &meta.effective_config,
                        execution.text_measurer(),
                        execution.math_renderer(),
                        svg_label_sidecar,
                        edge_style_plan,
                        execution.work_meter(),
                    )
                },
            )?))
        }
        DiagramFamilyId::FLOWCHART => {
            let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
                &model,
                &meta.effective_config,
                false,
                execution.work_meter_ref(),
            )?;
            Ok(BuiltinFamilyArtifact::Flowchart(
                prepare_flowchart_artifact(
                    model,
                    render_context,
                    execution.prepared_text_layout(),
                    execution.resolved_theme(),
                    execution.math_backend(),
                    &meta.effective_config,
                    crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                    execution.work_meter(),
                    edge_style_plan,
                    svg_label_preparation,
                    |model, render_context, svg_label_sidecar, edge_style_plan| {
                        crate::layout_flowchart_typed_with_render_labels_by_engine(
                            model,
                            render_context,
                            &meta.effective_config,
                            execution,
                            svg_label_sidecar,
                            edge_style_plan,
                        )
                    },
                )?,
            ))
        }
        planned_family => Err(Error::InvalidModel {
            message: format!(
                "planned render family {planned_family} cannot consume a Flowchart model"
            ),
        }),
    }
}

#[cfg(feature = "layout-cytoscape")]
#[inline(never)]
#[cfg(feature = "diagram-architecture")]
fn prepare_architecture_family(
    model: diagrams::architecture::ArchitectureDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let native_svg_text_count = model
        .nodes
        .iter()
        .filter(|node| {
            node.node_type == diagrams::architecture::ArchitectureRenderNodeType::Service
                && node
                    .title
                    .as_deref()
                    .is_some_and(|title| !title.trim().is_empty())
        })
        .count()
        .checked_add(
            model
                .groups
                .iter()
                .filter(|group| {
                    group
                        .title
                        .as_deref()
                        .is_some_and(|title| !title.trim().is_empty())
                })
                .count(),
        )
        .and_then(|count| {
            count.checked_add(
                model
                    .edges
                    .iter()
                    .filter(|edge| {
                        edge.title
                            .as_deref()
                            .is_some_and(|title| !title.trim().is_empty())
                    })
                    .count(),
            )
        })
        .ok_or_else(|| Error::InvalidModel {
            message: "Architecture native SVG text terminal count overflowed".to_string(),
        })?;
    let icon_text_count = model
        .nodes
        .iter()
        .filter(|node| {
            node.node_type == diagrams::architecture::ArchitectureRenderNodeType::Service
                && node.icon.is_none()
                && node
                    .icon_text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
        })
        .count();
    let typography_terminals = crate::architecture::ArchitectureTypographyTerminalInventory::new(
        native_svg_text_count,
        icon_text_count,
    )
    .ok_or_else(|| Error::InvalidModel {
        message: "Architecture typography terminal count overflowed".to_string(),
    })?;
    let group_theme = crate::architecture::ArchitectureGroupThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        model.groups.len(),
        model.edges.len(),
        typography_terminals,
        execution.work_meter().as_ref(),
    )?;
    let layout = crate::architecture::layout_architecture_diagram_typed(
        &model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
        execution.operation_seed(),
        execution.work_meter().as_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Architecture(Box::new(
        ArchitectureFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            group_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-c4")]
fn prepare_c4_family(
    model: diagrams::c4::C4DiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let text_paint = crate::c4::C4TextPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        meta.title.as_deref().or(model.title.as_deref()),
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::c4::C4TypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        meta.title.as_deref().or(model.title.as_deref()),
        &model,
    );
    let cluster_theme = crate::c4::C4ClusterThemePlan::resolve(
        execution.resolved_theme(),
        &model.boundaries,
        execution.work_meter_ref(),
    )?;
    let layout = crate::c4::layout_c4_diagram_typed(
        &model,
        meta.effective_config.as_value(),
        &typography_theme,
        execution.text_measurer(),
        execution.container_width,
        execution.container_height,
        execution.screen_available_width,
    )?;
    Ok(BuiltinFamilyArtifact::C4(Box::new(C4FamilyArtifact {
        pair: FamilyPair::new(model, layout),
        cluster_theme,
        typography_theme,
        text_paint,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-gantt")]
fn prepare_gantt_family(
    model: diagrams::gantt::GanttDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let task_theme = crate::gantt::GanttTaskTheme::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model.tasks,
        execution.work_meter_ref(),
    )?;
    let layout = crate::gantt::layout_gantt_diagram_typed(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &task_theme,
        execution.text_measurer(),
        execution.container_width,
        execution.local_time_zone(),
    )?;
    Ok(BuiltinFamilyArtifact::Gantt(Box::new(
        GanttFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            task_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-pie")]
fn prepare_pie_family(
    model: diagrams::pie::PieDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let title = model.title.as_deref().or(meta.title.as_deref());
    let theme = crate::pie::PieThemePlan::resolve_with_title(
        &model,
        &meta.effective_config,
        execution.resolved_theme(),
        title,
        execution.work_meter_ref(),
    )?;
    let layout = crate::pie::layout_pie_diagram_typed_with_paint_plan(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Pie(Box::new(PieFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-timeline")]
fn prepare_timeline_family(
    model: diagrams::timeline::TimelineDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let text_paint = crate::timeline::TimelineTextPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::timeline::TimelineTypographyThemePlan::resolve_with_text(
        execution.resolved_theme(),
        &meta.effective_config,
        text_paint.fill_css(),
    );
    let layout = crate::timeline::layout_timeline_diagram_typed_with_binding(
        &model,
        typography_theme.layout_settings(),
        execution.text_measurer(),
    )?;
    let event_theme = crate::timeline::TimelineEventTheme::resolve_with_binding(
        execution.resolved_theme(),
        &meta.effective_config,
        typography_theme.css_binding(),
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Timeline(Box::new(
        TimelineFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            event_theme,
            typography_theme,
            text_paint,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-journey")]
fn prepare_journey_family(
    model: diagrams::journey::JourneyDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let text_paint = crate::journey::JourneyTextPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        model
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .or(meta.title.as_deref()),
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::journey::JourneyTypographyThemePlan::resolve_with_text_paint(
        execution.resolved_theme(),
        &meta.effective_config,
        text_paint.fill_css(),
    );
    let layout = crate::journey::layout_journey_diagram_typed_with_resolved_typography(
        &model,
        &typography_theme,
        execution.text_measurer(),
    )?;
    let task_theme = crate::journey::JourneyTaskTheme::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout.tasks,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Journey(Box::new(
        JourneyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            task_theme,
            text_paint,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-radar")]
fn prepare_radar_family(
    model: diagrams::radar::RadarDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let effective_title = crate::radar::effective_title(&model, meta.title.as_deref());
    let typography_theme = crate::radar::RadarTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let series_paint = crate::radar::RadarSeriesPaintPlan::resolve_with_binding(
        execution.resolved_theme(),
        &meta.effective_config,
        typography_theme.css_binding(),
        &model,
        execution.work_meter_ref(),
    )?;
    let layout = crate::radar::layout_radar_diagram_typed_with_work_meter(
        &model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    let title_theme = crate::radar::RadarTitleThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        effective_title,
        execution.work_meter_ref(),
    )?;
    let axis_paint = crate::radar::RadarAxisPaintPlan::resolve_with_binding(
        execution.resolved_theme(),
        &meta.effective_config,
        typography_theme.css_binding(),
        layout.axes.len(),
        execution.work_meter_ref(),
    )?;
    let text_paint = crate::radar::RadarTextPaintPlan::resolve_with_binding(
        execution.resolved_theme(),
        &meta.effective_config,
        typography_theme.css_binding(),
        &layout,
        &model,
        effective_title,
        &title_theme,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Radar(Box::new(
        RadarFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            series_paint,
            title_theme,
            axis_paint,
            text_paint,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-treemap")]
fn prepare_treemap_family(
    model: diagrams::treemap::TreemapDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let layout = crate::treemap::layout_treemap_diagram_typed_with_work_meter(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        execution.work_meter_ref(),
    )?;
    let title_theme = crate::treemap::TreemapTitleThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        layout.title.as_deref(),
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::treemap::TreemapTypographyThemePlan::resolve_with_title_fill(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout,
        execution.work_meter(),
        title_theme.fill_css(),
    )?;
    Ok(BuiltinFamilyArtifact::Treemap(Box::new(
        TreemapFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            title_theme,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-er")]
fn prepare_er_family(
    model: diagrams::er::ErDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let inherited_font_stack = crate::family::InheritedFontStackPlan::resolve_property_local(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let base_font_size =
        crate::er::ErBaseFontSizePlan::resolve(execution.resolved_theme(), &meta.effective_config);
    let font_size_override = base_font_size.layout_override_px();
    #[cfg(feature = "layout-elk")]
    let layout =
        crate::er::layout_er_diagram_typed_with_elk_operation_seed_and_resolved_typography(
            &model,
            meta.effective_config.as_value(),
            execution.text_measurer(),
            execution.elk_operation_seed(),
            Some(inherited_font_stack.font_family_css()),
            font_size_override,
            execution.work_meter(),
        )?;
    #[cfg(not(feature = "layout-elk"))]
    let layout = crate::er::layout_er_diagram_typed_with_resolved_typography(
        &model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
        Some(inherited_font_stack.font_family_css()),
        font_size_override,
        execution.work_meter(),
    )?;
    let entity_theme = crate::er::ErEntityThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        inherited_font_stack,
        base_font_size,
        crate::er::ErConfigView::new(meta.effective_config.as_value()).relationship_html_labels(),
        meta.title.as_deref(),
        &model,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Er(Box::new(ErFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        entity_theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-quadrant-chart")]
fn prepare_quadrant_chart_family(
    model: diagrams::quadrant_chart::QuadrantChartRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let point_theme = crate::quadrantchart::QuadrantChartPointThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        execution.work_meter_ref(),
    )?;
    let mut layout = crate::quadrantchart::layout_quadrantchart_diagram_typed(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &point_theme,
        execution.text_measurer(),
    )?;
    let text_paint = crate::quadrantchart::QuadrantChartPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &mut layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::QuadrantChart(Box::new(
        QuadrantChartFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            point_theme,
            text_paint,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-xychart")]
fn prepare_xy_chart_family(
    model: diagrams::xychart::XyChartDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let series_paint = crate::xychart::XyChartSeriesPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::xychart::XyChartTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.work_meter_ref(),
    )?;
    let mut layout = crate::xychart::layout_xychart_diagram_typed(
        &model,
        meta.title.as_deref(),
        &series_paint,
        &typography_theme,
        execution.text_measurer(),
    )?;
    if execution.family.root_theme_plan().is_some_and(|plan| {
        plan.replaces_default_family_background(
            &meta.effective_config,
            "themeVariables.xyChart.backgroundColor",
        )
    }) {
        layout.background_color = "none".to_owned();
    }
    let paint_theme = crate::xychart::XyChartPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &mut layout,
        &typography_theme,
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::XyChart(Box::new(
        XyChartFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            series_paint,
            typography_theme,
            paint_theme,
            effect_evidence: Default::default(),
            expected_effect_applications: Default::default(),
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-git-graph")]
fn prepare_gitgraph_family(
    model: diagrams::git_graph::GitGraphRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let has_title =
        crate::gitgraph::resolve_gitgraph_title(&model, meta.title.as_deref()).is_some();
    let typography_theme = crate::gitgraph::GitGraphTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let layout = crate::gitgraph::layout_gitgraph_diagram_typed(
        &model,
        meta.effective_config.as_value(),
        &typography_theme,
        execution.text_measurer(),
    )?;
    let mut node_palette = crate::gitgraph::GitGraphNodePalettePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout,
        has_title,
        execution.work_meter_ref(),
    )?;
    node_palette.resolve_text_paint(
        execution.resolved_theme(),
        &meta.effective_config,
        crate::gitgraph::resolve_gitgraph_title(&model, meta.title.as_deref()),
        execution.work_meter_ref(),
    )?;
    node_palette.bind_terminal_palette(typography_theme.css_binding());
    let static_paint = crate::gitgraph::GitGraphStaticPaintPlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::GitGraph(Box::new(
        GitGraphFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            node_palette,
            static_paint,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-tree-view")]
fn prepare_tree_view_family(
    model: diagrams::tree_view::TreeViewDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let theme = crate::tree_view::TreeViewThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        execution.work_meter_ref(),
    )?;
    let layout = crate::tree_view::layout_tree_view_diagram_typed(
        &model,
        meta.effective_config.as_value(),
        &theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::TreeView(Box::new(
        TreeViewFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            theme,
        },
    )))
}

// Keep these families' large theme/layout temporaries out of the heterogeneous router frame.
// Constrained-stack Architecture preparation otherwise inherits the largest unused match arm.
#[inline(never)]
#[cfg(feature = "diagram-packet")]
fn prepare_packet_family(
    model: diagrams::packet::PacketDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let typography_theme = crate::packet::PacketTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        execution.work_meter_ref(),
    )?;
    let layout = crate::packet::layout_packet_diagram_typed(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Packet(Box::new(
        PacketFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-requirement")]
fn prepare_requirement_family(
    model: diagrams::requirement::RequirementDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve_with_title(
        execution.resolved_theme(),
        &meta.effective_config,
        &model,
        meta.title.as_deref(),
        execution.work_meter_ref(),
    )?;
    let layout =
        crate::requirement::layout_requirement_diagram_typed_with_work_meter_and_typography(
            &model,
            meta.effective_config.as_value(),
            execution.text_measurer(),
            paint_theme.css(),
            &execution.work_meter(),
            #[cfg(feature = "layout-elk")]
            execution.elk_operation_seed(),
        )?;
    Ok(BuiltinFamilyArtifact::Requirement(Box::new(
        RequirementFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            paint_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-class")]
fn prepare_class_family(
    model: ClassDiagram,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let relation_count = model.relations.len();
    execution.work_meter_ref().charge(model.notes.len())?;
    let note_attachment_indices = model
        .notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| {
            note.class_id
                .as_ref()
                .filter(|class_id| model.classes.contains_key(*class_id))
                .map(|_| index)
        })
        .collect::<Vec<_>>();
    let note_attachment_count = note_attachment_indices.len();
    let node_count = model.classes.len();
    let needs_unsupported_table_evidence = execution.resolved_theme().is_some_and(|theme| {
        theme.family_mechanism_routes().iter().any(|route| {
            let targets_table = matches!(
                route.mechanism(),
                crate::diagram_theme::FamilyThemeMechanism::RuleFacet {
                    target: crate::diagram_theme::ThemeTarget::Table,
                    ..
                } | crate::diagram_theme::FamilyThemeMechanism::OrdinalPalette {
                    target: crate::diagram_theme::ThemeTarget::Table,
                } | crate::diagram_theme::FamilyThemeMechanism::EffectBinding {
                    target: crate::diagram_theme::ThemeTarget::Table,
                    ..
                }
            );
            targets_table
                && route.disposition() == crate::diagram_theme::FamilyThemeDisposition::Unsupported
        })
    });
    let table_group_lengths = if needs_unsupported_table_evidence {
        model
            .classes
            .values()
            .flat_map(|class| [class.members.len(), class.methods.len()])
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let relation_theme = crate::class::ClassRelationThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        relation_count,
        node_count,
        execution.work_meter_ref(),
    )?
    .with_note_attachments(note_attachment_indices);
    let typography_theme = crate::class::ClassTextThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
    );
    let layout = crate::layout_class_typed_by_engine(
        &model,
        &meta.effective_config,
        execution,
        &typography_theme,
    )?;
    let cluster_label_count = layout.clusters.len();
    let relation_theme = relation_theme.with_cluster_domain(
        execution.resolved_theme(),
        &meta.effective_config,
        cluster_label_count,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Class(Box::new(
        ClassFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            relation_theme,
            typography_theme,
            theme_evidence: crate::class::ClassThemeEvidenceRecorder::new(
                execution.resolved_theme(),
                relation_count,
                note_attachment_count,
                node_count,
                cluster_label_count,
                table_group_lengths,
            ),
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-venn")]
fn prepare_venn_family(
    model: diagrams::venn::VennDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let effective_title = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            meta.title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
        });
    let layout = crate::venn::layout_venn_diagram_typed_with_work_meter(
        &model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        execution.work_meter_ref(),
    )?;
    let title_theme = crate::venn::VennTitleThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        effective_title,
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::venn::VennTypographyThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        effective_title,
        &layout,
        &model,
        &title_theme,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Venn(Box::new(VennFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        title_theme,
        typography_theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-zenuml")]
fn prepare_zenuml_family(
    model: diagrams::zenuml::ZenumlDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let layout = crate::zenuml::layout_zenuml_diagram_typed(&model, execution.text_measurer())?;
    let title_theme = crate::zenuml::ZenumlTitleThemePlan::resolve(
        execution.resolved_theme(),
        &model,
        &layout,
        meta.title.as_deref(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Zenuml(Box::new(
        ZenumlFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            title_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-event-modeling")]
fn prepare_eventmodeling_family(
    model: diagrams::eventmodeling::EventModelingDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let css_binding =
        crate::eventmodeling::EventModelingCssBinding::resolve(meta.effective_config.as_value());
    let layout = crate::eventmodeling::layout_eventmodeling_diagram_typed_with_binding(
        &model,
        &css_binding,
        execution.text_measurer(),
    )?;
    let text_theme = crate::eventmodeling::EventModelingTextThemePlan::resolve_with_binding(
        execution.resolved_theme(),
        &meta.effective_config,
        css_binding,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::EventModeling(Box::new(
        EventModelingFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            text_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-ishikawa")]
fn prepare_ishikawa_family(
    model: diagrams::ishikawa::IshikawaDiagramRenderModel,
    meta: &ParseMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let layout = crate::ishikawa::layout_ishikawa_diagram_typed(
        &model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
    )?;
    let text_theme = crate::ishikawa::IshikawaTextThemePlan::resolve(
        execution.resolved_theme(),
        &meta.effective_config,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Ishikawa(Box::new(
        IshikawaFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            text_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-class")]
pub(super) fn prepare_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    mut context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact> {
    let (meta, model) = parsed.into_parts();
    let RenderSemanticModel::Class(model) = model else {
        unreachable!("Class render dispatch requires a Class semantic model")
    };
    context.observe_compatibility(&meta);
    context.ensure_portable_before_svg()?;
    let execution = LayoutExecution::new(options, context.execution());
    let family = prepare_class_family(model, &meta, &execution)?;
    FamilyRenderArtifact::new(meta, family, context)
}

#[inline(never)]
#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
pub(super) fn prepare_architecture_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    mut context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact> {
    let (meta, model) = parsed.into_parts();
    let RenderSemanticModel::Architecture(model) = model else {
        unreachable!("Architecture render dispatch requires an Architecture semantic model")
    };
    context.observe_compatibility(&meta);
    context.ensure_portable_before_svg()?;
    let execution = LayoutExecution::new(options, context.execution());
    let family = prepare_architecture_family(model, &meta, &execution)?;
    FamilyRenderArtifact::new(meta, family, context)
}

#[inline(never)]
pub(super) fn prepare_non_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    mut context: FamilyRenderContext,
    svg_label_preparation: FlowchartSvgLabelPreparation,
) -> Result<FamilyRenderArtifact> {
    let (meta, model, render_context) = parsed.into_render_parts();
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    let flowchart_render_context = render_context.into_flowchart_render_context();
    let diagram_type = meta.diagram_type.as_str();
    let title = meta.title.as_deref();
    context.observe_compatibility(&meta);
    #[cfg(feature = "diagram-state")]
    if let RenderSemanticModel::State(model) = &model {
        context.adapt_state(model, &meta.effective_config, title)?;
    }
    context.ensure_portable_before_svg()?;
    let execution = LayoutExecution::new(options, context.execution());
    let family = match model {
        RenderSemanticModel::Error(model) => prepare_error_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => prepare_mindmap_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => prepare_state_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            BuiltinFamilyArtifact::Sequence(prepare_pair(model, |model| {
                crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
                    model,
                    title,
                    &meta.effective_config,
                    execution.resolved_theme(),
                    execution.prepared_text_layout(),
                    execution.text_measurer(),
                    execution.math_backend(),
                    execution.work_meter(),
                )
            })?)
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => prepare_zenuml_family(model, &meta, &execution)?,
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => prepare_flowchart_family(
            model,
            flowchart_render_context,
            &meta,
            &execution,
            svg_label_preparation,
        )?,
        #[cfg(feature = "layout-cytoscape")]
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(_) => {
            unreachable!("Architecture models use the stack-bounded family dispatch path")
        }
        #[cfg(not(feature = "layout-cytoscape"))]
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(_) => {
            return Err(Error::MissingCapability {
                capability: crate::RenderCapability::LayoutCytoscape,
                diagram_type: diagram_type.to_string(),
            });
        }
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(_) => {
            unreachable!("Class models use the stack-bounded family dispatch path")
        }
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(model) => prepare_c4_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(model) => prepare_cynefin_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(model) => prepare_wardley_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(model) => {
            prepare_railroad_family(model, diagram_type, &meta, &execution)?
        }
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => {
            BuiltinFamilyArtifact::Kanban(prepare_pair(model, |model| {
                crate::kanban::prepare_kanban_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.resolved_theme(),
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => prepare_gantt_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => prepare_pie_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => prepare_packet_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => prepare_timeline_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => prepare_journey_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => {
            prepare_requirement_family(model, &meta, &execution)?
        }
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(model) => prepare_sankey_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(model) => prepare_radar_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(model) => prepare_info_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => prepare_treemap_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => prepare_block_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => prepare_er_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(model) => {
            prepare_quadrant_chart_family(model, &meta, &execution)?
        }
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => prepare_xy_chart_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => prepare_gitgraph_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => prepare_tree_view_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(model) => prepare_ishikawa_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(model) => {
            prepare_eventmodeling_family(model, &meta, &execution)?
        }
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(model) => prepare_venn_family(model, &meta, &execution)?,
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => {
            BuiltinFamilyArtifact::Usecase(prepare_pair(model, |model| {
                crate::usecase::prepare_usecase_diagram(
                    model,
                    meta.effective_config.as_value(),
                    execution.text_measurer(),
                    execution.math_renderer(),
                    execution.work_meter(),
                    #[cfg(feature = "layout-elk")]
                    execution.elk_operation_seed(),
                )
            })?)
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => {
            let (flowchart, render_context) = model.to_flowchart_model();
            let config = project_agentflow_flowchart_config(&meta.effective_config);
            let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
                &flowchart,
                &config,
                false,
                execution.work_meter_ref(),
            )?;
            let flow = prepare_flowchart_artifact(
                flowchart,
                render_context,
                execution.prepared_text_layout(),
                execution.resolved_theme(),
                execution.math_backend(),
                &config,
                crate::flowchart::flowchart_typography_config_ownership(&config),
                execution.work_meter(),
                edge_style_plan,
                svg_label_preparation,
                |model, render_context, sidecar, edge_style_plan| {
                    crate::layout_flowchart_typed_with_render_labels_by_engine(
                        model,
                        render_context,
                        &config,
                        &execution,
                        sidecar,
                        edge_style_plan,
                    )
                },
            )?;
            BuiltinFamilyArtifact::Agentflow {
                semantic: Box::new(model),
                flow,
            }
        }
        RenderSemanticModel::CustomJson(_) => {
            unreachable!("custom JSON models return before built-in family dispatch")
        }
        // Core features can be widened independently of this renderer's handlers.
        #[allow(unreachable_patterns)]
        _ => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_owned(),
            });
        }
    };
    FamilyRenderArtifact::new(meta, family, context)
}
