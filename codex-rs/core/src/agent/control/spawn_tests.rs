use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;

use super::*;

fn user_message(text: &str) -> RolloutItem {
    RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

#[test]
fn user_message_with_encrypted_skill_token_is_kept() {
    // Fork isolation strips tokens, not the whole message — see the fork
    // pipeline. At the keep/drop filter level the message is always retained.
    let item = user_message("[SENSITIVE_SKILL_TOKEN:thread-1:abcdef0123456789abcdef0123456789]");
    assert!(keep_forked_rollout_item(&item, false));
}

#[test]
fn user_message_with_token_and_real_content_is_kept() {
    let item = user_message(
        "please do the task [SENSITIVE_SKILL_TOKEN:thread-1:abcdef0123456789abcdef0123456789]",
    );
    assert!(keep_forked_rollout_item(&item, false));
}

#[test]
fn user_message_without_token_is_kept() {
    let item = user_message("regular user content");
    assert!(keep_forked_rollout_item(&item, false));
}

#[test]
fn assistant_final_answer_is_kept() {
    let item = RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![],
        phase: Some(codex_protocol::models::MessagePhase::FinalAnswer),
        internal_chat_message_metadata_passthrough: None,
    });
    assert!(keep_forked_rollout_item(&item, false));
}

#[test]
fn function_call_items_are_dropped() {
    let item = RolloutItem::ResponseItem(ResponseItem::FunctionCall {
        id: None,
        call_id: "call-1".to_string(),
        name: "shell".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        internal_chat_message_metadata_passthrough: None,
    });
    assert!(!keep_forked_rollout_item(&item, false));
}
