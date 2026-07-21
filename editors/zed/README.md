# LALRPOP for Zed

This directory is a Zed language extension for `.lalrpop` files. It provides a
Tree-sitter grammar configuration and starts the editor-independent
`lalrpop-lsp` binary over standard input/output.

Install the server first:

```sh
cargo install --git https://github.com/LighghtEeloo/lalrpop-lsp.git --locked
```

For local development, run `zed: install dev extension` and select this
directory. If Zed cannot find the server on `PATH`, configure an explicit path:

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

See the [project README](../../README.md) for repository-wide development
instructions.
