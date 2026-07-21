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

When updating from upstream, preserve the adjacent-symbol corpus test and run
`tree-sitter test` before updating the revision used by the Zed extension.
