use super::{
    ArrowToken, ClickAction, ClickStmt, DirectionStatementToken, LabeledText, LexError, LinkToken,
    NodeLabelToken, SubgraphHeader, TitleKind, Tok,
    ast::{FlowchartClickEditorEvidence, FlowchartDirectiveEditorEvidence},
    destruct_end_link, destruct_labeled_end_link, destruct_start_link, is_ecmascript_trim_char,
    lex, parse_label_text,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, SourceSpan, editor::source_value_span,
};
use std::collections::VecDeque;

fn directive_argument_spans(
    rest: &str,
    rest_start: usize,
) -> (Option<SourceSpan>, Option<SourceSpan>) {
    let leading = rest
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let body = &rest[leading..];
    if body.is_empty() {
        return (None, None);
    }

    let first_len = body
        .as_bytes()
        .iter()
        .position(|byte| byte.is_ascii_whitespace())
        .unwrap_or(body.len());
    let first_start = rest_start + leading;
    let first_end = first_start + first_len;
    let first = SourceSpan::new(first_start, first_end);
    let remainder = &body[first_len..];
    if remainder.is_empty() {
        return (Some(first), None);
    }

    let remainder_leading = remainder
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let value_start = first_end + remainder_leading;
    let value_end = rest_start + rest.len();
    (Some(first), Some(SourceSpan::new(value_start, value_end)))
}

fn directive_error<I>(
    mut error: LexError,
    prefix: &'static str,
    statement_span: SourceSpan,
    expected_syntax: I,
) -> LexError
where
    I: IntoIterator<Item = (EditorExpectedSyntaxKind, SourceSpan)>,
{
    for (kind, span) in expected_syntax {
        error = error.expecting(kind, span);
    }
    error.in_directive(prefix, statement_span)
}

fn active_following_span(
    rest: &str,
    rest_start: usize,
    following: Option<SourceSpan>,
) -> Option<SourceSpan> {
    let following = following?;
    let local_start = following.start.checked_sub(rest_start)?;
    let raw = rest.get(local_start..)?;
    let trailing = raw
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    if raw.is_empty() || trailing > 0 {
        return Some(SourceSpan::new(following.end, following.end));
    }
    Some(following)
}

fn trailing_value_slot(
    rest: &str,
    rest_start: usize,
    following: Option<SourceSpan>,
) -> Option<SourceSpan> {
    let active = active_following_span(rest, rest_start, following)?;
    (active.start == active.end).then_some(active)
}

fn directive_editor_evidence(
    statement_span: SourceSpan,
    first: Option<(EditorExpectedSyntaxKind, SourceSpan)>,
    following: Option<(EditorExpectedSyntaxKind, SourceSpan)>,
) -> FlowchartDirectiveEditorEvidence {
    FlowchartDirectiveEditorEvidence::new(
        EditorExpectedSyntax::new(EditorExpectedSyntaxKind::Directive, statement_span),
        first.map(|(kind, span)| EditorExpectedSyntax::new(kind, span)),
        following.map(|(kind, span)| EditorExpectedSyntax::new(kind, span)),
    )
}

fn skip_ecmascript_whitespace(input: &str, mut pos: usize) -> usize {
    debug_assert!(input.is_char_boundary(pos));
    while pos < input.len() {
        let ch = input[pos..]
            .chars()
            .next()
            .expect("position before input end must contain a character");
        if !is_ecmascript_trim_char(ch) {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn find_pipe_label_end(input: &str, mut pos: usize) -> Option<usize> {
    #[derive(Clone, Copy)]
    enum State {
        Text,
        String,
        MarkdownString,
    }

    let mut state = State::Text;
    while pos < input.len() {
        let rest = &input[pos..];
        match state {
            State::Text => {
                if rest.starts_with('|') {
                    return Some(pos);
                }
                if rest.starts_with("\"`") {
                    state = State::MarkdownString;
                    pos += 2;
                    continue;
                }
                if rest.starts_with('"') {
                    state = State::String;
                    pos += 1;
                    continue;
                }
            }
            State::String => {
                if rest.starts_with('"') {
                    state = State::Text;
                    pos += 1;
                    continue;
                }
            }
            State::MarkdownString => {
                if rest.starts_with("`\"") {
                    state = State::Text;
                    pos += 2;
                    continue;
                }
            }
        }

        let ch = rest
            .chars()
            .next()
            .expect("pipe label scan position must contain a character");
        pos += ch.len_utf8();
    }
    None
}

/// Mermaid 11.17.2's `UNICODE_TEXT` token is an explicit BMP range list.
///
/// Keep this source-backed instead of using Rust's broader `is_alphabetic`, because the pinned
/// Mermaid lexer rejects supplementary-plane letters and combining marks outside this list.
const MERMAID_UNICODE_TEXT_RANGES: &[(u32, u32)] = &[
    (0xAA, 0xAA),
    (0xB5, 0xB5),
    (0xBA, 0xBA),
    (0xC0, 0xD6),
    (0xD8, 0xF6),
    (0xF8, 0x2C1),
    (0x2C6, 0x2D1),
    (0x2E0, 0x2E4),
    (0x2EC, 0x2EC),
    (0x2EE, 0x2EE),
    (0x370, 0x374),
    (0x376, 0x377),
    (0x37A, 0x37D),
    (0x386, 0x386),
    (0x388, 0x38A),
    (0x38C, 0x38C),
    (0x38E, 0x3A1),
    (0x3A3, 0x3F5),
    (0x3F7, 0x481),
    (0x48A, 0x527),
    (0x531, 0x556),
    (0x559, 0x559),
    (0x561, 0x587),
    (0x5D0, 0x5EA),
    (0x5F0, 0x5F2),
    (0x620, 0x64A),
    (0x66E, 0x66F),
    (0x671, 0x6D3),
    (0x6D5, 0x6D5),
    (0x6E5, 0x6E6),
    (0x6EE, 0x6EF),
    (0x6FA, 0x6FC),
    (0x6FF, 0x6FF),
    (0x710, 0x710),
    (0x712, 0x72F),
    (0x74D, 0x7A5),
    (0x7B1, 0x7B1),
    (0x7CA, 0x7EA),
    (0x7F4, 0x7F5),
    (0x7FA, 0x7FA),
    (0x800, 0x815),
    (0x81A, 0x81A),
    (0x824, 0x824),
    (0x828, 0x828),
    (0x840, 0x858),
    (0x8A0, 0x8A0),
    (0x8A2, 0x8AC),
    (0x904, 0x939),
    (0x93D, 0x93D),
    (0x950, 0x950),
    (0x958, 0x961),
    (0x971, 0x977),
    (0x979, 0x97F),
    (0x985, 0x98C),
    (0x98F, 0x990),
    (0x993, 0x9A8),
    (0x9AA, 0x9B0),
    (0x9B2, 0x9B2),
    (0x9B6, 0x9B9),
    (0x9BD, 0x9BD),
    (0x9CE, 0x9CE),
    (0x9DC, 0x9DD),
    (0x9DF, 0x9E1),
    (0x9F0, 0x9F1),
    (0xA05, 0xA0A),
    (0xA0F, 0xA10),
    (0xA13, 0xA28),
    (0xA2A, 0xA30),
    (0xA32, 0xA33),
    (0xA35, 0xA36),
    (0xA38, 0xA39),
    (0xA59, 0xA5C),
    (0xA5E, 0xA5E),
    (0xA72, 0xA74),
    (0xA85, 0xA8D),
    (0xA8F, 0xA91),
    (0xA93, 0xAA8),
    (0xAAA, 0xAB0),
    (0xAB2, 0xAB3),
    (0xAB5, 0xAB9),
    (0xABD, 0xABD),
    (0xAD0, 0xAD0),
    (0xAE0, 0xAE1),
    (0xB05, 0xB0C),
    (0xB0F, 0xB10),
    (0xB13, 0xB28),
    (0xB2A, 0xB30),
    (0xB32, 0xB33),
    (0xB35, 0xB39),
    (0xB3D, 0xB3D),
    (0xB5C, 0xB5D),
    (0xB5F, 0xB61),
    (0xB71, 0xB71),
    (0xB83, 0xB83),
    (0xB85, 0xB8A),
    (0xB8E, 0xB90),
    (0xB92, 0xB95),
    (0xB99, 0xB9A),
    (0xB9C, 0xB9C),
    (0xB9E, 0xB9F),
    (0xBA3, 0xBA4),
    (0xBA8, 0xBAA),
    (0xBAE, 0xBB9),
    (0xBD0, 0xBD0),
    (0xC05, 0xC0C),
    (0xC0E, 0xC10),
    (0xC12, 0xC28),
    (0xC2A, 0xC33),
    (0xC35, 0xC39),
    (0xC3D, 0xC3D),
    (0xC58, 0xC59),
    (0xC60, 0xC61),
    (0xC85, 0xC8C),
    (0xC8E, 0xC90),
    (0xC92, 0xCA8),
    (0xCAA, 0xCB3),
    (0xCB5, 0xCB9),
    (0xCBD, 0xCBD),
    (0xCDE, 0xCDE),
    (0xCE0, 0xCE1),
    (0xCF1, 0xCF2),
    (0xD05, 0xD0C),
    (0xD0E, 0xD10),
    (0xD12, 0xD3A),
    (0xD3D, 0xD3D),
    (0xD4E, 0xD4E),
    (0xD60, 0xD61),
    (0xD7A, 0xD7F),
    (0xD85, 0xD96),
    (0xD9A, 0xDB1),
    (0xDB3, 0xDBB),
    (0xDBD, 0xDBD),
    (0xDC0, 0xDC6),
    (0xE01, 0xE30),
    (0xE32, 0xE33),
    (0xE40, 0xE46),
    (0xE81, 0xE82),
    (0xE84, 0xE84),
    (0xE87, 0xE88),
    (0xE8A, 0xE8A),
    (0xE8D, 0xE8D),
    (0xE94, 0xE97),
    (0xE99, 0xE9F),
    (0xEA1, 0xEA3),
    (0xEA5, 0xEA5),
    (0xEA7, 0xEA7),
    (0xEAA, 0xEAB),
    (0xEAD, 0xEB0),
    (0xEB2, 0xEB3),
    (0xEBD, 0xEBD),
    (0xEC0, 0xEC4),
    (0xEC6, 0xEC6),
    (0xEDC, 0xEDF),
    (0xF00, 0xF00),
    (0xF40, 0xF47),
    (0xF49, 0xF6C),
    (0xF88, 0xF8C),
    (0x1000, 0x102A),
    (0x103F, 0x103F),
    (0x1050, 0x1055),
    (0x105A, 0x105D),
    (0x1061, 0x1061),
    (0x1065, 0x1066),
    (0x106E, 0x1070),
    (0x1075, 0x1081),
    (0x108E, 0x108E),
    (0x10A0, 0x10C5),
    (0x10C7, 0x10C7),
    (0x10CD, 0x10CD),
    (0x10D0, 0x10FA),
    (0x10FC, 0x1248),
    (0x124A, 0x124D),
    (0x1250, 0x1256),
    (0x1258, 0x1258),
    (0x125A, 0x125D),
    (0x1260, 0x1288),
    (0x128A, 0x128D),
    (0x1290, 0x12B0),
    (0x12B2, 0x12B5),
    (0x12B8, 0x12BE),
    (0x12C0, 0x12C0),
    (0x12C2, 0x12C5),
    (0x12C8, 0x12D6),
    (0x12D8, 0x1310),
    (0x1312, 0x1315),
    (0x1318, 0x135A),
    (0x1380, 0x138F),
    (0x13A0, 0x13F4),
    (0x1401, 0x166C),
    (0x166F, 0x167F),
    (0x1681, 0x169A),
    (0x16A0, 0x16EA),
    (0x1700, 0x170C),
    (0x170E, 0x1711),
    (0x1720, 0x1731),
    (0x1740, 0x1751),
    (0x1760, 0x176C),
    (0x176E, 0x1770),
    (0x1780, 0x17B3),
    (0x17D7, 0x17D7),
    (0x17DC, 0x17DC),
    (0x1820, 0x1877),
    (0x1880, 0x18A8),
    (0x18AA, 0x18AA),
    (0x18B0, 0x18F5),
    (0x1900, 0x191C),
    (0x1950, 0x196D),
    (0x1970, 0x1974),
    (0x1980, 0x19AB),
    (0x19C1, 0x19C7),
    (0x1A00, 0x1A16),
    (0x1A20, 0x1A54),
    (0x1AA7, 0x1AA7),
    (0x1B05, 0x1B33),
    (0x1B45, 0x1B4B),
    (0x1B83, 0x1BA0),
    (0x1BAE, 0x1BAF),
    (0x1BBA, 0x1BE5),
    (0x1C00, 0x1C23),
    (0x1C4D, 0x1C4F),
    (0x1C5A, 0x1C7D),
    (0x1CE9, 0x1CEC),
    (0x1CEE, 0x1CF1),
    (0x1CF5, 0x1CF6),
    (0x1D00, 0x1DBF),
    (0x1E00, 0x1F15),
    (0x1F18, 0x1F1D),
    (0x1F20, 0x1F45),
    (0x1F48, 0x1F4D),
    (0x1F50, 0x1F57),
    (0x1F59, 0x1F59),
    (0x1F5B, 0x1F5B),
    (0x1F5D, 0x1F5D),
    (0x1F5F, 0x1F7D),
    (0x1F80, 0x1FB4),
    (0x1FB6, 0x1FBC),
    (0x1FBE, 0x1FBE),
    (0x1FC2, 0x1FC4),
    (0x1FC6, 0x1FCC),
    (0x1FD0, 0x1FD3),
    (0x1FD6, 0x1FDB),
    (0x1FE0, 0x1FEC),
    (0x1FF2, 0x1FF4),
    (0x1FF6, 0x1FFC),
    (0x2071, 0x2071),
    (0x207F, 0x207F),
    (0x2090, 0x209C),
    (0x2102, 0x2102),
    (0x2107, 0x2107),
    (0x210A, 0x2113),
    (0x2115, 0x2115),
    (0x2119, 0x211D),
    (0x2124, 0x2124),
    (0x2126, 0x2126),
    (0x2128, 0x2128),
    (0x212A, 0x212D),
    (0x212F, 0x2139),
    (0x213C, 0x213F),
    (0x2145, 0x2149),
    (0x214E, 0x214E),
    (0x2183, 0x2184),
    (0x2C00, 0x2C2E),
    (0x2C30, 0x2C5E),
    (0x2C60, 0x2CE4),
    (0x2CEB, 0x2CEE),
    (0x2CF2, 0x2CF3),
    (0x2D00, 0x2D25),
    (0x2D27, 0x2D27),
    (0x2D2D, 0x2D2D),
    (0x2D30, 0x2D67),
    (0x2D6F, 0x2D6F),
    (0x2D80, 0x2D96),
    (0x2DA0, 0x2DA6),
    (0x2DA8, 0x2DAE),
    (0x2DB0, 0x2DB6),
    (0x2DB8, 0x2DBE),
    (0x2DC0, 0x2DC6),
    (0x2DC8, 0x2DCE),
    (0x2DD0, 0x2DD6),
    (0x2DD8, 0x2DDE),
    (0x2E2F, 0x2E2F),
    (0x3005, 0x3006),
    (0x3031, 0x3035),
    (0x303B, 0x303C),
    (0x3041, 0x3096),
    (0x309D, 0x309F),
    (0x30A1, 0x30FA),
    (0x30FC, 0x30FF),
    (0x3105, 0x312D),
    (0x3131, 0x318E),
    (0x31A0, 0x31BA),
    (0x31F0, 0x31FF),
    (0x3400, 0x4DB5),
    (0x4E00, 0x9FCC),
    (0xA000, 0xA48C),
    (0xA4D0, 0xA4FD),
    (0xA500, 0xA60C),
    (0xA610, 0xA61F),
    (0xA62A, 0xA62B),
    (0xA640, 0xA66E),
    (0xA67F, 0xA697),
    (0xA6A0, 0xA6E5),
    (0xA717, 0xA71F),
    (0xA722, 0xA788),
    (0xA78B, 0xA78E),
    (0xA790, 0xA793),
    (0xA7A0, 0xA7AA),
    (0xA7F8, 0xA801),
    (0xA803, 0xA805),
    (0xA807, 0xA80A),
    (0xA80C, 0xA822),
    (0xA840, 0xA873),
    (0xA882, 0xA8B3),
    (0xA8F2, 0xA8F7),
    (0xA8FB, 0xA8FB),
    (0xA90A, 0xA925),
    (0xA930, 0xA946),
    (0xA960, 0xA97C),
    (0xA984, 0xA9B2),
    (0xA9CF, 0xA9CF),
    (0xAA00, 0xAA28),
    (0xAA40, 0xAA42),
    (0xAA44, 0xAA4B),
    (0xAA60, 0xAA76),
    (0xAA7A, 0xAA7A),
    (0xAA80, 0xAAAF),
    (0xAAB1, 0xAAB1),
    (0xAAB5, 0xAAB6),
    (0xAAB9, 0xAABD),
    (0xAAC0, 0xAAC0),
    (0xAAC2, 0xAAC2),
    (0xAADB, 0xAADD),
    (0xAAE0, 0xAAEA),
    (0xAAF2, 0xAAF4),
    (0xAB01, 0xAB06),
    (0xAB09, 0xAB0E),
    (0xAB11, 0xAB16),
    (0xAB20, 0xAB26),
    (0xAB28, 0xAB2E),
    (0xABC0, 0xABE2),
    (0xAC00, 0xD7A3),
    (0xD7B0, 0xD7C6),
    (0xD7CB, 0xD7FB),
    (0xF900, 0xFA6D),
    (0xFA70, 0xFAD9),
    (0xFB00, 0xFB06),
    (0xFB13, 0xFB17),
    (0xFB1D, 0xFB1D),
    (0xFB1F, 0xFB28),
    (0xFB2A, 0xFB36),
    (0xFB38, 0xFB3C),
    (0xFB3E, 0xFB3E),
    (0xFB40, 0xFB41),
    (0xFB43, 0xFB44),
    (0xFB46, 0xFBB1),
    (0xFBD3, 0xFD3D),
    (0xFD50, 0xFD8F),
    (0xFD92, 0xFDC7),
    (0xFDF0, 0xFDFB),
    (0xFE70, 0xFE74),
    (0xFE76, 0xFEFC),
    (0xFF21, 0xFF3A),
    (0xFF41, 0xFF5A),
    (0xFF66, 0xFFBE),
    (0xFFC2, 0xFFC7),
    (0xFFCA, 0xFFCF),
    (0xFFD2, 0xFFD7),
    (0xFFDA, 0xFFDC),
];

fn is_mermaid_unicode_text(ch: char) -> bool {
    let code = ch as u32;
    MERMAID_UNICODE_TEXT_RANGES
        .binary_search_by(|&(start, end)| {
            if code < start {
                std::cmp::Ordering::Greater
            } else if code > end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn non_ascii_id_char_len(input: &str, pos: usize) -> Option<usize> {
    if !input.is_char_boundary(pos) {
        return None;
    }
    let ch = input[pos..].chars().next()?;
    (!ch.is_ascii() && is_mermaid_unicode_text(ch)).then(|| ch.len_utf8())
}

pub(super) struct Lexer<'input> {
    pub(super) input: &'input str,
    pub(super) pos: usize,
    pub(super) pending: VecDeque<std::result::Result<(usize, Tok, usize), LexError>>,
    pub(super) allow_header_direction: bool,
    pub(super) recover_partial_node_labels: bool,
}

impl<'input> Lexer<'input> {
    pub(super) fn normalize_direction_token(dir: &str) -> &str {
        if dir == "TD" { "TB" } else { dir }
    }

    pub(super) fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            allow_header_direction: false,
            recover_partial_node_labels: false,
        }
    }

    pub(super) fn recovering(input: &'input str) -> Self {
        Self {
            recover_partial_node_labels: true,
            ..Self::new(input)
        }
    }

    pub(super) fn bump(&mut self) -> Option<u8> {
        if self.pos >= self.input.len() {
            return None;
        }
        let b = self.input.as_bytes()[self.pos];
        self.pos += 1;
        Some(b)
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    pub(super) fn peek2(&self) -> Option<[u8; 2]> {
        if self.pos + 1 >= self.input.len() {
            return None;
        }
        Some([
            self.input.as_bytes()[self.pos],
            self.input.as_bytes()[self.pos + 1],
        ])
    }

    pub(super) fn starts_with_kw(&self, kw: &str) -> bool {
        let rest = &self.input[self.pos..];
        if !rest.starts_with(kw) {
            return false;
        }
        let after = self.pos + kw.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        !b.is_ascii_alphanumeric() && b != b'_' && b != b'-'
    }

    pub(super) fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    pub(super) fn lex_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'\n' => {
                let bytes = self.input.as_bytes();
                let mut look = self.pos + 1;
                while look < bytes.len() {
                    match bytes[look] {
                        b' ' | b'\t' | b'\r' => look += 1,
                        _ => break,
                    }
                }
                if look < bytes.len() {
                    let is_linkish = match bytes[look] {
                        b'~' => {
                            look + 2 < bytes.len()
                                && bytes[look + 1] == b'~'
                                && bytes[look + 2] == b'~'
                        }
                        b'=' => look + 1 < bytes.len() && bytes[look + 1] == b'=',
                        b'-' => {
                            look + 1 < bytes.len()
                                && (bytes[look + 1] == b'-' || bytes[look + 1] == b'.')
                        }
                        b'o' | b'x' | b'<' => {
                            look + 2 < bytes.len()
                                && ((bytes[look + 1] == b'-'
                                    && (bytes[look + 2] == b'-' || bytes[look + 2] == b'.'))
                                    || (bytes[look + 1] == b'=' && bytes[look + 2] == b'='))
                        }
                        _ => false,
                    };
                    if is_linkish {
                        self.pos = look;
                        return None;
                    }
                }

                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            b';' => {
                self.pos += 1;
                Some((start, Tok::Sep, self.pos))
            }
            _ => None,
        }
    }

    pub(super) fn lex_comment(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let Some([b'%', b'%']) = self.peek2() else {
            return None;
        };
        // Consume until newline or EOF. If newline exists, emit Sep to keep statement boundaries.
        self.pos += 2;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                self.pos += 1;
                return Some((start, Tok::Sep, self.pos));
            }
            self.pos += 1;
        }
        None
    }

    pub(super) fn lex_direction(&mut self) -> Option<(usize, Tok, usize)> {
        if !self.allow_header_direction {
            return None;
        }
        let start = self.pos;
        let rest = &self.input[self.pos..];
        for d in ["TB", "TD", "BT", "LR", "RL"] {
            if rest.starts_with(d) {
                let after = self.pos + d.len();
                if after < self.input.len() {
                    let b = self.input.as_bytes()[after];
                    if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                        continue;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                let d = Self::normalize_direction_token(d);
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        if let Some(&b) = rest.as_bytes().first() {
            let mapped = match b {
                b'>' => Some("LR"),
                b'<' => Some("RL"),
                b'^' => Some("BT"),
                b'v' => Some("TB"),
                _ => None,
            };
            if let Some(d) = mapped {
                let after = self.pos + 1;
                if after < self.input.len() {
                    let next = self.input.as_bytes()[after];
                    if next.is_ascii_alphanumeric() || next == b'_' || next == b'-' {
                        return None;
                    }
                }
                self.pos = after;
                self.allow_header_direction = false;
                return Some((start, Tok::Direction(d.to_string()), self.pos));
            }
        }

        None
    }

    pub(super) fn lex_direction_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("direction") {
            return None;
        }
        self.pos += "direction".len();
        self.skip_ws();

        let direction_start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() || b == b';' {
                break;
            }
            self.pos += 1;
        }
        let direction_end = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        let statement_end = self.pos;
        let direction = &self.input[direction_start..direction_end];
        let selection = SourceSpan::new(direction_start, direction_end);

        let Some(dir) = ["TB", "TD", "BT", "LR", "RL"]
            .into_iter()
            .find(|candidate| *candidate == direction)
        else {
            let error = LexError::with_span("invalid flowchart direction", selection).expecting(
                crate::EditorExpectedSyntaxKind::FlowchartDirectionValue,
                selection,
            );
            if self.recover_partial_node_labels {
                return Some(Ok((
                    start,
                    Tok::DirectionStmt(DirectionStatementToken {
                        direction: String::new(),
                        selection,
                        recovery_error: Some(error),
                    }),
                    statement_end,
                )));
            }
            return Some(Err(error));
        };

        Some(Ok((
            start,
            Tok::DirectionStmt(DirectionStatementToken {
                direction: dir.to_string(),
                selection,
                recovery_error: None,
            }),
            statement_end,
        )))
    }

    pub(super) fn capture_to_stmt_end(&mut self) -> (usize, String, usize) {
        let start = self.pos;
        let mut in_double_quote = false;
        let mut in_single_quote = false;
        let mut escaped = false;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if in_double_quote {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_double_quote = false;
                }
                self.pos += 1;
                continue;
            }
            if in_single_quote {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'\'' {
                    in_single_quote = false;
                }
                self.pos += 1;
                continue;
            }

            if b == b'"' {
                in_double_quote = true;
                self.pos += 1;
                continue;
            }
            if b == b'\'' {
                in_single_quote = true;
                self.pos += 1;
                continue;
            }

            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        (start, self.input[start..self.pos].to_string(), self.pos)
    }

    pub(super) fn capture_to_stmt_end_from(&mut self, start: usize) -> (usize, String, usize) {
        self.pos = start;
        self.capture_to_stmt_end()
    }

    pub(super) fn capture_recovery_to_stmt_end_from(
        &mut self,
        start: usize,
    ) -> (usize, String, usize) {
        self.pos = start;
        while self.pos < self.input.len() {
            match self.input.as_bytes()[self.pos] {
                b'\n' | b';' => break,
                _ => self.pos += 1,
            }
        }
        (start, self.input[start..self.pos].to_string(), self.pos)
    }

    pub(super) fn lex_style_sep(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.input[self.pos..].starts_with(":::") {
            self.pos += 3;
            return Some((start, Tok::StyleSep, self.pos));
        }
        None
    }

    pub(super) fn lex_shape_data(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.input[self.pos..].starts_with("@{") {
            return None;
        }
        self.pos += 2;

        // Mermaid's Jison lexer has dedicated states for shapeData strings:
        // - it allows `}` inside double-quoted strings
        // - it rewrites `\n\s*` inside double-quoted strings to `<br/>`
        //
        // We mimic that behavior here while returning a single `ShapeData` token.
        let bytes = self.input.as_bytes();
        let mut out = String::new();
        let mut segment_start = self.pos;
        let mut in_string = false;

        while self.pos < self.input.len() {
            let b = bytes[self.pos];
            if !in_string {
                if b == b'"' {
                    out.push_str(&self.input[segment_start..self.pos + 1]);
                    self.pos += 1;
                    segment_start = self.pos;
                    in_string = true;
                    continue;
                }
                if b == b'}' {
                    out.push_str(&self.input[segment_start..self.pos]);
                    self.pos += 1;
                    return Some(Ok((start, Tok::ShapeData(out), self.pos)));
                }
                self.pos += 1;
                continue;
            }

            if b == b'"' {
                out.push_str(&self.input[segment_start..self.pos + 1]);
                self.pos += 1;
                segment_start = self.pos;
                in_string = false;
                continue;
            }

            if b == b'\n' {
                out.push_str(&self.input[segment_start..self.pos]);
                out.push_str("<br/>");
                self.pos += 1;
                while self.pos < self.input.len() {
                    match bytes[self.pos] {
                        b' ' | b'\t' | b'\r' => self.pos += 1,
                        _ => break,
                    }
                }
                segment_start = self.pos;
                continue;
            }

            self.pos += 1;
        }

        out.push_str(&self.input[segment_start..self.pos]);
        let span = SourceSpan::new(start, self.pos);
        let expected = super::shape_value_expected_span(self.input, start, self.pos)
            .unwrap_or(SourceSpan::new(self.pos, self.pos));
        Some(Err(LexError::with_span(
            "Unterminated shape data (missing `}`)",
            span,
        )
        .expecting(
            crate::EditorExpectedSyntaxKind::ShapeValue,
            expected,
        )))
    }

    pub(super) fn lex_edge_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if start >= self.input.len() {
            return None;
        }

        // Mermaid's `[^\s"]+@(?=[^\{"])` LINK_ID rule runs before ordinary identifiers and
        // greedily captures the last eligible `@` in a non-whitespace run.
        let mut cursor = start;
        let mut at = None;
        while cursor < self.input.len() {
            let ch = self.input[cursor..]
                .chars()
                .next()
                .expect("edge id scan position must contain a character");
            if is_ecmascript_trim_char(ch) || ch == '"' {
                break;
            }
            if ch == '@' {
                let next = self.input[cursor + 1..].chars().next();
                if cursor > start && matches!(next, Some(next) if next != '{' && next != '"') {
                    at = Some(cursor);
                }
            }
            cursor += ch.len_utf8();
        }

        let at = at?;
        self.pos = at + 1;
        Some((
            start,
            Tok::EdgeId(self.input[start..at].to_string()),
            self.pos,
        ))
    }

    pub(super) fn lex_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("style") {
            return None;
        }
        self.pos += "style".len();
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        let statement_span = SourceSpan::new(start, end);
        let (target, style) = directive_argument_spans(&rest, rest_start);
        let style_slot = trailing_value_slot(&rest, rest_start, style);
        match lex::parse_style_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_style_stmt_spans(&mut stmt, &rest, rest_start);
                stmt.editor_evidence = directive_editor_evidence(
                    statement_span,
                    target.map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span)),
                    style_slot.map(|span| (EditorExpectedSyntaxKind::StyleValue, span)),
                );
                Some(Ok((start, Tok::StyleStmt(stmt), end)))
            }
            Err(error) => Some(Err(directive_error(
                error,
                "style",
                statement_span,
                target
                    .map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span))
                    .into_iter()
                    .chain((target.is_none()).then_some((
                        EditorExpectedSyntaxKind::NodeIdentifier,
                        SourceSpan::new(end, end),
                    ))),
            ))),
        }
    }

    pub(super) fn lex_classdef_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("classDef") {
            return None;
        }
        self.pos += "classDef".len();
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        let statement_span = SourceSpan::new(start, end);
        let (class_name, style) = directive_argument_spans(&rest, rest_start);
        let style_slot = trailing_value_slot(&rest, rest_start, style);
        match lex::parse_classdef_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_classdef_stmt_spans(&mut stmt, &rest, rest_start);
                stmt.editor_evidence = directive_editor_evidence(
                    statement_span,
                    class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                    style_slot.map(|span| (EditorExpectedSyntaxKind::StyleValue, span)),
                );
                Some(Ok((start, Tok::ClassDefStmt(stmt), end)))
            }
            Err(error) => Some(Err(directive_error(
                error,
                "classDef",
                statement_span,
                class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
            ))),
        }
    }

    pub(super) fn lex_class_assign_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("class") {
            return None;
        }
        self.pos += "class".len();
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        let statement_span = SourceSpan::new(start, end);
        let (targets, class_name) = directive_argument_spans(&rest, rest_start);
        let class_name = active_following_span(&rest, rest_start, class_name);
        match lex::parse_class_assign_stmt(&rest) {
            Ok(mut stmt) => {
                lex::attach_class_assign_stmt_spans(&mut stmt, &rest, rest_start);
                stmt.editor_evidence = directive_editor_evidence(
                    statement_span,
                    targets.map(|span| (EditorExpectedSyntaxKind::IdList, span)),
                    class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span)),
                );
                Some(Ok((start, Tok::ClassAssignStmt(stmt), end)))
            }
            Err(error) => {
                let target = targets.unwrap_or_else(|| SourceSpan::new(end, end));
                Some(Err(directive_error(
                    error,
                    "class",
                    statement_span,
                    [(EditorExpectedSyntaxKind::IdList, target)]
                        .into_iter()
                        .chain(class_name.map(|span| (EditorExpectedSyntaxKind::ClassName, span))),
                )))
            }
        }
    }

    pub(super) fn lex_click_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("click") {
            return None;
        }
        self.pos += "click".len();
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        let statement_span = SourceSpan::new(start, end);
        let (target, _) = directive_argument_spans(&rest, rest_start);
        match lex::parse_click_stmt(&rest, rest_start) {
            Ok(mut stmt) => {
                stmt.editor_evidence = directive_editor_evidence(
                    statement_span,
                    target.map(|span| (EditorExpectedSyntaxKind::NodeIdentifier, span)),
                    None,
                );
                Some(Ok((start, Tok::ClickStmt(stmt), end)))
            }
            Err(error) => {
                let target = target.unwrap_or_else(|| SourceSpan::new(end, end));
                let error = directive_error(
                    error,
                    "click",
                    statement_span,
                    [(EditorExpectedSyntaxKind::NodeIdentifier, target)],
                );
                if self.recover_partial_node_labels && target.start < target.end {
                    Some(Ok((
                        start,
                        Tok::ClickStmt(ClickStmt {
                            ids: vec![self.input[target.start..target.end].to_string()],
                            id_spans: vec![target],
                            tooltip: None,
                            action: ClickAction::Callback,
                            editor_evidence: directive_editor_evidence(
                                statement_span,
                                Some((EditorExpectedSyntaxKind::NodeIdentifier, target)),
                                None,
                            ),
                            interaction_evidence: FlowchartClickEditorEvidence::default(),
                            recovery_error: Some(error),
                        }),
                        end,
                    )))
                } else {
                    Some(Err(error))
                }
            }
        }
    }

    pub(super) fn lex_link_style_stmt(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.starts_with_kw("linkStyle") {
            return None;
        }
        self.pos += "linkStyle".len();
        self.skip_ws();
        let (rest_start, rest, end) = self.capture_to_stmt_end();
        match lex::parse_link_style_stmt(&rest, rest_start) {
            Ok(stmt) => Some(Ok((start, Tok::LinkStyleStmt(stmt), end))),
            Err(e) => Some(Err(e)),
        }
    }

    pub(super) fn lex_subgraph_header_after_keyword(
        &mut self,
        keyword_start: usize,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        // Match Mermaid's flowchart parser behavior: it consumes a single "SPACE" token after the
        // `subgraph` keyword, while any additional whitespace becomes part of the subgraph header
        // token (`textNoTags`). This affects whether `FlowDB.addSubGraph(...)` decides to auto-generate
        // a `subGraphN` id.
        //
        // Example:
        // - `subgraph main`   -> header text has no whitespace, id stays `main`
        // - `subgraph  main`  -> header text begins with whitespace, id becomes `subGraphN`
        let rest = &self.input[self.pos..];
        if rest.starts_with('\n') || rest.starts_with("\r\n") || rest.starts_with(';') {
            return None;
        }
        if let Some(ch) = rest.chars().next()
            && is_ecmascript_trim_char(ch)
        {
            self.pos += ch.len_utf8();
        }

        let start = self.pos;
        if start >= self.input.len() {
            return None;
        }
        match self.input.as_bytes()[start] {
            b'\n' | b'\r' | b';' => return None,
            _ => {}
        }

        let mut in_quote = false;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if in_quote {
                if b == b'"' {
                    in_quote = false;
                }
                self.pos += 1;
                continue;
            }
            if b == b'"' {
                in_quote = true;
                self.pos += 1;
                continue;
            }
            if b == b'\n' || b == b'\r' || b == b';' || b == b'[' {
                break;
            }
            self.pos += 1;
        }

        let raw_id_end = self.pos;
        let raw_id = self.input[start..raw_id_end].to_string();
        let mut raw_title = raw_id.clone();
        let mut title_kind = TitleKind::Text;
        let mut id_equals_title = true;

        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'[' {
            id_equals_title = false;
            self.pos += 1;
            let title_start = self.pos;
            in_quote = false;
            while self.pos < self.input.len() {
                let b = self.input.as_bytes()[self.pos];
                if in_quote {
                    if b == b'"' {
                        in_quote = false;
                    }
                    self.pos += 1;
                    continue;
                }
                if b == b'"' {
                    in_quote = true;
                    self.pos += 1;
                    continue;
                }
                if b == b']' {
                    break;
                }
                if b == b'\n' || b == b'\r' {
                    break;
                }
                self.pos += 1;
            }
            raw_title = self.input[title_start..self.pos].to_string();
            let parsed_title = match lex::parse_node_label_text(&raw_title) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Some(Err(LexError::with_span(
                        error.message,
                        SourceSpan::new(title_start, self.pos),
                    )));
                }
            };
            title_kind = parsed_title.kind;
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b']' {
                self.pos += 1;
            }
        } else if raw_id.contains('"') && !(raw_id.starts_with('"') && raw_id.ends_with('"')) {
            return Some(Err(LexError::with_span(
                "Invalid subgraph header: quoted strings cannot be mixed with unquoted text",
                SourceSpan::new(start, raw_id_end),
            )));
        }

        Some(Ok((
            start,
            Tok::SubgraphHeader(SubgraphHeader {
                raw_id,
                header_span: Some(SourceSpan::new(keyword_start, self.pos)),
                raw_id_span: Some(SourceSpan::new(start, raw_id_end)),
                raw_title,
                title_kind,
                id_equals_title,
            }),
            self.pos,
        )))
    }

    pub(super) fn lex_amp(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b'&' {
            return None;
        }
        self.pos += 1;
        Some((start, Tok::Amp, self.pos))
    }

    pub(super) fn lex_id(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if start >= bytes.len() {
            return None;
        }
        let first = bytes[start];
        if first.is_ascii_alphanumeric() || first == b'_' {
            self.pos += 1;
        } else if let Some(len) = non_ascii_id_char_len(self.input, start) {
            self.pos += len;
        } else {
            return None;
        }

        while self.pos < bytes.len() {
            if self.pos + 1 < bytes.len()
                && (bytes[self.pos] == b'-' && bytes[self.pos + 1] == b'-'
                    || bytes[self.pos] == b'=' && bytes[self.pos + 1] == b'=')
            {
                break;
            }
            let b = bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
                continue;
            }
            if let Some(len) = non_ascii_id_char_len(self.input, self.pos) {
                self.pos += len;
                continue;
            }
            if b == b'-' {
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                // Dotted edges start with `-.` (e.g. `A-.->B`). Avoid consuming the link start as
                // part of the id while still allowing ids like `subcontainer-child`.
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'.' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            if b == b'.' {
                // Allow dots inside ids (Mermaid supports nodes like `P1.5`), but avoid consuming
                // the `.` that starts a dotted link token like `.->` when it is directly adjacent
                // to an id (e.g. `A.->B`).
                if self.pos + 1 < bytes.len() && bytes[self.pos + 1] == b'-' {
                    break;
                }
                self.pos += 1;
                continue;
            }
            break;
        }

        if self.pos <= start {
            return None;
        }

        let id = self.input[start..self.pos].to_string();
        Some((start, Tok::Id(id), self.pos))
    }

    pub(super) fn lex_arrow_and_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let bytes = self.input.as_bytes();

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum LinkFamily {
            Normal,
            Thick,
            Dotted,
            Invisible,
        }

        struct LinkEndMatch {
            label_end: usize,
            match_end: usize,
            operator: String,
            operator_span: SourceSpan,
        }

        struct StartLinkMatch {
            family: LinkFamily,
            operator: String,
            operator_span: SourceSpan,
            match_end: usize,
        }

        let match_link_end = |pos: usize, family: LinkFamily| -> Option<LinkEndMatch> {
            let len = bytes.len();
            let label_end = pos;
            let operator_start = skip_ecmascript_whitespace(self.input, pos);
            if operator_start >= len {
                return None;
            }

            let mut cur = operator_start;
            let start_marker = bytes[cur];
            if matches!(start_marker, b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            match family {
                LinkFamily::Invisible => {
                    cur = operator_start;
                    let mut tildes = 0usize;
                    while cur < len && bytes[cur] == b'~' {
                        tildes += 1;
                        cur += 1;
                    }
                    if tildes < 3 {
                        return None;
                    }
                }
                LinkFamily::Normal => {
                    let hyphen_start = cur;
                    while cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let hyphens = cur - hyphen_start;
                    if hyphens < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `--+` + `-` requires at least 3 hyphens total.
                                if hyphens < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if hyphens < 3 {
                        return None;
                    }
                }
                LinkFamily::Thick => {
                    let eq_start = cur;
                    while cur < len && bytes[cur] == b'=' {
                        cur += 1;
                    }
                    let eqs = cur - eq_start;
                    if eqs < 2 {
                        return None;
                    }
                    if cur < len {
                        match bytes[cur] {
                            b'x' | b'o' | b'>' => {
                                cur += 1;
                            }
                            _ => {
                                // Open-ended edge: `==+` + `=` requires at least 3 '=' total.
                                if eqs < 3 {
                                    return None;
                                }
                            }
                        }
                    } else if eqs < 3 {
                        return None;
                    }
                }
                LinkFamily::Dotted => {
                    if cur < len && bytes[cur] == b'-' {
                        cur += 1;
                    }
                    let mut dots = 0usize;
                    while cur < len && bytes[cur] == b'.' {
                        dots += 1;
                        cur += 1;
                    }
                    if dots == 0 {
                        return None;
                    }
                    if cur >= len || bytes[cur] != b'-' {
                        return None;
                    }
                    cur += 1;
                    if cur < len && matches!(bytes[cur], b'x' | b'o' | b'>') {
                        cur += 1;
                    }
                }
            }

            let operator_end = cur;
            let match_end = skip_ecmascript_whitespace(self.input, operator_end);
            Some(LinkEndMatch {
                label_end,
                match_end,
                operator: self.input[operator_start..operator_end].to_string(),
                operator_span: SourceSpan::new(operator_start, operator_end),
            })
        };

        let compute_link =
            |end: String, start: Option<String>| -> std::result::Result<LinkToken, LexError> {
                let end_semantics = if start.is_some() {
                    destruct_labeled_end_link(&end)
                } else {
                    destruct_end_link(&end)
                };
                let mut start_marker = end_semantics.start_marker;

                if let Some(start_str) = start.as_deref() {
                    let start_semantics = destruct_start_link(start_str);
                    if start_semantics.stroke_kind != end_semantics.stroke_kind {
                        return Err(LexError::new(
                            "Invalid link: stroke mismatch between start and end".to_string(),
                        ));
                    }

                    start_marker = start_semantics.marker;
                }

                Ok(LinkToken {
                    end,
                    start_marker,
                    end_marker: end_semantics.end_marker,
                    stroke_kind: end_semantics.stroke_kind,
                    visibility: end_semantics.visibility,
                    length: end_semantics.length,
                })
            };

        // 1) Prefer full LINK tokens, matching their source-order priority before START_LINK.
        let families = [
            LinkFamily::Invisible,
            LinkFamily::Thick,
            LinkFamily::Normal,
            LinkFamily::Dotted,
        ];
        for family in families {
            if let Some(link_match) = match_link_end(self.pos, family) {
                self.pos = link_match.match_end;
                let arrow_end = link_match.match_end;
                let link = match compute_link(link_match.operator, None) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                let arrow = ArrowToken {
                    link,
                    recovery_error: None,
                };

                // Optional pipe label: `A--x|label|B` or `A --> |label| B`.
                let pipe_pos = self.pos;
                if pipe_pos < self.input.len() && bytes[pipe_pos] == b'|' {
                    self.pos = pipe_pos + 1;
                    let label_start = self.pos;
                    if let Some(label_end) = find_pipe_label_end(self.input, label_start) {
                        self.pos = label_end;
                        let raw = &self.input[label_start..self.pos];
                        let raw_span = SourceSpan::new(label_start, self.pos);
                        let parsed = match lex::parse_node_label_text(raw) {
                            Ok(parsed) => parsed,
                            Err(error) => {
                                self.pos += 1;
                                return Some(Err(LexError::with_span(error.message, raw_span)));
                            }
                        };
                        self.pos += 1;
                        let token_span = SourceSpan::new(pipe_pos, self.pos);
                        let label = labeled_text_with_spans(
                            self.input,
                            LabeledText {
                                text: parsed.text,
                                kind: parsed.kind,
                                span: None,
                                selection: None,
                            },
                            token_span,
                            raw_span,
                        );
                        self.pending
                            .push_back(Ok((pipe_pos, Tok::EdgeLabel(label), self.pos)));
                    } else {
                        self.pos = self.input.len();
                        let error = LexError::with_span(
                            "Unterminated flowchart pipe edge label",
                            SourceSpan::new(pipe_pos, self.pos),
                        )
                        .expecting(
                            crate::EditorExpectedSyntaxKind::Payload,
                            SourceSpan::new(self.pos, self.pos),
                        );
                        if self.recover_partial_node_labels {
                            let mut arrow = arrow;
                            arrow.recovery_error = Some(error);
                            return Some(Ok((start, Tok::Arrow(arrow), self.pos)));
                        }
                        return Some(Err(error));
                    }
                }

                return Some(Ok((start, Tok::Arrow(arrow), arrow_end)));
            }
        }

        // 2) START_LINK + edgeText + LINK (new notation): A-- text -->B
        let parse_start_link = |pos: usize| -> Option<StartLinkMatch> {
            let len = bytes.len();
            let operator_start = skip_ecmascript_whitespace(self.input, pos);
            if operator_start >= len {
                return None;
            }
            let mut cur = operator_start;
            if matches!(bytes[cur], b'x' | b'o' | b'<') {
                cur += 1;
                if cur >= len {
                    return None;
                }
            }

            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'-' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Normal,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            if cur + 1 < len && bytes[cur] == b'=' && bytes[cur + 1] == b'=' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Thick,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            if cur + 1 < len && bytes[cur] == b'-' && bytes[cur + 1] == b'.' {
                cur += 2;
                return Some(StartLinkMatch {
                    family: LinkFamily::Dotted,
                    operator: self.input[operator_start..cur].to_string(),
                    operator_span: SourceSpan::new(operator_start, cur),
                    match_end: skip_ecmascript_whitespace(self.input, cur),
                });
            }
            None
        };

        let Some(start_match) = parse_start_link(self.pos) else {
            let operator_start = skip_ecmascript_whitespace(self.input, self.pos);
            if self.input[operator_start..].starts_with("->") {
                self.pos = operator_start + 2;
                let selection = SourceSpan::new(operator_start, self.pos);
                return Some(Err(LexError::with_span(
                    "incomplete flowchart edge operator",
                    selection,
                )
                .expecting(
                    crate::EditorExpectedSyntaxKind::FlowchartOperator,
                    selection,
                )));
            }
            return None;
        };
        let family = start_match.family;
        let after_start = start_match.match_end;
        let edge_text_start = after_start;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum EdgeTextState {
            Plain,
            String,
            MarkdownString,
        }

        let arrow_token =
            |link: LinkToken, _end_span: SourceSpan, recovery_error: Option<LexError>| ArrowToken {
                link,
                recovery_error,
            };

        let mut scan = edge_text_start;
        let mut state = EdgeTextState::Plain;
        while scan < self.input.len() {
            let rest = &self.input[scan..];
            match state {
                EdgeTextState::Plain => {
                    if rest.starts_with("\"`") {
                        state = EdgeTextState::MarkdownString;
                        scan += 2;
                        continue;
                    }
                    if rest.starts_with('"') {
                        state = EdgeTextState::String;
                        scan += 1;
                        continue;
                    }
                    let whitespace_end = skip_ecmascript_whitespace(self.input, scan);
                    if let Some(link_match) = match_link_end(scan, family) {
                        let match_start = link_match.label_end;
                        let match_end = link_match.match_end;
                        let raw_text = &self.input[edge_text_start..match_start];
                        let raw_span = SourceSpan::new(edge_text_start, match_start);
                        self.pos = match_end;

                        let parsed = match lex::parse_edge_text(raw_text) {
                            Ok(parsed) => parsed,
                            Err(error) => {
                                let error = LexError::with_span(error.message, raw_span)
                                    .expecting(crate::EditorExpectedSyntaxKind::Payload, raw_span);
                                if self.recover_partial_node_labels
                                    && let Ok(link) = compute_link(
                                        link_match.operator,
                                        Some(start_match.operator.clone()),
                                    )
                                {
                                    let arrow =
                                        arrow_token(link, link_match.operator_span, Some(error));
                                    return Some(Ok((start, Tok::Arrow(arrow), match_end)));
                                }
                                return Some(Err(error));
                            }
                        };
                        let link = match compute_link(
                            link_match.operator,
                            Some(start_match.operator.clone()),
                        ) {
                            Ok(v) => v,
                            Err(e) => return Some(Err(e)),
                        };
                        let arrow = arrow_token(link, link_match.operator_span, None);

                        let label = labeled_text_with_spans(
                            self.input,
                            LabeledText {
                                text: parsed.text,
                                kind: parsed.kind,
                                span: None,
                                selection: None,
                            },
                            SourceSpan::new(edge_text_start, match_start),
                            raw_span,
                        );
                        self.pending.push_back(Ok((
                            edge_text_start,
                            Tok::EdgeLabel(label),
                            match_end,
                        )));
                        return Some(Ok((start, Tok::Arrow(arrow), after_start)));
                    }

                    if whitespace_end != scan {
                        // The terminator probe already inspected this whole whitespace run. Skip
                        // it once instead of probing every suffix, which would make long internal
                        // whitespace in an edge label quadratic.
                        scan = whitespace_end;
                        continue;
                    }

                    let invalid_edge_text = match family {
                        LinkFamily::Normal => rest.starts_with("--"),
                        LinkFamily::Thick => rest.starts_with('='),
                        LinkFamily::Dotted => rest.starts_with('.'),
                        LinkFamily::Invisible => false,
                    };
                    if invalid_edge_text {
                        let ch = rest
                            .chars()
                            .next()
                            .expect("edge label scan position must contain a character");
                        let error_end = scan + ch.len_utf8();
                        self.pos = error_end;
                        return Some(Err(LexError::with_span(
                            "Invalid character sequence in flowchart edge label",
                            SourceSpan::new(scan, error_end),
                        )));
                    }
                }
                EdgeTextState::String => {
                    if rest.starts_with('"') {
                        state = EdgeTextState::Plain;
                        scan += 1;
                        continue;
                    }
                }
                EdgeTextState::MarkdownString => {
                    if rest.starts_with("`\"") {
                        state = EdgeTextState::Plain;
                        scan += 2;
                        continue;
                    }
                    if rest.starts_with(['`', '"']) {
                        let ch = rest
                            .chars()
                            .next()
                            .expect("edge label scan position must contain a character");
                        let error_end = scan + ch.len_utf8();
                        self.pos = error_end;
                        return Some(Err(LexError::with_span(
                            "Invalid Markdown string in flowchart edge label",
                            SourceSpan::new(scan, error_end),
                        )));
                    }
                }
            }
            let ch = self.input[scan..]
                .chars()
                .next()
                .expect("edge label scan position must contain a character");
            scan += ch.len_utf8();
        }

        self.pos = self.input.len();
        Some(Err(LexError::with_span(
            "Unterminated edge label (missing link terminator)",
            SourceSpan::new(edge_text_start, self.pos),
        )
        .expecting(
            crate::EditorExpectedSyntaxKind::FlowchartOperator,
            SourceSpan::new(start_match.operator_span.start, after_start),
        )))
    }

    pub(super) fn lex_node_label(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        let rest = &self.input[self.pos..];

        if rest.starts_with("[\\") {
            let open = "[\\";
            let content_start = self.pos + open.len();
            let end_slash = lex::find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = lex::find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    if self.recover_partial_node_labels {
                        let (raw_start, raw, token_end) =
                            self.capture_recovery_to_stmt_end_from(content_start);
                        let token = build_partial_node_label_token_from_raw(
                            self.input,
                            "inv_trapezoid",
                            SourceSpan::new(start, token_end),
                            SourceSpan::new(content_start, token_end),
                            &raw,
                            SourceSpan::new(raw_start, token_end),
                            PartialNodeLabelRecovery {
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        "Unterminated node label (missing `/]` or `\\]`)",
                        SourceSpan::new(start, token_end),
                    )));
                }
                (Some(p), None) => (p, "/]", "inv_trapezoid"),
                (None, Some(p)) => (p, "\\]", "lean_left"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "inv_trapezoid")
                    } else {
                        (b, "\\]", "lean_left")
                    }
                }
            };

            let token_end = end_start + close.len();
            let token = match build_node_label_token(
                self.input,
                shape,
                SourceSpan::new(start, token_end),
                SourceSpan::new(content_start, end_start),
                None,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.pos = token_end;
                    return Some(Err(e));
                }
            };
            self.pos = token_end;
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("[/") {
            let open = "[/";
            let content_start = self.pos + open.len();
            let end_slash = lex::find_unquoted_delim(self.input, content_start, "/]");
            let end_backslash = lex::find_unquoted_delim(self.input, content_start, "\\]");

            let (end_start, close, shape) = match (end_slash, end_backslash) {
                (None, None) => {
                    if self.recover_partial_node_labels {
                        let (raw_start, raw, token_end) =
                            self.capture_recovery_to_stmt_end_from(content_start);
                        let token = build_partial_node_label_token_from_raw(
                            self.input,
                            "lean_right",
                            SourceSpan::new(start, token_end),
                            SourceSpan::new(content_start, token_end),
                            &raw,
                            SourceSpan::new(raw_start, token_end),
                            PartialNodeLabelRecovery {
                                trigger_span: Some(SourceSpan::new(start, content_start)),
                                error: LexError::with_span(
                                    "Unterminated node label (missing `/]` or `\\]`)",
                                    SourceSpan::new(start, token_end),
                                ),
                            },
                        );
                        self.pos = token_end;
                        return Some(Ok((start, token, self.pos)));
                    }
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        "Unterminated node label (missing `/]` or `\\]`)",
                        SourceSpan::new(start, token_end),
                    )));
                }
                (Some(p), None) => (p, "/]", "lean_right"),
                (None, Some(p)) => (p, "\\]", "trapezoid"),
                (Some(a), Some(b)) => {
                    if a <= b {
                        (a, "/]", "lean_right")
                    } else {
                        (b, "\\]", "trapezoid")
                    }
                }
            };

            let token_end = end_start + close.len();
            let token = match build_node_label_token(
                self.input,
                shape,
                SourceSpan::new(start, token_end),
                SourceSpan::new(content_start, end_start),
                None,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.pos = token_end;
                    return Some(Err(e));
                }
            };
            self.pos = token_end;
            return Some(Ok((start, token, self.pos)));
        }

        let candidates: [(&str, &str, &str); 8] = [
            ("(((", ")))", "doublecircle"),
            ("{{", "}}", "hexagon"),
            ("[[", "]]", "subroutine"),
            ("(-", "-)", "ellipse"),
            ("([", "])", "stadium"),
            ("[(", ")]", "cylinder"),
            ("((", "))", "circle"),
            (">", "]", "odd"),
        ];

        for (open, close, shape) in candidates {
            if !rest.starts_with(open) {
                continue;
            }
            let content_start = self.pos + open.len();
            let token = if let Some(end_start) =
                lex::find_unquoted_delim(self.input, content_start, close)
            {
                let token_end = end_start + close.len();
                let token = match build_node_label_token(
                    self.input,
                    shape,
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, end_start),
                    None,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        self.pos = token_end;
                        return Some(Err(e));
                    }
                };
                self.pos = token_end;
                token
            } else {
                if !self.recover_partial_node_labels {
                    let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                    self.pos = token_end;
                    return Some(Err(LexError::with_span(
                        format!("Unterminated node label (missing `{close}`)"),
                        SourceSpan::new(start, token_end),
                    )));
                }
                let (raw_start, raw, token_end) =
                    self.capture_recovery_to_stmt_end_from(content_start);
                let token = build_partial_node_label_token_from_raw(
                    self.input,
                    shape,
                    SourceSpan::new(start, token_end),
                    SourceSpan::new(content_start, token_end),
                    &raw,
                    SourceSpan::new(raw_start, token_end),
                    PartialNodeLabelRecovery {
                        trigger_span: Some(SourceSpan::new(start, content_start)),
                        error: LexError::with_span(
                            format!("Unterminated node label (missing `{close}`)"),
                            SourceSpan::new(start, token_end),
                        ),
                    },
                );
                self.pos = token_end;
                token
            };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("[") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, "]") {
                    let token_end = end_start + 1;
                    let raw = &self.input[content_start..end_start];
                    let raw_span = SourceSpan::new(content_start, end_start);
                    let (shape, label_raw, label_offset) = lex::parse_rect_border_label(raw);
                    let label_span = SourceSpan::new(
                        raw_span.start + label_offset,
                        raw_span.start + label_offset + label_raw.len(),
                    );
                    let token = match build_node_label_token_from_raw(
                        self.input,
                        shape,
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        label_raw,
                        label_span,
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `]`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let (shape, label_raw, label_offset) = lex::parse_rect_border_label(&raw);
                    let label_span = SourceSpan::new(
                        raw_start + label_offset,
                        raw_start + label_offset + label_raw.len(),
                    );
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        shape,
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        label_raw,
                        label_span,
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `]`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("{") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, "}") {
                    let token_end = end_start + 1;
                    let token = match build_node_label_token(
                        self.input,
                        "diamond",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `}`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        "diamond",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        &raw,
                        SourceSpan::new(raw_start, token_end),
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `}`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        if rest.starts_with("(") {
            let content_start = self.pos + 1;
            let token =
                if let Some(end_start) = lex::find_unquoted_delim(self.input, content_start, ")") {
                    let token_end = end_start + 1;
                    let token = match build_node_label_token(
                        self.input,
                        "round",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, end_start),
                        None,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            self.pos = token_end;
                            return Some(Err(e));
                        }
                    };
                    self.pos = token_end;
                    token
                } else {
                    if !self.recover_partial_node_labels {
                        let (_, _, token_end) = self.capture_to_stmt_end_from(content_start);
                        self.pos = token_end;
                        return Some(Err(LexError::with_span(
                            "Unterminated node label (missing `)`)",
                            SourceSpan::new(start, token_end),
                        )));
                    }
                    let (raw_start, raw, token_end) =
                        self.capture_recovery_to_stmt_end_from(content_start);
                    let token = build_partial_node_label_token_from_raw(
                        self.input,
                        "round",
                        SourceSpan::new(start, token_end),
                        SourceSpan::new(content_start, token_end),
                        &raw,
                        SourceSpan::new(raw_start, token_end),
                        PartialNodeLabelRecovery {
                            trigger_span: Some(SourceSpan::new(start, content_start)),
                            error: LexError::with_span(
                                "Unterminated node label (missing `)`)",
                                SourceSpan::new(start, token_end),
                            ),
                        },
                    );
                    self.pos = token_end;
                    token
                };
            return Some(Ok((start, token, self.pos)));
        }

        None
    }
}

fn build_node_label_token(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    content_span: SourceSpan,
    trigger_span: Option<SourceSpan>,
) -> std::result::Result<Tok, LexError> {
    let raw = &input[content_span.start..content_span.end];
    let raw_span = content_span;
    build_node_label_token_from_raw(
        input,
        shape,
        token_span,
        content_span,
        raw,
        raw_span,
        trigger_span,
    )
}

fn build_node_label_token_from_raw(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    _content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    trigger_span: Option<SourceSpan>,
) -> std::result::Result<Tok, LexError> {
    let text = lex::parse_node_label_text(raw)?;
    Ok(Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(input, text, token_span, raw_span),
        trigger_span,
        recovery_error: None,
    }))
}

struct PartialNodeLabelRecovery {
    trigger_span: Option<SourceSpan>,
    error: LexError,
}

fn build_partial_node_label_token_from_raw(
    input: &str,
    shape: &str,
    token_span: SourceSpan,
    _content_span: SourceSpan,
    raw: &str,
    raw_span: SourceSpan,
    recovery: PartialNodeLabelRecovery,
) -> Tok {
    let (text, kind) = parse_label_text(raw);
    Tok::NodeLabel(NodeLabelToken {
        shape: shape.to_string(),
        text: labeled_text_with_spans(
            input,
            LabeledText {
                text,
                kind,
                span: None,
                selection: None,
            },
            token_span,
            raw_span,
        ),
        trigger_span: recovery.trigger_span,
        recovery_error: Some(recovery.error),
    })
}

fn labeled_text_with_spans(
    input: &str,
    mut text: LabeledText,
    token_span: SourceSpan,
    content_span: SourceSpan,
) -> LabeledText {
    text.span = Some(token_span);
    text.selection = label_value_selection(input, content_span, &text.text).or(Some(content_span));
    text
}

fn label_value_selection(input: &str, content_span: SourceSpan, value: &str) -> Option<SourceSpan> {
    if value.is_empty() {
        return None;
    }
    source_value_span(input, content_span, value)
}
