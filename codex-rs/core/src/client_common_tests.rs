use codex_api::OpenAiVerbosity;
use codex_api::ResponsesApiRequest;
use codex_api::TextControls;
use codex_api::create_text_param_for_request;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use fm_encrypted_skills::registry::TtlConfig;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use fm_encrypted_skills::sdk::EnvelopeError;
use fm_encrypted_skills::sdk::EnvelopeSdk;
use fm_encrypted_skills::sdk::PackageEntry;
use pretty_assertions::assert_eq;
use serde_json::value::RawValue;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::*;

fn empty_tools() -> Arc<RawValue> {
    Arc::from(RawValue::from_string("[]".to_string()).expect("valid tool JSON"))
}

struct ClientTestSdk;

impl EnvelopeSdk for ClientTestSdk {
    fn decrypt_package(
        &self,
        _package_path: &Path,
    ) -> std::result::Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"# Framed content\nrun scripts/build.sh".to_vec(),
        }])
    }
}

#[test]
fn request_input_rehydrates_encrypted_skill_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new(
        Arc::new(ClientTestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    let token = runtime
        .load_or_register(
            "thread-1",
            "secret-skill",
            Path::new("/skills/secret.zip.enc"),
        )
        .expect("load encrypted skill");
    let rehydrator = EncryptedSkillRehydrator {
        runtime: Arc::new(runtime),
        session_id: "thread-1".to_string(),
    };
    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: token.clone(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }],
        encrypted_skills: Some(rehydrator),
        ..Default::default()
    };

    let formatted = prompt.get_formatted_input_for_request(/*use_responses_lite*/ false);

    let ResponseItem::Message { content, .. } = &formatted[0] else {
        panic!("expected message item");
    };
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert!(text.contains("<skill_name>secret-skill</skill_name>"));
    assert!(text.contains("# Framed content"));
    assert!(!text.contains(&token));
    assert!(!text.contains("/dev/shm/fm-agent-security"));
}

#[test]
fn request_input_without_rehydrator_is_unchanged() {
    let token = "[SENSITIVE_SKILL_TOKEN:thread-1:abcdef0123456789abcdef0123456789]";
    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: token.to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }],
        encrypted_skills: None,
        ..Default::default()
    };

    let formatted = prompt.get_formatted_input_for_request(/*use_responses_lite*/ false);

    let ResponseItem::Message { content, .. } = &formatted[0] else {
        panic!("expected message item");
    };
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert_eq!(text, token);
}

#[test]
fn request_input_injects_implicit_encrypted_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = EncryptedSkillRuntime::new(
        Arc::new(ClientTestSdk),
        TtlConfig::default(),
        tmp.path().join("mem-root"),
    );
    runtime.register_implicit_skill_package(
        "thread-1",
        "secret-skill",
        PathBuf::from("/skills/secret.zip.enc"),
    );
    let rehydrator = EncryptedSkillRehydrator {
        runtime: Arc::new(runtime),
        session_id: "thread-1".to_string(),
    };
    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "review the repo".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }],
        encrypted_skills: Some(rehydrator),
        ..Default::default()
    };

    let formatted = prompt.get_formatted_input_for_request(/*use_responses_lite*/ false);

    assert_eq!(
        formatted.len(),
        2,
        "implicit skill must be injected as a new item"
    );
    let ResponseItem::Message { role, content, .. } = &formatted[1] else {
        panic!("expected injected message item");
    };
    assert_eq!(role, "developer");
    let ContentItem::InputText { text } = &content[0] else {
        panic!("expected input text");
    };
    assert!(text.contains("<skill_name>secret-skill</skill_name>"));
    assert!(text.contains("# Framed content"));
    assert!(!text.contains("/dev/shm/fm-agent-security"));

    let second = prompt.get_formatted_input_for_request(/*use_responses_lite*/ false);
    assert_eq!(
        second.len(),
        1,
        "already injected skills must not be injected again"
    );
}

fn prompt_with_image_outputs() -> Prompt {
    Prompt {
        input: vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputImage {
                    image_url: "https://example.com/image.png".to_string(),
                    detail: Some(ImageDetail::Original),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::FunctionCallOutput {
                id: None,
                call_id: "function-call".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,function".to_string(),
                        detail: Some(ImageDetail::High),
                    },
                ]),
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::CustomToolCallOutput {
                id: None,
                call_id: "custom-call".to_string(),
                name: None,
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,custom".to_string(),
                        detail: Some(ImageDetail::Auto),
                    },
                ]),
                internal_chat_message_metadata_passthrough: None,
            },
        ],
        encrypted_skills: None,
        ..Default::default()
    }
}

#[test]
fn responses_lite_request_copies_strip_image_details() {
    let prompt = prompt_with_image_outputs();
    let original = prompt.input.clone();

    let stripped = prompt.get_formatted_input_for_request(/*use_responses_lite*/ true);

    assert_eq!(
        stripped,
        vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputImage {
                    image_url: "https://example.com/image.png".to_string(),
                    detail: None,
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::FunctionCallOutput {
                id: None,
                call_id: "function-call".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,function".to_string(),
                        detail: None,
                    },
                ]),
                internal_chat_message_metadata_passthrough: None,
            },
            ResponseItem::CustomToolCallOutput {
                id: None,
                call_id: "custom-call".to_string(),
                name: None,
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: "data:image/png;base64,custom".to_string(),
                        detail: None,
                    },
                ]),
                internal_chat_message_metadata_passthrough: None,
            },
        ]
    );
    assert_eq!(prompt.input, original);
    assert_eq!(
        prompt.get_formatted_input_for_request(/*use_responses_lite*/ false),
        original
    );
}

#[test]
fn serializes_text_verbosity_when_set() {
    let input: Vec<ResponseItem> = vec![];
    let req = ResponsesApiRequest {
        model: "gpt-5.4".to_string(),
        instructions: "i".to_string(),
        input,
        tools: Some(empty_tools().into()),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: Some(TextControls {
            verbosity: Some(OpenAiVerbosity::Low),
            format: None,
        }),
        client_metadata: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    assert_eq!(
        v.get("text")
            .and_then(|t| t.get("verbosity"))
            .and_then(|s| s.as_str()),
        Some("low")
    );
}

#[test]
fn serializes_text_schema_with_strict_format() {
    let input: Vec<ResponseItem> = vec![];
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string"}
        },
        "required": ["answer"],
    });
    let text_controls = create_text_param_for_request(
        /*verbosity*/ None,
        &Some(schema.clone()),
        /*output_schema_strict*/ true,
    )
    .expect("text controls");

    let req = ResponsesApiRequest {
        model: "gpt-5.4".to_string(),
        instructions: "i".to_string(),
        input,
        tools: Some(empty_tools().into()),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: Some(text_controls),
        client_metadata: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    let text = v.get("text").expect("text field");
    assert!(text.get("verbosity").is_none());
    let format = text.get("format").expect("format field");

    assert_eq!(
        format.get("name"),
        Some(&serde_json::Value::String("codex_output_schema".into()))
    );
    assert_eq!(
        format.get("type"),
        Some(&serde_json::Value::String("json_schema".into()))
    );
    assert_eq!(format.get("strict"), Some(&serde_json::Value::Bool(true)));
    assert_eq!(format.get("schema"), Some(&schema));
}

#[test]
fn serializes_text_schema_with_non_strict_format() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string"},
            "rationale": {"type": "string"}
        },
        "required": ["answer"],
        "additionalProperties": false
    });
    let text_controls = create_text_param_for_request(
        /*verbosity*/ None,
        &Some(schema.clone()),
        /*output_schema_strict*/ false,
    )
    .expect("text controls");

    let format = text_controls.format.expect("format field");
    assert!(!format.strict);
    assert_eq!(format.schema, schema);
}

#[test]
fn omits_text_when_not_set() {
    let input: Vec<ResponseItem> = vec![];
    let req = ResponsesApiRequest {
        model: "gpt-5.4".to_string(),
        instructions: "i".to_string(),
        input,
        tools: Some(empty_tools().into()),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: None,
        client_metadata: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    assert!(v.get("text").is_none());
}

#[test]
fn serializes_flex_service_tier_when_set() {
    let req = ResponsesApiRequest {
        model: "gpt-5.4".to_string(),
        instructions: "i".to_string(),
        input: vec![],
        tools: Some(empty_tools().into()),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: vec![],
        prompt_cache_key: None,
        service_tier: Some(ServiceTier::Flex.to_string()),
        text: None,
        client_metadata: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    assert_eq!(
        v.get("service_tier").and_then(|tier| tier.as_str()),
        Some("flex")
    );
}
