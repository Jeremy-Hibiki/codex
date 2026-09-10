//! Encrypted-skill RPC guard for thread-less app-server requests.
//!
//! These requests do not carry a thread id, so the guard uses the
//! process-level "any session engaged" signal (see
//! `codex_core::agent_security::rpc`). Unengaged processes behave exactly as
//! before (all checks pass through).

use std::path::Path;

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
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
        | ClientRequest::ExternalAgentConfigImport { .. } => {
            Err(invalid_request(CONFIG_MUTATION_POLICY_ERROR))
        }
        // fs/writeFile only bypasses the RPC-level config gates when it
        // targets a configuration file directly under codex_home.
        ClientRequest::FsWriteFile { params, .. } => {
            let Ok(codex_home) = codex_core::config::find_codex_home() else {
                return Ok(());
            };
            if is_codex_home_config_file(params.path.as_path(), codex_home.as_path()) {
                Err(invalid_request(CONFIG_MUTATION_POLICY_ERROR))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// True when `path` is a configuration file directly under `codex_home`
/// (config.toml, managed_config.toml, requirements*.toml, features*.toml).
fn is_codex_home_config_file(path: &Path, codex_home: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(codex_home) else {
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
