#include <string.h>

#include "tree_sitter/api.h"
#include "tree_sitter/tree-sitter-mermaid.h"

int main(void) {
  static const char source[] = "flowchart TD\nA --> B\n";
  const TSLanguage *language = tree_sitter_mermaid();
  if (language == NULL || ts_language_abi_version(language) != 15) {
    return 1;
  }

  TSParser *parser = ts_parser_new();
  if (parser == NULL || !ts_parser_set_language(parser, language)) {
    ts_parser_delete(parser);
    return 2;
  }

  TSTree *tree = ts_parser_parse_string(parser, NULL, source, strlen(source));
  if (tree == NULL) {
    ts_parser_delete(parser);
    return 3;
  }

  TSNode document = ts_tree_root_node(tree);
  TSNode diagram = ts_node_named_child(document, 0);
  int status = 0;
  if (strcmp(ts_node_type(document), "source_file") != 0 ||
      ts_node_has_error(document) || ts_node_is_null(diagram) ||
      strcmp(ts_node_type(diagram), "flowchart_diagram") != 0) {
    status = 4;
  }

  ts_tree_delete(tree);
  ts_parser_delete(parser);
  return status;
}
