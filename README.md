# LALRPOP language server

`lalrpop-lsp` provides Language Server Protocol support for
[LALRPOP](https://github.com/lalrpop/lalrpop), an LR(1) parser generator for
Rust. The language server communicates over standard input/output and is shared
by the VS Code and Zed integrations in this repository.

## Repository layout

- `src/` contains the editor-independent Rust language server.
- `grammars/lalrpop/` contains the patched Tree-sitter grammar used by the Zed
  integration.
- `editors/vscode/` contains the VS Code extension, TextMate grammar, and
  TypeScript client.
- `editors/zed/` contains the Zed WASM adapter and Tree-sitter language files.

## Install the language server

Both editors can run a `lalrpop-lsp` executable already available on `PATH`:

```sh
cargo install --git https://github.com/LighghtEeloo/lalrpop-lsp.git --locked
```

## VS Code

Install [LALRPOP Language Server](https://marketplace.visualstudio.com/items?itemName=LitiaEeloo.lalrpop-language-server)
from the VS Code Marketplace. If `lalrpop-lsp` is not available on `PATH`, the
extension offers to install it with Cargo on first use.

To use a specific server build, set `lalrpop-language-server.server.path` in VS
Code settings.

## Zed

Until the extension is published in Zed's extension registry, install it as a
development extension:

1. Install `lalrpop-lsp` with the command above.
2. Run `zed: install dev extension` from Zed's command palette.
3. Select the `editors/zed` directory from this checkout.

If the Zed process cannot see the server on `PATH`, set an explicit binary path
in Zed's `settings.json`:

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

## Features

- Go to definition
- Find references
- Hover information
- Document symbols
- Syntax error diagnostics
- Syntax highlighting in both editors

## Development

Build and test the language server from the repository root:

```sh
cargo test
```

Build the VS Code extension:

```sh
cd editors/vscode
pnpm install
pnpm run compile
```

You can then open the repository in VS Code and run the `Launch VS Code Client`
debug configuration.

Check the Zed adapter with:

```sh
cargo check --manifest-path editors/zed/Cargo.toml
```

Validate the vendored Tree-sitter grammar with:

```sh
cd grammars/lalrpop
pnpm dlx tree-sitter-cli@0.25.10 test
```

After changing the grammar, commit `grammars/lalrpop` first and update the
grammar revision in `editors/zed/extension.toml` to that commit's full hash.
Run `zed: rebuild dev extension` to compile and reload the changed grammar.

Zed compiles the adapter to WebAssembly when `editors/zed` is installed as a
development extension. The language definition uses
a patched vendored copy of
[`tree-sitter-lalrpop`](https://github.com/traxys/tree-sitter-lalrpop) for
syntax-aware editor features.

## Project status

This project is under active development. Please report issues in the
[GitHub issue tracker](https://github.com/LighghtEeloo/lalrpop-lsp/issues).

## Credits

The server uses [Tower LSP](https://github.com/ebkalderon/tower-lsp) and is based
on [tower-lsp-boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate).
The VS Code TextMate grammar originated in
[VSC_LalrpopHighlight](https://github.com/guyutongxue/VSC_LalrpopHighlight).
