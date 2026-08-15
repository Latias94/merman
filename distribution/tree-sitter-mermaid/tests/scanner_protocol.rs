use std::ffi::{c_char, c_void};

const TOKEN_COUNT: usize = 21;
const SERIALIZATION_BUFFER_SIZE: usize = 1024;
const MAX_SERIALIZED_SIZE: usize = 526;
const MAX_INDENTATION: usize = 65_534;

const MINDMAP_START: u16 = 0;
const MINDMAP_INDENT: u16 = 1;
const MINDMAP_REINDENT: u16 = 2;
const MINDMAP_DEDENT: u16 = 3;
const MINDMAP_OVERFLOW: u16 = 4;
const TREEMAP_START: u16 = 5;
const TREEMAP_INDENT: u16 = 6;
const TREEMAP_REINDENT: u16 = 7;
const TREEMAP_DEDENT: u16 = 8;
const TREEMAP_OVERFLOW: u16 = 9;
const TREE_VIEW_START: u16 = 10;
const TREE_VIEW_INDENT: u16 = 11;
const TREE_VIEW_REINDENT: u16 = 12;
const TREE_VIEW_DEDENT: u16 = 13;
const TREE_VIEW_OVERFLOW: u16 = 14;
const KANBAN_START: u16 = 15;
const KANBAN_INDENT: u16 = 16;
const KANBAN_REINDENT: u16 = 17;
const KANBAN_DEDENT: u16 = 18;
const KANBAN_OVERFLOW: u16 = 19;
const END_OF_INPUT: u16 = 20;

#[derive(Clone, Copy)]
struct TokenGroup {
    start: u16,
    indent: u16,
    reindent: u16,
    dedent: u16,
    overflow: u16,
    marker: &'static [u8],
}

const TOKEN_GROUPS: [TokenGroup; 4] = [
    TokenGroup {
        start: MINDMAP_START,
        indent: MINDMAP_INDENT,
        reindent: MINDMAP_REINDENT,
        dedent: MINDMAP_DEDENT,
        overflow: MINDMAP_OVERFLOW,
        marker: b"Node",
    },
    TokenGroup {
        start: TREEMAP_START,
        indent: TREEMAP_INDENT,
        reindent: TREEMAP_REINDENT,
        dedent: TREEMAP_DEDENT,
        overflow: TREEMAP_OVERFLOW,
        marker: b"\"Node\"",
    },
    TokenGroup {
        start: TREE_VIEW_START,
        indent: TREE_VIEW_INDENT,
        reindent: TREE_VIEW_REINDENT,
        dedent: TREE_VIEW_DEDENT,
        overflow: TREE_VIEW_OVERFLOW,
        marker: b"Node",
    },
    TokenGroup {
        start: KANBAN_START,
        indent: KANBAN_INDENT,
        reindent: KANBAN_REINDENT,
        dedent: KANBAN_DEDENT,
        overflow: KANBAN_OVERFLOW,
        marker: b"Node",
    },
];

#[repr(C)]
struct TsLexer {
    lookahead: i32,
    result_symbol: u16,
    advance: unsafe extern "C" fn(*mut TsLexer, bool),
    mark_end: unsafe extern "C" fn(*mut TsLexer),
    get_column: unsafe extern "C" fn(*mut TsLexer) -> u32,
    is_at_included_range_start: unsafe extern "C" fn(*const TsLexer) -> bool,
    eof: unsafe extern "C" fn(*const TsLexer) -> bool,
    log: *const c_void,
}

#[repr(C)]
struct MockLexer {
    lexer: TsLexer,
    input: *const u8,
    input_length: usize,
    position: usize,
    marked_end: usize,
    lookahead_width: usize,
}

impl MockLexer {
    fn new(input: &[u8]) -> Self {
        let (lookahead, lookahead_width) = decode_lookahead(input);
        Self {
            lexer: TsLexer {
                lookahead,
                result_symbol: u16::MAX,
                advance: mock_advance,
                mark_end: mock_mark_end,
                get_column: mock_get_column,
                is_at_included_range_start: mock_is_at_included_range_start,
                eof: mock_eof,
                log: std::ptr::null(),
            },
            input: input.as_ptr(),
            input_length: input.len(),
            position: 0,
            marked_end: 0,
            lookahead_width,
        }
    }
}

fn decode_lookahead(input: &[u8]) -> (i32, usize) {
    let Some(&first) = input.first() else {
        return (0, 0);
    };
    let width = match first {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return (0xfffd, 1),
    };
    let Some(encoded) = input.get(..width) else {
        return (0xfffd, 1);
    };
    std::str::from_utf8(encoded)
        .ok()
        .and_then(|text| text.chars().next())
        .map_or((0xfffd, 1), |character| (character as i32, width))
}

unsafe extern "C" fn mock_advance(lexer: *mut TsLexer, _skip: bool) {
    // SAFETY: TsLexer is the first field of every MockLexer passed to the scanner.
    let mock = unsafe { &mut *lexer.cast::<MockLexer>() };
    if mock.position < mock.input_length {
        mock.position += mock.lookahead_width.max(1);
    }
    let remaining = if mock.position < mock.input_length {
        // SAFETY: position is checked against the input length and the original input remains live.
        unsafe {
            std::slice::from_raw_parts(
                mock.input.add(mock.position),
                mock.input_length - mock.position,
            )
        }
    } else {
        &[]
    };
    (mock.lexer.lookahead, mock.lookahead_width) = decode_lookahead(remaining);
}

unsafe extern "C" fn mock_mark_end(lexer: *mut TsLexer) {
    // SAFETY: TsLexer is the first field of every MockLexer passed to the scanner.
    let mock = unsafe { &mut *lexer.cast::<MockLexer>() };
    mock.marked_end = mock.position;
}

unsafe extern "C" fn mock_get_column(lexer: *mut TsLexer) -> u32 {
    // SAFETY: TsLexer is the first field of every MockLexer passed to the scanner.
    let mock = unsafe { &*lexer.cast::<MockLexer>() };
    u32::try_from(mock.position).unwrap_or(u32::MAX)
}

unsafe extern "C" fn mock_is_at_included_range_start(_lexer: *const TsLexer) -> bool {
    false
}

unsafe extern "C" fn mock_eof(lexer: *const TsLexer) -> bool {
    // SAFETY: TsLexer is the first field of every MockLexer passed to the scanner.
    let mock = unsafe { &*lexer.cast::<MockLexer>() };
    mock.position >= mock.input_length
}

unsafe extern "C" {
    fn tree_sitter_mermaid_external_scanner_create() -> *mut c_void;
    fn tree_sitter_mermaid_external_scanner_destroy(payload: *mut c_void);
    fn tree_sitter_mermaid_external_scanner_scan(
        payload: *mut c_void,
        lexer: *mut TsLexer,
        valid_symbols: *const bool,
    ) -> bool;
    fn tree_sitter_mermaid_external_scanner_serialize(
        payload: *mut c_void,
        buffer: *mut c_char,
    ) -> u32;
    fn tree_sitter_mermaid_external_scanner_deserialize(
        payload: *mut c_void,
        buffer: *const c_char,
        length: u32,
    );
}

struct Scanner(*mut c_void);

impl Scanner {
    fn new() -> Self {
        let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
        assert_eq!(language.abi_version(), 14);

        // SAFETY: The scanner constructor has no preconditions.
        let scanner = unsafe { tree_sitter_mermaid_external_scanner_create() };
        assert!(!scanner.is_null());
        Self(scanner)
    }

    fn scan(&mut self, input: &[u8], valid_symbols: &[bool; TOKEN_COUNT]) -> ScanResult {
        let mut lexer = MockLexer::new(input);
        // SAFETY: The scanner, lexer, and symbol mask remain valid for the duration of the call.
        let matched = unsafe {
            tree_sitter_mermaid_external_scanner_scan(
                self.0,
                &mut lexer.lexer,
                valid_symbols.as_ptr(),
            )
        };
        ScanResult {
            matched,
            symbol: lexer.lexer.result_symbol,
            marked_end: lexer.marked_end,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = [0_u8; SERIALIZATION_BUFFER_SIZE];
        // SAFETY: Tree-sitter guarantees this buffer size to external scanners.
        let length = unsafe {
            tree_sitter_mermaid_external_scanner_serialize(
                self.0,
                buffer.as_mut_ptr().cast::<c_char>(),
            )
        } as usize;
        assert!(length <= SERIALIZATION_BUFFER_SIZE);
        buffer[..length].to_vec()
    }

    fn deserialize(&mut self, bytes: &[u8]) {
        // SAFETY: The byte slice remains valid for the duration of the call.
        unsafe {
            tree_sitter_mermaid_external_scanner_deserialize(
                self.0,
                bytes.as_ptr().cast::<c_char>(),
                u32::try_from(bytes.len()).expect("test state length fits in u32"),
            );
        }
    }

    fn reset(&mut self) {
        // SAFETY: A null buffer with zero length is the scanner ABI reset operation.
        unsafe {
            tree_sitter_mermaid_external_scanner_deserialize(self.0, std::ptr::null(), 0);
        }
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        // SAFETY: The payload was created by the paired constructor and is destroyed once.
        unsafe { tree_sitter_mermaid_external_scanner_destroy(self.0) };
    }
}

#[derive(Debug)]
struct ScanResult {
    matched: bool,
    symbol: u16,
    marked_end: usize,
}

fn family_mask(first_token: usize) -> [bool; TOKEN_COUNT] {
    let mut symbols = [false; TOKEN_COUNT];
    symbols[first_token..first_token + 5].fill(true);
    symbols
}

fn token_mask(token: u16) -> [bool; TOKEN_COUNT] {
    let mut symbols = [false; TOKEN_COUNT];
    symbols[usize::from(token)] = true;
    symbols
}

fn hierarchy_row(indentation: usize, marker: &[u8]) -> Vec<u8> {
    let mut row = vec![b' '; indentation];
    row.extend_from_slice(marker);
    row
}

fn restarted(scanner: &Scanner) -> Scanner {
    let encoded = scanner.serialize();
    assert!(encoded.len() <= MAX_SERIALIZED_SIZE);
    let mut restored = Scanner::new();
    restored.deserialize(&encoded);
    assert_eq!(restored.serialize(), encoded);
    restored
}

#[test]
fn scanner_state_round_trips_and_reindents_without_truncation() {
    let mut scanner = Scanner::new();
    let mindmap = family_mask(usize::from(MINDMAP_START));

    let start = scanner.scan(b"Root", &mindmap);
    assert!(start.matched);
    assert_eq!(start.symbol, MINDMAP_START);

    let indent = scanner.scan(b"    Child", &mindmap);
    assert!(indent.matched);
    assert_eq!(indent.symbol, MINDMAP_INDENT);
    assert_eq!(indent.marked_end, 4);

    let encoded = scanner.serialize();
    assert!(encoded.len() <= MAX_SERIALIZED_SIZE);
    assert_eq!(&encoded[..4], b"MM\x01\x01");

    let mut restored = Scanner::new();
    restored.deserialize(&encoded);
    assert_eq!(restored.serialize(), encoded);

    let reindent = restored.scan(b"  Sibling", &mindmap);
    assert!(reindent.matched);
    assert_eq!(reindent.symbol, MINDMAP_REINDENT);

    let dedent = restored.scan(b"Root sibling", &mindmap);
    assert!(dedent.matched);
    assert_eq!(dedent.symbol, MINDMAP_DEDENT);
}

#[test]
fn end_of_input_token_is_exact_and_stateless() {
    let mut scanner = Scanner::new();
    let end_of_input = token_mask(END_OF_INPUT);

    let matched = scanner.scan(b"", &end_of_input);
    assert!(matched.matched);
    assert_eq!(matched.symbol, END_OF_INPUT);
    assert_eq!(matched.marked_end, 0);
    assert!(scanner.serialize().is_empty());

    let rejected = scanner.scan(b";", &end_of_input);
    assert!(!rejected.matched);
    assert_eq!(rejected.symbol, u16::MAX);
    assert!(scanner.serialize().is_empty());
}

#[test]
fn every_external_token_survives_a_scanner_restart() {
    for group in TOKEN_GROUPS {
        let symbols = family_mask(usize::from(group.start));
        let mut scanner = Scanner::new();

        let start = scanner.scan(&hierarchy_row(0, group.marker), &symbols);
        assert!(start.matched);
        assert_eq!(start.symbol, group.start);
        scanner = restarted(&scanner);

        let indent = scanner.scan(&hierarchy_row(4, group.marker), &symbols);
        assert!(indent.matched);
        assert_eq!(indent.symbol, group.indent);
        scanner = restarted(&scanner);

        let reindent = scanner.scan(&hierarchy_row(2, group.marker), &symbols);
        assert!(reindent.matched);
        assert_eq!(reindent.symbol, group.reindent);
        scanner = restarted(&scanner);

        let dedent = scanner.scan(&hierarchy_row(0, group.marker), &symbols);
        assert!(dedent.matched);
        assert_eq!(dedent.symbol, group.dedent);
        scanner = restarted(&scanner);

        for indentation in 1..=255 {
            let indent = scanner.scan(&hierarchy_row(indentation, group.marker), &symbols);
            assert!(indent.matched);
            assert_eq!(indent.symbol, group.indent);
        }
        scanner = restarted(&scanner);
        let before_overflow = scanner.serialize();
        let overflow = scanner.scan(&hierarchy_row(256, group.marker), &symbols);
        assert!(overflow.matched);
        assert_eq!(overflow.symbol, group.overflow);
        assert_eq!(scanner.serialize(), before_overflow);
        let restored = restarted(&scanner);
        assert_eq!(restored.serialize(), before_overflow);
    }
}

#[test]
fn every_scanner_family_accepts_exact_maximum_indentation_then_overflows() {
    for group in TOKEN_GROUPS {
        let symbols = family_mask(usize::from(group.start));
        let mut scanner = Scanner::new();

        let maximum = scanner.scan(&hierarchy_row(MAX_INDENTATION, group.marker), &symbols);
        assert!(maximum.matched);
        assert_eq!(maximum.symbol, group.start);
        assert_eq!(maximum.marked_end, MAX_INDENTATION);
        scanner = restarted(&scanner);

        let before_overflow = scanner.serialize();
        let overflow = scanner.scan(&hierarchy_row(MAX_INDENTATION + 1, group.marker), &symbols);
        assert!(overflow.matched);
        assert_eq!(overflow.symbol, group.overflow);
        assert_eq!(overflow.marked_end, MAX_INDENTATION + 1);
        assert_eq!(scanner.serialize(), before_overflow);
    }
}

#[test]
fn scanner_rejects_corrupt_or_ambiguous_state() {
    let mut source = Scanner::new();
    let mindmap = family_mask(usize::from(MINDMAP_START));
    assert!(source.scan(b"Root", &mindmap).matched);
    assert!(source.scan(b"  Child", &mindmap).matched);
    let encoded = source.serialize();

    for length in 1..encoded.len() {
        let mut scanner = Scanner::new();
        scanner.deserialize(&encoded[..length]);
        assert!(
            scanner.serialize().is_empty(),
            "accepted truncated length {length}"
        );
    }

    let mut bit_flipped = encoded.clone();
    bit_flipped[10] ^= 1;
    let mut scanner = Scanner::new();
    scanner.deserialize(&bit_flipped);
    assert!(scanner.serialize().is_empty());

    let mut with_trailing_byte = encoded.clone();
    with_trailing_byte.push(0);
    scanner.deserialize(&with_trailing_byte);
    assert!(scanner.serialize().is_empty());

    let mut ambiguous = [false; TOKEN_COUNT];
    ambiguous[usize::from(MINDMAP_START)] = true;
    ambiguous[usize::from(TREEMAP_START)] = true;
    assert!(!source.scan(b"Root", &ambiguous).matched);

    source.reset();
    assert!(source.serialize().is_empty());
}

#[test]
fn scanner_bounds_depth_and_indentation_with_local_overflow() {
    let mut scanner = Scanner::new();
    let mindmap = family_mask(usize::from(MINDMAP_START));
    assert!(scanner.scan(b"Root", &mindmap).matched);

    for indentation in 1..256 {
        let mut row = vec![b' '; indentation];
        row.push(b'N');
        let result = scanner.scan(&row, &mindmap);
        assert!(result.matched);
        assert_eq!(result.symbol, MINDMAP_INDENT);
    }
    assert_eq!(scanner.serialize().len(), MAX_SERIALIZED_SIZE);

    let mut depth_overflow = vec![b' '; 256];
    depth_overflow.push(b'N');
    let overflow = scanner.scan(&depth_overflow, &mindmap);
    assert!(overflow.matched);
    assert_eq!(overflow.symbol, MINDMAP_OVERFLOW);
    assert_eq!(scanner.serialize().len(), MAX_SERIALIZED_SIZE);

    let mut indentation_overflow = vec![b' '; MAX_INDENTATION + 1];
    indentation_overflow.push(b'N');
    let overflow = scanner.scan(&indentation_overflow, &mindmap);
    assert!(overflow.matched);
    assert_eq!(overflow.symbol, MINDMAP_OVERFLOW);
    assert_eq!(overflow.marked_end, MAX_INDENTATION + 1);
    assert_eq!(scanner.serialize().len(), MAX_SERIALIZED_SIZE);
}

#[test]
fn every_scanner_family_reports_overlong_indentation_before_row_classification() {
    for (start, overflow, marker) in [
        (usize::from(MINDMAP_START), MINDMAP_OVERFLOW, b'N'),
        (usize::from(TREEMAP_START), TREEMAP_OVERFLOW, b'"'),
        (usize::from(TREE_VIEW_START), TREE_VIEW_OVERFLOW, b'N'),
        (usize::from(KANBAN_START), KANBAN_OVERFLOW, b'N'),
    ] {
        let mut scanner = Scanner::new();
        let symbols = family_mask(start);
        let mut row = vec![b' '; MAX_INDENTATION + 2];
        row.push(marker);

        let result = scanner.scan(&row, &symbols);
        assert!(
            result.matched,
            "scanner group at token {start} did not recover"
        );
        assert_eq!(result.symbol, overflow);
        assert_eq!(result.marked_end, MAX_INDENTATION + 1);
        assert!(scanner.serialize().is_empty());
    }
}

#[test]
fn scanner_switches_family_only_on_a_real_hierarchy_row() {
    let mut scanner = Scanner::new();
    let mindmap = family_mask(usize::from(MINDMAP_START));
    let treemap = family_mask(usize::from(TREEMAP_START));
    let tree_view = family_mask(usize::from(TREE_VIEW_START));
    let kanban = family_mask(usize::from(KANBAN_START));

    assert!(scanner.scan(b"Root", &mindmap).matched);
    let before = scanner.serialize();
    assert!(!scanner.scan(b"  ::icon(book)", &mindmap).matched);
    assert_eq!(scanner.serialize(), before);
    assert!(!scanner.scan(b"  %% comment", &mindmap).matched);
    assert_eq!(scanner.serialize(), before);
    assert!(!scanner.scan("  │ Child".as_bytes(), &tree_view).matched);
    assert_eq!(scanner.serialize(), before);
    for metadata in [
        b"title Example".as_slice(),
        b"accTitle: Example".as_slice(),
        b"accDescr: Example".as_slice(),
        b"accDescr {details}".as_slice(),
        b"accDescr\n{details}".as_slice(),
    ] {
        assert!(!scanner.scan(metadata, &tree_view).matched);
        assert_eq!(scanner.serialize(), before);
    }

    let switched = scanner.scan(b"\"Section\"", &treemap);
    assert!(switched.matched);
    assert_eq!(switched.symbol, TREEMAP_START);
    let encoded = scanner.serialize();
    assert_eq!(encoded[3], 2);

    let switched = scanner.scan(b"Todo", &kanban);
    assert!(switched.matched);
    assert_eq!(switched.symbol, KANBAN_START);
    let encoded = scanner.serialize();
    assert_eq!(encoded[3], 4);

    let before = scanner.serialize();
    assert!(!scanner.scan(b"  ::icon(book)", &kanban).matched);
    assert_eq!(scanner.serialize(), before);
    assert!(!scanner.scan(b"  :::urgent", &kanban).matched);
    assert_eq!(scanner.serialize(), before);

    let switched = scanner.scan(b"Node", &tree_view);
    assert!(switched.matched);
    assert_eq!(switched.symbol, TREE_VIEW_START);
    let encoded = scanner.serialize();
    assert_eq!(encoded[3], 3);

    let before = scanner.serialize();
    for metadata in [
        b"  title Example".as_slice(),
        b"  accTitle : Example".as_slice(),
        b"  accDescr : Example".as_slice(),
        b"  accDescr {details}".as_slice(),
        b"  accDescr\r\n{details}".as_slice(),
    ] {
        assert!(!scanner.scan(metadata, &tree_view).matched);
        assert_eq!(scanner.serialize(), before);
    }
}
