// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Kanban HTML-label layout/render constants in one
// place, so diagram-specific foreignObject/row-height numbers do not stay duplicated inline.

pub fn kanban_section_label_height_baseline_px() -> f64 {
    25.0
}

pub fn kanban_section_padding_px() -> f64 {
    10.0
}

pub fn kanban_label_foreign_object_height_px() -> f64 {
    24.0
}

pub fn kanban_item_one_row_height_px() -> f64 {
    44.0
}

pub fn kanban_item_label_inset_x_px() -> f64 {
    10.0
}

pub fn kanban_item_two_row_height_px() -> f64 {
    56.0
}

pub fn kanban_item_label_line_height_px() -> f64 {
    24.0
}
