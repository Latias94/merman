use merman_model::families::flowchart::{FlowNode, FlowchartModel};
use merman_render_core::{Error, Result};

use crate::shapes::FlowchartShape;

pub fn resolve_flowchart_shape(name: &str) -> Result<FlowchartShape> {
    FlowchartShape::resolve(name).ok_or_else(|| Error::InvalidModel {
        message: format!("No such shape: {name}. Please check your syntax."),
    })
}

pub fn validate_flowchart_model_shapes(model: &FlowchartModel) -> Result<()> {
    for node in &model.nodes {
        resolve_flowchart_shape(node.layout_shape.as_deref().unwrap_or("squareRect"))?;
    }
    Ok(())
}

pub fn validate_flowchart_node_asset_dimensions(node: &FlowNode) -> Result<()> {
    let shape = resolve_flowchart_shape(node.layout_shape.as_deref().unwrap_or("squareRect"))?;
    if let Some(name) =
        shape.non_finite_asset_dimension(node.img.as_deref(), node.asset_width, node.asset_height)
    {
        return Err(Error::InvalidModel {
            message: format!("Flowchart node {} has non-finite asset {name}", node.id),
        });
    }
    Ok(())
}
