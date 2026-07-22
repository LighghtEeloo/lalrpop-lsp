# tree-sitter-lalrpop

This is a minimal vendored copy of
[`traxys/tree-sitter-lalrpop`](https://github.com/traxys/tree-sitter-lalrpop)
at revision `27b0f7bb55b4cabd8f01a933d9ee6a49dbfc2192`. It contains only the
grammar definition, generated parser sources, and license needed to build the
Zed grammar.

The external scanner intentionally recognizes a LALRPOP macro identifier only
when `<` immediately follows the identifier. This matches LALRPOP's lexer and
prevents adjacent symbols such as `Start <value: T>` from being parsed as a
single macro invocation.

The scanner also exposes the Rust body of `=>` and `=>?` as a separate
`action_code` node. Its boundary rules follow LALRPOP's tokenizer: commas,
semicolons, and closing delimiters terminate a top-level action, while balanced
delimiters, strings, raw strings, line comments, and nested block comments are
skipped. Zed injects these nodes into the companion `lalrpop_rust` expression
grammar.

When updating from upstream, preserve the adjacent-symbol and action-boundary
corpus tests and run `tree-sitter test` before updating the revision used by the
Zed extension.
