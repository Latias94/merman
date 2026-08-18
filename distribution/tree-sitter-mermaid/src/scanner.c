#include "tree_sitter/parser.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

enum TokenType {
  MINDMAP_START,
  MINDMAP_INDENT,
  MINDMAP_REINDENT,
  MINDMAP_DEDENT,
  MINDMAP_INDENTATION_OVERFLOW,
  TREEMAP_START,
  TREEMAP_INDENT,
  TREEMAP_REINDENT,
  TREEMAP_DEDENT,
  TREEMAP_INDENTATION_OVERFLOW,
  TREE_VIEW_START,
  TREE_VIEW_INDENT,
  TREE_VIEW_REINDENT,
  TREE_VIEW_DEDENT,
  TREE_VIEW_INDENTATION_OVERFLOW,
  KANBAN_START,
  KANBAN_INDENT,
  KANBAN_REINDENT,
  KANBAN_DEDENT,
  KANBAN_INDENTATION_OVERFLOW,
  END_OF_INPUT,
  DIRECTIVE_BODY,
};

enum ScannerMode {
  MODE_NONE = 0,
  MODE_MINDMAP = 1,
  MODE_TREEMAP = 2,
  MODE_TREE_VIEW = 3,
  MODE_KANBAN = 4,
};

enum {
  STATE_VERSION = 1,
  MAX_DEPTH = 256,
  MAX_INDENTATION = UINT16_MAX - 1,
  STATE_PREFIX_SIZE = 10,
  STATE_CHECKSUM_SIZE = 4,
  MAX_SERIALIZED_SIZE =
      STATE_PREFIX_SIZE + (MAX_DEPTH * 2) + STATE_CHECKSUM_SIZE,
};

_Static_assert(MAX_SERIALIZED_SIZE <= TREE_SITTER_SERIALIZATION_BUFFER_SIZE,
               "scanner state exceeds Tree-sitter's serialization buffer");

typedef struct {
  uint8_t mode;
  uint16_t count;
  uint16_t levels[MAX_DEPTH];
} Scanner;

typedef struct {
  enum TokenType start;
  enum TokenType indent;
  enum TokenType reindent;
  enum TokenType dedent;
  enum TokenType overflow;
  enum ScannerMode mode;
} TokenGroup;

static const TokenGroup TOKEN_GROUPS[] = {
    {MINDMAP_START, MINDMAP_INDENT, MINDMAP_REINDENT, MINDMAP_DEDENT,
     MINDMAP_INDENTATION_OVERFLOW, MODE_MINDMAP},
    {TREEMAP_START, TREEMAP_INDENT, TREEMAP_REINDENT, TREEMAP_DEDENT,
     TREEMAP_INDENTATION_OVERFLOW, MODE_TREEMAP},
    {TREE_VIEW_START, TREE_VIEW_INDENT, TREE_VIEW_REINDENT, TREE_VIEW_DEDENT,
     TREE_VIEW_INDENTATION_OVERFLOW, MODE_TREE_VIEW},
    {KANBAN_START, KANBAN_INDENT, KANBAN_REINDENT, KANBAN_DEDENT,
     KANBAN_INDENTATION_OVERFLOW, MODE_KANBAN},
};

static void scanner_reset(Scanner *scanner) {
  scanner->mode = MODE_NONE;
  scanner->count = 0;
  memset(scanner->levels, 0, sizeof(scanner->levels));
}

static uint16_t read_u16(const char *buffer) {
  return (uint16_t)(uint8_t)buffer[0] |
         (uint16_t)((uint16_t)(uint8_t)buffer[1] << 8);
}

static uint32_t read_u32(const char *buffer) {
  return (uint32_t)(uint8_t)buffer[0] |
         ((uint32_t)(uint8_t)buffer[1] << 8) |
         ((uint32_t)(uint8_t)buffer[2] << 16) |
         ((uint32_t)(uint8_t)buffer[3] << 24);
}

static void write_u16(char *buffer, uint16_t value) {
  buffer[0] = (char)(value & 0xff);
  buffer[1] = (char)((value >> 8) & 0xff);
}

static void write_u32(char *buffer, uint32_t value) {
  buffer[0] = (char)(value & 0xff);
  buffer[1] = (char)((value >> 8) & 0xff);
  buffer[2] = (char)((value >> 16) & 0xff);
  buffer[3] = (char)((value >> 24) & 0xff);
}

static uint32_t checksum(const char *buffer, unsigned length) {
  uint32_t hash = UINT32_C(2166136261);
  for (unsigned index = 0; index < length; index++) {
    hash ^= (uint8_t)buffer[index];
    hash *= UINT32_C(16777619);
  }
  return hash;
}

static bool mode_is_valid(uint8_t mode) {
  return mode >= MODE_MINDMAP && mode <= MODE_KANBAN;
}

static bool token_group_requested(const bool *valid_symbols,
                                  const TokenGroup *group) {
  return valid_symbols[group->start] || valid_symbols[group->indent] ||
         valid_symbols[group->reindent] || valid_symbols[group->dedent] ||
         valid_symbols[group->overflow];
}

static const TokenGroup *requested_group(const bool *valid_symbols) {
  const TokenGroup *selected = NULL;
  for (unsigned index = 0;
       index < sizeof(TOKEN_GROUPS) / sizeof(TOKEN_GROUPS[0]); index++) {
    const TokenGroup *candidate = &TOKEN_GROUPS[index];
    if (!token_group_requested(valid_symbols, candidate)) {
      continue;
    }
    if (selected != NULL) {
      return NULL;
    }
    selected = candidate;
  }
  return selected;
}

static bool directive_body_is_unambiguous(const bool *valid_symbols) {
  if (!valid_symbols[DIRECTIVE_BODY] || valid_symbols[END_OF_INPUT]) {
    return false;
  }
  for (unsigned index = 0;
       index < sizeof(TOKEN_GROUPS) / sizeof(TOKEN_GROUPS[0]); index++) {
    if (token_group_requested(valid_symbols, &TOKEN_GROUPS[index])) {
      return false;
    }
  }
  return true;
}

static bool is_box_drawing_prefix(int32_t lookahead) {
  return lookahead == 0x2502 || lookahead == 0x251c || lookahead == 0x2514 ||
         lookahead == 0x2503 || lookahead == 0x2523 || lookahead == 0x2517 ||
         lookahead == '|';
}

static bool is_hierarchy_row(enum ScannerMode mode, int32_t lookahead) {
  if (lookahead == 0 || lookahead == '\n' || lookahead == '\r' ||
      lookahead == '%') {
    return false;
  }
  if (mode == MODE_MINDMAP || mode == MODE_KANBAN) {
    return lookahead != ':';
  }
  if (mode == MODE_TREEMAP) {
    return lookahead == '"' || lookahead == '\'';
  }
  return !is_box_drawing_prefix(lookahead);
}

static bool consume_literal(TSLexer *lexer, const char *literal) {
  for (; *literal != '\0'; literal++) {
    if (lexer->lookahead != (unsigned char)*literal) {
      return false;
    }
    lexer->advance(lexer, false);
  }
  return true;
}

static bool is_horizontal_space(int32_t lookahead) {
  return lookahead == ' ' || lookahead == '\t' || lookahead == '\f' ||
         lookahead == 0xa0;
}

static bool scan_directive_body(TSLexer *lexer) {
  bool consumed = false;

  while (!lexer->eof(lexer)) {
    if (lexer->lookahead == '}') {
      lexer->mark_end(lexer);
      lexer->advance(lexer, false);
      if (lexer->lookahead == '%') {
        lexer->advance(lexer, false);
        if (lexer->lookahead == '%') {
          if (!consumed) {
            return false;
          }
          lexer->result_symbol = DIRECTIVE_BODY;
          return true;
        }
      }
      consumed = true;
      lexer->mark_end(lexer);
      continue;
    }

    lexer->advance(lexer, false);
    consumed = true;
    lexer->mark_end(lexer);
  }

  if (!consumed) {
    return false;
  }
  lexer->result_symbol = DIRECTIVE_BODY;
  return true;
}

static bool is_tree_view_metadata_row(TSLexer *lexer) {
  if (lexer->lookahead == 't') {
    if (!consume_literal(lexer, "title")) {
      return false;
    }
    return lexer->lookahead == 0 || lexer->lookahead == '\n' ||
           lexer->lookahead == '\r' || lexer->lookahead == '%' ||
           is_horizontal_space(lexer->lookahead);
  }

  if (!consume_literal(lexer, "acc")) {
    return false;
  }
  if (lexer->lookahead == 'T') {
    if (!consume_literal(lexer, "Title")) {
      return false;
    }
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
      lexer->advance(lexer, false);
    }
    return lexer->lookahead == ':';
  }
  if (lexer->lookahead == 'D') {
    if (!consume_literal(lexer, "Descr")) {
      return false;
    }
    while (is_horizontal_space(lexer->lookahead)) {
      lexer->advance(lexer, false);
    }
    return lexer->lookahead == ':' || lexer->lookahead == '{' ||
           lexer->lookahead == '\n' || lexer->lookahead == '\r';
  }
  return false;
}

static bool emit_transition(Scanner *scanner, TSLexer *lexer,
                            const bool *valid_symbols, const TokenGroup *group,
                            uint16_t indentation) {
  Scanner next = *scanner;

  if (scanner->mode != group->mode || scanner->count == 0) {
    if (!valid_symbols[group->start]) {
      return false;
    }
    scanner_reset(&next);
    next.mode = (uint8_t)group->mode;
    next.count = 1;
    next.levels[0] = indentation;
    *scanner = next;
    lexer->result_symbol = group->start;
    return true;
  }

  const uint16_t top = scanner->levels[scanner->count - 1];
  if (indentation == top) {
    return false;
  }

  if (indentation > top) {
    if (scanner->count == MAX_DEPTH) {
      if (!valid_symbols[group->overflow]) {
        return false;
      }
      lexer->result_symbol = group->overflow;
      return true;
    }
    if (!valid_symbols[group->indent]) {
      return false;
    }
    next.levels[next.count] = indentation;
    next.count++;
    *scanner = next;
    lexer->result_symbol = group->indent;
    return true;
  }

  const uint16_t parent =
      scanner->count > 1 ? scanner->levels[scanner->count - 2] : 0;
  if (scanner->count > 1 && indentation > parent) {
    if (!valid_symbols[group->reindent]) {
      return false;
    }
    next.levels[next.count - 1] = indentation;
    *scanner = next;
    lexer->result_symbol = group->reindent;
    return true;
  }

  if (!valid_symbols[group->dedent]) {
    return false;
  }

  uint16_t keep = 0;
  while (keep < scanner->count && scanner->levels[keep] < indentation) {
    keep++;
  }
  if (keep < scanner->count && scanner->levels[keep] == indentation) {
    next.count = (uint16_t)(keep + 1);
  } else if (keep == 0) {
    next.count = 1;
    next.levels[0] = indentation;
  } else {
    next.count = (uint16_t)(keep + 1);
    next.levels[keep] = indentation;
  }
  *scanner = next;
  lexer->result_symbol = group->dedent;
  return true;
}

void *tree_sitter_mermaid_external_scanner_create(void) {
  Scanner *scanner = (Scanner *)calloc(1, sizeof(Scanner));
  return scanner;
}

void tree_sitter_mermaid_external_scanner_destroy(void *payload) {
  free(payload);
}

unsigned tree_sitter_mermaid_external_scanner_serialize(void *payload,
                                                        char *buffer) {
  const Scanner *scanner = (const Scanner *)payload;
  if (scanner == NULL || scanner->mode == MODE_NONE || scanner->count == 0) {
    return 0;
  }

  const unsigned body_length = STATE_PREFIX_SIZE + scanner->count * 2;
  const unsigned total_length = body_length + STATE_CHECKSUM_SIZE;
  buffer[0] = 'M';
  buffer[1] = 'M';
  buffer[2] = STATE_VERSION;
  buffer[3] = (char)scanner->mode;
  buffer[4] = 0;
  buffer[5] = 0;
  write_u16(buffer + 6, scanner->count);
  write_u16(buffer + 8, scanner->levels[scanner->count - 1]);
  for (uint16_t index = 0; index < scanner->count; index++) {
    write_u16(buffer + STATE_PREFIX_SIZE + index * 2, scanner->levels[index]);
  }
  write_u32(buffer + body_length, checksum(buffer, body_length));
  return total_length;
}

void tree_sitter_mermaid_external_scanner_deserialize(void *payload,
                                                      const char *buffer,
                                                      unsigned length) {
  Scanner *scanner = (Scanner *)payload;
  if (scanner == NULL) {
    return;
  }
  scanner_reset(scanner);
  if (length == 0) {
    return;
  }
  if (buffer == NULL || length < STATE_PREFIX_SIZE + STATE_CHECKSUM_SIZE ||
      buffer[0] != 'M' || buffer[1] != 'M' ||
      (uint8_t)buffer[2] != STATE_VERSION || !mode_is_valid(buffer[3]) ||
      buffer[4] != 0 || buffer[5] != 0) {
    return;
  }

  const uint16_t count = read_u16(buffer + 6);
  const unsigned body_length = STATE_PREFIX_SIZE + count * 2;
  if (count == 0 || count > MAX_DEPTH ||
      length != body_length + STATE_CHECKSUM_SIZE ||
      checksum(buffer, body_length) != read_u32(buffer + body_length)) {
    return;
  }

  Scanner decoded = {0};
  decoded.mode = (uint8_t)buffer[3];
  decoded.count = count;
  for (uint16_t index = 0; index < count; index++) {
    const uint16_t level =
        read_u16(buffer + STATE_PREFIX_SIZE + index * 2);
    if (level > MAX_INDENTATION ||
        (index > 0 && level <= decoded.levels[index - 1])) {
      return;
    }
    decoded.levels[index] = level;
  }
  if (read_u16(buffer + 8) != decoded.levels[count - 1]) {
    return;
  }
  *scanner = decoded;
}

bool tree_sitter_mermaid_external_scanner_scan(void *payload, TSLexer *lexer,
                                               const bool *valid_symbols) {
  Scanner *scanner = (Scanner *)payload;
  if (scanner == NULL) {
    return false;
  }

  if (valid_symbols[END_OF_INPUT] && lexer->eof(lexer)) {
    lexer->mark_end(lexer);
    lexer->result_symbol = END_OF_INPUT;
    return true;
  }

  if (directive_body_is_unambiguous(valid_symbols)) {
    return scan_directive_body(lexer);
  }

  const TokenGroup *group = requested_group(valid_symbols);
  if (group == NULL) {
    return false;
  }

  uint32_t indentation = 0;
  while ((lexer->lookahead == ' ' || lexer->lookahead == '\t') &&
         indentation <= MAX_INDENTATION) {
    lexer->advance(lexer, false);
    indentation++;
  }
  lexer->mark_end(lexer);

  if (indentation > MAX_INDENTATION) {
    if (!valid_symbols[group->overflow]) {
      return false;
    }
    lexer->result_symbol = group->overflow;
    return true;
  }
  if (!is_hierarchy_row(group->mode, lexer->lookahead)) {
    return false;
  }
  if (group->mode == MODE_TREE_VIEW &&
      is_tree_view_metadata_row(lexer)) {
    return false;
  }

  return emit_transition(scanner, lexer, valid_symbols, group,
                         (uint16_t)indentation);
}
