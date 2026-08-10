use crate::Result;
use crate::error::AsciiError;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

pub(crate) fn try_concat_layout_text(
    left: &str,
    right: &str,
    resources: &ResourceContext,
) -> Result<String> {
    try_concat_layout_text_impl(left, right, resources, || {})
}

pub(crate) fn try_clone_layout_text(value: &str, resources: &ResourceContext) -> Result<String> {
    try_concat_layout_text(value, "", resources)
}

pub(crate) fn try_repeat_layout_char(
    ch: char,
    count: usize,
    resources: &ResourceContext,
) -> Result<String> {
    let byte_count = resources.checked_work_mul(ch.len_utf8(), count)?;
    resources.charge_layout_work(byte_count)?;
    let mut output = String::new();
    output
        .try_reserve_exact(byte_count)
        .map_err(|_| layout_allocation_failed())?;
    for _ in 0..count {
        output.push(ch);
    }
    Ok(output)
}

fn try_concat_layout_text_impl(
    left: &str,
    right: &str,
    resources: &ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    let byte_count = resources.checked_work_add(left.len(), right.len())?;
    resources.charge_layout_work(byte_count)?;
    before_materialize();
    let mut output = String::new();
    output
        .try_reserve_exact(byte_count)
        .map_err(|_| layout_allocation_failed())?;
    output.push_str(left);
    output.push_str(right);
    Ok(output)
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    #[test]
    fn layout_text_accepts_exact_work_and_rejects_n_minus_one_before_materializing() {
        const REQUIRED_WORK: usize = 6;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, REQUIRED_WORK)
            .expect("exact layout-text work limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_materialized = Cell::new(false);

        let output = try_concat_layout_text_impl("abcd", "ef", &exact_resources, || {
            exact_materialized.set(true);
        })
        .expect("exact layout-text work should permit materialization");

        assert_eq!(output, "abcdef");
        assert!(exact_materialized.get());
        assert_eq!(exact_resources.layout_work_used(), REQUIRED_WORK);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, REQUIRED_WORK - 1)
            .expect("max-minus-one layout-text work limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let below_materialized = Cell::new(false);
        let error = try_concat_layout_text_impl("abcd", "ef", &below_resources, || {
            below_materialized.set(true);
        })
        .expect_err("max-minus-one work should fail before materialization");

        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == REQUIRED_WORK
                    && details.max == REQUIRED_WORK - 1
        ));
        assert_eq!(below_resources.layout_work_used(), 0);
    }
}
