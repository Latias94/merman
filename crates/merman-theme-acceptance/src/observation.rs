//! Artifact observations shared by the host-dependent preset qualification checks.

use merman::__theme_acceptance::TargetArtifactView;
use merman::TargetAdmissionReceipt;

pub(crate) struct C6TargetArtifact<'a> {
    view: TargetArtifactView<'a>,
}

impl<'a> C6TargetArtifact<'a> {
    pub(crate) const fn new(view: TargetArtifactView<'a>) -> Self {
        Self { view }
    }

    pub(crate) const fn receipt(&self) -> &TargetAdmissionReceipt {
        self.view.receipt()
    }

    pub(crate) const fn svg_artifact_receipt(
        &self,
    ) -> Option<&merman_render::__private::SvgArtifactReceipt> {
        self.view.svg_artifact_receipt()
    }

    pub(crate) fn bytes(&self) -> &'a [u8] {
        self.view.bytes()
    }
}
