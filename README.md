# LALRPOP language server

This repo holds a language server for [LALRPOP](https://github.com/lalrpop/lalrpop), an LR(1) parser generator for Rust.

## Installation
Install the extension from the [VSCode Marketplace](https://marketplace.visualstudio.com/items?itemName=LitiaEeloo.lalrpop-language-server).
The first cold start up will be **slow** because the extension will try to install the language server binary through `cargo` if it doesn't see `lalrpop-lsp` in `PATH`. You can always remove the downloaded binary by running `cargo uninstall lalrpop-lsp` and switch to a manually downloaded version.

## Head's up (!)

This extension is still in active development, so please report any issue you encounter [here](https://github.com/LighghtEeloo/lalrpop-lsp/issues).

## Features

<!--
- [ ] Semantic tokenization
make sure your semantic token is enabled, you could enable your `semantic token` by
adding this line  to your `settings.json`
```json
{
 "editor.semanticHighlighting.enabled": true,
}
```

- [ ] Syntactic error diagnostic

- [ ] Code completion
-->

- Go to Definition
    ![Go to Definition](https://github.com/user-attachments/assets/978cc716-eae6-48d6-828c-7f273982a4aa)

- Find References
    ![Find References](https://github.com/user-attachments/assets/e20bfdf5-6e0d-4d97-99fc-4ab06db0ddd2)

- Hover
    ![Hover](https://github.com/user-attachments/assets/38055a83-8d9d-489a-ace9-979337a635ac)

- Error Diagnostics
    ![Error Diagnostics](https://github.com/user-attachments/assets/beaf18a2-3bff-4065-9bbe-7a09be082c01)

## Development using VSCode
1. `pnpm i`
2. `cargo build`
3. Open the project in VSCode: `code .`
4. In VSCode, press <kbd>F5</kbd> or change to the Debug panel and click <kbd>Launch Client</kbd>.
5. In the newly launched VSCode instance, open a folder that contains a lalrpop file.
6. If the LSP is working correctly you should see syntax highlighting and the features described below should work.
> **Note**  
> 
> If encountered errors like `Cannot find module '/xxx/xxx/dist/extension.js'`
> please try run command `tsc -b` manually, you could refer https://github.com/IWANABETHATGUY/tower-lsp-boilerplate/issues/6 for more details

## Credits

The project is powered by [Language Server Protocol](https://microsoft.github.io/language-server-protocol) [implementation](https://github.com/ebkalderon/tower-lsp) for Rust based on [Tower](https://github.com/tower-rs/tower).
It's also based on [tower-lsp-boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate), a useful github project template which makes writing new language servers easier.
The syntax highlighting is provided by [LALRPOP syntax highlighting for VS Code](https://github.com/guyutongxue/VSC_LalrpopHighlight?tab=readme-ov-file) by [guyutongxue](https://github.com/guyutongxue).
