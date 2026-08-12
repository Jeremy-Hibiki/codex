use super::*;

#[test]
fn serialize_contains_event_type_and_fields() {
    let event = AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret-skill".into(),
        cache_hit: false,
        source: InvocationSource::Implicit,
    };
    let line = serialize(&event);
    assert!(line.contains("\"event\":\"decryption\""));
    assert!(line.contains("\"session_id\":\"t1\""));
    assert!(line.contains("\"skill_name\":\"secret-skill\""));
    assert!(line.contains("\"cache_hit\":false"));
    assert!(line.contains("\"source\":\"implicit\""));
    assert!(line.contains("\"timestamp_ms\":"));
}

#[test]
fn write_event_appends_one_line() {
    let event = AuditEvent::Rehydration {
        session_id: "t1".into(),
        token_count: 3,
        skills: vec!["secret".into()],
    };
    let mut buf = Vec::new();
    write_event(&mut buf, &event).unwrap();
    write_event(&mut buf, &event).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(
        text.lines()
            .all(|line| line.contains("\"event\":\"rehydration\""))
    );
}

#[test]
fn audit_lines_never_contain_plaintext() {
    let event = AuditEvent::Tokenization {
        session_id: "t1".into(),
        skill_name: "secret-skill".into(),
        source: InvocationSource::Explicit,
    };
    let line = serialize(&event);
    assert!(!line.contains("SKILL.md content"));
    assert!(!line.contains("/dev/shm/fm-agent-security"));
    assert!(line.contains("\"source\":\"explicit\""));
}

#[test]
fn serialize_implicit_injection_and_redaction_events() {
    let injection = AuditEvent::ImplicitInjection {
        session_id: "t1".into(),
        skill_name: "secret-skill".into(),
    };
    let line = serialize(&injection);
    assert!(line.contains("\"event\":\"implicit_injection\""));
    assert!(line.contains("\"skill_name\":\"secret-skill\""));

    let redaction = AuditEvent::Redaction {
        session_id: "t1".into(),
        skills: vec!["secret-skill".into()],
        surface: "tool_output",
    };
    let line = serialize(&redaction);
    assert!(line.contains("\"event\":\"redaction\""));
    assert!(line.contains("\"skills\":[\"secret-skill\"]"));
    assert!(line.contains("\"surface\":\"tool_output\""));
    assert!(!line.contains("REAL_SKILL_CONTENT"));
}

#[test]
fn serialize_escapes_quotes_in_metadata() {
    let event = AuditEvent::Blocked {
        session_id: "t1".into(),
        tool: "shell".into(),
        reason: "path \"leak\" blocked".into(),
    };
    let line = serialize(&event);
    assert!(line.contains("\\\"leak\\\""));
    let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(parsed["reason"], "path \"leak\" blocked");
}

#[test]
fn file_audit_sink_writes_jsonl_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let sink = FileAuditSink::new(path.clone()).unwrap();
    sink.emit(AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret".into(),
        cache_hit: false,
        source: InvocationSource::Explicit,
    });
    sink.emit(AuditEvent::Rehydration {
        session_id: "t1".into(),
        token_count: 2,
        skills: vec!["secret".into()],
    });

    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.lines().all(|line| line.contains("\"event\":")));
}

#[test]
fn file_audit_sink_rotates_at_size_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let sink = FileAuditSink::new_with_limit(path.clone(), 64).unwrap();
    for _ in 0..10 {
        sink.emit(AuditEvent::Decryption {
            session_id: "t1".into(),
            skill_name: "secret".into(),
            cache_hit: false,
            source: InvocationSource::Explicit,
        });
    }

    assert!(path.exists());
    let rotated = tmp.path().join("audit.log.1");
    assert!(rotated.exists(), "expected rotated audit file");
}

#[test]
fn shared_file_sink_reuses_instance_for_same_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let first = shared_file_sink(path.clone(), FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    let second = shared_file_sink(path, FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn shared_file_sink_returns_distinct_instances_for_different_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let first =
        shared_file_sink(tmp.path().join("a.log"), FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    let second =
        shared_file_sink(tmp.path().join("b.log"), FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn shared_file_sink_distinguishes_rotation_limits() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let default = shared_file_sink(path.clone(), FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    let small = shared_file_sink(path, 64).unwrap();
    assert!(!Arc::ptr_eq(&default, &small));
}

#[test]
fn shared_file_sink_recreates_instance_after_last_reference_drops() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("audit.log");
    let first = shared_file_sink(path.clone(), FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    drop(first);
    let second = shared_file_sink(path, FileAuditSink::DEFAULT_MAX_BYTES).unwrap();
    second.emit(AuditEvent::Decryption {
        session_id: "t1".into(),
        skill_name: "secret".into(),
        cache_hit: false,
        source: InvocationSource::Explicit,
    });
    let text = std::fs::read_to_string(tmp.path().join("audit.log")).unwrap();
    assert_eq!(text.lines().count(), 1);
}
