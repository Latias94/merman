//! Pure domain-derived Mindmap output identity from an explicit runtime context.

pub fn diagram_id_from_context(context: &crate::runtime::OperationContext) -> String {
    let mut hex = context.derive_hex("mindmap.diagram-id", 0, 32).into_bytes();
    hex[12] = b'4';
    let variant = hex_digit(hex[16]);
    hex[16] = b"89ab"[(variant & 0x03) as usize];

    let hex = String::from_utf8(hex).expect("generated hexadecimal ID is valid UTF-8");
    format!(
        "mindmap-{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("generated ID contains only lowercase hexadecimal digits"),
    }
}
