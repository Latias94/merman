use super::super::{SvgOutput, escape_attr};
use crate::c4::C4ShoalRelation;
use crate::{Error, Result};
use base64::Engine as _;

pub(super) fn open_shaft(
    out: &mut impl SvgOutput,
    route: &C4ShoalRelation,
    stroke: &str,
) -> Result<()> {
    let points = serde_json::to_vec(&route.points).map_err(|error| Error::InvalidModel {
        message: format!("c4: failed to encode canonical relation points: {error}"),
    })?;
    let points = base64::engine::general_purpose::STANDARD.encode(points);
    let _ = write!(
        out,
        r#"<path data-et="edge" data-id="{}" data-points="{points}" fill="none" stroke-width="1" stroke="{}" d="{}""#,
        escape_attr(&route.id),
        escape_attr(stroke),
        escape_attr(&route.path),
    );
    out.checkpoint()
}
