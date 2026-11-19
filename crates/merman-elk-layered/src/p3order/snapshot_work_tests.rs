use super::*;

#[test]
fn snapshot_relation_counts_include_copies_inserts_and_both_unions() {
    for (smaller, larger, expected) in [(0, 0, 2), (3, 0, 11), (0, 4, 14), (2, 2, 22), (2, 3, 29)] {
        assert_eq!(
            model_order_relation_update_work_from_counts(smaller, larger),
            Ok(expected),
            "s={smaller}, l={larger}",
        );
    }
}

#[test]
fn snapshot_relation_scalar_products_and_sums_fail_closed() {
    for (smaller, larger) in [
        (usize::MAX / 2 + 1, 2),
        (usize::MAX / 2 + 1, 0),
        (0, usize::MAX / 2 + 1),
        (usize::MAX / 3 + 1, 0),
        (0, usize::MAX / 3 + 1),
        (usize::MAX / 5, 2),
    ] {
        assert_eq!(
            model_order_relation_update_work_from_counts(smaller, larger),
            Err(WorkError::ArithmeticOverflow),
            "s={smaller}, l={larger}",
        );
    }
    assert_eq!(
        model_order_relation_update_work_from_counts((usize::MAX - 2) / 3, 0),
        Ok(usize::MAX - 1),
    );
}

#[test]
fn snapshot_relation_port_base_unit_can_overflow_after_a_valid_update_charge() {
    let units = model_order_relation_update_work_from_counts(usize::MAX / 5 - 1, 1).unwrap();
    assert_eq!(units, usize::MAX);
    assert_eq!(checked_add(1, units), Err(WorkError::ArithmeticOverflow));
}
