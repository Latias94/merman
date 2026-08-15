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
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_ID "5bea8f9c977e0232960e787cb3166de4f40b11e0c751b6d1aca476a1bc8d321a"
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_PATH "metadata/artifact-receipt.json"
#define TREE_SITTER_MERMAID_PORTABLE_HIGHLIGHTS_QUERY_PATH \
  "queries/portable/highlights.scm"

#ifdef __cplusplus
}
#endif

#endif
