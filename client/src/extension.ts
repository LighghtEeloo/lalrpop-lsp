/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import {
  languages,
  workspace,
  EventEmitter,
  ExtensionContext,
  window,
  InlayHintsProvider,
  TextDocument,
  CancellationToken,
  Range,
  InlayHint,
  TextDocumentChangeEvent,
  ProviderResult,
  commands,
  WorkspaceEdit,
  TextEdit,
  Selection,
  Uri,
  ProgressLocation,
} from "vscode";

import {
  Disposable,
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

import { exec, spawn } from 'child_process';
import which = require("which");

let client: LanguageClient;
// type a = Parameters<>;

export async function activate(context: ExtensionContext) {
  const traceOutputChannel = window.createOutputChannel("LALRPOP Language Server trace");

  const command = process.env.SERVER_PATH || "lalrpop-lsp";

  if (!(await isLanguageServerInstalled(command))) {
    try {
      await installLanguageServer();
    } catch (error) {
      window.showErrorMessage(error.message);
      return;
    }
  }

  const run: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        // eslint-disable-next-line @typescript-eslint/naming-convention
        RUST_LOG: "debug",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };
  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used
  // Options to control the language client
  let clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents
    documentSelector: [{ scheme: "file", language: "lalrpop" }],
    // synchronize: {
    //   // Notify the server about file changes to '.clientrc files contained in the workspace
    //   fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
    // },
    traceOutputChannel,
  };

  // Create the language client and start the client.
  client = new LanguageClient("lalrpop-language-server", "LALRPOP language server", serverOptions, clientOptions);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

function isLanguageServerInstalled(command): Promise<boolean> {
  return new Promise((resolve) => {
    which(command).then((path)=>{
      window.showInformationMessage(`Found ${path} for ${command}`)
      resolve(true)
    }).catch(()=>{
      resolve(false)
    })
  });
}

function installLanguageServer(): Promise<void> {
  return new Promise((resolve, reject) => {
    window.showInformationMessage(
      "No lalrpop-lsp installation found. Install it via cargo?",
      "Yes", "No"
    ).then((answer) => {
      if (answer !== "Yes") {
        return reject(new Error("Missing lalrpop-lsp binary \\(シ)/"));
      }
      window.withProgress(
        {
          location: ProgressLocation.Notification,
          title: 'Installing lalrpop-lsp from cargo (ノ*°▽°*)',
          cancellable: false
        },
        async (progress, _) => {
          await new Promise((resolve: (_: void) => void) => {
            const install = spawn("cargo", [
              "install", "--git",
              "https://github.com/LighghtEeloo/lalrpop-lsp.git"
            ]);
            install.on("exit", (code) => {
              if (code === 0) {
                window.showInformationMessage("Successfully installed lalrpop-lsp (❁´◡`❁)");
                resolve();
              } else {
                reject(new Error("Failed to install LALRPOP language server (×_×)"));
              }
            });
          })
        }
      ).then(() => { resolve() })
    });
  });
}