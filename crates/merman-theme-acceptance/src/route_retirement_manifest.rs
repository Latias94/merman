use std::collections::BTreeMap;

use merman_render::__private::{
    ThemeLegacyProjectionRetirementDescriptor, ThemeLegacyProjectionRetirementReceipt,
    ThemeLegacyRouteSelector, retired_legacy_theme_projection_receipts,
};

use crate::observation::{append_len_prefixed, sha256};
use crate::runner::{C6ProofError, C6ProofResult};

const RETIREMENT_MANIFEST_VERSION: u16 = 1;
const RETIREMENT_BASELINE_REVISION: &str = "a4b1db26d315f044140a33378723ca8834452a75";
const EXPECTED_RETIREMENT_COUNT: usize = 56;
const EXPECTED_VALUE_PROBE_COUNT: usize = EXPECTED_RETIREMENT_COUNT * 2;

// Audited independently from the production inventory. Any route, selector, facet, or historical
// projection change requires an explicit manifest update and review.
const EXPECTED_DESCRIPTOR_INVENTORY_DIGEST: [u8; 32] = [
    0xf8, 0x87, 0x8a, 0x9d, 0xd6, 0xa3, 0x28, 0x34, 0xe8, 0x06, 0x52, 0x9b, 0x3b, 0xda, 0x76, 0xb4,
    0x8e, 0xfc, 0x14, 0x40, 0x53, 0xf4, 0x58, 0x22, 0xc7, 0xdc, 0x22, 0x11, 0x7d, 0xb1, 0xa8, 0x28,
];

pub(crate) struct LegacyProjectionRetirementAuthorization {
    manifest_digest: [u8; 32],
    receipt_report_digest: [u8; 32],
    retirement_count: usize,
    value_probe_count: usize,
}

impl LegacyProjectionRetirementAuthorization {
    pub(crate) const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }

    pub(crate) const fn receipt_report_digest(&self) -> [u8; 32] {
        self.receipt_report_digest
    }

    pub(crate) const fn retirement_count(&self) -> usize {
        self.retirement_count
    }

    pub(crate) const fn value_probe_count(&self) -> usize {
        self.value_probe_count
    }
}

pub(crate) fn authorize_retired_legacy_projections()
-> C6ProofResult<LegacyProjectionRetirementAuthorization> {
    let receipts = retired_legacy_theme_projection_receipts()
        .map_err(|error| C6ProofError::new("route-retirement-production", error.to_string()))?;
    authorize_receipts(receipts)
}

fn authorize_receipts(
    receipts: Vec<ThemeLegacyProjectionRetirementReceipt>,
) -> C6ProofResult<LegacyProjectionRetirementAuthorization> {
    let mut by_descriptor = BTreeMap::new();
    for receipt in receipts {
        c6_ensure!(
            "route-retirement-receipt",
            receipt.digest() != [0; 32],
            "production retirement receipt digest is zero for {}",
            descriptor_label(receipt.descriptor()),
        );
        if by_descriptor
            .insert(receipt.descriptor(), receipt.digest())
            .is_some()
        {
            return Err(C6ProofError::new(
                "route-retirement-receipt",
                format!(
                    "duplicate production retirement receipt for {}",
                    descriptor_label(receipt.descriptor()),
                ),
            ));
        }
    }

    c6_ensure!(
        "route-retirement-manifest",
        by_descriptor.len() == EXPECTED_RETIREMENT_COUNT,
        "expected {EXPECTED_RETIREMENT_COUNT} retired legacy projection routes, observed {}",
        by_descriptor.len(),
    );
    let descriptor_inventory_digest = descriptor_inventory_digest(by_descriptor.keys().copied());
    c6_ensure!(
        "route-retirement-manifest",
        descriptor_inventory_digest == EXPECTED_DESCRIPTOR_INVENTORY_DIGEST,
        "retirement descriptor inventory drifted: expected {}, observed {}",
        hex_digest(EXPECTED_DESCRIPTOR_INVENTORY_DIGEST),
        hex_digest(descriptor_inventory_digest),
    );

    let manifest_digest = manifest_digest();
    let receipt_report_digest = receipt_report_digest(&by_descriptor);
    c6_ensure!(
        "route-retirement-manifest",
        manifest_digest != [0; 32] && receipt_report_digest != [0; 32],
        "retirement manifest or receipt report digest is zero",
    );

    Ok(LegacyProjectionRetirementAuthorization {
        manifest_digest,
        receipt_report_digest,
        retirement_count: by_descriptor.len(),
        value_probe_count: EXPECTED_VALUE_PROBE_COUNT,
    })
}

fn descriptor_inventory_digest(
    descriptors: impl IntoIterator<Item = ThemeLegacyProjectionRetirementDescriptor>,
) -> [u8; 32] {
    let descriptors = descriptors.into_iter().collect::<Vec<_>>();
    let mut value = b"merman.theme-legacy-projection-retirement-manifest-inventory.v1\0".to_vec();
    value.extend_from_slice(&crate::cutover::usize_to_u64(descriptors.len()).to_be_bytes());
    for descriptor in descriptors {
        append_descriptor(&mut value, descriptor);
    }
    sha256(value)
}

fn manifest_digest() -> [u8; 32] {
    let mut value = b"merman.theme-legacy-projection-retirement-manifest.v1\0".to_vec();
    value.extend_from_slice(&RETIREMENT_MANIFEST_VERSION.to_be_bytes());
    append_len_prefixed(&mut value, RETIREMENT_BASELINE_REVISION.as_bytes());
    value.extend_from_slice(&crate::cutover::usize_to_u64(EXPECTED_RETIREMENT_COUNT).to_be_bytes());
    value
        .extend_from_slice(&crate::cutover::usize_to_u64(EXPECTED_VALUE_PROBE_COUNT).to_be_bytes());
    value.extend_from_slice(&EXPECTED_DESCRIPTOR_INVENTORY_DIGEST);
    sha256(value)
}

fn receipt_report_digest(
    receipts: &BTreeMap<ThemeLegacyProjectionRetirementDescriptor, [u8; 32]>,
) -> [u8; 32] {
    let mut value = b"merman.theme-legacy-projection-retirement-report.v1\0".to_vec();
    value.extend_from_slice(&crate::cutover::usize_to_u64(receipts.len()).to_be_bytes());
    for (&descriptor, digest) in receipts {
        append_descriptor(&mut value, descriptor);
        value.extend_from_slice(digest);
    }
    sha256(value)
}

fn append_descriptor(value: &mut Vec<u8>, descriptor: ThemeLegacyProjectionRetirementDescriptor) {
    let id = descriptor.id();
    append_len_prefixed(value, id.family_id().as_str().as_bytes());
    append_len_prefixed(value, id.target().id().as_bytes());
    append_len_prefixed(value, id.selector().id().as_bytes());
    if let ThemeLegacyRouteSelector::StaticVariant(variant) = id.selector() {
        append_len_prefixed(value, variant.id().as_bytes());
    }
    append_len_prefixed(value, id.facet().id().as_bytes());
    value.extend_from_slice(
        &crate::cutover::usize_to_u64(descriptor.former_projections().len()).to_be_bytes(),
    );
    for projection in descriptor.former_projections() {
        append_len_prefixed(value, projection.contribution_suffix().as_bytes());
        append_len_prefixed(value, projection.assignment_path().as_bytes());
    }
}

fn descriptor_label(descriptor: ThemeLegacyProjectionRetirementDescriptor) -> String {
    let id = descriptor.id();
    format!(
        "{}/{}/{}/{}",
        id.family_id().as_str(),
        id.target().id(),
        id.selector().id(),
        id.facet().id(),
    )
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_production_retirement_inventory_is_authorized() {
        let authorization = authorize_retired_legacy_projections()
            .expect("authorize the production-sealed retirement inventory");

        assert_eq!(authorization.retirement_count(), EXPECTED_RETIREMENT_COUNT);
        assert_eq!(
            authorization.value_probe_count(),
            EXPECTED_VALUE_PROBE_COUNT
        );
        assert_ne!(authorization.manifest_digest(), [0; 32]);
        assert_ne!(authorization.receipt_report_digest(), [0; 32]);
    }

    #[test]
    fn missing_or_duplicate_production_receipt_fails_closed() {
        let receipts =
            retired_legacy_theme_projection_receipts().expect("production retirement receipts");
        let mut missing = receipts.clone();
        missing.pop();
        assert!(authorize_receipts(missing).is_err());

        let mut duplicate = receipts;
        duplicate.push(duplicate[0]);
        assert!(authorize_receipts(duplicate).is_err());
    }
}
