import type Parser = require('tree-sitter');

declare const mermaid: Parser.Language & { readonly name: 'mermaid' };

export = mermaid;
