((use) @content
  (#set! injection.language "rust"))

((normal_action
  code: (action_code) @content)
  (#set! injection.language "LALRPOP Rust"))

((failible_action
  code: (action_code) @content)
  (#set! injection.language "LALRPOP Rust"))
