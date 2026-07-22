/**
 * @file Rust expressions embedded in LALRPOP actions
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const Rust = require('./vendor/tree-sitter-rust/grammar');

module.exports = grammar(Rust, {
  name: 'lalrpop_rust',

  rules: {
    // An injected action is an expression, not a complete Rust source file.
    source_file: $ => field('action', $._expression),

    // LALRPOP expands `<>` to the values captured by the production.
    _expression_except_range: ($, original) => choice(
      original,
      $.lalrpop_placeholder,
    ),

    // `Type { <> }` is LALRPOP shorthand for a struct initializer assembled
    // from the production's named captures.
    shorthand_field_initializer: ($, original) => choice(
      original,
      $.lalrpop_placeholder,
    ),

    lalrpop_placeholder: _ => token('<>'),
  },
});
