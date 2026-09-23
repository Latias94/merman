use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};

use super::{C6ProofError, C6ProofResult, C6TargetArtifact};

pub(crate) fn sealed_svg_receipt<'a>(
    artifact: &'a C6TargetArtifact<'a>,
) -> C6ProofResult<&'a SvgArtifactReceipt> {
    let receipt = artifact.svg_artifact_receipt().ok_or_else(|| {
        C6ProofError::new(
            "renderer-svg-observation",
            "renderer did not seal a generic SVG artifact observation",
        )
    })?;
    c6_ensure!(
        "renderer-svg-observation",
        receipt.proves_artifact(artifact.receipt().artifact_digest()),
        "generic SVG artifact observation is not bound to the target admission artifact"
    );
    Ok(receipt)
}

pub(crate) fn has_ancestor_class(
    receipt: &SvgArtifactReceipt,
    element: &SvgElementObservation,
    class_name: &str,
) -> bool {
    let mut current = element.parent_index();
    while let Some(index) = current {
        let Some(ancestor) = receipt.element(index) else {
            return false;
        };
        if ancestor.has_class(class_name) {
            return true;
        }
        current = ancestor.parent_index();
    }
    false
}

pub(crate) fn subtree_style_value<'a>(
    receipt: &'a SvgArtifactReceipt,
    element: &'a SvgElementObservation,
    property: &str,
) -> Option<&'a str> {
    std::iter::once(element)
        .chain(receipt.descendants_of(element.index()))
        .find_map(|candidate| candidate.style_value(property))
}

pub(crate) fn descendant_text(element: &SvgElementObservation) -> &str {
    element.text()
}

pub(crate) fn local_fragment_id(value: &str) -> Option<&str> {
    value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
}

pub(crate) fn percent_value(value: &str) -> Option<f64> {
    value
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}
