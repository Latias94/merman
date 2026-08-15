#ifndef TREE_SITTER_MERMAID_H_
#define TREE_SITTER_MERMAID_H_

#ifdef __cplusplus
extern "C" {
#endif

typedef struct TSLanguage TSLanguage;

const TSLanguage *tree_sitter_mermaid(void);

#define TREE_SITTER_MERMAID_LANGUAGE_ABI 14
#define TREE_SITTER_MERMAID_NODE_SCHEMA_VERSION 1
#define TREE_SITTER_MERMAID_QUERY_SCHEMA_VERSION 1
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_ID "986b9c773948cc36b6077189ccbfb16d317a629b9c1fe1cab2419ae646bc25e2"
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_PATH "metadata/artifact-receipt.json"
#define TREE_SITTER_MERMAID_PORTABLE_HIGHLIGHTS_QUERY_PATH \
  "queries/portable/highlights.scm"

#ifdef __cplusplus
}
#endif

#endif
