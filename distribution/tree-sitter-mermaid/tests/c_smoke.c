#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "tree_sitter/api.h"
#include "tree_sitter/tree-sitter-mermaid.h"

static TSQuery *load_query(const TSLanguage *language, const char *path) {
  FILE *query_file = fopen(path, "rb");
  if (query_file == NULL || fseek(query_file, 0, SEEK_END) != 0) {
    if (query_file != NULL) {
      fclose(query_file);
    }
    return NULL;
  }
  long query_length = ftell(query_file);
  if (query_length < 0 || (unsigned long)query_length > UINT32_MAX ||
      fseek(query_file, 0, SEEK_SET) != 0) {
    fclose(query_file);
    return NULL;
  }
  char *query_source = malloc((size_t)query_length + 1);
  if (query_source == NULL ||
      fread(query_source, 1, (size_t)query_length, query_file) !=
          (size_t)query_length) {
    free(query_source);
    fclose(query_file);
    return NULL;
  }
  query_source[query_length] = '\0';
  fclose(query_file);

  uint32_t error_offset = 0;
  TSQueryError error = TSQueryErrorNone;
  TSQuery *query = ts_query_new(language, query_source, (uint32_t)query_length,
                                &error_offset, &error);
  free(query_source);
  if (query == NULL) {
    fprintf(stderr, "%s failed at %u with error %d\n", path, error_offset,
            error);
  }
  return query;
}

int main(int argc, char **argv) {
  if (argc < 5) {
    fprintf(stderr,
            "usage: c_smoke SOURCE EXPECTED_ROOT RECEIPT_ID QUERY_PATH...\n");
    return 1;
  }
  const TSLanguage *language = tree_sitter_mermaid();
  if (language == NULL) {
    return 2;
  }
  if (ts_language_abi_version(language) != TREE_SITTER_MERMAID_LANGUAGE_ABI) {
    return 3;
  }
  if (strcmp(TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_ID, argv[3]) != 0) {
    return 8;
  }

  TSQuery *query = NULL;
  for (int index = 4; index < argc; index++) {
    TSQuery *candidate = load_query(language, argv[index]);
    if (candidate == NULL) {
      if (query != NULL) {
        ts_query_delete(query);
      }
      return 12;
    }
    if (strcmp(argv[index], TREE_SITTER_MERMAID_PORTABLE_HIGHLIGHTS_QUERY_PATH) ==
        0) {
      if (query != NULL) {
        ts_query_delete(query);
      }
      query = candidate;
    } else {
      ts_query_delete(candidate);
    }
  }
  if (query == NULL) {
    fprintf(stderr, "receipt did not expose portable highlights\n");
    return 12;
  }
  TSParser *parser = ts_parser_new();
  if (parser == NULL) {
    ts_query_delete(query);
    return 4;
  }
  if (!ts_parser_set_language(parser, language)) {
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 4;
  }

  TSTree *tree = ts_parser_parse_string(parser, NULL, argv[1], strlen(argv[1]));
  if (tree == NULL) {
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 5;
  }
  TSNode document = ts_tree_root_node(tree);
  if (strcmp(ts_node_type(document), "source_file") != 0 ||
      ts_node_has_error(document)) {
    fprintf(stderr, "invalid document for %s\n", argv[2]);
    ts_tree_delete(tree);
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 6;
  }
  bool found = false;
  uint32_t count = ts_node_named_child_count(document);
  for (uint32_t index = 0; index < count; index++) {
    TSNode child = ts_node_named_child(document, index);
    if (strcmp(ts_node_type(child), argv[2]) == 0) {
      found = true;
      break;
    }
  }
  if (!found) {
    fprintf(stderr, "missing expected root %s\n", argv[2]);
    ts_tree_delete(tree);
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 7;
  }

  TSQueryCursor *cursor = ts_query_cursor_new();
  if (cursor == NULL) {
    ts_tree_delete(tree);
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 13;
  }
  ts_query_cursor_exec(cursor, query, document);
  TSQueryMatch match;
  uint32_t capture_index = 0;
  if (!ts_query_cursor_next_capture(cursor, &match, &capture_index)) {
    fprintf(stderr, "portable highlights produced no capture\n");
    ts_query_cursor_delete(cursor);
    ts_tree_delete(tree);
    ts_parser_delete(parser);
    ts_query_delete(query);
    return 14;
  }

  ts_query_cursor_delete(cursor);
  ts_tree_delete(tree);
  ts_parser_delete(parser);
  ts_query_delete(query);
  return 0;
}
