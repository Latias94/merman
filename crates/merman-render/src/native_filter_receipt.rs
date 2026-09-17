use std::str::FromStr as _;

use crate::diagram_theme::{EffectColorSpace, EffectInput};
use sha2::{Digest as _, Sha256};

const RECEIPT_DOMAIN: &[u8] = b"merman-native-shadow-application-receipt-v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[doc(hidden)]
pub struct NativeSvgShadowStage {
    input: EffectInput,
    offset_bits: [u32; 2],
    std_deviation_bits: [u32; 2],
    color_rgba: [u8; 4],
}

impl NativeSvgShadowStage {
    pub fn new(
        input: EffectInput,
        offset: [f32; 2],
        std_deviation: [f32; 2],
        color_css: &str,
    ) -> Option<Self> {
        if offset
            .into_iter()
            .chain(std_deviation)
            .any(|value| !value.is_finite())
            || std_deviation.into_iter().any(|value| value < 0.0)
        {
            return None;
        }
        let color = svgtypes::Color::from_str(color_css.trim()).ok()?;
        Some(Self {
            input,
            offset_bits: offset.map(normalized_f32_bits),
            std_deviation_bits: std_deviation.map(normalized_f32_bits),
            color_rgba: [color.red, color.green, color.blue, color.alpha],
        })
    }
    pub const fn input(&self) -> EffectInput {
        self.input
    }
    pub fn offset(&self) -> [f32; 2] {
        self.offset_bits.map(f32::from_bits)
    }
    pub fn std_deviation(&self) -> [f32; 2] {
        self.std_deviation_bits.map(f32::from_bits)
    }
    pub const fn color_rgba(&self) -> [u8; 4] {
        self.color_rgba
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[doc(hidden)]
pub struct NativeSvgFilterApplication {
    filter_id: String,
    region_bits: [u32; 4],
    color_space: EffectColorSpace,
    stages: Vec<NativeSvgShadowStage>,
    reference_count: u32,
}

impl NativeSvgFilterApplication {
    pub fn new(
        filter_id: impl Into<String>,
        region: [f32; 4],
        color_space: EffectColorSpace,
        stages: Vec<NativeSvgShadowStage>,
        reference_count: usize,
    ) -> Option<Self> {
        let filter_id = filter_id.into();
        if filter_id.is_empty()
            || filter_id.len() > 1024
            || filter_id
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            || region.into_iter().any(|value| !value.is_finite())
            || region[2] <= 0.0
            || region[3] <= 0.0
            || stages.is_empty()
            || stages.len() > crate::__private::MAX_NATIVE_SHADOW_STAGES
            || stages[0].input() != EffectInput::SourceGraphic
        {
            return None;
        }
        let reference_count = u32::try_from(reference_count).ok()?;
        if reference_count == 0 {
            return None;
        }
        Some(Self {
            filter_id,
            region_bits: region.map(normalized_f32_bits),
            color_space,
            stages,
            reference_count,
        })
    }
    pub fn filter_id(&self) -> &str {
        &self.filter_id
    }
    pub fn region(&self) -> [f32; 4] {
        self.region_bits.map(f32::from_bits)
    }
    pub const fn color_space(&self) -> EffectColorSpace {
        self.color_space
    }
    pub fn stages(&self) -> &[NativeSvgShadowStage] {
        &self.stages
    }
    pub const fn reference_count(&self) -> u32 {
        self.reference_count
    }
}

/// Opaque equality receipt for exact bounded shadow applications emitted by family writers.
///
/// The exporter reconstructs it independently from terminal SVG and its resolved `usvg` tree.
/// Filter applications are unordered; stages inside an application are ordered.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeSvgFilterReceipt {
    digest: [u8; 32],
    filter_count: u32,
    drop_shadow_count: u32,
    reference_count: u32,
}

impl NativeSvgFilterReceipt {
    pub fn from_applications(
        applications: impl IntoIterator<Item = NativeSvgFilterApplication>,
    ) -> Option<Self> {
        let mut applications = applications.into_iter().collect::<Vec<_>>();
        if applications.is_empty() {
            return None;
        }
        applications.sort_unstable_by(|a, b| a.filter_id.cmp(&b.filter_id));
        if applications
            .windows(2)
            .any(|pair| pair[0].filter_id == pair[1].filter_id)
        {
            return None;
        }
        let filter_count = u32::try_from(applications.len()).ok()?;
        let reference_count = applications.iter().try_fold(0u32, |total, application| {
            total.checked_add(application.reference_count)
        })?;
        let drop_shadow_count = applications.iter().try_fold(0u32, |total, application| {
            total.checked_add(u32::try_from(application.stages.len()).ok()?)
        })?;
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, RECEIPT_DOMAIN);
        hasher.update(filter_count.to_le_bytes());
        hasher.update(drop_shadow_count.to_le_bytes());
        hasher.update(reference_count.to_le_bytes());
        for application in applications {
            update_len_prefixed(&mut hasher, application.filter_id.as_bytes());
            for bits in application.region_bits {
                hasher.update(bits.to_le_bytes());
            }
            hasher.update([match application.color_space {
                EffectColorSpace::LinearRgb => 0,
                EffectColorSpace::Srgb => 1,
            }]);
            hasher.update((application.stages.len() as u32).to_le_bytes());
            for stage in application.stages {
                hasher.update([match stage.input {
                    EffectInput::SourceGraphic => 0,
                    EffectInput::Previous => 1,
                }]);
                for bits in stage
                    .offset_bits
                    .into_iter()
                    .chain(stage.std_deviation_bits)
                {
                    hasher.update(bits.to_le_bytes());
                }
                hasher.update(stage.color_rgba);
            }
            hasher.update(application.reference_count.to_le_bytes());
        }
        Some(Self {
            digest: hasher.finalize().into(),
            filter_count,
            drop_shadow_count,
            reference_count,
        })
    }
    pub const fn filter_count(self) -> u32 {
        self.filter_count
    }
    pub const fn drop_shadow_count(self) -> u32 {
        self.drop_shadow_count
    }
    pub const fn reference_count(self) -> u32 {
        self.reference_count
    }
    pub const fn identity_digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

fn normalized_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}
fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn application(
        id: &str,
        offset: f32,
        deviation: f32,
        references: usize,
    ) -> NativeSvgFilterApplication {
        NativeSvgFilterApplication::new(
            id,
            [-0.2, -0.2, 1.4, 1.4],
            EffectColorSpace::LinearRgb,
            vec![
                NativeSvgShadowStage::new(
                    EffectInput::SourceGraphic,
                    [offset, 5.0],
                    [deviation; 2],
                    "#111827",
                )
                .unwrap(),
            ],
            references,
        )
        .unwrap()
    }
    #[test]
    fn receipt_is_order_independent_but_value_sensitive() {
        let a = application("alpha", 4.0, 0.0, 2);
        let b = application("beta", 6.0, 8.0, 1);
        let first = NativeSvgFilterReceipt::from_applications([a.clone(), b.clone()]).unwrap();
        let reordered = NativeSvgFilterReceipt::from_applications([b, a]).unwrap();
        let changed = NativeSvgFilterReceipt::from_applications([
            application("alpha", 4.0, 1.0, 2),
            application("beta", 6.0, 8.0, 1),
        ])
        .unwrap();
        assert_eq!(first, reordered);
        assert_ne!(first, changed);
        assert_eq!(first.identity_digest(), reordered.identity_digest());
        assert_ne!(first.identity_digest(), changed.identity_digest());
        assert_eq!(
            (
                first.filter_count(),
                first.drop_shadow_count(),
                first.reference_count()
            ),
            (2, 2, 3)
        );
    }
    #[test]
    fn receipt_binds_stage_order_input_color_space_and_values() {
        let mut a = application("composed", 4.0, 2.0, 1);
        a.stages.push(
            NativeSvgShadowStage::new(EffectInput::Previous, [0.0, 0.0], [3.0; 2], "#ff0000")
                .unwrap(),
        );
        let original = NativeSvgFilterReceipt::from_applications([a.clone()]).unwrap();
        assert_eq!(
            (
                original.filter_count(),
                original.drop_shadow_count(),
                original.reference_count()
            ),
            (1, 2, 1)
        );
        let mut changed = a.clone();
        changed.stages[1].input = EffectInput::SourceGraphic;
        assert_ne!(
            Some(original),
            NativeSvgFilterReceipt::from_applications([changed])
        );
        let mut changed = a.clone();
        changed.color_space = EffectColorSpace::Srgb;
        assert_ne!(
            Some(original),
            NativeSvgFilterReceipt::from_applications([changed])
        );
        let mut changed = a.clone();
        changed.stages[1].std_deviation_bits = [4.0f32.to_bits(); 2];
        assert_ne!(
            Some(original),
            NativeSvgFilterReceipt::from_applications([changed])
        );
        a.stages[1].input = EffectInput::SourceGraphic;
        let forward = NativeSvgFilterReceipt::from_applications([a.clone()]);
        a.stages.reverse();
        assert_ne!(forward, NativeSvgFilterReceipt::from_applications([a]));
    }
    #[test]
    fn invalid_applications_are_rejected() {
        assert!(NativeSvgFilterReceipt::from_applications([]).is_none());
        assert!(
            NativeSvgFilterReceipt::from_applications([
                application("x", 1.0, 0.0, 1),
                application("x", 2.0, 0.0, 1)
            ])
            .is_none()
        );
        let stage =
            NativeSvgShadowStage::new(EffectInput::SourceGraphic, [0.0; 2], [0.0; 2], "#000")
                .unwrap();
        for stages in [
            vec![],
            vec![stage; crate::__private::MAX_NATIVE_SHADOW_STAGES + 1],
            vec![
                NativeSvgShadowStage::new(EffectInput::Previous, [0.0; 2], [0.0; 2], "#000")
                    .unwrap(),
            ],
        ] {
            assert!(
                NativeSvgFilterApplication::new(
                    "x",
                    [0.0, 0.0, 1.0, 1.0],
                    EffectColorSpace::LinearRgb,
                    stages,
                    1
                )
                .is_none()
            );
        }
        for deviation in [[-1.0, 0.0], [0.0, -1.0], [f32::NAN, 1.0]] {
            assert!(
                NativeSvgShadowStage::new(
                    EffectInput::SourceGraphic,
                    [0.0; 2],
                    deviation,
                    "#111827"
                )
                .is_none()
            );
        }
    }
}
