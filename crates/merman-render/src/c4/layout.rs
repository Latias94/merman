use super::{C4Conf, C4ConfigView, C4Model, measure_c4_text};
use crate::json::from_value_ref;
use crate::model::{
    Bounds, C4BoundaryLayout, C4DiagramLayout, C4ImageLayout, C4RelLayout, C4ShapeLayout,
    C4TextBlockLayout, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use merman_core::diagrams::c4::{C4BoundaryRenderModel, C4DiagramRenderModel};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
struct BoundsData {
    startx: Option<f64>,
    stopx: Option<f64>,
    starty: Option<f64>,
    stopy: Option<f64>,
    width_limit: f64,
}

#[derive(Debug, Clone, Default)]
struct BoundsNext {
    startx: f64,
    stopx: f64,
    starty: f64,
    stopy: f64,
    cnt: usize,
}

#[derive(Debug, Clone, Default)]
struct BoundsState {
    data: BoundsData,
    next: BoundsNext,
}

impl BoundsState {
    fn set_data(&mut self, startx: f64, stopx: f64, starty: f64, stopy: f64) {
        self.next.startx = startx;
        self.data.startx = Some(startx);
        self.next.stopx = stopx;
        self.data.stopx = Some(stopx);
        self.next.starty = starty;
        self.data.starty = Some(starty);
        self.next.stopy = stopy;
        self.data.stopy = Some(stopy);
    }

    fn bump_last_margin(&mut self, margin: f64) {
        if let Some(v) = self.data.stopx.as_mut() {
            *v += margin;
        }
        if let Some(v) = self.data.stopy.as_mut() {
            *v += margin;
        }
    }

    fn update_val_opt(target: &mut Option<f64>, val: f64, fun: fn(f64, f64) -> f64) {
        match target {
            None => *target = Some(val),
            Some(existing) => *existing = fun(val, *existing),
        }
    }

    fn update_val(target: &mut f64, val: f64, fun: fn(f64, f64) -> f64) {
        *target = fun(val, *target);
    }

    fn insert_rect(&mut self, rect: &mut Rect, c4_shape_in_row: usize, conf: &C4Conf) {
        self.next.cnt += 1;

        let startx = if self.next.startx == self.next.stopx {
            self.next.stopx + rect.margin
        } else {
            self.next.stopx + rect.margin * 2.0
        };
        let mut stopx = startx + rect.size.width;
        let starty = self.next.starty + rect.margin * 2.0;
        let mut stopy = starty + rect.size.height;

        if startx >= self.data.width_limit
            || stopx >= self.data.width_limit
            || self.next.cnt > c4_shape_in_row
        {
            let startx2 = self.next.startx + rect.margin + conf.next_line_padding_x;
            let starty2 = self.next.stopy + rect.margin * 2.0;

            stopx = startx2 + rect.size.width;
            stopy = starty2 + rect.size.height;

            self.next.stopx = stopx;
            self.next.starty = self.next.stopy;
            self.next.stopy = stopy;
            self.next.cnt = 1;

            rect.origin.x = startx2;
            rect.origin.y = starty2;
        } else {
            rect.origin.x = startx;
            rect.origin.y = starty;
        }

        Self::update_val_opt(&mut self.data.startx, rect.origin.x, f64::min);
        Self::update_val_opt(&mut self.data.starty, rect.origin.y, f64::min);
        Self::update_val_opt(&mut self.data.stopx, stopx, f64::max);
        Self::update_val_opt(&mut self.data.stopy, stopy, f64::max);

        Self::update_val(&mut self.next.startx, rect.origin.x, f64::min);
        Self::update_val(&mut self.next.starty, rect.origin.y, f64::min);
        Self::update_val(&mut self.next.stopx, stopx, f64::max);
        Self::update_val(&mut self.next.stopy, stopy, f64::max);
    }
}

#[derive(Debug, Clone)]
struct Rect {
    origin: merman_core::geom::Point,
    size: merman_core::geom::Size,
    margin: f64,
}

struct C4LayoutContext<'a> {
    model: &'a C4Model,
    cfg: &'a C4ConfigView<'a>,
    conf: &'a C4Conf,
    c4_shape_in_row: usize,
    c4_boundary_in_row: usize,
    measurer: &'a dyn TextMeasurer,
    boundary_children: &'a HashMap<String, Vec<usize>>,
    shape_children: &'a HashMap<String, Vec<usize>>,
}

struct C4LayoutState {
    boundaries: HashMap<String, C4BoundaryLayout>,
    shapes: HashMap<String, C4ShapeLayout>,
    global_max_x: f64,
    global_max_y: f64,
}

fn has_sprite(v: &Option<Value>) -> bool {
    v.as_ref().is_some_and(|v| match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(_) => true,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    })
}

fn intersect_point(from: &Rect, end_point: LayoutPoint) -> LayoutPoint {
    let x1 = from.origin.x;
    let y1 = from.origin.y;
    let x2 = end_point.x;
    let y2 = end_point.y;

    let from_center_x = x1 + from.size.width / 2.0;
    let from_center_y = y1 + from.size.height / 2.0;

    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let tan_dyx = dy / dx;
    let from_dyx = from.size.height / from.size.width;

    let mut return_point: Option<LayoutPoint> = None;

    if y1 == y2 && x1 < x2 {
        return_point = Some(LayoutPoint {
            x: x1 + from.size.width,
            y: from_center_y,
        });
    } else if y1 == y2 && x1 > x2 {
        return_point = Some(LayoutPoint {
            x: x1,
            y: from_center_y,
        });
    } else if x1 == x2 && y1 < y2 {
        return_point = Some(LayoutPoint {
            x: from_center_x,
            y: y1 + from.size.height,
        });
    } else if x1 == x2 && y1 > y2 {
        return_point = Some(LayoutPoint {
            x: from_center_x,
            y: y1,
        });
    }

    if x1 > x2 && y1 < y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1,
                y: from_center_y + (tan_dyx * from.size.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x - ((dx / dy) * from.size.height) / 2.0,
                y: y1 + from.size.height,
            });
        }
    } else if x1 < x2 && y1 < y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1 + from.size.width,
                y: from_center_y + (tan_dyx * from.size.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x + ((dx / dy) * from.size.height) / 2.0,
                y: y1 + from.size.height,
            });
        }
    } else if x1 < x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1 + from.size.width,
                y: from_center_y - (tan_dyx * from.size.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x + ((from.size.height / 2.0) * dx) / dy,
                y: y1,
            });
        }
    } else if x1 > x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1,
                y: from_center_y - (from.size.width / 2.0) * tan_dyx,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x - ((from.size.height / 2.0) * dx) / dy,
                y: y1,
            });
        }
    }

    return_point.unwrap_or(LayoutPoint {
        x: from_center_x,
        y: from_center_y,
    })
}

fn intersect_points(from: &Rect, to: &Rect) -> (LayoutPoint, LayoutPoint) {
    let end_intersect_point = LayoutPoint {
        x: to.origin.x + to.size.width / 2.0,
        y: to.origin.y + to.size.height / 2.0,
    };
    let start_point = intersect_point(from, end_intersect_point);

    let end_intersect_point = LayoutPoint {
        x: from.origin.x + from.size.width / 2.0,
        y: from.origin.y + from.size.height / 2.0,
    };
    let end_point = intersect_point(to, end_intersect_point);

    (start_point, end_point)
}

fn layout_c4_shape_array(
    current_bounds: &mut BoundsState,
    shape_indices: &[usize],
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) {
    for idx in shape_indices {
        let shape = &ctx.model.shapes[*idx];
        let mut y = ctx.conf.c4_shape_padding;

        let type_c4_shape = shape.type_c4_shape.as_str().to_string();
        let mut type_conf = ctx.cfg.shape_font(&type_c4_shape);
        type_conf.font_size -= 2.0;

        let type_text = format!("«{}»", type_c4_shape);
        let type_metrics = ctx.measurer.measure(&type_text, &type_conf);
        let type_block = C4TextBlockLayout {
            text: type_text,
            y,
            width: type_metrics.width,
            height: type_conf.font_size + 2.0,
            line_count: 1,
        };
        y = y + type_block.height - 4.0;

        let mut image = C4ImageLayout {
            width: 0.0,
            height: 0.0,
            y: 0.0,
        };
        if matches!(type_c4_shape.as_str(), "person" | "external_person") {
            image.width = 48.0;
            image.height = 48.0;
            image.y = y;
            y = image.y + image.height;
        }
        if has_sprite(&shape.sprite) {
            image.width = 48.0;
            image.height = 48.0;
            image.y = y;
            y = image.y + image.height;
        }

        let text_wrap = shape.wrap && ctx.conf.wrap;
        let text_limit_width = ctx.conf.width - ctx.conf.c4_shape_padding * 2.0;

        let mut label_conf = ctx.cfg.shape_font(&type_c4_shape);
        label_conf.font_size += 2.0;
        label_conf.font_weight = Some("bold".to_string());

        let label_text = shape.label.as_str().to_string();
        let label_m = measure_c4_text(
            ctx.measurer,
            &label_text,
            &label_conf,
            text_wrap,
            text_limit_width,
        );
        let label = C4TextBlockLayout {
            text: label_text,
            y: y + 8.0,
            width: label_m.width,
            height: label_m.height,
            line_count: label_m.line_count,
        };
        y = label.y + label.height;

        let mut ty_block: Option<C4TextBlockLayout> = None;
        let mut techn_block: Option<C4TextBlockLayout> = None;

        if let Some(ty) = shape.ty.as_ref().filter(|t| !t.as_str().is_empty()) {
            let type_text = format!("[{}]", ty.as_str());
            let type_conf = ctx.cfg.shape_font(&type_c4_shape);
            let m = measure_c4_text(
                ctx.measurer,
                &type_text,
                &type_conf,
                text_wrap,
                text_limit_width,
            );
            let block = C4TextBlockLayout {
                text: type_text,
                y: y + 5.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
            };
            y = block.y + block.height;
            ty_block = Some(block);
        } else if let Some(techn) = shape.techn.as_ref().filter(|t| !t.as_str().is_empty()) {
            let techn_text = format!("[{}]", techn.as_str());
            // Mermaid@11.12.2 C4 renderer quirk: `techn` text is measured with
            // `c4ShapeFont(conf, c4Shape.techn.text)`, where `c4Shape.techn.text` already contains
            // the bracketed string (e.g. `[Rust]`). That key does not exist in the config object,
            // so the downstream `calculateTextDimensions` falls back to its defaults
            // (`fontSize=12`, `fontFamily='Arial'`).
            //
            // Upstream SVG baselines encode this behavior into shape heights and ultimately the
            // root viewBox. Mirror it here for parity.
            let techn_conf = TextStyle {
                font_family: Some("Arial".to_string()),
                font_size: 12.0,
                font_weight: None,
            };
            let m = measure_c4_text(
                ctx.measurer,
                &techn_text,
                &techn_conf,
                text_wrap,
                text_limit_width,
            );
            let block = C4TextBlockLayout {
                text: techn_text,
                y: y + 5.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
            };
            y = block.y + block.height;
            techn_block = Some(block);
        }

        let mut rect_height = y;
        let mut rect_width = label.width;

        let mut descr_block: Option<C4TextBlockLayout> = None;
        if let Some(descr) = shape.descr.as_ref().filter(|t| !t.as_str().is_empty()) {
            let descr_text = descr.as_str().to_string();
            let descr_conf = ctx.cfg.shape_font(&type_c4_shape);
            let m = measure_c4_text(
                ctx.measurer,
                &descr_text,
                &descr_conf,
                text_wrap,
                text_limit_width,
            );
            let block = C4TextBlockLayout {
                text: descr_text,
                y: y + 20.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
            };
            y = block.y + block.height;
            rect_width = rect_width.max(block.width);
            rect_height = y - block.line_count as f64 * 5.0;
            descr_block = Some(block);
        }

        rect_width += ctx.conf.c4_shape_padding;

        let width = ctx.conf.width.max(rect_width);
        let height = ctx.conf.height.max(rect_height);
        let margin = ctx.conf.c4_shape_margin;

        let mut rect = Rect {
            origin: merman_core::geom::point(0.0, 0.0),
            size: merman_core::geom::Size::new(width, height),
            margin,
        };
        current_bounds.insert_rect(&mut rect, ctx.c4_shape_in_row, ctx.conf);

        state.shapes.insert(
            shape.alias.clone(),
            C4ShapeLayout {
                alias: shape.alias.clone(),
                parent_boundary: shape.parent_boundary.clone(),
                type_c4_shape: type_c4_shape.clone(),
                x: rect.origin.x,
                y: rect.origin.y,
                width: rect.size.width,
                height: rect.size.height,
                margin: rect.margin,
                image,
                type_block,
                label,
                ty: ty_block,
                techn: techn_block,
                descr: descr_block,
            },
        );
    }

    current_bounds.bump_last_margin(ctx.conf.c4_shape_margin);
}

struct PendingC4BoundaryLayout {
    alias: String,
    parent_boundary: String,
    image: C4ImageLayout,
    label: C4TextBlockLayout,
    ty: Option<C4TextBlockLayout>,
    descr: Option<C4TextBlockLayout>,
}

struct C4BoundaryFrame {
    boundary_indices: Vec<usize>,
    next_index: usize,
    parent_bounds: BoundsState,
    current_bounds: BoundsState,
    pending: Option<PendingC4BoundaryLayout>,
}

impl C4BoundaryFrame {
    fn new(
        boundary_indices: Vec<usize>,
        parent_bounds: BoundsState,
        ctx: &C4LayoutContext<'_>,
    ) -> Self {
        let denom = ctx.c4_boundary_in_row.min(boundary_indices.len().max(1));
        let width_limit = parent_bounds.data.width_limit / denom as f64;
        let mut current_bounds = BoundsState::default();
        current_bounds.data.width_limit = width_limit;

        Self {
            boundary_indices,
            next_index: 0,
            parent_bounds,
            current_bounds,
            pending: None,
        }
    }
}

fn prepare_c4_boundary_layout(
    boundary: &C4BoundaryRenderModel,
    width_limit: f64,
    ctx: &C4LayoutContext<'_>,
) -> (PendingC4BoundaryLayout, f64) {
    let mut y = 0.0;

    let mut image = C4ImageLayout {
        width: 0.0,
        height: 0.0,
        y: 0.0,
    };
    if has_sprite(&boundary.sprite) {
        image.width = 48.0;
        image.height = 48.0;
        image.y = y;
        y = image.y + image.height;
    }

    let text_wrap = boundary.wrap.unwrap_or(ctx.model.wrap) && ctx.conf.wrap;
    let mut label_conf = ctx.conf.boundary_font();
    label_conf.font_size += 2.0;
    label_conf.font_weight = Some("bold".to_string());

    let label_text = boundary.label.as_str().to_string();
    let label_m = measure_c4_text(
        ctx.measurer,
        &label_text,
        &label_conf,
        text_wrap,
        width_limit,
    );
    let label = C4TextBlockLayout {
        text: label_text,
        y: y + 8.0,
        width: label_m.width,
        height: label_m.height,
        line_count: label_m.line_count,
    };
    y = label.y + label.height;

    let mut ty: Option<C4TextBlockLayout> = None;
    if let Some(boundary_ty) = boundary.ty.as_ref().filter(|t| !t.as_str().is_empty()) {
        let ty_text = format!("[{}]", boundary_ty.as_str());
        let ty_conf = ctx.conf.boundary_font();
        let m = measure_c4_text(ctx.measurer, &ty_text, &ty_conf, text_wrap, width_limit);
        let block = C4TextBlockLayout {
            text: ty_text,
            y: y + 5.0,
            width: m.width,
            height: m.height,
            line_count: m.line_count,
        };
        y = block.y + block.height;
        ty = Some(block);
    }

    let mut descr: Option<C4TextBlockLayout> = None;
    if let Some(boundary_descr) = boundary.descr.as_ref().filter(|t| !t.as_str().is_empty()) {
        let descr_text = boundary_descr.as_str().to_string();
        let mut descr_conf = ctx.conf.boundary_font();
        descr_conf.font_size -= 2.0;
        let m = measure_c4_text(
            ctx.measurer,
            &descr_text,
            &descr_conf,
            text_wrap,
            width_limit,
        );
        let block = C4TextBlockLayout {
            text: descr_text,
            y: y + 20.0,
            width: m.width,
            height: m.height,
            line_count: m.line_count,
        };
        y = block.y + block.height;
        descr = Some(block);
    }

    (
        PendingC4BoundaryLayout {
            alias: boundary.alias.clone(),
            parent_boundary: boundary.parent_boundary.clone(),
            image,
            label,
            ty,
            descr,
        },
        y,
    )
}

fn finish_c4_boundary_layout(
    parent_bounds: &mut BoundsState,
    current_bounds: &BoundsState,
    pending: PendingC4BoundaryLayout,
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) {
    let startx = current_bounds.data.startx.unwrap_or(0.0);
    let stopx = current_bounds.data.stopx.unwrap_or(startx);
    let starty = current_bounds.data.starty.unwrap_or(0.0);
    let stopy = current_bounds.data.stopy.unwrap_or(starty);

    state.boundaries.insert(
        pending.alias.clone(),
        C4BoundaryLayout {
            alias: pending.alias,
            parent_boundary: pending.parent_boundary,
            x: startx,
            y: starty,
            width: stopx - startx,
            height: stopy - starty,
            image: pending.image,
            label: pending.label,
            ty: pending.ty,
            descr: pending.descr,
        },
    );

    let stopx_with_margin = stopx + ctx.conf.c4_shape_margin;
    let stopy_with_margin = stopy + ctx.conf.c4_shape_margin;
    parent_bounds.data.stopx = Some(
        parent_bounds
            .data
            .stopx
            .unwrap_or(stopx_with_margin)
            .max(stopx_with_margin),
    );
    parent_bounds.data.stopy = Some(
        parent_bounds
            .data
            .stopy
            .unwrap_or(stopy_with_margin)
            .max(stopy_with_margin),
    );

    state.global_max_x = state
        .global_max_x
        .max(parent_bounds.data.stopx.unwrap_or(state.global_max_x));
    state.global_max_y = state
        .global_max_y
        .max(parent_bounds.data.stopy.unwrap_or(state.global_max_y));
}

fn layout_inside_boundary(
    parent_bounds: &mut BoundsState,
    boundary_indices: &[usize],
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) -> Result<()> {
    let mut stack = vec![C4BoundaryFrame::new(
        boundary_indices.to_vec(),
        parent_bounds.clone(),
        ctx,
    )];

    while let Some(frame) = stack.last_mut() {
        if let Some(pending) = frame.pending.take() {
            finish_c4_boundary_layout(
                &mut frame.parent_bounds,
                &frame.current_bounds,
                pending,
                ctx,
                state,
            );
            continue;
        }

        if frame.next_index >= frame.boundary_indices.len() {
            let Some(finished) = stack.pop() else {
                break;
            };
            if let Some(parent) = stack.last_mut() {
                parent.current_bounds = finished.parent_bounds;
                continue;
            }

            *parent_bounds = finished.parent_bounds;
            return Ok(());
        }

        let i = frame.next_index;
        let idx = frame.boundary_indices[i];
        frame.next_index += 1;

        let boundary = &ctx.model.boundaries[idx];
        let width_limit = frame.current_bounds.data.width_limit;
        let (pending, y) = prepare_c4_boundary_layout(boundary, width_limit, ctx);

        let parent_startx = frame
            .parent_bounds
            .data
            .startx
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing startx".to_string(),
            })?;
        let parent_stopy = frame
            .parent_bounds
            .data
            .stopy
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing stopy".to_string(),
            })?;

        if i == 0 || i % ctx.c4_boundary_in_row == 0 {
            let x = parent_startx + ctx.conf.diagram_margin_x;
            let y0 = parent_stopy + ctx.conf.diagram_margin_y + y;
            frame.current_bounds.set_data(x, x, y0, y0);
        } else {
            let startx = frame.current_bounds.data.startx.unwrap_or(parent_startx);
            let stopx = frame.current_bounds.data.stopx.unwrap_or(startx);
            let x = if stopx != startx {
                stopx + ctx.conf.diagram_margin_x
            } else {
                startx
            };
            let y0 = frame.current_bounds.data.starty.unwrap_or(parent_stopy);
            frame.current_bounds.set_data(x, x, y0, y0);
        }

        if let Some(shape_indices) = ctx.shape_children.get(&boundary.alias)
            && !shape_indices.is_empty()
        {
            layout_c4_shape_array(&mut frame.current_bounds, shape_indices, ctx, state);
        }

        if let Some(next_boundaries) = ctx.boundary_children.get(&boundary.alias)
            && !next_boundaries.is_empty()
        {
            frame.pending = Some(pending);
            let child_parent_bounds = frame.current_bounds.clone();
            stack.push(C4BoundaryFrame::new(
                next_boundaries.clone(),
                child_parent_bounds,
                ctx,
            ));
            continue;
        }

        finish_c4_boundary_layout(
            &mut frame.parent_bounds,
            &frame.current_bounds,
            pending,
            ctx,
            state,
        );
    }

    Ok(())
}

pub(crate) fn layout_c4_diagram_typed(
    model: &C4DiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    viewport_width: f64,
    viewport_height: f64,
) -> Result<C4DiagramLayout> {
    let c4_cfg = C4ConfigView::new(effective_config);
    let conf = c4_cfg.layout_settings();

    let c4_shape_in_row = (model.layout.c4_shape_in_row.max(1)) as usize;
    let c4_boundary_in_row = (model.layout.c4_boundary_in_row.max(1)) as usize;

    let mut boundary_children: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, b) in model.boundaries.iter().enumerate() {
        boundary_children
            .entry(b.parent_boundary.clone())
            .or_default()
            .push(i);
    }
    let mut shape_children: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, s) in model.shapes.iter().enumerate() {
        shape_children
            .entry(s.parent_boundary.clone())
            .or_default()
            .push(i);
    }

    let mut state = C4LayoutState {
        boundaries: HashMap::new(),
        shapes: HashMap::new(),
        global_max_x: conf.diagram_margin_x,
        global_max_y: conf.diagram_margin_y,
    };

    let mut screen_bounds = BoundsState::default();
    screen_bounds.set_data(
        conf.diagram_margin_x,
        conf.diagram_margin_x,
        conf.diagram_margin_y,
        conf.diagram_margin_y,
    );
    screen_bounds.data.width_limit = viewport_width;

    let root_boundaries = boundary_children.get("").cloned().unwrap_or_default();
    if root_boundaries.is_empty() {
        return Err(Error::InvalidModel {
            message: "c4: expected at least the implicit global boundary".to_string(),
        });
    }

    let ctx = C4LayoutContext {
        model,
        cfg: &c4_cfg,
        conf: &conf,
        c4_shape_in_row,
        c4_boundary_in_row,
        measurer,
        boundary_children: &boundary_children,
        shape_children: &shape_children,
    };

    layout_inside_boundary(&mut screen_bounds, &root_boundaries, &ctx, &mut state)?;

    screen_bounds.data.stopx = Some(state.global_max_x);
    screen_bounds.data.stopy = Some(state.global_max_y);

    let box_startx = screen_bounds.data.startx.unwrap_or(0.0);
    let box_starty = screen_bounds.data.starty.unwrap_or(0.0);
    let box_stopx = screen_bounds.data.stopx.unwrap_or(conf.diagram_margin_x);
    let box_stopy = screen_bounds.data.stopy.unwrap_or(conf.diagram_margin_y);

    let width = (box_stopx - box_startx) + 2.0 * conf.diagram_margin_x;
    let height = (box_stopy - box_starty) + 2.0 * conf.diagram_margin_y;

    let bounds = Some(Bounds {
        min_x: box_startx,
        min_y: box_starty,
        max_x: box_stopx,
        max_y: box_stopy,
    });

    let mut shape_rects: HashMap<&str, Rect> = HashMap::new();
    for s in model.shapes.iter() {
        let Some(l) = state.shapes.get(&s.alias) else {
            continue;
        };
        shape_rects.insert(
            s.alias.as_str(),
            Rect {
                origin: merman_core::geom::point(l.x, l.y),
                size: merman_core::geom::Size::new(l.width, l.height),
                margin: l.margin,
            },
        );
    }

    let rel_font = conf.message_font();
    let mut rels_out: Vec<C4RelLayout> = Vec::new();
    for (i, rel) in model.rels.iter().enumerate() {
        let mut label_text = rel.label.as_str().to_string();
        if model.c4_type == "C4Dynamic" {
            label_text = format!("{}: {}", i + 1, label_text);
        }

        let rel_text_wrap = rel.wrap && conf.wrap;

        let label_limit = measurer.measure(&label_text, &rel_font).width;
        let label_m = measure_c4_text(measurer, &label_text, &rel_font, rel_text_wrap, label_limit);
        let label = C4TextBlockLayout {
            text: label_text,
            y: 0.0,
            width: label_m.width,
            height: label_m.height,
            line_count: label_m.line_count,
        };

        let techn = rel
            .techn
            .as_ref()
            .filter(|t| !t.as_str().is_empty())
            .map(|t| {
                let text = t.as_str().to_string();
                let limit = measurer.measure(&text, &rel_font).width;
                let m = measure_c4_text(measurer, &text, &rel_font, rel_text_wrap, limit);
                C4TextBlockLayout {
                    text,
                    y: 0.0,
                    width: m.width,
                    height: m.height,
                    line_count: m.line_count,
                }
            });

        let descr = rel
            .descr
            .as_ref()
            .filter(|t| !t.as_str().is_empty())
            .map(|t| {
                let text = t.as_str().to_string();
                let limit = measurer.measure(&text, &rel_font).width;
                let m = measure_c4_text(measurer, &text, &rel_font, rel_text_wrap, limit);
                C4TextBlockLayout {
                    text,
                    y: 0.0,
                    width: m.width,
                    height: m.height,
                    line_count: m.line_count,
                }
            });

        let from = shape_rects
            .get(rel.from_alias.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "c4: relationship references missing from shape {}",
                    rel.from_alias
                ),
            })?;
        let to = shape_rects
            .get(rel.to_alias.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "c4: relationship references missing to shape {}",
                    rel.to_alias
                ),
            })?;

        let (start_point, end_point) = intersect_points(from, to);

        rels_out.push(C4RelLayout {
            from: rel.from_alias.clone(),
            to: rel.to_alias.clone(),
            rel_type: rel.rel_type.clone(),
            start_point,
            end_point,
            offset_x: rel.offset_x,
            offset_y: rel.offset_y,
            label,
            techn,
            descr,
        });
    }

    let mut boundaries_out = Vec::with_capacity(model.boundaries.len());
    for b in &model.boundaries {
        let Some(l) = state.boundaries.get(&b.alias) else {
            return Err(Error::InvalidModel {
                message: format!("c4: missing boundary layout for {}", b.alias),
            });
        };
        boundaries_out.push(l.clone());
    }

    let mut shapes_out = Vec::with_capacity(model.shapes.len());
    for s in &model.shapes {
        let Some(l) = state.shapes.get(&s.alias) else {
            return Err(Error::InvalidModel {
                message: format!("c4: missing shape layout for {}", s.alias),
            });
        };
        shapes_out.push(l.clone());
    }

    Ok(C4DiagramLayout {
        bounds,
        width,
        height,
        viewport_width,
        viewport_height,
        c4_type: model.c4_type.clone(),
        title: model.title.clone(),
        use_max_width: conf.use_max_width,
        boundaries: boundaries_out,
        shapes: shapes_out,
        rels: rels_out,
    })
}

pub(crate) fn layout_c4_diagram(
    model: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    viewport_width: f64,
    viewport_height: f64,
) -> Result<C4DiagramLayout> {
    let model: C4DiagramRenderModel = from_value_ref(model)?;
    layout_c4_diagram_typed(
        &model,
        effective_config,
        measurer,
        viewport_width,
        viewport_height,
    )
}

#[cfg(test)]
mod tests {
    use crate::c4::{c4_svg_bbox_line_height_px, c4_text_width_override_px};
    use crate::text::TextStyle;

    #[test]
    fn c4_svg_bbox_line_height_uses_owner_rules() {
        fn style(font_size: f64) -> TextStyle {
            TextStyle {
                font_size,
                ..Default::default()
            }
        }

        assert_eq!(c4_svg_bbox_line_height_px(&style(12.0)), 14.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(14.0)), 16.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(16.0)), 17.0);

        assert_eq!(c4_svg_bbox_line_height_px(&style(15.0)), 17.0);
    }

    #[test]
    fn c4_text_width_override_uses_headless_shell_metric() {
        let style = TextStyle {
            font_family: Some(r#""Open Sans", sans-serif"#.to_string()),
            font_size: 14.0,
            font_weight: None,
        };

        assert_eq!(
            c4_text_width_override_px(
                &style,
                "Allows customers to view information about their bank accounts, and make payments."
            ),
            Some(532.484375)
        );
    }
}
