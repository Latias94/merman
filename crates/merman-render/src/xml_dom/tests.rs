use super::{Document, ParsingOptions};

#[test]
fn namespace_text_and_source_ranges_match_published_roxmltree() {
    for text in [
        "<?target value?><!--before--><root xmlns='urn:root' xmlns:p='urn:one' p:key='value' xml:lang='en'><p:child xmlns:p='urn:two' p:key='nested'/><plain xmlns=''>tail</plain></root><!--after-->",
        "<root key='a&#9;b\r\nc &amp; d'>before<![CDATA[<raw>&text]]>&amp;&#x1F600;\r\nafter<empty/>tail</root>",
        "<root>one<![CDATA[two]]><![CDATA[three]]>four<!--gap--><?target value?>five</root>",
    ] {
        let actual = Document::parse(text).unwrap();
        let expected = roxmltree::Document::parse(text).unwrap();
        assert_eq!(actual.input_text(), expected.input_text());
        assert_eq!(actual.descendants().count(), expected.descendants().count());
        for (actual_node, expected_node) in actual.descendants().zip(expected.descendants()) {
            assert_eq!(
                format!("{:?}", actual_node.node_type()),
                format!("{:?}", expected_node.node_type())
            );
            assert_eq!(actual_node.id().get(), expected_node.id().get());
            assert_eq!(
                actual_node.parent().map(|node| node.id().get()),
                expected_node.parent().map(|node| node.id().get())
            );
            assert_eq!(
                actual_node.tag_name().name(),
                expected_node.tag_name().name()
            );
            assert_eq!(
                actual_node.tag_name().namespace(),
                expected_node.tag_name().namespace()
            );
            assert_eq!(actual_node.text(), expected_node.text());
            assert_eq!(actual_node.tail(), expected_node.tail());
            assert_eq!(actual_node.range(), expected_node.range());
            assert_eq!(
                actual_node.pi().map(|pi| (pi.target, pi.value)),
                expected_node.pi().map(|pi| (pi.target, pi.value))
            );
            assert_eq!(
                actual_node
                    .namespaces()
                    .map(|ns| (ns.name(), ns.uri()))
                    .collect::<Vec<_>>(),
                expected_node
                    .namespaces()
                    .map(|ns| (ns.name(), ns.uri()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                actual_node.attributes().len(),
                expected_node.attributes().len()
            );
            for (actual_attr, expected_attr) in
                actual_node.attributes().zip(expected_node.attributes())
            {
                assert_eq!(actual_attr.namespace(), expected_attr.namespace());
                assert_eq!(actual_attr.name(), expected_attr.name());
                assert_eq!(actual_attr.value(), expected_attr.value());
                assert_eq!(actual_attr.range(), expected_attr.range());
                assert_eq!(actual_attr.range_qname(), expected_attr.range_qname());
                assert_eq!(actual_attr.range_value(), expected_attr.range_value());
            }
            for pos in [actual_node.range().start, actual_node.range().end] {
                let actual_pos = actual.text_pos_at(pos);
                let expected_pos = expected.text_pos_at(pos);
                assert_eq!(
                    (actual_pos.row, actual_pos.col),
                    (expected_pos.row, expected_pos.col)
                );
            }
        }
    }
}

#[test]
fn malformed_xml_and_default_dtd_errors_match_published_roxmltree() {
    for text in [
        "<root><child></root>",
        "<root a='1' a='2'/>",
        "<root xmlns:p='urn:same' xmlns:q='urn:same' p:a='1' q:a='2'/>",
        "<root><unbound:child/></root>",
        "<root>&unknown;</root>",
        "<root><![CDATA[unfinished</root>",
        "<root><!--invalid--comment--></root>",
        "<root><child/>",
        "<root/><extra/>",
        "<!DOCTYPE root [<!ENTITY value 'text'>]><root>&value;</root>",
    ] {
        let actual = Document::parse(text).unwrap_err();
        let expected = roxmltree::Document::parse(text).unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "{text}");
        assert_eq!(actual.to_string(), expected.to_string(), "{text}");
        assert_eq!(
            (actual.pos().row, actual.pos().col),
            (expected.pos().row, expected.pos().col),
            "{text}"
        );
    }
}

#[test]
fn admitted_entities_and_existing_limits_match_published_roxmltree() {
    let text = "<!DOCTYPE root [<!ENTITY word 'owned &amp; normalized'><!ENTITY label '<span>&word;</span>'>]><root key='&word;'>&label;<![CDATA[tail]]></root>";
    let actual = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        },
    )
    .unwrap();
    let expected = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        actual.root_element().attribute("key"),
        Some("owned & normalized")
    );
    assert_eq!(actual.descendants().count(), expected.descendants().count());
    for (actual_node, expected_node) in actual.descendants().zip(expected.descendants()) {
        assert_eq!(
            actual_node.tag_name().name(),
            expected_node.tag_name().name()
        );
        assert_eq!(actual_node.text(), expected_node.text());
        assert_eq!(actual_node.tail(), expected_node.tail());
        assert_eq!(actual_node.range(), expected_node.range());
    }

    let mut text = String::from("<!DOCTYPE root [<!ENTITY e0 'text'>");
    for depth in 1..=12 {
        text.push_str(&format!("<!ENTITY e{depth} '&e{};'>", depth - 1));
    }
    text.push_str("]><root>&e12;</root>");
    let actual = Document::parse_with_options(
        &text,
        ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        },
    )
    .unwrap_err();
    let expected = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));

    let text = "<root><a/><b/></root>";
    let actual = Document::parse_with_options(
        text,
        ParsingOptions {
            nodes_limit: 3,
            ..ParsingOptions::default()
        },
    )
    .unwrap_err();
    let expected = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            nodes_limit: 3,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    let _: &dyn std::error::Error = &actual;
}
