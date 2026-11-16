#![cfg(all(feature = "diagram-class", feature = "diagram-er"))]

mod support;

use merman_ascii::{
    AsciiOutputOutcome, AsciiRenderOptions, AsciiViewportPolicy, OverflowPolicy,
    TerminalWidthProfile,
};
use support::{parse_model, render_model_report, terminal_extent_with_profile};

#[test]
fn private_channels_fit_a_smaller_reading_area_without_losing_relations() {
    let examples = [
        (
            "classDiagram\nA --> B : sends\nB --> C : forwards\nC --> A : returns\n",
            vec!["sends", "forwards", "returns"],
            18,
        ),
        (
            "classDiagram\nRoot <|-- Worker\nRoot <|-- Store\nWorker --> Queue : writes\nStore --> Queue : reads\n",
            vec!["Root", "Worker", "Store", "Queue", "writes", "reads"],
            21,
        ),
        (
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\nCUSTOMER ||--o{ ADDRESS : uses\nORDER ||--|{ LINE_ITEM : contains\nLINE_ITEM }o--|| PRODUCT : references\n",
            vec![
                "CUSTOMER",
                "ORDER",
                "ADDRESS",
                "LINE_ITEM",
                "PRODUCT",
                "places",
                "uses",
                "contains",
                "references",
            ],
            27,
        ),
    ];
    for (source, labels, max_height) in examples {
        let model = parse_model(source);
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let options = options.with_terminal_width_profile(profile);
                for width in [60, 80, 100, 120] {
                    let report = render_model_report(
                        &model,
                        &options,
                        AsciiViewportPolicy::with_max_width(width)
                            .overflow(OverflowPolicy::Fallback),
                    )
                    .expect("bounded relation scene should remain diagrammatic");
                    assert_eq!(
                        report.outcome,
                        AsciiOutputOutcome::Primary,
                        "{}",
                        report.text
                    );
                    assert!(!report.text.contains("relations:"), "{}", report.text);
                    let extent = terminal_extent_with_profile(&report.text, profile);
                    assert!(
                        extent.0 <= width && extent.1 <= max_height,
                        "{extent:?}:\n{}",
                        report.text
                    );
                    for label in &labels {
                        assert_eq!(report.text.matches(label).count(), 1, "{}", report.text);
                    }
                }
            }
        }
    }
}

#[test]
fn compacted_scene_preserves_wide_and_multiline_text_in_every_direction() {
    for direction in ["TB", "BT", "LR", "RL"] {
        let model = parse_model(&format!(
            "classDiagram\ndirection {direction}\nclass A[\"入口 👩‍💻\"]\nclass B[\"服务 e\u{301}\"]\nclass C[\"缓存\"]\nA --> B : 提交\nB --> C : 读取\nC --> A : 回传\n"
        ));
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let options = options.with_terminal_width_profile(profile);
                let report =
                    render_model_report(&model, &options, AsciiViewportPolicy::unrestricted())
                        .expect("wide text and endpoint labels should survive compaction");
                assert!(!report.text.contains("relations:"), "{}", report.text);
                for text in ["入口 👩‍💻", "服务 e\u{301}", "缓存", "提交", "读取", "回传"]
                {
                    assert_eq!(report.text.matches(text).count(), 1, "{}", report.text);
                }
                let extent = terminal_extent_with_profile(&report.text, profile);
                assert_eq!(extent.0, report.emitted_extent.width);
                assert_eq!(extent.1, report.emitted_extent.height);
                if profile == TerminalWidthProfile::Cjk {
                    assert!(!report.text.chars().any(|ch| "┌┐└┘─│╭╮╰╯".contains(ch)));
                }
            }
        }
    }

    for direction in ["LR", "RL"] {
        let model = parse_model(&format!(
            "classDiagram\ndirection {direction}\nclass C[\"缓存 👩‍💻\"]\nA --> B\nB --> C\nC \"一个<br>多个\" --> D\nD --> E\nE --> F\nF --> G\n"
        ));
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let report = render_model_report(
                &model,
                &AsciiRenderOptions::unicode().with_terminal_width_profile(profile),
                AsciiViewportPolicy::unrestricted(),
            )
            .expect("wide multiline endpoint labels should remain drawable");
            assert!(!report.text.contains("relations:"), "{}", report.text);
            for text in ["缓存 👩‍💻", "一个", "多个"] {
                assert!(report.text.contains(text), "{}", report.text);
            }
            let extent = terminal_extent_with_profile(&report.text, profile);
            assert_eq!(extent.0, report.emitted_extent.width);
            assert_eq!(extent.1, report.emitted_extent.height);
        }
    }
}
