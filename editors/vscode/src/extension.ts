import { spawn } from "node:child_process";
import {
  ExtensionContext,
  OutputChannel,
  ProgressLocation,
  window,
  workspace,
} from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import which = require("which");

const SERVER_BINARY = "lalrpop-lsp";
const SERVER_REPOSITORY = "https://github.com/LighghtEeloo/lalrpop-lsp.git";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const traceOutputChannel = window.createOutputChannel("LALRPOP Language Server trace");
  context.subscriptions.push(traceOutputChannel);

  const configuredPath = workspace
    .getConfiguration("lalrpop-language-server")
    .get<string>("server.path")
    ?.trim();
  const command = process.env.SERVER_PATH || configuredPath || SERVER_BINARY;

  if (!(await isLanguageServerInstalled(command))) {
    if (process.env.SERVER_PATH || configuredPath) {
      void window.showErrorMessage(`LALRPOP language server not found at ${command}.`);
      return;
    }

    try {
      await installLanguageServer(traceOutputChannel);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      void window.showErrorMessage(message);
      return;
    }
  }

  const run: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        RUST_LOG: process.env.RUST_LOG || "info",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "lalrpop" }],
    traceOutputChannel,
  };

  client = new LanguageClient(
    "lalrpop-language-server",
    "LALRPOP language server",
    serverOptions,
    clientOptions,
  );
  await client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

async function isLanguageServerInstalled(command: string): Promise<boolean> {
  try {
    await which(command);
    return true;
  } catch {
    return false;
  }
}

async function installLanguageServer(output: OutputChannel): Promise<void> {
  const answer = await window.showInformationMessage(
    "No lalrpop-lsp installation was found. Install it with Cargo?",
    "Install",
    "Cancel",
  );
  if (answer !== "Install") {
    throw new Error("The lalrpop-lsp binary is required to enable language features.");
  }

  await window.withProgress(
    {
      location: ProgressLocation.Notification,
      title: "Installing lalrpop-lsp with Cargo",
      cancellable: false,
    },
    () => runCargoInstall(output),
  );

  void window.showInformationMessage("Successfully installed lalrpop-lsp.");
}

function runCargoInstall(output: OutputChannel): Promise<void> {
  output.show(true);
  output.appendLine(`Running cargo install --git ${SERVER_REPOSITORY} --locked`);

  return new Promise((resolve, reject) => {
    const install = spawn("cargo", ["install", "--git", SERVER_REPOSITORY, "--locked"]);

    install.stdout.on("data", (data: Buffer) => output.append(data.toString()));
    install.stderr.on("data", (data: Buffer) => output.append(data.toString()));
    install.on("error", (error) => reject(new Error(`Failed to start Cargo: ${error.message}`)));
    install.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Cargo failed to install lalrpop-lsp (exit code ${code ?? "unknown"}).`));
      }
    });
  });
}
