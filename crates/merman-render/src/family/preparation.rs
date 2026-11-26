use super::*;

#[inline(never)]
fn prepare_pair<S, L>(
    semantic: &S,
    layout: impl FnOnce(&S) -> Result<L>,
) -> Result<Box<FamilyPair<&S, L>>> {
    let layout = layout(semantic)?;
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
fn prepare_flowchart_artifact<'parsed, L>(
    semantic: std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartModel>,
    render_context: std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartRenderContext>,
    execution: &LayoutExecution<'_>,
    effective_config: &merman_model::presentation::PresentationConfig,
    swimlane: bool,
    preparation: FamilyPreparation,
    layout_view: impl for<'a> Fn(&'a L) -> crate::svg::FlowchartNodeLayoutView<'a>,
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderContext,
        &crate::flowchart::FlowchartClassDefinitions<'_>,
        Option<&crate::flowchart::FlowchartSvgLabelSidecarBuilder>,
        &crate::svg::FlowchartEdgeStylePlan,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<'parsed, L>>> {
    let native = execution.native_family_theme();
    let work_meter = execution.work_meter();
    let class_defs = crate::flowchart::FlowchartClassDefinitions::new(&semantic.class_defs);
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_with_class_definitions(
        &semantic,
        effective_config,
        swimlane,
        &class_defs,
        work_meter.as_ref(),
    )?;
    let prepared_theme = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::ThemePrepare,
        );
        crate::flowchart::FlowchartPreparedTheme::resolve(
            effective_config,
            native,
            execution.family.session().canvas_background(),
        )
    };
    let edge_style_plan = edge_style_plan
        .with_typography_work_meter(Arc::clone(&work_meter))
        .with_resolved_stroke_widths(
            &semantic,
            &prepared_theme.compatibility,
            work_meter.as_ref(),
        )?;
    let base_typography = native.map_or_else(
        || crate::flowchart::FlowchartBaseTypographyPlan::resolve(effective_config),
        |theme| crate::flowchart::FlowchartBaseTypographyPlan::native(&theme.typography),
    );
    let prepare_svg_labels = match preparation {
        FamilyPreparation::Layout | FamilyPreparation::Render => true,
        #[cfg(all(test, feature = "all-diagrams"))]
        FamilyPreparation::RenderWithoutFlowchartSvgLabels => false,
    };
    let svg_label_sidecar = prepare_svg_labels.then(|| {
        crate::flowchart::FlowchartSvgLabelSidecarBuilder::new_with_work_meter(Arc::clone(
            &work_meter,
        ))
        .with_base_typography(base_typography)
        .with_native_density(native.map(|theme| theme.form.density))
        .with_prepared_math_backend(execution.math_backend(), &prepared_theme.compatibility)
    });
    let layout = layout(
        &semantic,
        &render_context,
        &class_defs,
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
    let uses_elk = match layout_view(&layout) {
        crate::svg::FlowchartNodeLayoutView::Flowchart(layout) => {
            layout.route_contract.is_elk_parity()
        }
        crate::svg::FlowchartNodeLayoutView::Swimlane(_) => false,
    };
    let mut render_config = crate::svg::prepare_flowchart_render_config(
        &semantic,
        effective_config,
        &prepared_theme.compatibility,
        uses_elk,
        svg_label_sidecar.base_typography(),
    );
    if let Some(native) = &prepared_theme.native {
        render_config.apply_native(native, &semantic, effective_config, uses_elk);
    }
    edge_style_plan.bind_terminal_typography(
        &render_config.text_style,
        matches!(
            layout_view(&layout),
            crate::svg::FlowchartNodeLayoutView::Swimlane(_)
        ),
    )?;
    let prepare_node_paint = preparation != FamilyPreparation::Layout;
    let prepared_nodes = crate::svg::FlowchartPreparedNodes::prepare(
        &semantic,
        &render_context,
        &class_defs,
        layout_view(&layout),
        &svg_label_sidecar,
        effective_config,
        &prepared_theme,
        &render_config,
        &work_meter,
        prepare_node_paint,
    )?;
    Ok(Box::new(FlowchartFamilyArtifact {
        pair: FamilyPair::new(semantic, layout),
        render_context,
        edge_style_plan,
        prepared_theme,
        prepared_nodes: prepare_node_paint.then_some(prepared_nodes),
        render_config,
        svg_label_sidecar,
    }))
}

#[inline(never)]
#[cfg(feature = "diagram-mindmap")]
fn prepare_mindmap_family<'parsed>(
    model: &'parsed diagrams::mindmap::MindmapDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::mindmap::{MindmapAppearance, MindmapCssBinding, MindmapNativeStyle};

    let appearance = match execution.native_family_theme() {
        Some(theme) => MindmapAppearance::Native(MindmapNativeStyle::resolve(theme)),
        None => {
            execution
                .family
                .session()
                .checkpoint(merman_model::OperationPhase::Layout)?;
            let binding = MindmapCssBinding::new(&meta.effective_config);
            execution
                .family
                .session()
                .checkpoint(merman_model::OperationPhase::Layout)?;
            MindmapAppearance::Mermaid(Box::new(binding))
        }
    };
    let (layout, prepared_labels) = crate::mindmap::layout_mindmap_diagram_typed_with_work_meter(
        model,
        &meta.effective_config,
        appearance.text_style(),
        execution,
    )?;
    Ok(BuiltinFamilyArtifact::Mindmap(Box::new(
        MindmapFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            appearance,
            prepared_labels,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-sankey")]
fn prepare_sankey_family<'parsed>(
    model: &'parsed diagrams::sankey::SankeyDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let layout = merman_render_charts::sankey::layout_sankey_diagram_typed(
        model,
        meta.effective_config.as_value(),
        execution.work_meter_ref(),
    )?;
    let native = execution.native_family_theme();
    let style = crate::sankey::SankeyStyle::resolve(
        &meta.effective_config,
        &layout,
        native,
        execution.family.session().canvas_background(),
        &execution.work_meter(),
    )?;
    Ok(BuiltinFamilyArtifact::Sankey(Box::new(
        SankeyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-block")]
fn prepare_block_family<'parsed>(
    model: &'parsed diagrams::block::BlockDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let mut typography_theme = match native {
        Some(theme) => crate::block::BlockTypographyThemePlan::resolve_native(
            theme,
            execution.family.session().canvas_background(),
            meta.effective_config.as_value(),
        ),
        None => crate::block::BlockTypographyThemePlan::resolve(meta.effective_config.as_value())?,
    };
    typography_theme.prepare_class_definitions(&model.class_defs);
    let (mut layout, prepared_labels) = crate::block::layout_block_diagram_typed_with_text_style(
        model,
        typography_theme.padding(),
        crate::block::BlockShapeSettings::resolve(
            meta.effective_config.as_value(),
            typography_theme.native(),
        ),
        typography_theme.css_binding().html_labels,
        typography_theme.text_style().clone(),
        execution.text_measurer(),
        &meta.effective_config,
        &execution.work_meter(),
    )?;
    let edge_routes = crate::block::BlockEdgeRoutes::prepare(
        model,
        &mut layout,
        &typography_theme,
        #[cfg(feature = "layout-shoal")]
        execution.layout_backend(),
        &execution.work_meter(),
    )?;
    let node_bindings =
        crate::block::BlockNodeBindings::prepare(model, &layout, execution.work_meter_ref())?;
    Ok(BuiltinFamilyArtifact::Block(Box::new(
        BlockFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            prepared_labels,
            node_bindings,
            edge_routes,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-railroad")]
fn prepare_railroad_family<'parsed>(
    model: &'parsed diagrams::railroad::RailroadDiagramModel,
    diagram_type: &str,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::railroad::resolve_railroad_style(&meta.effective_config, native);
    let layout = crate::railroad::layout_railroad_diagram_typed_for_type_with_style(
        model,
        diagram_type,
        &style,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Railroad(Box::new(
        RailroadFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
fn prepare_error_family<'parsed>(
    model: &'parsed diagrams::error::ErrorDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let typography_theme = crate::error::ErrorTypographyThemePlan::resolve_with_message(
        &meta.effective_config,
        execution.text_measurer(),
        model.error_message.as_deref(),
    );
    let layout = crate::error::layout_error_diagram_typed(model, &typography_theme)?;
    Ok(BuiltinFamilyArtifact::Error(Box::new(
        ErrorFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-info")]
fn prepare_info_family<'parsed>(
    model: &'parsed diagrams::info::InfoDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let (layout, typography_theme) = crate::info::prepare_info_diagram(
        &meta.effective_config,
        execution.native_family_theme(),
        execution.text_measurer(),
    );
    Ok(BuiltinFamilyArtifact::Info(Box::new(InfoFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        typography_theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-cynefin")]
fn prepare_cynefin_family<'parsed>(
    model: &'parsed diagrams::cynefin::CynefinDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::cynefin::CynefinStyle::resolve(&meta.effective_config, native);
    let layout = crate::cynefin::layout_cynefin_diagram_typed_with_style(
        model,
        meta.effective_config.as_value(),
        &style,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Cynefin(Box::new(
        CynefinFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-wardley")]
fn prepare_wardley_family<'parsed>(
    model: &'parsed diagrams::wardley::WardleyDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::wardley::WardleyStyle::resolve(&meta.effective_config, native);
    let layout = crate::wardley::layout_wardley_diagram_typed_with_style(
        model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &style,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Wardley(Box::new(
        WardleyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-state")]
fn prepare_state_family<'parsed>(
    model: &'parsed diagrams::state::StateDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let label_sidecar = crate::state::StateLabelSidecarBuilder::default();
    let config = meta.effective_config.as_value();
    let style = crate::state::StateStylePlan::resolve(
        model,
        &meta.effective_config,
        execution.layout_backend(),
        execution.native_family_theme(),
        execution.family.session().canvas_background(),
    );
    let (layout, route_contract) = crate::state::layout_state_diagram_typed_with_work_meter(
        model,
        config,
        &style,
        Some(&label_sidecar),
        execution,
    )?;
    let label_sidecar = label_sidecar.finish();
    Ok(BuiltinFamilyArtifact::State(Box::new(
        StateFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            label_sidecar,
            route_contract,
            style,
        },
    )))
}

#[inline(never)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn prepare_flowchart_family<'parsed>(
    model: &'parsed diagrams::flowchart::FlowchartModel,
    render_context: std::borrow::Cow<'parsed, diagrams::flowchart::FlowchartRenderContext>,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
    preparation: FamilyPreparation,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    match execution.family_id() {
        DiagramFamilyId::SWIMLANE => {
            Ok(BuiltinFamilyArtifact::Swimlane(prepare_flowchart_artifact(
                std::borrow::Cow::Borrowed(model),
                render_context,
                execution,
                &meta.effective_config,
                true,
                preparation,
                |layout| crate::svg::FlowchartNodeLayoutView::Swimlane(layout),
                |model, render_context, _, svg_label_sidecar, edge_style_plan| {
                    crate::swimlane::layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
                        model,
                        render_context,
                        &meta.effective_config,
                        execution,
                        svg_label_sidecar,
                        edge_style_plan,
                    )
                },
            )?))
        }
        DiagramFamilyId::FLOWCHART => Ok(BuiltinFamilyArtifact::Flowchart(
            prepare_flowchart_artifact(
                std::borrow::Cow::Borrowed(model),
                render_context,
                execution,
                &meta.effective_config,
                false,
                preparation,
                |layout| crate::svg::FlowchartNodeLayoutView::Flowchart(layout),
                |model, render_context, class_defs, svg_label_sidecar, edge_style_plan| {
                    crate::layout_flowchart_typed_with_render_labels_by_engine(
                        model,
                        render_context,
                        class_defs,
                        &meta.effective_config,
                        execution,
                        svg_label_sidecar,
                        edge_style_plan,
                        preparation != FamilyPreparation::Layout,
                    )
                },
            )?,
        )),
        planned_family => Err(Error::InvalidModel {
            message: format!(
                "planned render family {planned_family} cannot consume a Flowchart model"
            ),
        }),
    }
}

#[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
#[inline(never)]
#[cfg(feature = "diagram-architecture")]
fn prepare_architecture_family<'parsed>(
    model: &'parsed diagrams::architecture::ArchitectureDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let group_theme =
        crate::architecture::ArchitectureGroupThemePlan::resolve(native, &meta.effective_config);
    #[cfg(feature = "layout-cytoscape")]
    let config = meta.effective_config.as_value();
    #[cfg(feature = "layout-shoal")]
    if execution.layout_backend() == GraphLayoutBackend::Shoal {
        let (layout, geometry) = crate::architecture::layout_architecture_shoal(
            model,
            &meta.effective_config,
            &group_theme,
            execution,
        )?;
        return Ok(BuiltinFamilyArtifact::Architecture(Box::new(
            ArchitectureFamilyArtifact {
                pair: FamilyPair::new(model, layout),
                group_theme,
                shoal: Some(geometry),
            },
        )));
    }
    #[cfg(feature = "layout-cytoscape")]
    if execution.layout_backend() == GraphLayoutBackend::CoseBilkent {
        let layout = crate::architecture::layout_architecture_diagram_typed(
            model,
            config,
            execution.text_measurer(),
            execution.operation_seed(),
            execution.work_meter_ref(),
            group_theme.native_style().map(|native| &native.layout_text),
        )?;
        return Ok(BuiltinFamilyArtifact::Architecture(Box::new(
            ArchitectureFamilyArtifact {
                pair: FamilyPair::new(model, layout),
                group_theme,
                #[cfg(feature = "layout-shoal")]
                shoal: None,
            },
        )));
    }
    Err(execution
        .layout_backend()
        .unavailable_for(&meta.diagram_type))
}

#[inline(never)]
#[cfg(feature = "diagram-c4")]
fn prepare_c4_family<'parsed>(
    model: &'parsed diagrams::c4::C4DiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let mut typography_theme =
        crate::c4::C4TypographyThemePlan::resolve(native, &meta.effective_config, model);
    let cluster_theme = crate::c4::C4ClusterThemePlan::resolve(
        native,
        &model.boundaries,
        &meta.effective_config,
        execution.layout_backend() == GraphLayoutBackend::Shoal,
    );
    let config = meta.effective_config.as_value();
    let grid = || {
        crate::c4::layout_c4_diagram_typed(
            model,
            config,
            &typography_theme,
            execution.text_measurer(),
            crate::geom::Size::new(execution.container_width, execution.container_height),
            execution.screen_available_width,
            execution.work_meter_ref(),
        )
    };
    #[cfg(feature = "layout-shoal")]
    let (layout, shoal) = if execution.layout_backend() == GraphLayoutBackend::Shoal {
        let (layout, geometry) = crate::c4::layout_c4_shoal(
            model,
            config,
            &typography_theme,
            &cluster_theme,
            execution,
        )?;
        (layout, Some(geometry))
    } else {
        (grid()?, None)
    };
    #[cfg(not(feature = "layout-shoal"))]
    let layout = grid()?;
    let text_paint = crate::c4::C4TextPaintPlan::prepare(
        model,
        &layout,
        &mut typography_theme,
        &meta.effective_config,
        execution.layout_backend() == GraphLayoutBackend::Shoal,
    )?;
    Ok(BuiltinFamilyArtifact::C4(Box::new(C4FamilyArtifact {
        pair: FamilyPair::new(model, layout),
        cluster_theme,
        typography_theme,
        text_paint,
        #[cfg(feature = "layout-shoal")]
        shoal,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-gantt")]
fn prepare_gantt_family<'parsed>(
    model: &'parsed diagrams::gantt::GanttDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::gantt::GanttStyle::resolve(&meta.effective_config, native);
    let (layout, task_source_indices) = crate::gantt::layout_gantt_diagram_typed(
        model,
        meta.title.as_deref(),
        &meta.effective_config,
        &style,
        execution.text_measurer(),
        execution.container_width,
        execution.local_time_zone(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Gantt(Box::new(
        GanttFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
            task_source_indices,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-pie")]
fn prepare_pie_family<'parsed>(
    model: &'parsed diagrams::pie::PieDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::pie::PieAppearance;

    if let Some(theme) = execution.native_family_theme() {
        let (layout, style) = crate::pie::prepare_native_pie(
            theme,
            model,
            meta.title.as_deref(),
            &meta.effective_config,
            execution.text_measurer(),
        )?;
        return Ok(BuiltinFamilyArtifact::Pie(Box::new(PieFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            appearance: PieAppearance::Native(style),
        })));
    }
    let theme = crate::pie::PieThemePlan::baseline(model, &meta.effective_config);
    let layout = crate::pie::layout_pie_diagram_typed_with_paint_plan(
        model,
        meta.title.as_deref(),
        &meta.effective_config,
        &theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Pie(Box::new(PieFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        appearance: PieAppearance::Mermaid(theme),
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-timeline")]
fn prepare_timeline_family<'parsed>(
    model: &'parsed diagrams::timeline::TimelineDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::timeline::TimelineStyle::resolve(&meta.effective_config, native);
    let layout = crate::timeline::layout_timeline_diagram_typed_with_binding(
        model,
        style.layout_settings(),
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Timeline(Box::new(
        TimelineFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-journey")]
fn prepare_journey_family<'parsed>(
    model: &'parsed diagrams::journey::JourneyDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::journey::JourneyStyle::resolve(&meta.effective_config, native);
    let layout = crate::journey::layout_journey_diagram_typed_with_resolved_typography(
        model,
        meta.title.as_deref(),
        &style,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::Journey(Box::new(
        JourneyFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-radar")]
fn prepare_radar_family<'parsed>(
    model: &'parsed diagrams::radar::RadarDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let typography_theme =
        crate::radar::RadarTypographyThemePlan::resolve(native, &meta.effective_config);
    let series_paint =
        crate::radar::RadarSeriesPaintPlan::resolve(native, &meta.effective_config, model);
    let layout = crate::radar::layout_radar_diagram_typed_with_work_meter(
        model,
        meta.effective_config.as_value(),
        execution.text_measurer(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Radar(Box::new(
        RadarFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            series_paint,
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-treemap")]
fn prepare_treemap_family<'parsed>(
    model: &'parsed diagrams::treemap::TreemapDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let layout = crate::treemap::layout_treemap_diagram_typed_with_work_meter(
        model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        execution.work_meter_ref(),
    )?;
    let native = execution.native_family_theme();
    let style = crate::treemap::TreemapStyle::resolve(
        &meta.effective_config,
        &layout,
        native,
        execution.family.session().canvas_background(),
        &execution.work_meter(),
    )?;
    Ok(BuiltinFamilyArtifact::Treemap(Box::new(
        TreemapFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-er")]
fn prepare_er_family<'parsed>(
    model: &'parsed diagrams::er::ErDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let theme = execution.native_family_theme();
    let native = theme.map(|theme| {
        crate::er::ErNativeStyle::resolve(
            theme,
            &meta.effective_config,
            execution.family.session().canvas_background(),
        )
    });
    let font_family = crate::config::config_font_family_css(&meta.effective_config);
    let config = crate::er::ErConfigView::new(meta.effective_config.as_value());
    let backend = execution.layout_backend();
    let relationship_html_labels =
        config.relationship_html_labels(backend == GraphLayoutBackend::Swimlane);
    let settings = config.layout_settings_with_native_style(
        &model.direction,
        Some(&font_family),
        native.as_ref(),
        relationship_html_labels,
    );
    let crate::er::ErPreparedLayout {
        layout,
        route_contract,
    } = crate::er::layout_er_diagram_typed(
        model,
        meta.effective_config.as_value(),
        backend,
        execution.text_measurer(),
        #[cfg(feature = "layout-elk")]
        execution.elk_operation_seed(),
        settings,
        #[cfg(feature = "layout-shoal")]
        Some(execution),
        execution.work_meter(),
    )?;
    let entity_theme = crate::er::ErEntityThemePlan::resolve(
        &meta.effective_config,
        theme.zip(native),
        relationship_html_labels,
        model,
        &layout,
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Er(Box::new(ErFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        entity_theme,
        route_contract,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-quadrant-chart")]
fn prepare_quadrant_chart_family<'parsed>(
    model: &'parsed diagrams::quadrant_chart::QuadrantChartDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let point_theme = crate::quadrantchart::QuadrantChartPointThemePlan::resolve(
        native,
        &meta.effective_config,
        model,
        execution.work_meter_ref(),
    )?;
    let layout = crate::quadrantchart::layout_quadrantchart_diagram_typed(
        model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        &point_theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::QuadrantChart(Box::new(
        QuadrantChartFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            point_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-xychart")]
fn prepare_xy_chart_family<'parsed>(
    model: &'parsed diagrams::xy_chart::XyChartDiagramRenderModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::xychart::{XyChartTypographyThemePlan, resolve_series_paint};

    let native = execution.native_family_theme();
    let config = &meta.effective_config;
    let series_paint = resolve_series_paint(config, model, native);
    let typography_theme = native.map_or_else(
        || XyChartTypographyThemePlan::resolve(config),
        |theme| XyChartTypographyThemePlan::native(config, &theme.typography),
    );
    let (mut layout, bar_legend_plots) = crate::xychart::layout_xychart_diagram_typed(
        model,
        meta.title.as_deref(),
        &series_paint,
        &typography_theme,
        native,
        execution.text_measurer(),
    )?;
    if execution.family.session().canvas_background().is_some() {
        layout.background_color = "none".to_owned();
    }
    Ok(BuiltinFamilyArtifact::XyChart(Box::new(
        XyChartFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
            bar_legend_plots,
            data_label_color: series_paint.into_data_label_color(),
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-git-graph")]
fn prepare_gitgraph_family<'parsed>(
    model: &'parsed diagrams::git_graph::GitGraphDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::gitgraph::GitGraphTypographyThemePlan;

    let typography_theme = match execution.native_family_theme() {
        Some(theme) => GitGraphTypographyThemePlan::for_native(&meta.effective_config, theme),
        None => GitGraphTypographyThemePlan::resolve(&meta.effective_config),
    };
    let layout = crate::gitgraph::layout_gitgraph_diagram_typed(
        model,
        meta.effective_config.as_value(),
        &typography_theme,
        execution.text_measurer(),
    )?;
    Ok(BuiltinFamilyArtifact::GitGraph(Box::new(
        GitGraphFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            typography_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-tree-view")]
fn prepare_tree_view_family<'parsed>(
    model: &'parsed diagrams::tree_view::TreeViewDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::tree_view::TreeViewCssBinding;

    let theme = match execution.native_family_theme() {
        Some(theme) => TreeViewCssBinding::for_native(&meta.effective_config, theme),
        None => TreeViewCssBinding::from_config(meta.effective_config.as_value()),
    };
    let layout = crate::tree_view::layout_tree_view_diagram_typed(
        model,
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
fn prepare_packet_family<'parsed>(
    model: &'parsed diagrams::packet::PacketDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::packet::PacketStyle::resolve(native, &meta.effective_config);
    let layout = crate::packet::layout_packet_diagram_typed(
        model,
        meta.title.as_deref(),
        &meta.effective_config,
    )?;
    Ok(BuiltinFamilyArtifact::Packet(Box::new(
        PacketFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-requirement")]
fn prepare_requirement_family<'parsed>(
    model: &'parsed diagrams::requirement::RequirementDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution
        .native_family_theme()
        .map(|theme| (theme, execution.family.session().canvas_background()));
    let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve(
        native,
        &meta.effective_config,
        model,
        execution.work_meter_ref(),
    )?;
    let layout =
        crate::requirement::layout_requirement_diagram_typed_with_work_meter_and_typography(
            model,
            meta.effective_config.as_config(),
            execution.layout_backend(),
            execution.text_measurer(),
            paint_theme.css(),
            &execution.work_meter(),
            #[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
            Some(execution),
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
fn prepare_class_family<'parsed>(
    model: &'parsed ClassDiagram,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let relation_theme = crate::class::ClassRelationThemePlan::resolve(
        native,
        &meta.effective_config,
        execution.family.session().canvas_background(),
    );
    let typography_theme =
        crate::class::ClassTextThemePlan::resolve(native, &meta.effective_config);
    let mut render_config =
        crate::class::ClassRenderConfig::resolve(&meta.effective_config, &typography_theme);
    if let Some(native) = native {
        render_config = render_config.with_native_style(native, &meta.effective_config);
    }
    let (layout, prepared_labels, route_contract) = crate::layout_class_typed_by_engine(
        model,
        &meta.effective_config,
        execution,
        &render_config,
    )?;
    let node_visual_plan =
        crate::class::ClassNodeVisualPlan::resolve(model, execution.work_meter_ref())?;
    Ok(BuiltinFamilyArtifact::Class(Box::new(
        ClassFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            prepared_labels,
            route_contract,
            relation_theme,
            typography_theme,
            render_config,
            node_visual_plan,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-venn")]
fn prepare_venn_family<'parsed>(
    model: &'parsed diagrams::venn::VennDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let layout = crate::venn::layout_venn_diagram_typed_with_work_meter(
        model,
        meta.title.as_deref(),
        meta.effective_config.as_value(),
        crate::venn::native_font_size(native, &meta.effective_config),
        execution.work_meter_ref(),
    )?;
    let typography_theme = crate::venn::VennTypographyThemePlan::resolve(
        native,
        &meta.effective_config,
        &layout,
        model,
        execution.family.session().canvas_background(),
        execution.work_meter_ref(),
    )?;
    Ok(BuiltinFamilyArtifact::Venn(Box::new(VennFamilyArtifact {
        pair: FamilyPair::new(model, layout),
        typography_theme,
    })))
}

#[inline(never)]
#[cfg(feature = "diagram-zenuml")]
fn prepare_zenuml_family<'parsed>(
    model: &'parsed diagrams::zenuml::ZenumlDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    use crate::zenuml::ZenumlSourceStyle;

    let background = execution.family.session().canvas_background();
    let mut style = if let Some(native) = execution.native_family_theme() {
        ZenumlSourceStyle::for_native(native, background)
    } else {
        let style = ZenumlSourceStyle::new(meta.effective_config.as_value());
        if background.is_some() {
            style.with_canvas_replacement()
        } else {
            style
        }
    };
    let measurer = execution.text_measurer();
    let mut layout =
        crate::zenuml::layout_zenuml_diagram_typed(model, &style.typography, measurer)?;
    if let Some(native) = style.native.as_mut() {
        if let Some(title) = crate::zenuml::resolve_zenuml_title(model, meta.title.as_deref()) {
            let title = crate::zenuml::resolve_zenuml_emojis_in_text(title);
            let title_style = style.typography.native_title_style();
            layout.width = layout
                .width
                .max(measurer.measure(&title, &title_style).width);
        }
        native.measure_numbers(&mut layout, &style.typography, measurer);
    }
    Ok(BuiltinFamilyArtifact::Zenuml(Box::new(
        ZenumlFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-event-modeling")]
fn prepare_eventmodeling_family<'parsed>(
    model: &'parsed diagrams::event_modeling::EventModelingDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let style = crate::eventmodeling::EventModelingStyle::resolve(&meta.effective_config, native);
    let (layout, prepared_labels) =
        crate::eventmodeling::prepare_eventmodeling_diagram_typed_with_binding(
            model,
            style.css_binding(),
            style.native(),
            &meta.effective_config,
            execution.text_measurer(),
            &execution.work_meter(),
        )?;
    Ok(BuiltinFamilyArtifact::EventModeling(Box::new(
        EventModelingFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            style,
            prepared_labels,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-ishikawa")]
fn prepare_ishikawa_family<'parsed>(
    model: &'parsed diagrams::ishikawa::IshikawaDiagramModel,
    meta: &PresentationMetadata,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact<'parsed>> {
    let native = execution.native_family_theme();
    let mut cfg =
        crate::ishikawa::IshikawaConfigView::new(&meta.effective_config).layout_settings();
    let mut typography =
        crate::ishikawa::IshikawaTypographyPlan::resolve(&meta.effective_config, cfg.font_size);
    if let Some(theme) = native {
        typography.apply_native(theme, &meta.effective_config, &mut cfg);
    }
    let layout = crate::ishikawa::layout_ishikawa_diagram_typed(
        model,
        &cfg,
        &typography,
        execution.text_measurer(),
    )?;
    let text_theme =
        crate::ishikawa::IshikawaTextThemePlan::resolve(native, &meta.effective_config, typography);
    Ok(BuiltinFamilyArtifact::Ishikawa(Box::new(
        IshikawaFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            text_theme,
        },
    )))
}

#[inline(never)]
#[cfg(feature = "diagram-class")]
pub(super) fn prepare_class_render<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact<'parsed>> {
    let meta = context.render_metadata(parsed.metadata());
    let model = parsed.model();
    let RenderSemanticModel::Class(model) = model else {
        unreachable!("Class render dispatch requires a Class semantic model")
    };
    let execution =
        LayoutExecution::new(options, context.execution(), Some(&meta.effective_config));
    let family = prepare_class_family(model, meta, &execution)?;
    FamilyRenderArtifact::new(parsed, family, context)
}

#[inline(never)]
#[cfg(all(
    feature = "diagram-architecture",
    any(feature = "layout-cytoscape", feature = "layout-shoal")
))]
pub(super) fn prepare_architecture_render<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact<'parsed>> {
    let meta = context.render_metadata(parsed.metadata());
    let model = parsed.model();
    let RenderSemanticModel::Architecture(model) = model else {
        unreachable!("Architecture render dispatch requires an Architecture semantic model")
    };
    let execution =
        LayoutExecution::new(options, context.execution(), Some(&meta.effective_config));
    let family = prepare_architecture_family(model, meta, &execution)?;
    FamilyRenderArtifact::new(parsed, family, context)
}

#[inline(never)]
pub(super) fn prepare_non_class_render<'parsed>(
    parsed: &'parsed PresentationDiagram,
    options: &LayoutOptions,
    context: FamilyRenderContext,
    preparation: FamilyPreparation,
) -> Result<FamilyRenderArtifact<'parsed>> {
    let meta = context.render_metadata(parsed.metadata());
    let model = parsed.model();
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    let flowchart_render_context = parsed
        .render_context()
        .flowchart_render_context()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| {
            std::borrow::Cow::Owned(match model {
                RenderSemanticModel::Flowchart(model) => {
                    diagrams::flowchart::FlowchartRenderContext::from_model(model)
                }
                _ => Default::default(),
            })
        });
    let diagram_type = meta.diagram_type.as_str();
    let title = meta.title.as_deref();
    let execution =
        LayoutExecution::new(options, context.execution(), Some(&meta.effective_config));
    let family = match model {
        RenderSemanticModel::Error(model) => prepare_error_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => prepare_mindmap_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => prepare_state_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            BuiltinFamilyArtifact::Sequence(prepare_pair(model, |model| {
                crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
                    model,
                    title,
                    &meta.effective_config,
                    execution
                        .native_family_theme()
                        .map(|theme| (theme, execution.family.session().canvas_background())),
                    execution.text_measurer(),
                    execution.math_backend(),
                    execution.work_meter(),
                )
            })?)
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => prepare_zenuml_family(model, meta, &execution)?,
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => prepare_flowchart_family(
            model,
            flowchart_render_context,
            meta,
            &execution,
            preparation,
        )?,
        #[cfg(any(feature = "layout-cytoscape", feature = "layout-shoal"))]
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(_) => {
            unreachable!("Architecture models use the stack-bounded family dispatch path")
        }
        #[cfg(not(any(feature = "layout-cytoscape", feature = "layout-shoal")))]
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
        RenderSemanticModel::C4(model) => prepare_c4_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(model) => prepare_cynefin_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(model) => prepare_wardley_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(model) => {
            prepare_railroad_family(model, diagram_type, meta, &execution)?
        }
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => {
            BuiltinFamilyArtifact::Kanban(prepare_pair(model, |model| {
                crate::kanban::prepare_kanban_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.native_family_theme(),
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                    options,
                )?
                .with_retained_text_accounting(&execution.work_meter())
            })?)
        }
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => prepare_gantt_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => prepare_pie_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => prepare_packet_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => prepare_timeline_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => prepare_journey_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => {
            prepare_requirement_family(model, meta, &execution)?
        }
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(model) => prepare_sankey_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(model) => prepare_radar_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(model) => prepare_info_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => prepare_treemap_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => prepare_block_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => prepare_er_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(model) => {
            prepare_quadrant_chart_family(model, meta, &execution)?
        }
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => prepare_xy_chart_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => prepare_gitgraph_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => prepare_tree_view_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(model) => prepare_ishikawa_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(model) => {
            prepare_eventmodeling_family(model, meta, &execution)?
        }
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(model) => prepare_venn_family(model, meta, &execution)?,
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => {
            BuiltinFamilyArtifact::Usecase(prepare_pair(model, |model| {
                crate::usecase::prepare_usecase_diagram(model, &meta.effective_config, &execution)
            })?)
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => {
            let (flowchart, render_context) = model.to_flowchart_model();
            let mut projected_metadata = Box::new(meta.clone());
            projected_metadata.effective_config = project_agentflow_flowchart_config(
                &meta.effective_config,
                execution.layout_backend(),
                execution.work_meter_ref().control(),
            )?;
            let config = &projected_metadata.effective_config;
            let flow = prepare_flowchart_artifact(
                std::borrow::Cow::Owned(flowchart),
                std::borrow::Cow::Owned(render_context),
                &execution,
                config,
                false,
                preparation,
                |layout| crate::svg::FlowchartNodeLayoutView::Flowchart(layout),
                |model, render_context, class_defs, sidecar, edge_style_plan| {
                    crate::layout_flowchart_typed_with_render_labels_by_engine(
                        model,
                        render_context,
                        class_defs,
                        config,
                        &execution,
                        sidecar,
                        edge_style_plan,
                        preparation != FamilyPreparation::Layout,
                    )
                },
            )?;
            BuiltinFamilyArtifact::Agentflow {
                semantic: model,
                flow,
                projected_metadata,
            }
        }
        RenderSemanticModel::CustomJson(_) => {
            unreachable!("custom JSON models return before built-in family dispatch")
        }
        // Shared model features can be widened independently of this renderer's handlers.
        #[allow(unreachable_patterns)]
        _ => {
            return Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_owned(),
            });
        }
    };
    FamilyRenderArtifact::new(parsed, family, context)
}
