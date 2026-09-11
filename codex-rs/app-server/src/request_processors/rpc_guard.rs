//! Encrypted-skill RPC guard for thread-less app-server requests.
//!
//! These requests do not carry a thread id, so the guard uses the
//! process-level "any session engaged" signal (see
//! `codex_core::agent_security::rpc`). Unengaged processes behave exactly as
//! before (all checks pass through).

use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::LoginAccountParams;
use fm_encrypted_skills::rpc;

use crate::error_code::invalid_request;

pub(crate) const RPC_BLOCK_MESSAGE: &str =
    "Blocked by encrypted skill policy: operation is not allowed while encrypted skills are in use";
pub(crate) const CONFIG_MUTATION_POLICY_ERROR: &str =
    "configuration changes are disabled while encrypted skills are in use";

pub(crate) fn ensure_path_not_guarded(path: &Path) -> Result<(), JSONRPCErrorError> {
    if rpc::is_guarded_path(path) {
        Err(invalid_request(RPC_BLOCK_MESSAGE))
    } else {
        Ok(())
    }
}

pub(crate) fn ensure_command_not_guarded(command: &str) -> Result<(), JSONRPCErrorError> {
    if rpc::command_references_guarded_path(command) {
        Err(invalid_request(RPC_BLOCK_MESSAGE))
    } else {
        Ok(())
    }
}

pub(crate) fn ensure_args_not_guarded(value: &serde_json::Value) -> Result<(), JSONRPCErrorError> {
    if rpc::args_reference_guarded_path(value) {
        Err(invalid_request(RPC_BLOCK_MESSAGE))
    } else {
        Ok(())
    }
}

pub(crate) fn ensure_serializable_args_not_guarded<T: serde::Serialize>(
    params: &T,
    invalid_message: &str,
) -> Result<(), JSONRPCErrorError> {
    let value = serde_json::to_value(params)
        .map_err(|err| invalid_request(format!("{invalid_message}: {err}")))?;
    ensure_args_not_guarded(&value)
}

pub(crate) fn ensure_not_engaged_unsandboxed() -> Result<(), JSONRPCErrorError> {
    if rpc::any_engaged() {
        Err(invalid_request(RPC_BLOCK_MESSAGE))
    } else {
        Ok(())
    }
}

/// Product policy: plugin and marketplace changes can be restricted to managed
/// entries via flat requirements fields; defaults preserve upstream behavior.
pub(crate) fn ensure_plugin_management_allowed(
    request: &ClientRequest,
    allow_managed_plugins_only: bool,
    allow_managed_marketplaces_only: bool,
) -> Result<(), JSONRPCErrorError> {
    let marketplace_blocked = allow_managed_marketplaces_only
        && matches!(
            request,
            ClientRequest::MarketplaceAdd { .. }
                | ClientRequest::MarketplaceRemove { .. }
                | ClientRequest::MarketplaceUpgrade { .. }
        );
    let plugin_blocked = allow_managed_plugins_only
        && matches!(
            request,
            ClientRequest::PluginShareSave { .. }
                | ClientRequest::PluginShareUpdateTargets { .. }
                | ClientRequest::PluginShareCheckout { .. }
                | ClientRequest::PluginShareDelete { .. }
                | ClientRequest::PluginInstall { .. }
                | ClientRequest::PluginUninstall { .. }
        );
    if marketplace_blocked {
        Err(invalid_request(
            fm_product_policy::MANAGED_MARKETPLACES_ONLY_MESSAGE,
        ))
    } else if plugin_blocked {
        Err(invalid_request(
            fm_product_policy::MANAGED_PLUGINS_ONLY_MESSAGE,
        ))
    } else {
        Ok(())
    }
}

/// Product policy (D10/TODO-9): while any session is engaged, clients must
/// not be able to mutate configuration or feature enablement. Unengaged
/// processes keep existing behavior so internal tooling and tests are
/// unaffected.
pub(crate) fn ensure_config_mutation_allowed(
    request: &ClientRequest,
) -> Result<(), JSONRPCErrorError> {
    if !rpc::any_engaged() {
        return Ok(());
    }
    match request {
        ClientRequest::ConfigValueWrite { .. }
        | ClientRequest::ConfigBatchWrite { .. }
        | ClientRequest::ExperimentalFeatureEnablementSet { .. }
        | ClientRequest::SkillsConfigWrite { .. }
        | ClientRequest::SkillsExtraRootsSet { .. }
        // Config imports always rewrite the codex_home configuration.
        | ClientRequest::ExternalAgentConfigImport { .. }
        // Bedrock setup and Bedrock login rewrite codex_home configuration
        // (`model_provider` + `model_providers.amazon-bedrock.*`) through the
        // same internal config-manager batch-write path.
        | ClientRequest::BedrockSetup { .. }
        | ClientRequest::LoginAccount {
            params:
                LoginAccountParams::AmazonBedrock { .. }
                | LoginAccountParams::AmazonBedrockAccessKeys { .. },
            ..
        } => {
            Err(invalid_request(CONFIG_MUTATION_POLICY_ERROR))
        }
        // fs/writeFile only bypasses the RPC-level config gates when it
        // targets a configuration file directly under codex_home.
        ClientRequest::FsWriteFile { params, .. } => {
            block_if_codex_home_config_file(params.path.as_path())
        }
        ClientRequest::FsCreateDirectory { params, .. } => {
            block_if_codex_home_config_file(params.path.as_path())
        }
        ClientRequest::FsRemove { params, .. } => {
            block_if_codex_home_config_file(params.path.as_path())
        }
        // Copying onto a codex_home configuration file is a config mutation;
        // the source path is not a mutation target.
        ClientRequest::FsCopy { params, .. } => {
            block_if_codex_home_config_file(params.destination_path.as_path())
        }
        _ => Ok(()),
    }
}

fn block_if_codex_home_config_file(path: &Path) -> Result<(), JSONRPCErrorError> {
    let Ok(codex_home) = codex_core::config::find_codex_home() else {
        return Ok(());
    };
    if is_codex_home_config_file(path, codex_home.as_path()) {
        Err(invalid_request(CONFIG_MUTATION_POLICY_ERROR))
    } else {
        Ok(())
    }
}

/// Resolves `.` and `..` lexically so a path cannot dress up as a different
/// location than the one it addresses (component-wise, without touching the
/// filesystem).
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// True when `path` is a configuration file directly under `codex_home`
/// (config.toml, managed_config.toml, requirements*.toml, features*.toml).
fn is_codex_home_config_file(path: &Path, codex_home: &Path) -> bool {
    // Normalize both sides first: `skills/../config.toml` under codex_home
    // must resolve to the config file, and a `..` escape must not be able to
    // smuggle a different location past the prefix check.
    let path = lexical_normalize(path);
    let codex_home = lexical_normalize(codex_home);
    let Ok(relative) = path.strip_prefix(&codex_home) else {
        return false;
    };
    if relative.parent() != Some(Path::new("")) {
        return false;
    }
    let Some(name) = relative.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let is_requirements_or_features =
        |prefix: &str| name.starts_with(prefix) && name.ends_with(".toml");
    name == "config.toml"
        || name == "managed_config.toml"
        || is_requirements_or_features("requirements")
        || is_requirements_or_features("features")
}

#[cfg(test)]
mod tests {
    use super::is_codex_home_config_file;
    use std::path::Path;

    #[test]
    fn is_codex_home_config_file_resolves_lexical_traversal() {
        let codex_home = Path::new("/home/u/work/codex-home");
        // Traversal back into codex_home still addresses the configuration
        // file even though it is not a direct child lexically.
        assert!(is_codex_home_config_file(
            Path::new("/home/u/work/codex-home/skills/../config.toml"),
            codex_home
        ));
        assert!(is_codex_home_config_file(
            Path::new("/home/u/work/codex-home/./features.toml"),
            codex_home
        ));
        // Paths that merely strip under codex_home lexically but resolve
        // outside it are not codex_home configuration files.
        assert!(!is_codex_home_config_file(
            Path::new("/home/u/work/codex-home/../x/.codex/config.toml"),
            codex_home
        ));
        assert!(!is_codex_home_config_file(
            Path::new("/home/u/work/codex-home/other/config.toml"),
            codex_home
        ));
        assert!(!is_codex_home_config_file(
            Path::new("/home/u/work/elsewhere/config.toml"),
            codex_home
        ));
    }
}
