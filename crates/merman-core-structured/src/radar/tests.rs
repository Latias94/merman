use super::*;

#[test]
fn radar_projection_can_cancel_while_indexing_detailed_entries() {
    let axes = (0..512)
        .map(|index| AxisAst {
            name: SpannedText {
                text: format!("axis_{index}"),
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(index, index + 1),
            },
            label: None,
        })
        .collect();
    let entries = (0..512)
        .map(|index| EntryAst {
            axis: Some(SpannedText {
                text: format!("axis_{index}"),
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(index, index + 1),
            }),
            value: SpannedRadarValue {
                value: index as f64,
                #[cfg(feature = "editor-facts")]
                span: SourceSpan::new(index, index + 1),
            },
        })
        .collect();
    let mut db = RadarDb::new();
    db.set_axes_controlled(axes, &OperationControl::new())
        .unwrap();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(10);

    assert!(matches!(
        db.set_curves_controlled(
            vec![CurveAst {
                name: SpannedText {
                    text: "current".to_string(),
                    #[cfg(feature = "editor-facts")]
                    span: SourceSpan::new(0, 1),
                },
                label: None,
                entries,
                span: SourceSpan::new(0, 1),
            }],
            &control,
        ),
        Err(merman_model::OperationCancelled { .. })
    ));
}

#[test]
fn radar_detailed_entries_keep_the_first_duplicate_value() {
    let axes = vec![RadarAxis {
        name: "cost".to_string(),
        label: "Cost".to_string(),
    }];
    let entries = [1, 2].map(|value| EntryAst {
        axis: Some(SpannedText {
            text: "cost".to_string(),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(0, 1),
        }),
        value: SpannedRadarValue {
            value: f64::from(value),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(0, 1),
        },
    });

    assert_eq!(
        compute_curve_entries_controlled(&axes, &entries, &OperationControl::new())
            .unwrap()
            .unwrap(),
        vec![1.0]
    );
}
