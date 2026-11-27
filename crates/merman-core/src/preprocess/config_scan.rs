//! Controlled depth scans for source configuration.

use crate::{OperationControl, OperationControlResult};
pub(super) use merman_core_preprocess::scan::config::{
    max_flow_collection_depth_controlled, max_yaml_indent_depth_controlled,
};
use merman_core_preprocess::scan::controlled::ControlledScanCheckpoints;

pub(super) const MAX_CONFIG_NESTING_DEPTH: usize = crate::MAX_DIAGRAM_NESTING_DEPTH;

#[cfg(test)]
pub(super) fn config_nesting_exceeds_limit(text: &str) -> bool {
    let control = OperationControl::new();
    config_nesting_exceeds_limit_controlled(text, &control)
        .expect("a private parse control cannot be cancelled")
}

pub(super) fn config_nesting_exceeds_limit_controlled(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    if max_flow_collection_depth_controlled::<false>(text, &mut checkpoints)?
        > MAX_CONFIG_NESTING_DEPTH
    {
        checkpoints.finish()?;
        return Ok(true);
    }
    let exceeds =
        max_yaml_indent_depth_controlled(text, &mut checkpoints)? > MAX_CONFIG_NESTING_DEPTH;
    checkpoints.finish()?;
    Ok(exceeds)
}
