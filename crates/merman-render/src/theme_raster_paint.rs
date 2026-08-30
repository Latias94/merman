use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::DiagramFamilyId;
use crate::diagram_theme::ThemeTarget;
use crate::theme_route_cutover::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverSelector,
};

/// Native paint channel used by one renderer-owned raster proof terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRasterPaintBinding {
    Native,
    FillFromStroke,
    FillAndStrokeFromStroke,
}

impl ThemeRasterPaintBinding {
    const fn id(self) -> &'static [u8] {
        match self {
            Self::Native => b"native",
            Self::FillFromStroke => b"fill-from-stroke",
            Self::FillAndStrokeFromStroke => b"fill-and-stroke-from-stroke",
        }
    }

    fn admits(self, facet: ThemeRouteCutoverFacet) -> bool {
        match (self, facet) {
            (Self::FillFromStroke, ThemeRouteCutoverFacet::Fill)
            | (Self::Native, _)
            | (Self::FillAndStrokeFromStroke, _) => true,
            (Self::FillFromStroke, ThemeRouteCutoverFacet::Stroke) => false,
        }
    }
}

/// Exact finalized SVG terminal selected by one renderer writer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRasterPaintTerminal {
    terminal_id: String,
    binding: ThemeRasterPaintBinding,
    semantic: ThemeRasterPaintTerminalSemantic,
}

impl ThemeRasterPaintTerminal {
    pub(crate) fn sequence_lifeline(
        terminal_id: impl Into<String>,
        binding: ThemeRasterPaintBinding,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke_width: f64,
    ) -> Option<Self> {
        let terminal_id = terminal_id.into();
        let geometry = ThemeRasterPaintLineGeometry::new(x1, y1, x2, y2, stroke_width)?;
        (!terminal_id.is_empty()).then_some(Self {
            terminal_id,
            binding,
            semantic: ThemeRasterPaintTerminalSemantic::SequenceLifeline { geometry },
        })
    }

    pub fn terminal_id(&self) -> &str {
        &self.terminal_id
    }

    pub const fn binding(&self) -> ThemeRasterPaintBinding {
        self.binding
    }

    pub const fn semantic(&self) -> ThemeRasterPaintTerminalSemantic {
        self.semantic
    }
}

/// Renderer-owned semantic role and layout geometry for a raster proof terminal.
///
/// The exporter validates this fact against the finalized SVG independently of the renderer's
/// receipt. Keeping the role narrow avoids turning the raster proof into a second SVG model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRasterPaintTerminalSemantic {
    SequenceLifeline {
        geometry: ThemeRasterPaintLineGeometry,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRasterPaintLineGeometry {
    x1_bits: u64,
    y1_bits: u64,
    x2_bits: u64,
    y2_bits: u64,
    stroke_width_bits: u64,
}

impl ThemeRasterPaintLineGeometry {
    fn new(x1: f64, y1: f64, x2: f64, y2: f64, stroke_width: f64) -> Option<Self> {
        (x1.is_finite()
            && y1.is_finite()
            && x2.is_finite()
            && y2.is_finite()
            && stroke_width.is_finite()
            && x1 == x2
            && y2 > y1
            && stroke_width > 0.0)
            .then_some(Self {
                x1_bits: x1.to_bits(),
                y1_bits: y1.to_bits(),
                x2_bits: x2.to_bits(),
                y2_bits: y2.to_bits(),
                stroke_width_bits: stroke_width.to_bits(),
            })
    }

    pub const fn x1(self) -> f64 {
        f64::from_bits(self.x1_bits)
    }

    pub const fn y1(self) -> f64 {
        f64::from_bits(self.y1_bits)
    }

    pub const fn x2(self) -> f64 {
        f64::from_bits(self.x2_bits)
    }

    pub const fn y2(self) -> f64 {
        f64::from_bits(self.y2_bits)
    }

    pub const fn stroke_width(self) -> f64 {
        f64::from_bits(self.stroke_width_bits)
    }
}

/// Writer-owned terminal selection before the finalized native SVG digest is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeRasterPaintBindingFact {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    terminals: Box<[ThemeRasterPaintTerminal]>,
}

impl ThemeRasterPaintBindingFact {
    pub(crate) fn new(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        selector: ThemeRouteCutoverSelector,
        terminals: impl IntoIterator<Item = ThemeRasterPaintTerminal>,
    ) -> Option<Self> {
        let mut canonical = BTreeMap::<String, ThemeRasterPaintTerminal>::new();
        for terminal in terminals {
            if terminal.terminal_id.is_empty() {
                return None;
            }
            match canonical.entry(terminal.terminal_id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(terminal);
                }
                std::collections::btree_map::Entry::Occupied(_) => return None,
            }
        }
        (!canonical.is_empty()).then(|| Self {
            family_id,
            target,
            selector,
            terminals: canonical
                .into_iter()
                .map(|(_, terminal)| terminal)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    pub(crate) const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    fn seal(&self, native_artifact_digest: [u8; 32]) -> ThemeRasterPaintBindingReceipt {
        let digest = binding_receipt_digest(
            self.family_id,
            self.target,
            self.selector,
            &self.terminals,
            native_artifact_digest,
        );
        ThemeRasterPaintBindingReceipt {
            family_id: self.family_id,
            target: self.target,
            selector: self.selector,
            terminals: self.terminals.clone(),
            native_artifact_digest,
            digest,
        }
    }
}

/// Opaque renderer receipt binding exact paint terminals to the finalized native SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRasterPaintBindingReceipt {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    terminals: Box<[ThemeRasterPaintTerminal]>,
    native_artifact_digest: [u8; 32],
    digest: [u8; 32],
}

impl ThemeRasterPaintBindingReceipt {
    /// Returns whether this receipt belongs to the descriptor's route identity, independent of
    /// the requested paint facet and value.
    pub fn matches_route_identity(&self, descriptor: ThemeRouteCutoverDescriptor) -> bool {
        self.family_id == descriptor.family_id()
            && self.target == descriptor.target()
            && self.selector == descriptor.selector()
    }

    pub fn proves_route(
        &self,
        descriptor: ThemeRouteCutoverDescriptor,
        native_artifact_digest: [u8; 32],
    ) -> bool {
        self.matches_route_identity(descriptor)
            && self
                .terminals
                .iter()
                .all(|terminal| terminal.binding.admits(descriptor.facet()))
            && native_artifact_digest != [0; 32]
            && self.native_artifact_digest == native_artifact_digest
            && self.digest
                == binding_receipt_digest(
                    self.family_id,
                    self.target,
                    self.selector,
                    &self.terminals,
                    self.native_artifact_digest,
                )
    }

    pub fn terminals(&self) -> &[ThemeRasterPaintTerminal] {
        &self.terminals
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

pub(crate) fn seal_theme_raster_paint_binding_receipts(
    facts: &[ThemeRasterPaintBindingFact],
    native_artifact_digest: [u8; 32],
) -> Vec<ThemeRasterPaintBindingReceipt> {
    facts
        .iter()
        .map(|fact| fact.seal(native_artifact_digest))
        .collect()
}

fn binding_receipt_digest(
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    terminals: &[ThemeRasterPaintTerminal],
    native_artifact_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.theme-raster-paint-binding-receipt.v2");
    update_len_prefixed(&mut hasher, family_id.as_str().as_bytes());
    update_len_prefixed(&mut hasher, target.id().as_bytes());
    update_len_prefixed(&mut hasher, selector.id().as_bytes());
    update_usize(&mut hasher, terminals.len());
    for terminal in terminals {
        update_len_prefixed(&mut hasher, terminal.terminal_id.as_bytes());
        update_len_prefixed(&mut hasher, terminal.binding.id());
        match terminal.semantic {
            ThemeRasterPaintTerminalSemantic::SequenceLifeline { geometry } => {
                update_len_prefixed(&mut hasher, b"sequence-lifeline");
                hasher.update(geometry.x1_bits.to_be_bytes());
                hasher.update(geometry.y1_bits.to_be_bytes());
                hasher.update(geometry.x2_bits.to_be_bytes());
                hasher.update(geometry.y2_bits.to_be_bytes());
                hasher.update(geometry.stroke_width_bits.to_be_bytes());
            }
        }
    }
    hasher.update(native_artifact_digest);
    hasher.finalize().into()
}

fn update_usize(hasher: &mut Sha256, value: usize) {
    hasher.update(u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes());
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    update_usize(hasher, bytes.len());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_route_cutover::{
        ThemeRouteCutoverId, ThemeRouteCutoverProjectionSet, ThemeRouteCutoverValue,
    };

    fn descriptor(facet: ThemeRouteCutoverFacet) -> ThemeRouteCutoverDescriptor {
        ThemeRouteCutoverDescriptor::new(
            ThemeRouteCutoverId::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Lifeline,
                ThemeRouteCutoverSelector::StaticUnqualified,
                facet,
                ThemeRouteCutoverValue::Solid,
            ),
            ThemeRouteCutoverProjectionSet::REPLACE_LIFELINE_STROKE,
        )
    }

    fn lifeline_terminal(
        terminal_id: &str,
        binding: ThemeRasterPaintBinding,
    ) -> ThemeRasterPaintTerminal {
        ThemeRasterPaintTerminal::sequence_lifeline(
            terminal_id,
            binding,
            10.0,
            2.0,
            10.0,
            18.0,
            0.5,
        )
        .expect("valid Sequence lifeline terminal")
    }

    #[test]
    fn receipt_binds_sorted_terminal_ids_and_native_svg() {
        let fact = ThemeRasterPaintBindingFact::new(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            ThemeRouteCutoverSelector::StaticUnqualified,
            [
                lifeline_terminal("actor1", ThemeRasterPaintBinding::FillAndStrokeFromStroke),
                lifeline_terminal("actor0", ThemeRasterPaintBinding::FillAndStrokeFromStroke),
            ],
        )
        .expect("valid writer terminals");
        let native_digest = [7; 32];
        let receipt = fact.seal(native_digest);

        assert_eq!(receipt.terminals()[0].terminal_id(), "actor0");
        assert!(receipt.proves_route(descriptor(ThemeRouteCutoverFacet::Fill), native_digest));
        assert!(receipt.proves_route(descriptor(ThemeRouteCutoverFacet::Stroke), native_digest));
        assert!(!receipt.proves_route(descriptor(ThemeRouteCutoverFacet::Stroke), [8; 32]));
    }

    #[test]
    fn conflicting_duplicate_terminal_is_rejected() {
        assert!(
            ThemeRasterPaintBindingFact::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Lifeline,
                ThemeRouteCutoverSelector::StaticUnqualified,
                [
                    lifeline_terminal("actor0", ThemeRasterPaintBinding::FillFromStroke),
                    lifeline_terminal("actor0", ThemeRasterPaintBinding::Native),
                ],
            )
            .is_none()
        );
    }

    #[test]
    fn non_vertical_or_degenerate_lifeline_geometry_is_rejected() {
        assert!(
            ThemeRasterPaintTerminal::sequence_lifeline(
                "actor0",
                ThemeRasterPaintBinding::FillAndStrokeFromStroke,
                10.0,
                2.0,
                11.0,
                18.0,
                0.5,
            )
            .is_none()
        );
        assert!(
            ThemeRasterPaintTerminal::sequence_lifeline(
                "actor0",
                ThemeRasterPaintBinding::FillAndStrokeFromStroke,
                10.0,
                18.0,
                10.0,
                2.0,
                0.5,
            )
            .is_none()
        );
    }

    #[test]
    fn repeated_terminal_is_rejected_even_when_the_binding_matches() {
        assert!(
            ThemeRasterPaintBindingFact::new(
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Lifeline,
                ThemeRouteCutoverSelector::StaticUnqualified,
                [
                    lifeline_terminal("actor0", ThemeRasterPaintBinding::FillAndStrokeFromStroke),
                    lifeline_terminal("actor0", ThemeRasterPaintBinding::FillAndStrokeFromStroke),
                ],
            )
            .is_none()
        );
    }

    #[test]
    fn zero_native_digest_retains_identity_but_does_not_prove_route() {
        let fact = ThemeRasterPaintBindingFact::new(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            ThemeRouteCutoverSelector::StaticUnqualified,
            [lifeline_terminal(
                "actor0",
                ThemeRasterPaintBinding::FillAndStrokeFromStroke,
            )],
        )
        .expect("valid writer fact");
        let route = descriptor(ThemeRouteCutoverFacet::Stroke);
        let receipt = fact.seal([0; 32]);

        assert!(receipt.matches_route_identity(route));
        assert!(!receipt.proves_route(route, [0; 32]));
    }
}
