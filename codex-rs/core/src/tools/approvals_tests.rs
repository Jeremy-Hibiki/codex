use super::*;
use crate::session::tests::make_session_and_context_with_rx;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::approvals::NetworkPolicyAmendment;
use pretty_assertions::assert_eq;

#[test]
fn approval_resolution_rejects_denied_network_policy_amendment() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::NetworkPolicyAmendment {
            network_policy_amendment: NetworkPolicyAmendment {
                host: "denied.example.com".to_string(),
                action: NetworkPolicyRuleAction::Deny,
            },
        },
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Rejected(rejection)) if rejection == "rejected by user"
    ));
}

#[test]
fn approval_resolution_rejects_mcp_policy_amendment() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::ApprovedMcpPolicyAmendment,
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Rejected(rejection)) if rejection == "Error while requesting approval"
    ));
}

#[test]
fn approval_resolution_aborts_turn_when_approval_is_aborted() {
    let resolution = ApprovalResolution {
        decision: ReviewDecision::Abort,
        source: ApprovalResolutionSource::User,
    };

    assert!(matches!(
        resolution.into_tool_result(&model_info_from_slug("acting-model")),
        Err(ToolError::Codex(error))
            if matches!(
                error.details(),
                codex_protocol::error::CodexErrorDetails::TurnAborted
            )
    ));
}

#[test]
fn approval_resolution_uses_acting_model_timeout_instructions() {
    let mut model = model_info_from_slug("acting-model");
    for timeout_instructions in ["Catalog timeout instructions.", ""] {
        model.model_messages = Some(
            serde_json::from_value(serde_json::json!({
                "auto_review": {
                    "timeout_instructions": timeout_instructions,
                },
            }))
            .expect("model messages should deserialize"),
        );
        let resolution = ApprovalResolution {
            decision: ReviewDecision::TimedOut,
            source: ApprovalResolutionSource::Guardian,
        };

        assert!(matches!(
            resolution.into_tool_result(&model),
            Err(ToolError::Rejected(rejection)) if rejection == timeout_instructions
        ));
    }
}

#[cfg(unix)]
#[test_case::test_case(ApprovalsReviewer::User, codex_extension_api::ApprovalDecision::AskUser; "manual prompt")]
#[test_case::test_case(ApprovalsReviewer::AutoReview, codex_extension_api::ApprovalDecision::Allow; "cached allow")]
#[tokio::test]
async fn non_utf8_cwd_preserves_approval_routing(
    reviewer: ApprovalsReviewer,
    decision: codex_extension_api::ApprovalDecision,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use codex_extension_api::ApprovalDecision;
    use std::os::unix::ffi::OsStringExt;

    struct Contributor {
        cwd: codex_utils_path_uri::LegacyAppPathString,
        decision: ApprovalDecision,
    }

    impl codex_extension_api::ApprovalReviewContributor for Contributor {
        fn decide<'a>(
            &'a self,
            input: &'a codex_extension_api::ApprovalDecisionInput<'_>,
        ) -> codex_extension_api::ExtensionFuture<'a, Option<ApprovalDecision>> {
            Box::pin(async move {
                assert_eq!(input.action["cwd"], serde_json::json!(self.cwd));
                Some(self.decision.clone())
            })
        }
    }

    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::try_from(PathBuf::from(
        std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xe9".to_vec()),
    ))?);
    let (mut session, turn, events) = make_session_and_context_with_rx().await;
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.approval_review_contributor(Arc::new(Contributor {
        cwd: codex_utils_path_uri::LegacyAppPathString::from_path_uri(&cwd, PathConvention::Posix)?,
        decision,
    }));
    Arc::get_mut(&mut session)
        .context("session is uniquely owned")?
        .services
        .extensions = Arc::new(extensions.build());
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());
    let mut review_context = GuardianReviewContext::from(&turn);
    review_context.approval_policy = AskForApproval::OnRequest;
    review_context.approvals_reviewer = reviewer;
    let context = ApprovalContext {
        review_context,
        cancellation_token: None,
        call_id: "non-utf8-cwd".to_string(),
        tool_name: ToolName::plain("exec_command"),
        strict_auto_review: false,
        approval_reason: None,
        retry_reason: None,
        network_approval_context: None,
    };
    let action = ApprovalAction::ExecCommand {
        id: context.call_id.clone(),
        environment_id: codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string(),
        command: vec!["npm".to_string(), "install".to_string()],
        hook_command: "npm install".to_string(),
        cwd: cwd.clone(),
        sandbox_permissions: if reviewer == ApprovalsReviewer::User {
            SandboxPermissions::RequireEscalated
        } else {
            SandboxPermissions::UseDefault
        },
        additional_permissions: None,
        justification: None,
        tty: false,
        proposed_execpolicy_amendment: None,
    };
    let approval = session.request_reviewer_approval(action, &context);
    tokio::pin!(approval);
    let expected = if reviewer == ApprovalsReviewer::User {
        tokio::select! {
            resolution = &mut approval => panic!("expected a user prompt, got {resolution:?}"),
            event = events.recv() => {
                let codex_protocol::protocol::EventMsg::ExecApprovalRequest(request) =
                    event.context("receive user prompt")?.msg
                else {
                    panic!("expected a command approval prompt");
                };
                assert_eq!(request.cwd, codex_utils_path_uri::LegacyAppPathString::from(cwd));
                assert_eq!(request.command, vec!["npm", "install"]);
                session.notify_approval(&request.call_id, ReviewDecision::Approved).await;
            }
        }
        ApprovalResolution {
            decision: ReviewDecision::Approved,
            source: ApprovalResolutionSource::User,
        }
    } else {
        ApprovalResolution {
            decision: ReviewDecision::Approved,
            source: ApprovalResolutionSource::Guardian,
        }
    };
    assert_eq!(approval.await, expected);
    assert!(events.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn explicit_mcp_reviewer_override_takes_precedence_over_action_context() {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    let action = ApprovalAction::McpToolCall {
        id: "mcp-override".to_string(),
        server: "example".to_string(),
        tool_name: "dangerous".to_string(),
        arguments: None,
        connector_id: None,
        connector_name: None,
        connector_description: None,
        connected_account_email: None,
        tool_title: None,
        tool_description: None,
        annotations: None,
        hook_tool_name: HookToolName::new("mcp__example__dangerous"),
        approval_policy: AskForApproval::OnRequest,
        reviewer: ApprovalsReviewer::User,
        approval_mode: AppToolApproval::Prompt,
        allow_session_remember: false,
        allow_persistent_approval: false,
    };
    let mut review_context = GuardianReviewContext::from(&turn);
    review_context.approval_policy = AskForApproval::OnRequest;
    review_context.approvals_reviewer = ApprovalsReviewer::AutoReview;
    let context = ApprovalContext {
        review_context,
        cancellation_token: None,
        call_id: "mcp-override".to_string(),
        tool_name: ToolName::plain("dangerous"),
        strict_auto_review: false,
        approval_reason: None,
        retry_reason: None,
        network_approval_context: None,
    };

    tokio::select! {
        resolution = session.request_reviewer_approval(action, &context) => {
            panic!("expected a user approval request, got {resolution:?}");
        }
        event = events.recv() => {
            let codex_protocol::protocol::EventMsg::ElicitationRequest(request) =
                event.expect("receive user approval request").msg
            else {
                panic!("expected an MCP user approval request");
            };
            assert_eq!(request.server_name, "example");
            assert_eq!(
                request.id,
                codex_protocol::mcp::RequestId::String(
                    "mcp_tool_call_approval_mcp-override".to_string()
                )
            );
        }
    }
}

struct D9TestSdk;

impl fm_encrypted_skills::sdk::EnvelopeSdk for D9TestSdk {
    fn decrypt_package(
        &self,
        _package_path: &std::path::Path,
    ) -> Result<Vec<fm_encrypted_skills::sdk::PackageEntry>, fm_encrypted_skills::sdk::EnvelopeError>
    {
        Ok(vec![fm_encrypted_skills::sdk::PackageEntry {
            rel_path: std::path::PathBuf::from("SKILL.md"),
            contents: b"# Encrypted skill".to_vec(),
        }])
    }
}

#[test]
fn d9_auto_permit_requires_every_segment_to_be_skill_script_execution() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let runtime = Arc::new(fm_encrypted_skills::runtime::EncryptedSkillRuntime::new(
        Arc::new(D9TestSdk),
        fm_encrypted_skills::registry::TtlConfig::default(),
        tmp.path().join("mem-root"),
    ));
    runtime
        .load_or_register(
            "t1",
            "secret",
            std::path::Path::new("/skills/secret.zip.enc"),
        )
        .expect("test sdk should decrypt the skill package");
    let engaged = runtime.guard("t1");
    let unengaged = runtime.guard("t2");

    // A pure skill-script execution stays auto-permitted (D9).
    assert!(super::d9_skill_script_auto_permit(
        &engaged,
        "bash /skills/run.sh",
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
    ));

    // A mixed command with a review-worthy segment must go through approval.
    assert!(!super::d9_skill_script_auto_permit(
        &engaged,
        "bash /skills/run.sh; sudo rm -rf /tmp/x",
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
    ));
    assert!(!super::d9_skill_script_auto_permit(
        &engaged,
        "bash /skills/run.sh; cat /skills/SKILL.md",
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
    ));

    // Sandbox escalation always needs review.
    assert!(!super::d9_skill_script_auto_permit(
        &engaged,
        "bash /skills/run.sh",
        SandboxPermissions::RequireEscalated,
        /*additional_permissions*/ None,
    ));
    assert!(!super::d9_skill_script_auto_permit(
        &engaged,
        "bash /skills/run.sh",
        SandboxPermissions::UseDefault,
        Some(&AdditionalPermissionProfile::default()),
    ));

    // Unengaged sessions never auto-permit via D9.
    assert!(!super::d9_skill_script_auto_permit(
        &unengaged,
        "bash /skills/run.sh",
        SandboxPermissions::UseDefault,
        /*additional_permissions*/ None,
    ));
}
