# LALRPOP for Zed

This directory is a Zed language extension for `.lalrpop` files. It provides a
Tree-sitter grammar configuration and starts the editor-independent
`lalrpop-lsp` binary over standard input/output.

Action bodies after `=>` and `=>?` are injected into the hidden `LALRPOP Rust`
language. That language parses one Rust expression plus LALRPOP's `<>`
placeholder, so its highlighting follows the same query as native Rust without
relying on Rust parser error recovery.

Install the server first:

```sh
cargo install --git https://github.com/LighghtEeloo/lalrpop-lsp.git --locked
```

For local development, run `zed: install dev extension` and select this
directory. After changing the grammar or language queries, run
`zed: rebuild dev extension` from the command palette.

`languages/lalrpop-rust/highlights.scm` tracks Zed's native Rust query. The
current copy comes from Zed revision
`1817b43c80707a3e49c3946a72bd39ee3134c654`; preserve the local
`lalrpop_placeholder` capture when refreshing it.

If Zed cannot find the server on `PATH`, configure an explicit path:

```json
{
  "lsp": {
    "lalrpop-lsp": {
      "binary": {
        "path": "/absolute/path/to/lalrpop-lsp"
      }
    }
  }
}
```

Zed disables language-server semantic tokens by default. To overlay semantic
highlighting on the Tree-sitter syntax highlighting, enable combined tokens for
LALRPOP in `settings.json`:

```json
{
  "languages": {
    "LALRPOP": {
      "semantic_tokens": "combined"
    }
  }
}
```

See the [project README](../../README.md) for repository-wide development
instructions.
