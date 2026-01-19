use crate::model::{
    Bounds, C4BoundaryLayout, C4DiagramLayout, C4ImageLayout, C4RelLayout, C4ShapeLayout,
    C4TextBlockLayout, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum C4Text {
    Wrapped { text: Value },
    String(String),
    Value(Value),
}

impl Default for C4Text {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl C4Text {
    fn as_str(&self) -> &str {
        match self {
            Self::Wrapped { text } => text.as_str().unwrap_or(""),
            Self::String(s) => s.as_str(),
            Self::Value(v) => v.as_str().unwrap_or(""),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct C4LayoutConfig {
    #[serde(default, rename = "c4ShapeInRow")]
    c4_shape_in_row: i64,
    #[serde(default, rename = "c4BoundaryInRow")]
    c4_boundary_in_row: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct C4Shape {
    alias: String,
    #[serde(default, rename = "parentBoundary")]
    parent_boundary: String,
    #[serde(default, rename = "typeC4Shape")]
    type_c4_shape: C4Text,
    #[serde(default)]
    label: C4Text,
    #[serde(default)]
    #[allow(dead_code)]
    wrap: bool,
    #[serde(default)]
    #[allow(dead_code)]
    sprite: Option<Value>,
    #[serde(default, rename = "type")]
    ty: Option<C4Text>,
    #[serde(default)]
    techn: Option<C4Text>,
    #[serde(default)]
    descr: Option<C4Text>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4Boundary {
    alias: String,
    #[serde(default, rename = "parentBoundary")]
    parent_boundary: String,
    #[serde(default)]
    label: C4Text,
    #[serde(default, rename = "type")]
    ty: Option<C4Text>,
    #[serde(default)]
    descr: Option<C4Text>,
    #[serde(default)]
    #[allow(dead_code)]
    wrap: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    sprite: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4Rel {
    #[serde(rename = "from")]
    from_alias: String,
    #[serde(rename = "to")]
    to_alias: String,
    #[serde(rename = "type")]
    rel_type: String,
    #[serde(default)]
    label: C4Text,
    #[serde(default)]
    techn: Option<C4Text>,
    #[serde(default)]
    descr: Option<C4Text>,
    #[serde(default)]
    #[allow(dead_code)]
    wrap: bool,
    #[serde(default, rename = "offsetX")]
    offset_x: Option<i64>,
    #[serde(default, rename = "offsetY")]
    offset_y: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct C4Model {
    #[serde(default, rename = "c4Type")]
    c4_type: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    wrap: bool,
    #[serde(default)]
    layout: C4LayoutConfig,
    #[serde(default)]
    shapes: Vec<C4Shape>,
    #[serde(default)]
    boundaries: Vec<C4Boundary>,
    #[serde(default)]
    rels: Vec<C4Rel>,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

#[derive(Debug, Clone)]
struct C4Conf {
    diagram_margin_x: f64,
    diagram_margin_y: f64,
    c4_shape_margin: f64,
    c4_shape_padding: f64,
    width: f64,
    height: f64,
    wrap: bool,
    next_line_padding_x: f64,
    boundary_font_family: Option<String>,
    boundary_font_size: f64,
    boundary_font_weight: Option<String>,
    message_font_family: Option<String>,
    message_font_size: f64,
    message_font_weight: Option<String>,
}

impl C4Conf {
    fn from_effective_config(effective_config: &Value) -> Self {
        let global_font_family = config_string(effective_config, &["fontFamily"]);
        let global_font_size = config_f64(effective_config, &["fontSize"]);
        let global_font_weight = config_string(effective_config, &["fontWeight"]);

        let message_font_family = global_font_family
            .clone()
            .or_else(|| config_string(effective_config, &["c4", "messageFontFamily"]));
        let message_font_size = global_font_size
            .or_else(|| config_f64(effective_config, &["c4", "messageFontSize"]))
            .unwrap_or(12.0);
        let message_font_weight = global_font_weight
            .clone()
            .or_else(|| config_string(effective_config, &["c4", "messageFontWeight"]));

        let boundary_font_family = config_string(effective_config, &["c4", "boundaryFontFamily"]);
        let boundary_font_size =
            config_f64(effective_config, &["c4", "boundaryFontSize"]).unwrap_or(14.0);
        let boundary_font_weight = config_string(effective_config, &["c4", "boundaryFontWeight"]);

        Self {
            diagram_margin_x: config_f64(effective_config, &["c4", "diagramMarginX"])
                .unwrap_or(50.0),
            diagram_margin_y: config_f64(effective_config, &["c4", "diagramMarginY"])
                .unwrap_or(10.0),
            c4_shape_margin: config_f64(effective_config, &["c4", "c4ShapeMargin"]).unwrap_or(50.0),
            c4_shape_padding: config_f64(effective_config, &["c4", "c4ShapePadding"])
                .unwrap_or(20.0),
            width: config_f64(effective_config, &["c4", "width"]).unwrap_or(216.0),
            height: config_f64(effective_config, &["c4", "height"]).unwrap_or(60.0),
            wrap: config_bool(effective_config, &["c4", "wrap"]).unwrap_or(true),
            next_line_padding_x: config_f64(effective_config, &["c4", "nextLinePaddingX"])
                .unwrap_or(0.0),
            boundary_font_family,
            boundary_font_size,
            boundary_font_weight,
            message_font_family,
            message_font_size,
            message_font_weight,
        }
    }

    fn boundary_font(&self) -> TextStyle {
        TextStyle {
            font_family: self.boundary_font_family.clone(),
            font_size: self.boundary_font_size,
            font_weight: self.boundary_font_weight.clone(),
        }
    }

    fn message_font(&self) -> TextStyle {
        TextStyle {
            font_family: self.message_font_family.clone(),
            font_size: self.message_font_size,
            font_weight: self.message_font_weight.clone(),
        }
    }

    fn c4_shape_font(&self, effective_config: &Value, type_c4_shape: &str) -> TextStyle {
        let global_font_family = config_string(effective_config, &["fontFamily"]);
        let global_font_size = config_f64(effective_config, &["fontSize"]);
        let global_font_weight = config_string(effective_config, &["fontWeight"]);

        let can_override = matches!(type_c4_shape, "person" | "system");

        let key_family = format!("{type_c4_shape}FontFamily");
        let key_size = format!("{type_c4_shape}FontSize");
        let key_weight = format!("{type_c4_shape}FontWeight");

        let font_family = (if can_override {
            global_font_family
        } else {
            None
        })
        .or_else(|| config_string(effective_config, &["c4", &key_family]));

        let font_size = (if can_override { global_font_size } else { None })
            .or_else(|| config_f64(effective_config, &["c4", &key_size]))
            .unwrap_or(14.0);

        let font_weight = (if can_override {
            global_font_weight
        } else {
            None
        })
        .or_else(|| config_string(effective_config, &["c4", &key_weight]));

        TextStyle {
            font_family,
            font_size,
            font_weight,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TextMeasure {
    width: f64,
    height: f64,
    line_count: usize,
}

fn measure_c4_text(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap: bool,
    text_limit_width: f64,
) -> TextMeasure {
    if wrap {
        let m = measurer.measure_wrapped(text, style, Some(text_limit_width), WrapMode::SvgLike);
        return TextMeasure {
            width: text_limit_width,
            height: m.height,
            line_count: m.line_count,
        };
    }

    let mut width: f64 = 0.0;
    let mut height: f64 = 0.0;
    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text);
    for line in &lines {
        let m = measurer.measure(line, style);
        width = width.max(m.width);
        height += m.height;
    }
    TextMeasure {
        width,
        height,
        line_count: lines.len().max(1),
    }
}

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
        let mut stopx = startx + rect.width;
        let starty = self.next.starty + rect.margin * 2.0;
        let mut stopy = starty + rect.height;

        if startx >= self.data.width_limit
            || stopx >= self.data.width_limit
            || self.next.cnt > c4_shape_in_row
        {
            let startx2 = self.next.startx + rect.margin + conf.next_line_padding_x;
            let starty2 = self.next.stopy + rect.margin * 2.0;

            stopx = startx2 + rect.width;
            stopy = starty2 + rect.height;

            self.next.stopx = stopx;
            self.next.starty = self.next.stopy;
            self.next.stopy = stopy;
            self.next.cnt = 1;

            rect.x = startx2;
            rect.y = starty2;
        } else {
            rect.x = startx;
            rect.y = starty;
        }

        Self::update_val_opt(&mut self.data.startx, rect.x, f64::min);
        Self::update_val_opt(&mut self.data.starty, rect.y, f64::min);
        Self::update_val_opt(&mut self.data.stopx, stopx, f64::max);
        Self::update_val_opt(&mut self.data.stopy, stopy, f64::max);

        Self::update_val(&mut self.next.startx, rect.x, f64::min);
        Self::update_val(&mut self.next.starty, rect.y, f64::min);
        Self::update_val(&mut self.next.stopx, stopx, f64::max);
        Self::update_val(&mut self.next.stopy, stopy, f64::max);
    }
}

#[derive(Debug, Clone)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    margin: f64,
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
    let x1 = from.x;
    let y1 = from.y;
    let x2 = end_point.x;
    let y2 = end_point.y;

    let from_center_x = x1 + from.width / 2.0;
    let from_center_y = y1 + from.height / 2.0;

    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let tan_dyx = dy / dx;
    let from_dyx = from.height / from.width;

    let mut return_point: Option<LayoutPoint> = None;

    if y1 == y2 && x1 < x2 {
        return_point = Some(LayoutPoint {
            x: x1 + from.width,
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
            y: y1 + from.height,
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
                y: from_center_y + (tan_dyx * from.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x - ((dx / dy) * from.height) / 2.0,
                y: y1 + from.height,
            });
        }
    } else if x1 < x2 && y1 < y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1 + from.width,
                y: from_center_y + (tan_dyx * from.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x + ((dx / dy) * from.height) / 2.0,
                y: y1 + from.height,
            });
        }
    } else if x1 < x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1 + from.width,
                y: from_center_y - (tan_dyx * from.width) / 2.0,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x + ((from.height / 2.0) * dx) / dy,
                y: y1,
            });
        }
    } else if x1 > x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            return_point = Some(LayoutPoint {
                x: x1,
                y: from_center_y - (from.width / 2.0) * tan_dyx,
            });
        } else {
            return_point = Some(LayoutPoint {
                x: from_center_x - ((from.height / 2.0) * dx) / dy,
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
        x: to.x + to.width / 2.0,
        y: to.y + to.height / 2.0,
    };
    let start_point = intersect_point(from, end_intersect_point);

    let end_intersect_point = LayoutPoint {
        x: from.x + from.width / 2.0,
        y: from.y + from.height / 2.0,
    };
    let end_point = intersect_point(to, end_intersect_point);

    (start_point, end_point)
}

fn layout_c4_shape_array(
    current_bounds: &mut BoundsState,
    shape_indices: &[usize],
    model: &C4Model,
    effective_config: &Value,
    conf: &C4Conf,
    c4_shape_in_row: usize,
    measurer: &dyn TextMeasurer,
    out_shapes: &mut HashMap<String, C4ShapeLayout>,
) {
    for idx in shape_indices {
        let shape = &model.shapes[*idx];
        let mut y = conf.c4_shape_padding;

        let type_c4_shape = shape.type_c4_shape.as_str().to_string();
        let mut type_conf = conf.c4_shape_font(effective_config, &type_c4_shape);
        type_conf.font_size -= 2.0;

        let type_text = format!("«{}»", type_c4_shape);
        let type_metrics = measurer.measure(&type_text, &type_conf);
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

        let text_wrap = shape.wrap && conf.wrap;
        let text_limit_width = conf.width - conf.c4_shape_padding * 2.0;

        let mut label_conf = conf.c4_shape_font(effective_config, &type_c4_shape);
        label_conf.font_size += 2.0;
        label_conf.font_weight = Some("bold".to_string());

        let label_text = shape.label.as_str().to_string();
        let label_m = measure_c4_text(
            measurer,
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
            let type_conf = conf.c4_shape_font(effective_config, &type_c4_shape);
            let m = measure_c4_text(
                measurer,
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
            let techn_conf = conf.c4_shape_font(effective_config, &techn_text);
            let m = measure_c4_text(
                measurer,
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
            let descr_conf = conf.c4_shape_font(effective_config, &type_c4_shape);
            let m = measure_c4_text(
                measurer,
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

        rect_width += conf.c4_shape_padding;

        let width = conf.width.max(rect_width);
        let height = conf.height.max(rect_height);
        let margin = conf.c4_shape_margin;

        let mut rect = Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
            margin,
        };
        current_bounds.insert_rect(&mut rect, c4_shape_in_row, conf);

        out_shapes.insert(
            shape.alias.clone(),
            C4ShapeLayout {
                alias: shape.alias.clone(),
                parent_boundary: shape.parent_boundary.clone(),
                type_c4_shape: type_c4_shape.clone(),
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
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

    current_bounds.bump_last_margin(conf.c4_shape_margin);
}

fn layout_inside_boundary(
    parent_bounds: &mut BoundsState,
    boundary_indices: &[usize],
    model: &C4Model,
    effective_config: &Value,
    conf: &C4Conf,
    c4_shape_in_row: usize,
    c4_boundary_in_row: usize,
    measurer: &dyn TextMeasurer,
    boundary_children: &HashMap<String, Vec<usize>>,
    shape_children: &HashMap<String, Vec<usize>>,
    out_boundaries: &mut HashMap<String, C4BoundaryLayout>,
    out_shapes: &mut HashMap<String, C4ShapeLayout>,
    global_max_x: &mut f64,
    global_max_y: &mut f64,
) -> Result<()> {
    let mut current_bounds = BoundsState::default();

    let denom = c4_boundary_in_row.min(boundary_indices.len().max(1));
    let width_limit = parent_bounds.data.width_limit / denom as f64;
    current_bounds.data.width_limit = width_limit;

    for (i, idx) in boundary_indices.iter().enumerate() {
        let boundary = &model.boundaries[*idx];
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

        let text_wrap = boundary.wrap.unwrap_or(model.wrap) && conf.wrap;
        let mut label_conf = conf.boundary_font();
        label_conf.font_size += 2.0;
        label_conf.font_weight = Some("bold".to_string());

        let label_text = boundary.label.as_str().to_string();
        let label_m = measure_c4_text(measurer, &label_text, &label_conf, text_wrap, width_limit);
        let label = C4TextBlockLayout {
            text: label_text,
            y: y + 8.0,
            width: label_m.width,
            height: label_m.height,
            line_count: label_m.line_count,
        };
        y = label.y + label.height;

        let mut ty_block: Option<C4TextBlockLayout> = None;
        if let Some(ty) = boundary.ty.as_ref().filter(|t| !t.as_str().is_empty()) {
            let ty_text = format!("[{}]", ty.as_str());
            let ty_conf = conf.boundary_font();
            let m = measure_c4_text(measurer, &ty_text, &ty_conf, text_wrap, width_limit);
            let block = C4TextBlockLayout {
                text: ty_text,
                y: y + 5.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
            };
            y = block.y + block.height;
            ty_block = Some(block);
        }

        let mut descr_block: Option<C4TextBlockLayout> = None;
        if let Some(descr) = boundary.descr.as_ref().filter(|t| !t.as_str().is_empty()) {
            let descr_text = descr.as_str().to_string();
            let mut descr_conf = conf.boundary_font();
            descr_conf.font_size -= 2.0;
            let m = measure_c4_text(measurer, &descr_text, &descr_conf, text_wrap, width_limit);
            let block = C4TextBlockLayout {
                text: descr_text,
                y: y + 20.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
            };
            y = block.y + block.height;
            descr_block = Some(block);
        }

        let parent_startx = parent_bounds
            .data
            .startx
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing startx".to_string(),
            })?;
        let parent_stopy = parent_bounds
            .data
            .stopy
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing stopy".to_string(),
            })?;

        if i == 0 || i % c4_boundary_in_row == 0 {
            let x = parent_startx + conf.diagram_margin_x;
            let y0 = parent_stopy + conf.diagram_margin_y + y;
            current_bounds.set_data(x, x, y0, y0);
        } else {
            let startx = current_bounds.data.startx.unwrap_or(parent_startx);
            let stopx = current_bounds.data.stopx.unwrap_or(startx);
            let x = if stopx != startx {
                stopx + conf.diagram_margin_x
            } else {
                startx
            };
            let y0 = current_bounds.data.starty.unwrap_or(parent_stopy);
            current_bounds.set_data(x, x, y0, y0);
        }

        if let Some(shape_indices) = shape_children.get(&boundary.alias) {
            if !shape_indices.is_empty() {
                layout_c4_shape_array(
                    &mut current_bounds,
                    shape_indices,
                    model,
                    effective_config,
                    conf,
                    c4_shape_in_row,
                    measurer,
                    out_shapes,
                );
            }
        }

        if let Some(next_boundaries) = boundary_children.get(&boundary.alias) {
            if !next_boundaries.is_empty() {
                layout_inside_boundary(
                    &mut current_bounds,
                    next_boundaries,
                    model,
                    effective_config,
                    conf,
                    c4_shape_in_row,
                    c4_boundary_in_row,
                    measurer,
                    boundary_children,
                    shape_children,
                    out_boundaries,
                    out_shapes,
                    global_max_x,
                    global_max_y,
                )?;
            }
        }

        let startx = current_bounds.data.startx.unwrap_or(0.0);
        let stopx = current_bounds.data.stopx.unwrap_or(startx);
        let starty = current_bounds.data.starty.unwrap_or(0.0);
        let stopy = current_bounds.data.stopy.unwrap_or(starty);

        out_boundaries.insert(
            boundary.alias.clone(),
            C4BoundaryLayout {
                alias: boundary.alias.clone(),
                parent_boundary: boundary.parent_boundary.clone(),
                x: startx,
                y: starty,
                width: stopx - startx,
                height: stopy - starty,
                image,
                label,
                ty: ty_block,
                descr: descr_block,
            },
        );

        let stopx_with_margin = stopx + conf.c4_shape_margin;
        let stopy_with_margin = stopy + conf.c4_shape_margin;
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

        *global_max_x = global_max_x.max(parent_bounds.data.stopx.unwrap_or(*global_max_x));
        *global_max_y = global_max_y.max(parent_bounds.data.stopy.unwrap_or(*global_max_y));
    }

    Ok(())
}

pub(crate) fn layout_c4_diagram(
    model: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    viewport_width: f64,
    viewport_height: f64,
) -> Result<C4DiagramLayout> {
    let model: C4Model = serde_json::from_value(model.clone())?;
    let conf = C4Conf::from_effective_config(effective_config);

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

    let mut out_boundaries: HashMap<String, C4BoundaryLayout> = HashMap::new();
    let mut out_shapes: HashMap<String, C4ShapeLayout> = HashMap::new();

    let mut screen_bounds = BoundsState::default();
    screen_bounds.set_data(
        conf.diagram_margin_x,
        conf.diagram_margin_x,
        conf.diagram_margin_y,
        conf.diagram_margin_y,
    );
    screen_bounds.data.width_limit = viewport_width;

    let mut global_max_x = conf.diagram_margin_x;
    let mut global_max_y = conf.diagram_margin_y;

    let root_boundaries = boundary_children.get("").cloned().unwrap_or_default();
    if root_boundaries.is_empty() {
        return Err(Error::InvalidModel {
            message: "c4: expected at least the implicit global boundary".to_string(),
        });
    }

    layout_inside_boundary(
        &mut screen_bounds,
        &root_boundaries,
        &model,
        effective_config,
        &conf,
        c4_shape_in_row,
        c4_boundary_in_row,
        measurer,
        &boundary_children,
        &shape_children,
        &mut out_boundaries,
        &mut out_shapes,
        &mut global_max_x,
        &mut global_max_y,
    )?;

    screen_bounds.data.stopx = Some(global_max_x);
    screen_bounds.data.stopy = Some(global_max_y);

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
        let Some(l) = out_shapes.get(&s.alias) else {
            continue;
        };
        shape_rects.insert(
            s.alias.as_str(),
            Rect {
                x: l.x,
                y: l.y,
                width: l.width,
                height: l.height,
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
        let Some(l) = out_boundaries.get(&b.alias) else {
            return Err(Error::InvalidModel {
                message: format!("c4: missing boundary layout for {}", b.alias),
            });
        };
        boundaries_out.push(l.clone());
    }

    let mut shapes_out = Vec::with_capacity(model.shapes.len());
    for s in &model.shapes {
        let Some(l) = out_shapes.get(&s.alias) else {
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
        c4_type: model.c4_type,
        title: model.title,
        boundaries: boundaries_out,
        shapes: shapes_out,
        rels: rels_out,
    })
}
