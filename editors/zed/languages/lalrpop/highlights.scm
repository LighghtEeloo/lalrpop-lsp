[
  "grammar"
  "pub"
  "extern"
  "type"
  "enum"
  "match"
  "else"
  "where"
  "for"
  "dyn"
  "in"
  "if"
] @keyword

(mut) @keyword

[
  "+"
  "*"
  "?"
  "="
  "=="
  "!="
  "~~"
  "!~"
  "->"
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "<"
  ">"
] @punctuation.bracket

[
  ","
  ";"
  ":"
  "::"
] @punctuation.delimiter

(nonterminal_name
  [
    (identifier)
    (macro_id)
    (escape)
  ] @function)

(macro (macro_id) @function)
(bare_symbol (identifier) @function)
(terminal (identifier) @constant)
(binding_symbol name: (identifier) @variable.parameter)
(grammar_parameter (identifier) @variable.parameter)
(type_parameter (identifier) @type)
(type_ref) @type
(lifetime (identifier) @label)
(annotation (id) @attribute)

(string_literal) @string
(regex_literal) @string.regex
(escape_sequence) @string.escape
(comment) @comment

[
  (lookahead)
  (lookbehind)
  (error)
] @constant.builtin
