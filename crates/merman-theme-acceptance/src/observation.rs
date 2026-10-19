//! Artifact observations shared by the host-dependent preset qualification checks.

use crate::svg_observation::SvgArtifactObservation;
use merman::__theme_acceptance::TargetArtifactView;
use merman::{RenderArtifactKind, TargetAdmissionReceipt};

pub(crate) struct C6TargetArtifact<'a> {
    view: TargetArtifactView<'a>,
    svg: Option<SvgArtifactObservation>,
}

impl<'a> C6TargetArtifact<'a> {
    pub(crate) fn new(view: TargetArtifactView<'a>) -> Self {
        let svg = (view.receipt().artifact_kind() == RenderArtifactKind::Svg)
            .then(|| {
                std::str::from_utf8(view.bytes())
                    .ok()
                    .and_then(SvgArtifactObservation::observe)
            })
            .flatten();
        Self { view, svg }
    }

    pub(crate) const fn receipt(&self) -> &TargetAdmissionReceipt {
        self.view.receipt()
    }

    pub(crate) fn svg_observation(&self) -> Option<&SvgArtifactObservation> {
        self.svg.as_ref()
    }

    pub(crate) fn bytes(&self) -> &'a [u8] {
        self.view.bytes()
    }
}
