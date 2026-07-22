# tree-sitter-lalrpop-rust

This grammar parses the Rust expressions embedded after `=>` and `=>?` in a
LALRPOP production. It extends the Rust grammar with LALRPOP's `<>` action
placeholder and uses an expression as its root rule.

The base grammar is vendored from
[`tree-sitter-rust` 0.24.2](https://github.com/tree-sitter/tree-sitter-rust/tree/77a3747266f4d621d0757825e6b11edcbf991ca5),
the version used by Zed when this dialect was introduced. Its MIT license is
included in `vendor/tree-sitter-rust/LICENSE`.

To update the base grammar:

1. Replace `vendor/tree-sitter-rust/grammar.js` and `src/scanner.c` from one
   upstream revision.
2. Rename the scanner's exported `tree_sitter_rust_*` symbols to
   `tree_sitter_lalrpop_rust_*`.
3. Regenerate the parser and run the corpus tests.
4. Refresh Zed's Rust highlight query from the same Zed revision it targets.
