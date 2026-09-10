use codex_protocol::AgentPath;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::RolloutItem;

use super::*;

const TOKEN: &str = "[SENSITIVE_SKILL_TOKEN:thread-1:abcdef0123456789abcdef0123456789]";
const REPLACEMENT: &str = "[encrypted-skill unavailable in this context]";

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
fn user_message_with_token_and_real_content_is_kept() {
    let item = user_message(&format!("please do the task {TOKEN}"));
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

#[test]
fn strip_encrypted_skill_tokens_covers_response_item_text_surfaces() {
    let token_message = user_message(&format!("prefix {TOKEN} suffix"));
    let function_call = RolloutItem::ResponseItem(ResponseItem::FunctionCall {
        id: None,
        call_id: "call-1".to_string(),
        name: "shell".to_string(),
        namespace: None,
        arguments: format!("{{\"command\":\"echo {TOKEN}\"}}"),
        internal_chat_message_metadata_passthrough: None,
    });
    let custom_tool_call = RolloutItem::ResponseItem(ResponseItem::CustomToolCall {
        id: None,
        status: None,
        call_id: "call-2".to_string(),
        name: "apply_patch".to_string(),
        namespace: None,
        input: format!("patch body {TOKEN}"),
        internal_chat_message_metadata_passthrough: None,
    });
    let function_output = RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-3".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(format!("stdout {TOKEN}")),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    });
    let custom_output = RolloutItem::ResponseItem(ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "call-4".to_string(),
        name: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: format!("notes {TOKEN}"),
                },
            ]),
            ..Default::default()
        },
        internal_chat_message_metadata_passthrough: None,
    });
    let agent_message = RolloutItem::ResponseItem(ResponseItem::AgentMessage {
        id: None,
        author: "worker".to_string(),
        recipient: "root".to_string(),
        content: vec![AgentMessageInputContent::InputText {
            text: format!("done {TOKEN}"),
        }],
        internal_chat_message_metadata_passthrough: None,
    });
    let reasoning = RolloutItem::ResponseItem(ResponseItem::Reasoning {
        id: None,
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: format!("summary {TOKEN}"),
        }],
        content: Some(vec![ReasoningItemContent::ReasoningText {
            text: format!("raw {TOKEN}"),
        }]),
        encrypted_content: None,
        internal_chat_message_metadata_passthrough: None,
    });
    let mut items = vec![
        token_message,
        function_call,
        custom_tool_call,
        function_output,
        custom_output,
        agent_message,
        reasoning,
    ];

    strip_encrypted_skill_tokens(&mut items);

    for item in items {
        let mut texts = Vec::new();
        collect_response_item_text(&item, &mut texts);
        assert!(
            texts.iter().all(|text| !text.contains(TOKEN)),
            "token leaked in {texts:?}"
        );
        assert!(
            texts
                .iter()
                .filter(|text| text.contains("prefix") || text.contains("suffix"))
                .all(|text| text.contains(REPLACEMENT)),
            "surrounding text must be preserved with the token replaced: {texts:?}"
        );
    }
}

#[test]
fn strip_encrypted_skill_tokens_covers_inter_agent_communication() {
    let mut communication = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").unwrap(),
        Vec::new(),
        format!("content {TOKEN}"),
        false,
    );
    communication.encrypted_content = Some(format!("encrypted {TOKEN}"));
    let mut items = vec![RolloutItem::InterAgentCommunication(communication)];

    strip_encrypted_skill_tokens(&mut items);

    let RolloutItem::InterAgentCommunication(communication) = &items[0] else {
        panic!("expected inter-agent communication item");
    };
    assert_eq!(communication.content, format!("content {REPLACEMENT}"));
    assert_eq!(
        communication.encrypted_content.as_deref(),
        Some(format!("encrypted {REPLACEMENT}").as_str())
    );
}

#[test]
fn strip_encrypted_skill_tokens_covers_compacted_items() {
    let item = CompactedItem {
        message: format!("summary {TOKEN}"),
        replacement_history: Some(vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("history {TOKEN}"),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }]),
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    };
    let mut items = vec![RolloutItem::Compacted(item.clone())];

    strip_encrypted_skill_tokens(&mut items);

    let RolloutItem::Compacted(compacted) = &items[0] else {
        panic!("expected compacted item");
    };
    assert_eq!(compacted.message, format!("summary {REPLACEMENT}"));
    let ResponseItem::Message { content, .. } = &compacted.replacement_history.as_ref().unwrap()[0]
    else {
        panic!("expected message in replacement history");
    };
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert_eq!(text, &format!("history {REPLACEMENT}"));
    assert_eq!(item.message, format!("summary {TOKEN}"));
}

#[test]
fn strip_encrypted_skill_tokens_leaves_token_free_items_unchanged() {
    let mut items = vec![
        user_message("regular user content"),
        RolloutItem::ResponseItem(ResponseItem::FunctionCall {
            id: None,
            call_id: "call-1".to_string(),
            name: "shell".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            internal_chat_message_metadata_passthrough: None,
        }),
    ];
    let mut texts = Vec::new();
    for item in &items {
        collect_response_item_text(item, &mut texts);
    }

    strip_encrypted_skill_tokens(&mut items);

    let mut stripped_texts = Vec::new();
    for item in &items {
        collect_response_item_text(item, &mut stripped_texts);
    }
    assert_eq!(stripped_texts, texts);
}

fn collect_response_item_text(item: &RolloutItem, out: &mut Vec<String>) {
    let RolloutItem::ResponseItem(response_item) = item else {
        return;
    };
    match response_item {
        ResponseItem::Message { content, .. } => {
            for content_item in content {
                if let ContentItem::InputText { text } | ContentItem::OutputText { text } =
                    content_item
                {
                    out.push(text.clone());
                }
            }
        }
        ResponseItem::AgentMessage { content, .. } => {
            for content_item in content {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    out.push(text.clone());
                }
            }
        }
        ResponseItem::FunctionCall { arguments, .. } => out.push(arguments.clone()),
        ResponseItem::CustomToolCall { input, .. } => out.push(input.clone()),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => match &output.body {
            FunctionCallOutputBody::Text(text) => out.push(text.clone()),
            FunctionCallOutputBody::ContentItems(items) => {
                for content_item in items {
                    if let FunctionCallOutputContentItem::InputText { text } = content_item {
                        out.push(text.clone());
                    }
                }
            }
        },
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            for entry in summary {
                let ReasoningItemReasoningSummary::SummaryText { text } = entry;
                out.push(text.clone());
            }
            if let Some(content) = content {
                for entry in content {
                    match entry {
                        ReasoningItemContent::ReasoningText { text }
                        | ReasoningItemContent::Text { text } => out.push(text.clone()),
                    }
                }
            }
        }
        _ => {}
    }
}
