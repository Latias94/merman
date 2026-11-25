use crate::error::WorkFailure;

pub(super) fn reset_dense_cells(
    cells: &mut Vec<Vec<usize>>,
    cell_count: usize,
) -> std::result::Result<(), WorkFailure> {
    if cell_count > cells.len() {
        cells
            .try_reserve_exact(cell_count - cells.len())
            .map_err(|_| WorkFailure::ArithmeticOverflow)?;
        cells.resize_with(cell_count, Vec::new);
    } else {
        cells.truncate(cell_count);
    }
    clear_dense_cells(cells);
    Ok(())
}

pub(super) fn clear_dense_cells(cells: &mut [Vec<usize>]) {
    for cell in cells {
        cell.clear();
    }
}
