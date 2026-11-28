//! Grid option projection from Mermaid 12.4.0 `readGridConfig` and `normalizeLineHops`.

use super::geometry::{GridSpacing, reserve};
use super::placement::{
    GridCoordinate, GridHorizontalAlign, GridPlacementOptions, GridPlacementSpec, GridVerticalAlign,
};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;

const CURVES: &[&str] = &[
    "basis",
    "bumpX",
    "bumpY",
    "cardinal",
    "catmullRom",
    "linear",
    "monotoneX",
    "monotoneY",
    "natural",
    "step",
    "stepAfter",
    "stepBefore",
    "rounded",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GridLineHops {
    Disabled,
    #[default]
    Arc,
    Gap,
}

/// Normalized Grid drawing options shared by layout and edge paint.
#[derive(Debug, Clone, Copy)]
pub struct GridPaint {
    pub curve: &'static str,
    pub corner_radius: f64,
    pub line_hops: GridLineHops,
}

#[derive(Debug)]
pub(super) struct GridSettings<'a> {
    pub occurrences: HashMap<&'a str, Vec<GridPlacementSpec>>,
    pub placement: GridPlacementOptions,
    pub spacing: GridSpacing,
    pub paint: GridPaint,
}

impl<'a> GridSettings<'a> {
    pub(super) fn read(section: Option<&'a Value>, work: &OperationWorkMeter) -> Result<Self> {
        let section = section.and_then(Value::as_object);
        let get = |key: &str| section.and_then(|map| map.get(key));
        let nonnegative = |key, fallback| {
            get(key)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0)
                .unwrap_or(fallback)
        };
        let mut occurrences = HashMap::new();
        if let Some(placements) = get("placements").and_then(Value::as_object) {
            work.charge(placements.len())?;
            occurrences
                .try_reserve(placements.len())
                .map_err(|_| Error::LayoutAllocationFailed { backend: "grid" })?;
            for (id, entry) in placements {
                work.charge(id.len().max(1))?;
                let items = entry
                    .as_array()
                    .map_or(std::slice::from_ref(entry), Vec::as_slice);
                work.charge(items.len())?;
                let mut slots = Vec::new();
                reserve(&mut slots, items.len())?;
                for item in items {
                    work.charge(1)?;
                    slots.push(placement_spec(item.as_object(), id)?);
                }
                occurrences.insert(id.as_str(), slots);
            }
        }
        Ok(Self {
            occurrences,
            placement: GridPlacementOptions {
                // Saturating positive widths cannot affect cell choice once the track count has
                // exceeded both the supported graph size and JavaScript's safe coordinate range.
                columns: nonnegative("columns", 0.0) as u64,
                horizontal_align: horizontal(get("horizontalAlign")).unwrap_or_default(),
                vertical_align: vertical(get("verticalAlign")).unwrap_or_default(),
            },
            spacing: GridSpacing {
                row_gap: nonnegative("rowGap", 50.0),
                column_gap: nonnegative("columnGap", 50.0),
                cell_gap: nonnegative("cellGap", 20.0),
                container_padding: nonnegative("containerPadding", 20.0),
                title_gap: nonnegative("titleGap", 8.0),
            },
            paint: GridPaint {
                curve: get("curve")
                    .and_then(Value::as_str)
                    .and_then(|value| CURVES.iter().copied().find(|&curve| curve == value))
                    .unwrap_or("rounded"),
                corner_radius: nonnegative("edgeCornerRadius", 5.0),
                line_hops: match get("lineHops") {
                    Some(Value::Bool(false)) => GridLineHops::Disabled,
                    Some(Value::String(value)) if value == "gap" => GridLineHops::Gap,
                    _ => GridLineHops::Arc,
                },
            },
        })
    }
}

fn placement_spec(placement: Option<&Map<String, Value>>, id: &str) -> Result<GridPlacementSpec> {
    let get = |key: &str| placement.and_then(|placement| placement.get(key));
    let coordinate = |field| {
        let Some(value) = get(field) else {
            return Ok(None);
        };
        value.as_f64().filter(|v| v.fract() == 0.0).and_then(|value| GridCoordinate::new(value as i64)).map(Some)
            .ok_or_else(|| Error::InvalidModel { message: format!("GRID_INVALID_COORDINATE: Invalid {field} for placement \"{id}\": expected a safe integer.") })
    };
    Ok(GridPlacementSpec {
        row: coordinate("row")?,
        column: coordinate("column")?,
        horizontal_align: horizontal(get("horizontalAlign")),
        vertical_align: vertical(get("verticalAlign")),
    })
}

fn horizontal(value: Option<&Value>) -> Option<GridHorizontalAlign> {
    match value.and_then(Value::as_str) {
        Some("left") => Some(GridHorizontalAlign::Left),
        Some("center") => Some(GridHorizontalAlign::Center),
        Some("right") => Some(GridHorizontalAlign::Right),
        _ => None,
    }
}

fn vertical(value: Option<&Value>) -> Option<GridVerticalAlign> {
    match value.and_then(Value::as_str) {
        Some("top") => Some(GridVerticalAlign::Top),
        Some("center") => Some(GridVerticalAlign::Center),
        Some("bottom") => Some(GridVerticalAlign::Bottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

    #[test]
    fn coordinates_keep_slots_and_visual_settings_follow_grid_defaults() {
        let input = json!({
            "placements": {"a": [null, {"row": -23, "column": 0, "horizontalAlign": "left", "verticalAlign": "bottom"}, false], "b": {"row": 4}},
            "columns": 3.75, "rowGap": 0, "columnGap": -1, "cellGap": "0", "containerPadding": 10.5,
            "titleGap": false, "horizontalAlign": "right", "verticalAlign": "invalid",
            "curve": "stepAfter", "edgeCornerRadius": 0.0, "lineHops": "gap",
        });
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let settings = GridSettings::read(Some(&input), &work).unwrap();
        let slots = &settings.occurrences["a"];
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0], GridPlacementSpec::default());
        assert_eq!(
            slots[1],
            GridPlacementSpec {
                row: GridCoordinate::new(-23),
                column: GridCoordinate::new(0),
                horizontal_align: Some(GridHorizontalAlign::Left),
                vertical_align: Some(GridVerticalAlign::Bottom)
            }
        );
        assert_eq!(slots[2], GridPlacementSpec::default());
        assert_eq!(settings.occurrences["b"][0].row, GridCoordinate::new(4));
        assert_eq!(settings.placement.columns, 3);
        assert_eq!(
            settings.placement.horizontal_align,
            GridHorizontalAlign::Right
        );
        assert_eq!(settings.placement.vertical_align, GridVerticalAlign::Center);
        assert_eq!(settings.spacing.row_gap, 0.0);
        assert_eq!(settings.spacing.column_gap, 50.0);
        assert_eq!(settings.spacing.cell_gap, 20.0);
        assert_eq!(settings.spacing.container_padding, 10.5);
        assert_eq!(settings.spacing.title_gap, 8.0);
        assert_eq!(settings.paint.curve, "stepAfter");
        assert_eq!(settings.paint.corner_radius, 0.0);
        assert_eq!(settings.paint.line_hops, GridLineHops::Gap);
    }

    #[test]
    fn unknown_style_values_default_and_invalid_coordinates_fail() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let input = json!({"curve": "Linear", "edgeCornerRadius": -1, "lineHops": false});
        let settings = GridSettings::read(Some(&input), &work).unwrap();
        assert_eq!(settings.paint.curve, "rounded");
        assert_eq!(settings.paint.corner_radius, 5.0);
        assert_eq!(settings.paint.line_hops, GridLineHops::Disabled);
        let fallback = GridSettings::read(None, &work).unwrap();
        assert_eq!(fallback.paint.line_hops, GridLineHops::Arc);
        for row in [
            json!(null),
            json!("1"),
            json!(1.25),
            json!(9007199254740992_i64),
        ] {
            let input = json!({"placements": {"a": {"row": row}}});
            assert!(
                GridSettings::read(Some(&input), &work)
                    .unwrap_err()
                    .to_string()
                    .contains("GRID_INVALID_COORDINATE")
            );
        }
    }
}
