#![no_main]

use std::ffi::{c_char, c_void};

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 1031;
const SERIALIZATION_BUFFER_SIZE: usize = 1024;
const MAX_SERIALIZED_SIZE: usize = 526;
const TOKEN_COUNT: usize = 22;
const DIRECTIVE_BODY: usize = 21;
const REGRESSION_SEED_PREFIX: &[u8] = b"seed\n";
const DIRECTIVE_SEED_PREFIX: &[u8] = b"directive\n";

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

unsafe extern "C" fn mock_mark_end(_lexer: *mut TsLexer) {}

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
        assert_eq!(language.abi_version(), 15);
        // SAFETY: The scanner constructor has no preconditions.
        let scanner = unsafe { tree_sitter_mermaid_external_scanner_create() };
        assert!(!scanner.is_null());
        Self(scanner)
    }

    fn deserialize(&mut self, bytes: &[u8]) {
        // SAFETY: The slice is valid for the duration of the call.
        unsafe {
            tree_sitter_mermaid_external_scanner_deserialize(
                self.0,
                bytes.as_ptr().cast::<c_char>(),
                u32::try_from(bytes.len()).expect("bounded fuzz input fits u32"),
            );
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = [0_u8; SERIALIZATION_BUFFER_SIZE];
        // SAFETY: The output buffer has Tree-sitter's required capacity.
        let length = unsafe {
            tree_sitter_mermaid_external_scanner_serialize(
                self.0,
                buffer.as_mut_ptr().cast::<c_char>(),
            )
        } as usize;
        assert!(length <= MAX_SERIALIZED_SIZE);
        buffer[..length].to_vec()
    }

    fn scan(&mut self, row: &[u8], valid_symbols: &[bool; TOKEN_COUNT]) -> ScanResult {
        let mut lexer = MockLexer::new(row);
        // SAFETY: The scanner, lexer, and symbol mask remain valid for the call.
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
        }
    }
}

struct ScanResult {
    matched: bool,
    symbol: u16,
}

struct FuzzInput<'a> {
    state: &'a [u8],
    valid_symbols: [bool; TOKEN_COUNT],
    rows: &'a [u8],
    seed_valid_state: bool,
}

impl Drop for Scanner {
    fn drop(&mut self) {
        // SAFETY: This payload is destroyed exactly once.
        unsafe { tree_sitter_mermaid_external_scanner_destroy(self.0) };
    }
}

fn split_input(data: &[u8]) -> Option<FuzzInput<'_>> {
    if let Some(row) = data.strip_prefix(DIRECTIVE_SEED_PREFIX) {
        let mut symbols = [false; TOKEN_COUNT];
        symbols[DIRECTIVE_BODY] = true;
        return Some(FuzzInput {
            state: &[],
            valid_symbols: symbols,
            rows: row,
            seed_valid_state: false,
        });
    }
    if let Some(row) = data.strip_prefix(REGRESSION_SEED_PREFIX) {
        let mut symbols = [false; TOKEN_COUNT];
        symbols[..5].fill(true);
        return Some(FuzzInput {
            state: &[],
            valid_symbols: symbols,
            rows: row,
            seed_valid_state: true,
        });
    }
    if data.len() < 6 {
        return None;
    }
    let available_state = (data.len() - 6).min(SERIALIZATION_BUFFER_SIZE + 1);
    let declared = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let state_length = declared % (available_state + 1);
    let mask_start = 2 + state_length;
    let mask = u32::from_le_bytes([
        data[mask_start],
        data[mask_start + 1],
        data[mask_start + 2],
        data[mask_start + 3],
    ]);
    let mut symbols = [false; TOKEN_COUNT];
    for (index, symbol) in symbols.iter_mut().enumerate() {
        *symbol = mask & (1_u32 << index) != 0;
    }
    Some(FuzzInput {
        state: &data[2..mask_start],
        valid_symbols: symbols,
        rows: &data[mask_start + 4..],
        seed_valid_state: false,
    })
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let Some(FuzzInput {
        state,
        valid_symbols,
        rows,
        seed_valid_state,
    }) = split_input(data)
    else {
        return;
    };
    let mut scanner = Scanner::new();
    scanner.deserialize(state);
    if seed_valid_state {
        let start = scanner.scan(b"Root", &valid_symbols);
        assert!(start.matched);
        let indent = scanner.scan(b"  Child", &valid_symbols);
        assert!(indent.matched);
    }
    let initial = scanner.serialize();

    let mut initial_restore = Scanner::new();
    initial_restore.deserialize(&initial);
    assert_eq!(initial_restore.serialize(), initial);
    scanner = initial_restore;

    for row in rows.split(|byte| matches!(byte, b'\n' | b'\r')).take(8) {
        let before_scan = scanner.serialize();
        let result = scanner.scan(row, &valid_symbols);
        if result.matched {
            let index = usize::from(result.symbol);
            assert!(index < TOKEN_COUNT);
            assert!(valid_symbols[index]);
        } else {
            assert_eq!(
                scanner.serialize(),
                before_scan,
                "a failed external scan must not mutate serialized state"
            );
        }
        let canonical = scanner.serialize();

        let mut restored = Scanner::new();
        restored.deserialize(&canonical);
        assert_eq!(restored.serialize(), canonical);
        scanner = restored;
    }
});
