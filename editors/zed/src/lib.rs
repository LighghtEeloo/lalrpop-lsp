use std::collections::BTreeMap;

use zed_extension_api::{self as zed, settings::LspSettings};

const SERVER_BINARY: &str = "lalrpop-lsp";

struct LalrpopExtension;

impl zed::Extension for LalrpopExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let binary = settings.binary;

        let command = binary
            .as_ref()
            .and_then(|binary| binary.path.clone())
            .or_else(|| worktree.which(SERVER_BINARY))
            .ok_or_else(|| {
                format!(
                    "{SERVER_BINARY} was not found in PATH. Install it with `cargo install --git \
                     https://github.com/LighghtEeloo/lalrpop-lsp.git --locked`, or configure \
                     `lsp.{SERVER_BINARY}.binary.path` in Zed settings."
                )
            })?;

        let args = binary
            .as_ref()
            .and_then(|binary| binary.arguments.clone())
            .unwrap_or_default();

        let mut env: BTreeMap<String, String> = worktree.shell_env().into_iter().collect();
        if let Some(overrides) = binary.and_then(|binary| binary.env) {
            env.extend(overrides);
        }

        Ok(zed::Command {
            command,
            args,
            env: env.into_iter().collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(
            LspSettings::for_worktree(language_server_id.as_ref(), worktree)?
                .initialization_options,
        )
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(LspSettings::for_worktree(language_server_id.as_ref(), worktree)?.settings)
    }
}

zed::register_extension!(LalrpopExtension);
