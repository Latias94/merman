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
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_ID "33ad48cbc9d2dd2f0dbe390c3010cc073513313b1a3dd47c0ba37b2f77d5384f"
#define TREE_SITTER_MERMAID_ARTIFACT_RECEIPT_PATH "metadata/artifact-receipt.json"
#define TREE_SITTER_MERMAID_PORTABLE_HIGHLIGHTS_QUERY_PATH \
  "queries/portable/highlights.scm"

#ifdef __cplusplus
}
#endif

#endif
