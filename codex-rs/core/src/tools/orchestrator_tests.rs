use super::*;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::sandboxing::ApprovalAction;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use std::path::Path;
use std::sync::Arc;

struct EncryptedSkillTestSdk;

impl fm_encrypted_skills::sdk::EnvelopeSdk for EncryptedSkillTestSdk {
    fn decrypt_package(
        &self,
        _package_path: &Path,
    ) -> Result<Vec<fm_encrypted_skills::sdk::PackageEntry>, fm_encrypted_skills::sdk::EnvelopeError>
    {
        Ok(vec![fm_encrypted_skills::sdk::PackageEntry {
            rel_path: Path::new("SKILL.md").to_path_buf(),
            contents: b"# Encrypted skill".to_vec(),
        }])
    }
}

/// Executes under an executor-managed sandbox (remote / shell snapshot): the
/// host never selects a concrete `initial_sandbox`, but execution is still
/// sandboxed by the executor.
#[derive(Default)]
struct ExecutorManagedProbe {
    sandbox_requested: Vec<bool>,
    sandboxes: Vec<SandboxType>,
}

impl crate::tools::sandboxing::Approvable<TurnEnvironment> for ExecutorManagedProbe {
    fn approval_action(
        &self,
        _req: &TurnEnvironment,
        call_id: &str,
    ) -> std::io::Result<ApprovalAction> {
        Ok(ApprovalAction::ExecCommand {
            id: call_id.to_string(),
            environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
            command: Vec::new(),
            hook_command: String::new(),
            cwd: PathUri::from_abs_path(&std::env::temp_dir().abs()),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
            tty: false,
            proposed_execpolicy_amendment: None,
        })
    }

    fn should_auto_approve(&self, _req: &TurnEnvironment, _ctx: &ToolCtx) -> bool {
        true
    }
}

impl Sandboxable for ExecutorManagedProbe {
    fn sandbox_preference(&self) -> codex_sandboxing::SandboxablePreference {
        codex_sandboxing::SandboxablePreference::Auto
    }
}

impl ToolRuntime<TurnEnvironment, ()> for ExecutorManagedProbe {
    fn turn_environment<'a>(&self, req: &'a TurnEnvironment) -> &'a TurnEnvironment {
        req
    }

    fn uses_executor_managed_process_sandbox(&self, _req: &TurnEnvironment) -> bool {
        true
    }

    async fn run(
        &mut self,
        _req: &TurnEnvironment,
        attempt: &SandboxAttempt<'_>,
        _ctx: &ToolCtx,
    ) -> Result<(), ToolError> {
        self.sandbox_requested.push(attempt.sandbox_requested);
        self.sandboxes.push(attempt.sandbox);
        Ok(())
    }
}

#[tokio::test]
async fn engaged_session_allows_executor_managed_sandboxed_execution() {
    let (mut session, _turn, _rx) = make_session_and_context_with_auth_and_config_and_rx(
        codex_login::CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config.product_policy.allow_sandbox_bypass = false;
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("test setup should allow permission profile");
        },
    )
    .await;

    // Engage encrypted skills for this thread with a decrypting test SDK.
    let tmp = tempfile::tempdir().expect("temp dir");
    let runtime = fm_encrypted_skills::runtime::EncryptedSkillRuntime::new(
        Arc::new(EncryptedSkillTestSdk),
        fm_encrypted_skills::registry::TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    let thread_id = session.thread_id.to_string();
    runtime
        .load_or_register(&thread_id, "secret", Path::new("/skills/secret.zip.enc"))
        .expect("test sdk should decrypt the skill package");
    Arc::get_mut(&mut session)
        .expect("session should be uniquely owned")
        .services
        .encrypted_skills_runtime = Arc::new(runtime);

    let turn = session.new_default_turn().await;
    let mut orchestrator = ToolOrchestrator::new();
    let mut tool = ExecutorManagedProbe::default();
    let tool_ctx = ToolCtx {
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        session: Arc::clone(&session),
        step_context: StepContext::for_test(Arc::clone(&turn)),
        call_id: "probe-call".to_string(),
        tool_name: codex_tools::ToolName::plain("probe"),
    };

    orchestrator
        .run(
            &mut tool,
            turn.environments
                .primary()
                .expect("turn should have a primary environment"),
            &tool_ctx,
        )
        .await
        .expect(
            "executor-managed sandboxed execution must satisfy the encrypted-skill sandbox gate",
        );

    // The executor-managed path leaves the host-side sandbox unselected but
    // still counts as sandboxed for the I6 gate.
    assert_eq!(tool.sandbox_requested, vec![true]);
    assert_eq!(tool.sandboxes, vec![SandboxType::None]);
}

/// Fails the first attempt with a sandbox denial, then succeeds. Records the
/// `sandbox_requested` flag of every attempt it sees.
#[derive(Default)]
struct SandboxDenialProbe {
    sandbox_requested: Vec<bool>,
}

impl crate::tools::sandboxing::Approvable<TurnEnvironment> for SandboxDenialProbe {
    fn exec_approval_requirement(
        &self,
        _request: &TurnEnvironment,
    ) -> Option<ExecApprovalRequirement> {
        Some(ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        })
    }

    fn should_auto_approve(&self, _req: &TurnEnvironment, _ctx: &ToolCtx) -> bool {
        true
    }

    fn approval_action(
        &self,
        _req: &TurnEnvironment,
        call_id: &str,
    ) -> std::io::Result<crate::tools::sandboxing::ApprovalAction> {
        Ok(crate::tools::sandboxing::ApprovalAction::ExecCommand {
            id: call_id.to_string(),
            environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
            command: Vec::new(),
            hook_command: String::new(),
            cwd: PathUri::from_abs_path(&std::env::temp_dir().abs()),
            sandbox_permissions: crate::sandboxing::SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
            tty: false,
            proposed_execpolicy_amendment: None,
        })
    }
}

impl Sandboxable for SandboxDenialProbe {
    fn sandbox_preference(&self) -> codex_sandboxing::SandboxablePreference {
        codex_sandboxing::SandboxablePreference::Auto
    }
}

impl ToolRuntime<TurnEnvironment, ()> for SandboxDenialProbe {
    fn turn_environment<'a>(&self, req: &'a TurnEnvironment) -> &'a TurnEnvironment {
        req
    }

    async fn run(
        &mut self,
        _req: &TurnEnvironment,
        attempt: &SandboxAttempt<'_>,
        _ctx: &ToolCtx,
    ) -> Result<(), ToolError> {
        self.sandbox_requested.push(attempt.sandbox_requested);
        if self.sandbox_requested.len() == 1 {
            return Err(ToolError::Codex(codex_protocol::error::CodexErr::Sandbox(
                codex_protocol::error::SandboxErr::Denied {
                    output: Box::new(codex_protocol::exec_output::ExecToolCallOutput {
                        exit_code: 1,
                        ..Default::default()
                    }),
                    network_policy_decision: None,
                },
            )));
        }
        Ok(())
    }
}

// fm I6/G2-sec: a sandboxed failure in an engaged session is expected behavior,
// never a justification to retry without the sandbox.
#[tokio::test]
async fn engaged_session_sandbox_denial_does_not_retry_unsandboxed() {
    let (mut session, _turn, _rx) = make_session_and_context_with_auth_and_config_and_rx(
        codex_login::CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config.product_policy.allow_sandbox_bypass = false;
            config
                .permissions
                .approval_policy
                .set(codex_protocol::protocol::AskForApproval::UnlessTrusted)
                .expect("test setup should allow approval policy");
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("test setup should allow permission profile");
        },
    )
    .await;

    let tmp = tempfile::tempdir().expect("temp dir");
    let runtime = fm_encrypted_skills::runtime::EncryptedSkillRuntime::new(
        Arc::new(EncryptedSkillTestSdk),
        fm_encrypted_skills::registry::TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    let thread_id = session.thread_id.to_string();
    runtime
        .load_or_register(&thread_id, "secret", Path::new("/skills/secret.zip.enc"))
        .expect("test sdk should decrypt the skill package");
    Arc::get_mut(&mut session)
        .expect("session should be uniquely owned")
        .services
        .encrypted_skills_runtime = Arc::new(runtime);

    let turn = session.new_default_turn().await;
    let mut orchestrator = ToolOrchestrator::new();
    let mut tool = SandboxDenialProbe::default();
    let tool_ctx = ToolCtx {
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        session: Arc::clone(&session),
        step_context: StepContext::for_test(Arc::clone(&turn)),
        call_id: "denial-probe-call".to_string(),
        tool_name: codex_tools::ToolName::plain("probe"),
    };

    let result = orchestrator
        .run(
            &mut tool,
            turn.environments
                .primary()
                .expect("turn should have a primary environment"),
            &tool_ctx,
        )
        .await;

    assert!(
        result.is_err(),
        "sandbox denial must surface instead of an unsandboxed retry"
    );
    assert_eq!(
        tool.sandbox_requested,
        vec![true],
        "engaged session must never execute an attempt without the sandbox"
    );
}
